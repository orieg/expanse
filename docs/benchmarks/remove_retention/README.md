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
| — | Allocator census and rebuild arm (`NodeAlloc::census`, `set_rebuild_drained` / `map_rebuild_drained`) | measured; pinning confirmed, no option chosen (§3) |
| — | Explicit `compact()` for the 64-bit `ExpanseSet` / `ExpanseMap`: bounds in `scripts/compact_bounds.py`, gates in [`METHODOLOGY.md`](METHODOLOGY.md) §12 | evaluated: G-held, G-peak, G-cost and G-valid met; **G-ins not met** (three `from_sorted_iter` arms moved down by more than 0.1%), so not at the bar to ship (§4) |

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

## 3. Allocator census and rebuild

A measurement step, with no engine change and no decision. Condensing drained
branches (phase 3) lowers `mem_used()`; `mem_held()` after `shrink_to_fit()`
stays far above a fresh build's even where `mem_used()` already matches it
(`r64_range`, R = 1.000). This section asks where those held bytes are, and
what a rebuild of the drained tree would hold.

**Instrument.** The same grid, keys, removal orders and assertions as §1, with
three additions per cell (`crates/expanse/examples/remove_retention.rs`):

- an allocator census (`NodeAlloc::census`, read-only, out of line,
  `#[doc(hidden)]`) of the drained tree, of the drained tree after
  `shrink_to_fit()`, of the fresh build and of the rebuilt tree. Per slab size
  class (the classes of 256 B or less, carved from 4 KiB pages in
  `crates/expanse/src/alloc.rs`): pages total, pages with no live block, partly
  used and full, live and free blocks, and pages bucketed by live fraction.
  Each census is asserted to sum to `mem_used()` and `mem_held()` exactly;
- a rebuild arm: `let rebuilt = drained.clone(); drop(drained);` on the shrunk
  drained tree. The set's `Clone` is `from_sorted_iter` (`set.rs`,
  `impl Clone for ExpanseSet`). The map has no `from_sorted_iter`: its `Clone`
  is `self.iter().collect()`, an ascending insert of every entry (`map.rs`,
  `impl Clone for ExpanseMap`), so the map arm measures that path;
- the heap the `clone()` call requests on top of what was live before it, from
  a counting global allocator (requested bytes, net of `realloc`, without the
  system allocator's chunk overhead).

Every figure below is an exact count with no interval (§8.4), and every cell's
§1 fields reproduce the Step 0a artifact byte for byte: all 42 cells, every
field (measured: Apple M1, macOS, rustc 1.98.1, `a154bc57`; workload:
example_remove_retention; artifact
[`results/census_rebuild.json`](results/census_rebuild.json)). Peak RSS of the
full grid was 534 MB (measured: Apple M1, `/usr/bin/time -l`, `a154bc57`).

**Reproduce.** `EXPANSE_COMMIT=<sha> EXPANSE_RUSTC="$(rustc -V)" cargo run --release -p expanse-trie --example remove_retention -- --json docs/benchmarks/remove_retention/results/census_rebuild.json`.

### 3.1 Held against a fresh build, after shrink and after a rebuild

`held_fresh` is the fresh build's `mem_held()` without `shrink_to_fit()`, the
denominator the phase-3 comparison used. "Shrunk ÷ held_fresh" is `main` after
`shrink_to_fit()`; "rebuilt ÷ held_fresh" is the `clone()` of that tree, read
after the drained tree is dropped. Peak held is both trees' `mem_held()` at the
end of `clone()` (`mem_held()` falls only on `shrink_to_fit`, `clear` or drop,
so neither term is higher earlier in the call). All figures (measured: Apple
M1, `a154bc57`; workload: example_remove_retention; artifact
`results/census_rebuild.json`).

| Cell | Flavor | R | used drained / fresh / rebuilt (MB) | held shrunk / fresh / rebuilt (MB) | shrunk ÷ held_fresh | rebuilt ÷ held_fresh | peak held (MB) | heap added by `clone()` (MB) |
|---|---|---|---|---|---|---|---|---|
| `headline` | set | 3.297 | 27.08 / 8.21 / 8.21 | 67.48 / 12.69 / 8.37 | 5.318 | 0.659 | 75.85 | 16.76 |
| `headline` | map | 1.700 | 29.90 / 17.58 / 17.58 | 78.08 / 24.67 / 17.74 | 3.164 | 0.719 | 95.82 | 17.63 |
| `r64_sorted` | set | 3.297 | 27.08 / 8.21 / 8.21 | 63.20 / 12.69 / 8.37 | 4.981 | 0.659 | 71.57 | 16.76 |
| `r64_sorted` | map | 1.700 | 29.90 / 17.58 / 17.58 | 73.52 / 24.67 / 17.74 | 2.980 | 0.719 | 91.26 | 17.63 |
| `r64_range` | set | 1.000 | 20.99 / 20.99 / 20.99 | 70.54 / 26.64 / 21.58 | 2.647 | 0.810 | 92.12 | 29.97 |
| `r64_range` | map | 1.000 | 23.89 / 23.89 / 23.89 | 79.75 / 29.21 / 24.55 | 2.731 | 0.841 | 104.31 | 24.55 |
| `r64_2m_to_1m` | set | 1.915 | 15.72 / 8.21 / 8.21 | 34.74 / 12.68 / 8.36 | 2.740 | 0.660 | 43.11 | 16.76 |
| `r64_2m_to_1m` | map | 1.251 | 22.00 / 17.58 / 17.58 | 38.85 / 24.66 / 17.73 | 1.576 | 0.719 | 56.58 | 17.62 |
| `r64_4m_to_1m` | set | 3.309 | 27.17 / 8.21 / 8.21 | 67.51 / 12.70 / 8.36 | 5.316 | 0.658 | 75.86 | 16.75 |
| `r64_4m_to_1m` | map | 1.739 | 30.57 / 17.58 / 17.58 | 84.25 / 24.67 / 17.73 | 3.415 | 0.719 | 101.99 | 17.62 |
| `r64_to_2m` | set | 1.706 | 46.85 / 27.46 / 27.46 | 77.19 / 40.82 / 28.11 | 1.891 | 0.689 | 105.30 | 44.89 |
| `r64_to_2m` | map | 1.328 | 52.52 / 39.56 / 39.56 | 86.87 / 52.65 / 40.15 | 1.650 | 0.762 | 127.02 | 40.15 |
| `r64_to_320k` | set | 2.991 | 10.70 / 3.58 / 3.58 | 31.87 / 4.35 / 3.63 | 7.327 | 0.834 | 35.50 | 7.83 |
| `r64_to_320k` | map | 1.695 | 11.60 / 6.84 / 6.84 | 40.87 / 8.20 / 6.98 | 4.985 | 0.851 | 47.84 | 6.95 |
| `r64_1m_to_312k` | set | 1.029 | 3.62 / 3.52 / 3.52 | 6.77 / 4.28 / 3.57 | 1.583 | 0.834 | 10.34 | 7.77 |
| `r64_1m_to_312k` | map | 1.006 | 6.75 / 6.71 / 6.71 | 8.71 / 8.01 / 6.85 | 1.087 | 0.854 | 15.55 | 6.82 |
| `r62` | set | 1.005 | 19.78 / 19.68 / 19.68 | 31.55 / 25.55 / 20.21 | 1.235 | 0.791 | 51.76 | 28.60 |
| `r62` | map | 1.222 | 28.33 / 23.18 / 23.18 | 65.41 / 28.81 / 23.80 | 2.270 | 0.826 | 89.21 | 23.80 |
| `r56` | set | 3.769 | 27.07 / 7.18 / 7.18 | 67.17 / 11.17 / 7.38 | 6.014 | 0.661 | 74.56 | 15.77 |
| `r56` | map | 1.806 | 29.89 / 16.56 / 16.56 | 77.65 / 23.77 / 16.83 | 3.268 | 0.708 | 94.48 | 16.59 |
| `r56_sorted` | set | 3.769 | 27.07 / 7.18 / 7.18 | 62.93 / 11.17 / 7.38 | 5.634 | 0.661 | 70.31 | 15.77 |
| `r56_sorted` | map | 1.806 | 29.89 / 16.56 / 16.56 | 73.14 / 23.77 / 16.83 | 3.078 | 0.708 | 89.97 | 16.59 |
| `r56_range` | set | 1.000 | 21.00 / 21.00 / 21.00 | 71.02 / 27.90 / 21.59 | 2.546 | 0.774 | 92.61 | 29.97 |
| `r56_range` | map | 1.000 | 23.81 / 23.81 / 23.81 | 79.51 / 29.10 / 24.49 | 2.733 | 0.842 | 104.00 | 24.49 |
| `seq_shuffled` | set | 1.000 | 1.00 / 1.00 / 1.00 | 1.02 / 1.08 / 1.02 | 0.951 | 0.951 | 2.05 | 9.41 |
| `seq_shuffled` | map | 1.000 | 11.03 / 11.03 / 11.03 | 19.08 / 11.32 / 11.32 | 1.686 | 1.000 | 30.40 | 11.32 |
| `seq_sorted` | set | 1.000 | 1.00 / 1.00 / 1.00 | 1.02 / 1.08 / 1.02 | 0.951 | 0.951 | 2.05 | 9.41 |
| `seq_sorted` | map | 1.000 | 11.03 / 11.03 / 11.03 | 11.28 / 11.32 / 11.32 | 0.997 | 1.000 | 22.61 | 11.32 |
| `seq_range` | set | 1.000 | 0.07 / 0.07 / 0.06 | 0.07 / 0.13 / 0.07 | 0.577 | 0.577 | 0.15 | 8.47 |
| `seq_range` | map | 1.000 | 8.56 / 8.56 / 8.56 | 8.60 / 8.65 / 8.65 | 0.994 | 1.000 | 17.26 | 8.65 |
| `sparse_shuffled` | set | 1.000 | 20.25 / 20.25 / 20.25 | 20.73 / 20.64 / 20.60 | 1.004 | 0.998 | 41.33 | 28.99 |
| `sparse_shuffled` | map | 1.000 | 20.25 / 20.25 / 20.25 | 20.73 / 20.64 / 20.64 | 1.005 | 1.000 | 41.37 | 20.64 |
| `sparse_sorted` | set | 1.000 | 20.25 / 20.25 / 20.25 | 20.60 / 20.64 / 20.60 | 0.998 | 0.998 | 41.20 | 28.99 |
| `sparse_sorted` | map | 1.000 | 20.25 / 20.25 / 20.25 | 20.60 / 20.64 / 20.64 | 0.998 | 1.000 | 41.24 | 20.64 |
| `sparse_range` | set | 1.000 | 16.31 / 16.31 / 16.31 | 16.32 / 16.38 / 16.32 | 0.996 | 0.996 | 32.64 | 24.71 |
| `sparse_range` | map | 1.000 | 16.31 / 16.31 / 16.31 | 16.32 / 16.38 / 16.38 | 0.997 | 1.000 | 32.69 | 16.38 |
| `clust_shuffled` | set | 1.000 | 1.13 / 1.13 / 1.13 | 1.21 / 1.28 / 1.16 | 0.949 | 0.910 | 2.38 | 9.56 |
| `clust_shuffled` | map | 1.000 | 11.15 / 11.15 / 11.15 | 19.26 / 11.48 / 11.47 | 1.677 | 0.999 | 30.73 | 11.48 |
| `clust_sorted` | set | 1.000 | 1.13 / 1.13 / 1.13 | 1.21 / 1.28 / 1.16 | 0.949 | 0.910 | 2.38 | 9.56 |
| `clust_sorted` | map | 1.000 | 11.15 / 11.15 / 11.15 | 11.42 / 11.48 / 11.47 | 0.995 | 0.999 | 22.89 | 11.48 |
| `clust_range` | set | 1.000 | 0.35 / 0.35 / 0.35 | 1.21 / 0.45 / 0.37 | 2.675 | 0.819 | 1.58 | 8.76 |
| `clust_range` | map | 1.000 | 8.60 / 8.60 / 8.60 | 10.05 / 8.72 / 8.71 | 1.153 | 0.999 | 18.76 | 8.71 |


What the table shows (same artifact):

- The rebuilt tree holds less than a fresh build of the same keys on every
  uniform random cell: rebuilt ÷ held_fresh is 0.658 to 0.854, against 1.087 to
  7.327 for `main` after `shrink_to_fit()`. On the headline cell the set goes
  from 5.318 to 0.659 and the map from 3.164 to 0.719; on `r64_range` from
  2.647 to 0.810 (set) and 2.731 to 0.841 (map).
- On the construction-fixed distributions the rebuild reads 0.577 to 1.000:
  the set is at or below a fresh build everywhere, and the map equals one or
  sits at most 0.15% below it.
- The rebuilt tree's `mem_used()` equals the fresh build's on every map cell,
  as order invariance requires, and on every set cell but `seq_range`, where
  `from_sorted_iter` builds 64,832 B against the fresh 65,792 B.
- `seq_shuffled` and `clust_shuffled` maps have R = 1.000 but hold 1.686 and
  1.677 times a fresh build after shrink, with no excess `mem_used()`.
- For comparison with condensing, from §2's phase-3 artifact
  ([`results/phase3_retention_h1.json`](results/phase3_retention_h1.json), engine
  `1e2b31df`, H1 arm; a different build from the one measured here, whose
  `main` baseline `phase3_retention_main.json` reads the same headline
  `held_shrunk`, 67,481,664 B): H1 leaves the headline set at 1.380 and the map
  at 0.895, and `r64_range` unchanged at 2.647 and 2.731 (measured: Apple M1,
  `1e2b31df`; workload: example_remove_retention).

### 3.2 Census verdict on slab-page pinning

The hypothesis: `release_free` frees a slab page only when every block carved
from it is free (`crates/expanse/src/alloc.rs`, `NodeAlloc::release_free`, the
`is_free` test), live nodes never move, so survivors of random removals keep
most of the peak's pages. The census reads the pages directly.

(measured: Apple M1, `a154bc57`; workload: example_remove_retention; artifact
`results/census_rebuild.json`. "Dense floor" is Σ over classes of
⌈live blocks ÷ blocks per page⌉, the fewest pages the live blocks fit on.
"Pages ≤ 10% live" is `live_hist[1]`. Occupancy is slab live bytes ÷ slab page
bytes.)

| Cell | Flavor | Tree | slab pages | dense floor | partly used / full / free | pages ≤ 10% live | live (MB) | free blocks (MB) | page overhead (MB) | system live (MB) | slab occupancy |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `headline` | set | shrunk | 16,214 | 6,499 | 13,140 / 3,074 / 0 | 8,399 | 26.01 | 38.69 | 1.71 | 1.07 | 0.392 |
| `headline` | set | fresh | 2,837 | 1,782 | 1,485 / 1,103 / 249 | 481 | 7.14 | 4.27 | 0.20 | 1.07 | 0.615 |
| `headline` | set | rebuilt | 1,782 | 1,782 | 11 / 1,771 / 0 | 5 | 7.14 | 0.03 | 0.12 | 1.07 | 0.979 |
| `headline` | map | shrunk | 18,790 | 7,188 | 15,716 / 3,074 / 0 | 9,363 | 28.79 | 46.31 | 1.87 | 1.11 | 0.374 |
| `headline` | map | fresh | 3,198 | 1,499 | 1,372 / 1,040 / 786 | 892 | 6.01 | 6.88 | 0.21 | 11.58 | 0.459 |
| `headline` | map | rebuilt | 1,505 | 1,499 | 10 / 1,489 / 6 | 8 | 6.01 | 0.06 | 0.10 | 11.58 | 0.975 |
| `r64_range` | set | shrunk | 17,120 | 5,168 | 17,120 / 0 / 0 | 708 | 20.58 | 47.68 | 1.86 | 0.41 | 0.294 |
| `r64_range` | set | fresh | 6,404 | 5,168 | 924 / 4,425 / 1,055 | 3 | 20.58 | 5.00 | 0.65 | 0.41 | 0.785 |
| `r64_range` | set | rebuilt | 5,168 | 5,168 | 8 / 5,160 / 0 | 2 | 20.58 | 0.02 | 0.57 | 0.41 | 0.972 |
| `r64_range` | map | shrunk | 19,355 | 5,869 | 19,355 / 0 / 0 | 692 | 23.41 | 53.86 | 2.00 | 0.48 | 0.295 |
| `r64_range` | map | fresh | 7,014 | 5,869 | 925 / 5,125 / 964 | 1 | 23.41 | 4.63 | 0.69 | 0.48 | 0.815 |
| `r64_range` | map | rebuilt | 5,878 | 5,869 | 10 / 5,859 / 9 | 0 | 23.41 | 0.05 | 0.62 | 0.48 | 0.972 |
| `r64_to_320k` | set | shrunk | 7,520 | 2,422 | 7,370 / 150 / 0 | 3,568 | 9.63 | 20.55 | 0.63 | 1.07 | 0.313 |
| `r64_to_320k` | set | fresh | 801 | 625 | 215 / 450 / 136 | 1 | 2.51 | 0.72 | 0.06 | 1.07 | 0.765 |
| `r64_to_320k` | set | rebuilt | 625 | 625 | 5 / 620 / 0 | 1 | 2.51 | 0.01 | 0.04 | 1.07 | 0.980 |
| `r64_to_320k` | map | shrunk | 9,716 | 2,647 | 9,566 / 150 / 0 | 5,238 | 10.53 | 28.50 | 0.77 | 1.07 | 0.265 |
| `r64_to_320k` | map | fresh | 1,573 | 1,264 | 564 / 873 / 136 | 1 | 5.09 | 1.25 | 0.11 | 1.75 | 0.790 |
| `r64_to_320k` | map | rebuilt | 1,275 | 1,264 | 4 / 1,260 / 11 | 0 | 5.09 | 0.05 | 0.08 | 1.75 | 0.974 |
| `r62` | set | shrunk | 7,555 | 4,810 | 4,855 / 2,700 / 0 | 127 | 19.18 | 11.08 | 0.69 | 0.60 | 0.620 |
| `r62` | set | fresh | 6,090 | 4,786 | 1,390 / 3,854 / 846 | 2 | 19.08 | 5.27 | 0.60 | 0.60 | 0.765 |
| `r62` | set | rebuilt | 4,786 | 4,786 | 8 / 4,778 / 0 | 1 | 19.08 | 0.02 | 0.51 | 0.60 | 0.973 |
| `r62` | map | shrunk | 15,822 | 6,930 | 13,122 / 2,700 / 0 | 2,768 | 27.72 | 35.87 | 1.21 | 0.60 | 0.428 |
| `r62` | map | fresh | 6,887 | 5,654 | 1,391 / 4,721 / 775 | 2 | 22.58 | 4.99 | 0.64 | 0.61 | 0.800 |
| `r62` | map | rebuilt | 5,663 | 5,654 | 10 / 5,644 / 9 | 1 | 22.58 | 0.05 | 0.56 | 0.61 | 0.973 |
| `r56` | set | shrunk | 16,139 | 6,500 | 13,103 / 3,036 / 0 | 8,349 | 26.00 | 38.40 | 1.70 | 1.07 | 0.393 |
| `r56` | set | fresh | 2,466 | 1,541 | 1,356 / 910 / 200 | 478 | 6.11 | 3.76 | 0.23 | 1.07 | 0.605 |
| `r56` | set | rebuilt | 1,541 | 1,541 | 11 / 1,530 / 0 | 4 | 6.11 | 0.03 | 0.16 | 1.07 | 0.969 |
| `r56` | map | shrunk | 18,689 | 7,193 | 15,658 / 3,031 / 0 | 9,291 | 28.79 | 45.89 | 1.87 | 1.10 | 0.376 |
| `r56` | map | fresh | 3,126 | 1,422 | 1,501 / 866 / 759 | 921 | 5.59 | 6.90 | 0.31 | 10.96 | 0.437 |
| `r56` | map | rebuilt | 1,432 | 1,422 | 10 / 1,412 / 10 | 5 | 5.59 | 0.07 | 0.20 | 10.96 | 0.954 |
| `seq_shuffled` | set | shrunk | 200 | 200 | 2 / 198 / 0 | 1 | 0.80 | 0.01 | 0.01 | 0.20 | 0.977 |
| `seq_shuffled` | set | fresh | 213 | 200 | 2 / 198 / 13 | 1 | 0.80 | 0.06 | 0.01 | 0.20 | 0.917 |
| `seq_shuffled` | set | rebuilt | 200 | 200 | 2 / 198 / 0 | 1 | 0.80 | 0.01 | 0.01 | 0.20 | 0.977 |
| `seq_shuffled` | map | shrunk | 4,609 | 2,705 | 2,977 / 1,632 / 0 | 802 | 10.82 | 7.63 | 0.43 | 0.20 | 0.573 |
| `seq_shuffled` | map | fresh | 2,714 | 2,705 | 9 / 2,696 / 9 | 2 | 10.82 | 0.06 | 0.23 | 0.20 | 0.974 |
| `seq_shuffled` | map | rebuilt | 2,714 | 2,705 | 9 / 2,696 / 9 | 2 | 10.82 | 0.06 | 0.23 | 0.20 | 0.974 |
| `clust_shuffled` | set | shrunk | 295 | 283 | 28 / 267 / 0 | 3 | 1.13 | 0.06 | 0.02 | 0.01 | 0.931 |
| `clust_shuffled` | set | fresh | 311 | 283 | 28 / 267 / 16 | 3 | 1.13 | 0.13 | 0.02 | 0.01 | 0.883 |
| `clust_shuffled` | set | rebuilt | 283 | 283 | 7 / 276 / 0 | 1 | 1.13 | 0.01 | 0.02 | 0.01 | 0.971 |
| `clust_shuffled` | map | shrunk | 4,700 | 2,785 | 2,990 / 1,710 / 0 | 808 | 11.15 | 7.66 | 0.44 | 0.01 | 0.579 |
| `clust_shuffled` | map | fresh | 2,802 | 2,785 | 11 / 2,776 / 15 | 2 | 11.15 | 0.09 | 0.24 | 0.01 | 0.971 |
| `clust_shuffled` | map | rebuilt | 2,800 | 2,785 | 9 / 2,776 / 15 | 1 | 11.15 | 0.08 | 0.24 | 0.01 | 0.972 |
| `clust_range` | set | shrunk | 295 | 90 | 295 / 0 / 0 | 13 | 0.35 | 0.84 | 0.02 | 0.00 | 0.291 |
| `clust_range` | set | fresh | 110 | 90 | 13 / 81 / 16 | 3 | 0.35 | 0.09 | 0.01 | 0.00 | 0.779 |
| `clust_range` | set | rebuilt | 90 | 90 | 6 / 84 / 0 | 1 | 0.35 | 0.01 | 0.01 | 0.00 | 0.953 |
| `clust_range` | map | shrunk | 500 | 154 | 500 / 0 / 0 | 25 | 0.60 | 1.39 | 0.06 | 8.00 | 0.294 |
| `clust_range` | map | fresh | 175 | 154 | 13 / 145 / 17 | 3 | 0.60 | 0.09 | 0.02 | 8.00 | 0.839 |
| `clust_range` | map | rebuilt | 172 | 154 | 6 / 148 / 18 | 1 | 0.60 | 0.08 | 0.02 | 8.00 | 0.853 |


**Verdict: confirmed.** After `shrink_to_fit()` no slab page is free, and the
pages that stay are sparse:

- The headline set holds 16,214 slab pages where its live blocks fit on 6,499;
  13,140 are partly used, 8,399 of them with at most 10% of their blocks live,
  and slab occupancy is 0.392. Its rebuild holds 1,782 pages, its dense floor,
  at 0.979.
- On `r64_range`, where R = 1.000, the set's 17,120 pages are all partly used,
  none full, at 0.294 occupancy against a dense floor of 5,168.
- The sparse pages concentrate in few classes. On the headline set the raw
  128-byte class alone holds 8,369 pages, 18,501 live blocks and 30.84 MB of
  the 38.69 MB of free-block bytes, with 7,957 of those pages at most 10% live.
  Which node forms those blocks belong to is not attributed here.

The table below splits `held_shrunk − held_fresh` by what the two censuses
count: extra live bytes (`mem_used()`, the branch retention of §1), extra free
blocks on slab pages, and extra page header and tail bytes. The split is exact
(asserted per cell); cells whose difference is under 0.5 MB are omitted. System
free blocks are 0 after shrink, and the system-served live bytes are part of the
live column.

| Cell | Flavor | held_shrunk − held_fresh (MB) | extra live bytes (MB) | extra free blocks on slab pages (MB) | extra page overhead (MB) | extra system bytes (MB) | free-block share |
|---|---|---|---|---|---|---|---|
| `headline` | set | 54.79 | 18.87 | 34.42 | 1.51 | 0.00 | 0.628 |
| `headline` | map | 53.40 | 12.31 | 39.42 | 1.66 | 0.00 | 0.738 |
| `r64_sorted` | set | 50.51 | 18.87 | 30.21 | 1.44 | 0.00 | 0.598 |
| `r64_sorted` | map | 48.85 | 12.31 | 34.94 | 1.59 | 0.00 | 0.715 |
| `r64_range` | set | 43.89 | 0.00 | 42.69 | 1.21 | 0.00 | 0.973 |
| `r64_range` | map | 50.55 | 0.00 | 49.23 | 1.31 | 0.00 | 0.974 |
| `r64_2m_to_1m` | set | 22.06 | 7.51 | 14.01 | 0.54 | 0.00 | 0.635 |
| `r64_2m_to_1m` | map | 14.19 | 4.42 | 9.27 | 0.50 | 0.00 | 0.653 |
| `r64_4m_to_1m` | set | 54.81 | 18.96 | 34.32 | 1.53 | 0.00 | 0.626 |
| `r64_4m_to_1m` | map | 59.58 | 12.99 | 44.81 | 1.78 | 0.00 | 0.752 |
| `r64_to_2m` | set | 36.37 | 19.39 | 15.87 | 1.11 | 0.00 | 0.436 |
| `r64_to_2m` | map | 34.23 | 12.96 | 19.89 | 1.38 | 0.00 | 0.581 |
| `r64_to_320k` | set | 27.52 | 7.12 | 19.83 | 0.57 | 0.00 | 0.721 |
| `r64_to_320k` | map | 32.67 | 4.76 | 27.25 | 0.66 | 0.00 | 0.834 |
| `r64_1m_to_312k` | set | 2.49 | 0.10 | 2.34 | 0.05 | 0.00 | 0.937 |
| `r64_1m_to_312k` | map | 0.70 | 0.04 | 0.65 | 0.01 | 0.00 | 0.929 |
| `r62` | set | 6.00 | 0.10 | 5.82 | 0.09 | 0.00 | 0.969 |
| `r62` | map | 36.60 | 5.14 | 30.88 | 0.57 | 0.00 | 0.844 |
| `r56` | set | 56.00 | 19.89 | 34.64 | 1.48 | 0.00 | 0.618 |
| `r56` | map | 53.89 | 13.34 | 38.99 | 1.55 | 0.00 | 0.724 |
| `r56_sorted` | set | 51.76 | 19.89 | 30.46 | 1.41 | 0.00 | 0.589 |
| `r56_sorted` | map | 49.38 | 13.34 | 34.56 | 1.48 | 0.00 | 0.700 |
| `r56_range` | set | 43.12 | 0.00 | 41.96 | 1.16 | 0.00 | 0.973 |
| `r56_range` | map | 50.41 | 0.00 | 49.13 | 1.28 | 0.00 | 0.975 |
| `seq_shuffled` | map | 7.76 | 0.00 | 7.57 | 0.20 | 0.00 | 0.975 |
| `clust_shuffled` | map | 7.77 | 0.00 | 7.58 | 0.20 | 0.00 | 0.975 |
| `clust_range` | set | 0.76 | 0.00 | 0.74 | 0.01 | 0.00 | 0.982 |
| `clust_range` | map | 1.33 | 0.00 | 1.29 | 0.04 | 0.00 | 0.970 |


Free blocks on partly used pages are the larger part of the excess on 23 of the
24 uniform random rows (share 0.581 to 0.975; the exception is the
`r64_to_2m` set at 0.436), and nearly all of it where R = 1.000 (the range
cells at 0.973 to 0.975, the `seq_shuffled` and `clust_shuffled` maps at
0.975). Pages retained by a few live blocks are therefore where most of the
held bytes are, on every cell with retention.

The split is an accounting of where bytes sit, **not** an attribution of what a
fix removes. The live branch blocks of §1 also pin pages: on the headline set,
H1 condensing lowers `mem_used()` after the drain by 18.86 MB (27,076,928 B on
`main` against 8,217,456 B under H1) and `held_shrunk` by 49.97 MB (67,481,664 B
against 17,510,464 B), so condensing released 31.11 MB of pages beyond the
live bytes it removed (measured: Apple M1, `86adbf15` and `1e2b31df`;
`results/phase3_retention_main.json` and `results/phase3_retention_h1.json`;
engine at tag `poc/subtree-condense`). Which pages those were is not measured: no census
of the condensing build was taken. On `r64_range` condensing removes nothing
and the whole 43.89 MB (set) stays.

### 3.3 Why a fresh build holds 1.55× its `mem_used()`

The headline set's fresh build holds 12,689,472 B against 8,211,872 B used,
1.545. The census splits the 4.48 MB: 4.27 MB of free blocks on slab pages
(1.00 MB of them on 249 pages with no live block, 3.27 MB on partly used
pages) and 0.20 MB of page header and tail bytes; the system-served classes
hold no free block (measured: Apple M1, `a154bc57`; artifact
`results/census_rebuild.json`). `shrink_to_fit()` returns the 249 free pages
and leaves 1.421.

Most of those free blocks sit in classes the finished tree barely uses: the raw
24-, 48- and 72-byte classes hold 0.85, 1.70 and 1.30 MB of them (3.85 of the
4.27 MB) against 39, 2,089 and 14,016 live blocks. A fresh build only inserts,
so each of those frees is the engine replacing a block it allocated earlier
with another. That the replacements are linear leaves stepping up their size
classes as they fill is a **hypothesis**: no call-site attribution was taken.
The map's fresh build is the same shape at 1.403 (6.88 MB of free blocks,
0.21 MB of overhead). The table below gives the other censused cells (same artifact).


| Cell | Flavor | held_fresh ÷ used_fresh | free blocks (MB) | of which on fully free pages (MB) | page overhead (MB) | held after shrink ÷ used |
|---|---|---|---|---|---|---|
| `headline` | set | 1.545 | 4.27 | 1.00 | 0.20 | 1.421 |
| `headline` | map | 1.403 | 6.88 | 3.16 | 0.21 | 1.220 |
| `r64_range` | set | 1.269 | 5.00 | 4.25 | 0.65 | 1.063 |
| `r64_range` | map | 1.223 | 4.63 | 3.89 | 0.69 | 1.057 |
| `r64_to_320k` | set | 1.216 | 0.72 | 0.55 | 0.06 | 1.060 |
| `r64_to_320k` | map | 1.198 | 1.25 | 0.54 | 0.11 | 1.116 |
| `r62` | set | 1.298 | 5.27 | 3.41 | 0.60 | 1.122 |
| `r62` | map | 1.243 | 4.99 | 3.12 | 0.64 | 1.106 |
| `r56` | set | 1.555 | 3.76 | 0.80 | 0.23 | 1.441 |
| `r56` | map | 1.435 | 6.90 | 3.06 | 0.31 | 1.248 |
| `seq_shuffled` | set | 1.072 | 0.06 | 0.05 | 0.01 | 1.019 |
| `seq_shuffled` | map | 1.026 | 0.06 | 0.04 | 0.23 | 1.023 |
| `clust_shuffled` | set | 1.131 | 0.13 | 0.06 | 0.02 | 1.074 |
| `clust_shuffled` | map | 1.029 | 0.09 | 0.06 | 0.24 | 1.024 |
| `clust_range` | set | 1.281 | 0.09 | 0.06 | 0.01 | 1.096 |
| `clust_range` | map | 1.013 | 0.09 | 0.07 | 0.02 | 1.005 |

### 3.4 The rebuild: peak and cost

**Peak.** Table 1's peak held is the shrunk drained tree plus the rebuilt one:
1.110 to 1.786 times `held_shrunk` across the uniform random cells, 75.85 MB
for the headline set and 95.82 MB for the map. A rebuild taken before
`shrink_to_fit()` peaks at `held_drained + held_rebuilt` instead, 103.02 MB for
the headline set (derived from the same artifact). On top of the new tree, the
set's `clone()` requests a transient buffer: heap added minus `held_rebuilt` is
8,393,152 B at M = 1M, 4,198,848 B at M = 312,500 and 16,781,760 B at M = 2M,
consistent with the `Vec<u64>` of keys that `from_sorted_iter` collects at a
power-of-two capacity (2^20 × 8 B = 8,388,608 B). The map's `clone()` inserts
straight into the new tree and adds at most 3,072 B beyond it. (measured:
Apple M1, `a154bc57`; artifact `results/census_rebuild.json`.)

**Cost.** Instructions per surviving key are counted by the
`set_rebuild_drained` and `map_rebuild_drained` Callgrind arms
(`crates/expanse/benches/instructions.rs`): the `*_remove_partial` tree
(200,000 random 60-bit keys, drained to 62,500 in the same Fisher–Yates order)
is cloned and the drained tree dropped inside the measured region.
`set_rebuild_drained/random60` retires 16,960,355 instructions, 271.4 per
surviving key, and `map_rebuild_drained/random60` 40,329,898, 645.3 per
surviving key (measured: CI `instruction-counts`, x86_64 Callgrind,
[run 36205856578](https://github.com/orieg/expanse/actions/runs/36205856578/job/108303982725)
at `f9782220`, whose `crates/` tree is identical to `a154bc57`'s; exact
counts, no interval). The map's figure is the ascending-insert `Clone`; a map
bulk builder would be a different arm and does not exist. These are counts on
the 62,500-key arm, not on the 1M-key grid cells, and no per-key figure is
carried to the grid. No wall-clock figure is claimed.

### 3.5 What the numbers support

No option is chosen here. The ones the measurements bear on:

1. **An explicit `compact()` / `rebuild()` API.** The measured effect is the
   rebuild arm above: held at 0.658 to 0.854 of a fresh build on every uniform
   random cell, including the range cells condensing does not move, at the
   cost of one transient peak of both trees and the instruction count above.
   It is a new contract: nodes move, so the value pointers the C ABI hands
   out (`JudyLGet` / `JudyLIns` slots) are
   invalidated, it is O(n), and it peaks at old plus new. `shrink_to_fit()`
   keeps its contract that nothing moves (`crates/expanse/src/set.rs`,
   `shrink_to_fit` rustdoc) and is not changed by it.
2. **A sweep-condense.** Condensing drained subtrees during a sweep rather than
   on each remove. Its effect on held bytes is measured only through H1, which
   condenses on remove: 1.380 on the headline set, no change on `r64_range`.
   The census puts `r64_range`'s excess in sparse pages, with no excess
   `mem_used()` for condensing to remove.
3. **No change.** `shrink_to_fit()` already returns every page with no live
   block. What it cannot return is measured above: on the headline set,
   38.69 MB of free blocks on 13,140 partly used pages.

Next measurements the numbers name, none of them run here: a census of the H1
build, to see which pages condensing frees; a node-form attribution of the
sparse classes (the raw 128-byte class on the headline set); a call-site
attribution of the fresh build's free blocks (§3.3).

## 4. Explicit `compact()`: gate evaluation

`ExpanseSet::compact()` and `ExpanseMap::compact()` (64-bit) rebuild the
surviving keys into a new allocator and drop the old tree. The gates,
predictions and expected losses were locked in [`METHODOLOGY.md`](METHODOLOGY.md)
§12 before any `compact()` code existed; none was changed after results were
seen. **Verdict (METHODOLOGY §12.5): G-ins is not met, so `compact()` does not
meet the pre-registered bar to ship**; the other four gates are met. Whether
to accept the G-ins outcome is a maintainer decision, recorded in §4.2.

| Gate | Verdict | Instrument |
|---|---|---|
| G-held | **met**: 0.5770–0.9982 of held_fresh on all 42 cells; `mem_used()` clause met on all 42 | `remove_retention.rs`, laptop, §4.1 |
| G-peak | **met** on all 42 cells; no-free clause met on all 42 | `remove_retention.rs`, laptop, §4.1 |
| G-cost | **met**: set 16,261,644 against 16,681,923 instructions, map 16,833,877 against 40,334,038, each compact arm against the rebuild arm of the same run | CI x86_64 Callgrind, §4.2 |
| G-ins | **not met**: three existing arms moved by more than 0.1%, all downward (§4.2) | CI Callgrind, §4.2 |
| G-valid | **met** (§4.3) | CI, §4.3 |

**Reproduce.** `EXPANSE_COMMIT=<sha> EXPANSE_RUSTC="$(rustc -V)" cargo run --release -p expanse-trie --example remove_retention -- --json docs/benchmarks/remove_retention/results/compact_retention.json`.

### 4.1 G-held and G-peak

The same grid, keys and removal orders as §1 and §3. Per cell the drained tree
is built a second time (asserted byte-identical to the first drain), and
`compact()` is called on it without `shrink_to_fit()`; held before is its
`mem_held()` at the call. Every §1 and §3 field of the 42 cells reproduces
`results/census_rebuild.json` exactly, census objects included (derived: field
by field comparison of the two artifacts; the new censuses add only
`live_allocs` and `total_allocs`). Every figure is an exact count with no
interval (§8.4) (measured: Apple M1, macOS, rustc 1.98.1, `c953ea80`; workload:
example_remove_retention; artifact
[`results/compact_retention.json`](results/compact_retention.json)). Peak RSS
of the full grid was 1,005 MB (measured: Apple M1, `/usr/bin/time -l`,
`c953ea80`).

"Predicted" is `compact_bounds.predicted_compact_over_fresh` over the fresh
build's census in `results/census_rebuild.json` (METHODOLOGY §12.4). "Peak" is
held before plus held after compact, the G-peak reading; the ceiling is held
before + 1.10 × held_fresh. "Heap peak added" is the counting global
allocator's highest requested bytes during `compact()` above the live figure
before it, and "buffer" the key buffer `sort_buffer_bytes` derives (8 B per
key, 16 B per entry).

| Cell | Flavor | held before (MB) | held after compact (MB) | held_fresh (MB) | compact ÷ held_fresh | predicted | rebuilt ÷ held_fresh | used compact / fresh / rebuilt (B) | peak (MB) | G-peak ceiling (MB) | heap peak added (MB) | buffer (MB) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `headline` | set | 94.65 | 8.37 | 12.69 | 0.6595 | 0.6595 | 0.6595 | 8,211,872 / 8,211,872 / 8,211,872 | 103.02 | 108.61 | 16.37 | 8.00 |
| `headline` | map | 104.79 | 17.72 | 24.67 | 0.7180 | 0.7180 | 0.7190 | 17,583,680 / 17,583,680 / 17,583,680 | 122.51 | 131.94 | 33.61 | 16.00 |
| `r64_sorted` | set | 90.63 | 8.37 | 12.69 | 0.6595 | 0.6595 | 0.6595 | 8,211,872 / 8,211,872 / 8,211,872 | 99.00 | 104.59 | 16.37 | 8.00 |
| `r64_sorted` | map | 100.50 | 17.72 | 24.67 | 0.7180 | 0.7180 | 0.7190 | 17,583,680 / 17,583,680 / 17,583,680 | 118.22 | 127.64 | 33.61 | 16.00 |
| `r64_range` | set | 84.36 | 21.58 | 26.64 | 0.8100 | 0.8100 | 0.8100 | 20,993,296 / 20,993,296 / 20,993,296 | 105.94 | 113.67 | 29.59 | 8.00 |
| `r64_range` | map | 92.37 | 24.52 | 29.21 | 0.8394 | 0.8394 | 0.8407 | 23,886,704 / 23,886,704 / 23,886,704 | 116.89 | 124.50 | 40.52 | 16.00 |
| `r64_2m_to_1m` | set | 42.85 | 8.36 | 12.68 | 0.6596 | 0.6596 | 0.6596 | 8,210,688 / 8,210,688 / 8,210,688 | 51.22 | 56.80 | 16.37 | 8.00 |
| `r64_2m_to_1m` | map | 42.11 | 17.71 | 24.66 | 0.7182 | 0.7182 | 0.7192 | 17,581,376 / 17,581,376 / 17,581,376 | 59.82 | 69.23 | 33.60 | 16.00 |
| `r64_4m_to_1m` | set | 110.33 | 8.36 | 12.70 | 0.6581 | 0.6581 | 0.6581 | 8,209,904 / 8,209,904 / 8,209,904 | 118.69 | 124.30 | 16.36 | 8.00 |
| `r64_4m_to_1m` | map | 126.57 | 17.70 | 24.67 | 0.7176 | 0.7176 | 0.7187 | 17,576,560 / 17,576,560 / 17,576,560 | 144.28 | 153.71 | 33.59 | 16.00 |
| `r64_to_2m` | set | 91.09 | 28.11 | 40.82 | 0.6886 | 0.6886 | 0.6886 | 27,459,984 / 27,459,984 / 27,459,984 | 119.20 | 135.99 | 44.11 | 16.00 |
| `r64_to_2m` | map | 101.17 | 40.11 | 52.65 | 0.7619 | 0.7619 | 0.7625 | 39,558,096 / 39,558,096 / 39,558,096 | 141.29 | 159.08 | 72.12 | 32.00 |
| `r64_to_320k` | set | 95.50 | 3.63 | 4.35 | 0.8343 | 0.8343 | 0.8343 | 3,577,424 / 3,577,424 / 3,577,424 | 99.13 | 100.29 | 6.19 | 2.56 |
| `r64_to_320k` | map | 105.60 | 6.93 | 8.20 | 0.8456 | 0.8456 | 0.8511 | 6,843,344 / 6,843,344 / 6,843,344 | 112.53 | 114.62 | 12.03 | 5.12 |
| `r64_1m_to_312k` | set | 12.82 | 3.57 | 4.28 | 0.8343 | 0.8343 | 0.8343 | 3,515,696 / 3,515,696 / 3,515,696 | 16.39 | 17.52 | 6.07 | 2.50 |
| `r64_1m_to_312k` | map | 14.81 | 6.80 | 8.01 | 0.8487 | 0.8487 | 0.8543 | 6,710,480 / 6,710,480 / 6,710,480 | 21.61 | 23.63 | 11.78 | 5.00 |
| `r62` | set | 35.20 | 20.21 | 25.55 | 0.7909 | 0.7909 | 0.7909 | 19,683,696 / 19,683,696 / 19,683,696 | 55.41 | 63.30 | 28.21 | 8.00 |
| `r62` | map | 69.70 | 23.76 | 28.81 | 0.8247 | 0.8247 | 0.8260 | 23,183,824 / 23,183,824 / 23,183,824 | 93.46 | 101.39 | 39.77 | 16.00 |
| `r56` | set | 98.55 | 7.38 | 11.17 | 0.6608 | 0.6608 | 0.6608 | 7,183,920 / 7,183,920 / 7,183,920 | 105.93 | 110.83 | 15.38 | 8.00 |
| `r56` | map | 104.29 | 16.79 | 23.77 | 0.7063 | 0.7063 | 0.7080 | 16,555,312 / 16,555,312 / 16,555,312 | 121.08 | 130.43 | 32.55 | 16.00 |
| `r56_sorted` | set | 94.54 | 7.38 | 11.17 | 0.6608 | 0.6608 | 0.6608 | 7,183,920 / 7,183,920 / 7,183,920 | 101.92 | 106.82 | 15.38 | 8.00 |
| `r56_sorted` | map | 100.02 | 16.79 | 23.77 | 0.7063 | 0.7063 | 0.7080 | 16,555,312 / 16,555,312 / 16,555,312 | 116.80 | 126.16 | 32.55 | 16.00 |
| `r56_range` | set | 88.26 | 21.59 | 27.90 | 0.7736 | 0.7736 | 0.7736 | 20,999,104 / 20,999,104 / 20,999,104 | 109.84 | 118.95 | 29.59 | 8.00 |
| `r56_range` | map | 91.88 | 24.44 | 29.10 | 0.8401 | 0.8401 | 0.8418 | 23,810,672 / 23,810,672 / 23,810,672 | 116.32 | 123.89 | 40.44 | 16.00 |
| `seq_shuffled` | set | 1.08 | 1.02 | 1.08 | 0.9506 | 0.9506 | 0.9506 | 1,004,928 / 1,004,928 / 1,004,928 | 2.10 | 2.26 | 9.03 | 8.00 |
| `seq_shuffled` | map | 32.75 | 11.28 | 11.32 | 0.9967 | 0.9967 | 1.0000 | 11,029,344 / 11,029,344 / 11,029,344 | 44.04 | 45.20 | 27.28 | 16.00 |
| `seq_sorted` | set | 1.08 | 1.02 | 1.08 | 0.9506 | 0.9506 | 0.9506 | 1,004,928 / 1,004,928 / 1,004,928 | 2.10 | 2.26 | 9.03 | 8.00 |
| `seq_sorted` | map | 11.32 | 11.28 | 11.32 | 0.9967 | 0.9967 | 1.0000 | 11,029,344 / 11,029,344 / 11,029,344 | 22.61 | 23.77 | 27.28 | 16.00 |
| `seq_range` | set | 0.13 | 0.07 | 0.13 | 0.5770 | 0.5770 | 0.5770 | 64,832 / 65,792 / 64,832 | 0.20 | 0.26 | 8.08 | 8.00 |
| `seq_range` | map | 9.79 | 8.59 | 8.65 | 0.9924 | 0.9924 | 1.0000 | 8,564,864 / 8,564,864 / 8,564,864 | 18.38 | 19.31 | 24.59 | 16.00 |
| `sparse_shuffled` | set | 20.78 | 20.60 | 20.64 | 0.9978 | 0.9978 | 0.9978 | 20,253,632 / 20,253,632 / 20,253,632 | 41.38 | 43.49 | 28.60 | 8.00 |
| `sparse_shuffled` | map | 20.77 | 20.60 | 20.64 | 0.9982 | 0.9982 | 1.0000 | 20,253,632 / 20,253,632 / 20,253,632 | 41.37 | 43.47 | 36.60 | 16.00 |
| `sparse_sorted` | set | 20.64 | 20.60 | 20.64 | 0.9978 | 0.9978 | 0.9978 | 20,253,632 / 20,253,632 / 20,253,632 | 41.24 | 43.35 | 28.60 | 8.00 |
| `sparse_sorted` | map | 20.64 | 20.60 | 20.64 | 0.9982 | 0.9982 | 1.0000 | 20,253,632 / 20,253,632 / 20,253,632 | 41.24 | 43.34 | 36.60 | 16.00 |
| `sparse_range` | set | 16.38 | 16.32 | 16.38 | 0.9960 | 0.9960 | 0.9960 | 16,314,816 / 16,314,816 / 16,314,816 | 32.70 | 34.41 | 24.32 | 8.00 |
| `sparse_range` | map | 16.38 | 16.32 | 16.38 | 0.9965 | 0.9965 | 1.0000 | 16,314,816 / 16,314,816 / 16,314,816 | 32.69 | 34.39 | 32.32 | 16.00 |
| `clust_shuffled` | set | 1.28 | 1.16 | 1.28 | 0.9103 | 0.9103 | 0.9103 | 1,130,400 / 1,130,400 / 1,130,400 | 2.44 | 2.69 | 9.17 | 8.00 |
| `clust_shuffled` | map | 32.91 | 11.41 | 11.48 | 0.9939 | 0.9939 | 0.9993 | 11,154,816 / 11,154,816 / 11,154,816 | 44.32 | 45.54 | 27.42 | 16.00 |
| `clust_sorted` | set | 1.28 | 1.16 | 1.28 | 0.9103 | 0.9103 | 0.9103 | 1,130,400 / 1,130,400 / 1,130,400 | 2.44 | 2.69 | 9.17 | 8.00 |
| `clust_sorted` | map | 11.48 | 11.41 | 11.48 | 0.9939 | 0.9939 | 0.9993 | 11,154,816 / 11,154,816 / 11,154,816 | 22.89 | 24.11 | 27.42 | 16.00 |
| `clust_range` | set | 1.28 | 0.37 | 0.45 | 0.8190 | 0.8190 | 0.8190 | 353,088 / 353,088 / 353,088 | 1.65 | 1.78 | 8.37 | 8.00 |
| `clust_range` | map | 10.14 | 8.63 | 8.72 | 0.9901 | 0.9901 | 0.9986 | 8,603,136 / 8,603,136 / 8,603,136 | 18.77 | 19.73 | 24.64 | 16.00 |

What the table shows (same artifact):

- **G-held.** held ÷ held_fresh after `compact()` is 0.5770 to 0.9982, the
  pre-registered prediction to four decimals on every cell. On the headline
  cell the set goes from 5.318 after `shrink_to_fit()` (§3.1) to 0.6595 and the
  map from 3.164 to 0.7180; on `r64_range`, where condensing moved nothing
  (§2), from 2.647 to 0.8100 (set) and 2.731 to 0.8394 (map). The map's
  `mem_used()` after `compact()` equals the fresh build's on all 21 map cells.
  The set's equals the `from_sorted_iter` rebuild's on all 21 set cells and the
  fresh build's on 20; on `seq_range` it is 64,832 B against the fresh
  65,792 B, the difference seen before the lock (METHODOLOGY §12.2).
- **The map holds less than its ascending-insert rebuild** on all 21 map
  cells, by up to 0.0085 of held_fresh (`clust_range`, 0.9901 against
  0.9986): the bulk builder frees nothing, the insert path does.
- **No free during the build.** On all 42 cells the compacted allocator's live
  allocation count equals its total allocation count, and its `mem_held()`
  equals `compact_bounds.no_free_held` of its own census, the fewest pages its
  live blocks fit on.
- **G-peak.** The peak is held before plus held after, so with no free the
  gate reduces to held after ≤ 1.10 × held_fresh, the G-held ratio clause.
  The closest cell is the `r64_to_320k` set, 99.13 MB against a ceiling of
  100.29 MB, because held before is large there and the ceiling adds only
  1.10 × held_fresh.
- **The key buffer is outside G-peak, as pre-registered.** The heap peak
  added by the call equals held after plus the buffer to within −0.24 MB to
  +0.01 MB on every cell (derived: `compact_heap_peak_extra − held_compact −
  sort_buffer_bytes`). The negative side is on map cells, where requested
  bytes run below `mem_held()`'s accounting; which requests account for it is
  not attributed here. At M = 1M keys the buffer is 8.00 MB (set) and
  16.00 MB (map), 0.96 and 0.90 of the new tree's held bytes on the headline
  cell.

### 4.2 G-cost and G-ins

Instruction counts from the x86_64 `instruction-counts` job of [run 36215845201](https://github.com/orieg/expanse/actions/runs/36215845201) at
`1e6bd27b`, the PR head it ran on (measured: CI x86_64 Callgrind,
deterministic exact counts, no interval). Every count below is the job's
PR-head pass against its `main_base` pass in the same job.

**G-cost: met.** Same run, per surviving key (62,500):

| Flavor | `*_compact_drained` | `*_rebuild_drained` | compact per key | rebuild per key | compact ÷ rebuild |
|---|---|---|---|---|---|
| set | 16,261,644 | 16,681,923 | 260.2 | 266.9 | 0.975 |
| map | 16,833,877 | 40,334,038 | 269.3 | 645.3 | 0.417 |

The set arm is the `from_sorted_iter` builder without the sort check and with
an exact-capacity buffer. The map arm is the bulk builder against the
rebuild's one insert per entry. Where the saved instructions go is not
attributed here.

**G-ins: not met.** Of the 169 existing arms of the `instructions` harness, 15
moved against `main_base`, three by more than the 0.1% review threshold, all
downward:

| Arm | head | `main_base` | change |
|---|---|---|---|
| `set_clone/sequential` | 2,598,886 | 2,799,992 | −7.182% |
| `set_clone/random` | 15,482,213 | 15,870,083 | −2.444% |
| `set_rebuild_drained/random60` | 16,681,923 | 16,960,355 | −1.642% |

The other twelve moved by at most 0.043% (`map_clone/random` +0.043%,
`blobmap_insert_inline/random` +0.043%, `blobmap_insert/random` +0.039%,
`map_rebuild_drained/random60` +0.010%, the rest under 0.004%). The 50 arms of
the search harness are unchanged. `callgrind-smoke`: one of its arms moved,
`strmap_insert/routes` +0.00006%; the AArch64 Callgrind job: one,
`strmap_insert/routes` +0.00012%.

The three arms all build a set through `ExpanseSet::from_sorted_iter`
(`Clone` is `from_sorted_iter`), whose builder `compact()` now also calls. No
function on that path changed its source; the change is therefore in code
generation (AGENTS.md §6), and which function's instructions it removed is
**not attributed**: no base-against-head `callgrind_annotate` or `objdump` diff
was taken. The pre-registered falsifier counts a move in either direction, so
the gate is recorded as not met, not re-thresholded (§8.19). The options it
leaves are the maintainer's: accept the outcome as a change to the gate
(which relabels this result `INTERMEDIATE`), or change the code so the
existing arms stay within 0.1% and re-measure against the same gate.

### 4.3 G-valid

Met on the same run ([run 36215845201](https://github.com/orieg/expanse/actions/runs/36215845201), conclusion `success`, `CI Gate / All Checks Passed`
green at `1e6bd27b`):

- the Tier-1 Miri shards ran the six `compact_` tests: shard 1
  (`set::tests::compact_*`) 36 passed in 488 s of test time, shard 2
  (`map::tests::compact_*`) 76 passed in 587 s, each under the job's
  15-minute limit;
- the workspace test jobs ran `set_matches_btreeset_with_compact` and
  `map_matches_btreemap_with_compact` at `PROPTEST_CASES=500`, and the
  `compact_` unit tests, which run the structural validator after every
  `compact()`;
- the ASan job is green;
- the instrument ran the validator after `compact()` on all 42 cells (§4.1).
