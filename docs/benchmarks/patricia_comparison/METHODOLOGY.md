# Patricia and Radix Tries vs. Expanse: Methodology and Pre-Registration

## 1. Question and twins

How do `ExpanseMap` (`u64` keys) and `ExpanseStrMap` (string keys) compare with
Patricia (compressed radix) tries? No single implementation answers that: a
radix trie's cost is set mostly by how a node finds its child, so the suite
measures three designs that differ exactly there. All three are pinned in
`crates/expanse/Cargo.toml`, and every statement below is about these versions.

| Twin | Child lookup | Notes read from the source |
|---|---|---|
| [`patricia_tree`](https://github.com/sile/patricia_tree) `=0.10.2` (MIT) | byte-sorted, singly linked **sibling list**, walked linearly (`node.rs` `Node::get`) | the crate calls itself "patricia tree (a.k.a, radix tree)"; each node is one allocation `flags, label_len, label, value?, child?, sibling?` (`new_with_boundary`) |
| [`fast_radix_trie`](https://github.com/bluecatengineering/fast_radix_trie) `=1.2.0` (MIT) | inline **child-pointer array** | a fork of `patricia_tree` (its README); child count stored as `u8` |
| [`qp-trie`](https://github.com/sdleffler/qp-trie-rs) `=0.8.2` (MPL-2.0) | **nybble** branch with a popcount-indexed sparse child array | owns its keys; iterates low nybble first, so traversal is not byte-ordered |

`qp-trie`'s licence is admitted by a crate-scoped `deny.toml` exception: it is
a dev-dependency of the benchmarks only.

**`fast_radix_trie` cannot hold a node with 256 children.** Its child count is
a `u8` (`node.rs`: `children_len: self.children_len() as u8 + 1`, guarded only
by a `debug_assert!`). With its default `realloc` feature, inserting a 256th
child panics inside the crate's unsafe node code, and the unwind aborts the
process. Without `realloc`, the count wraps to 0: 256 one-byte keys report
`len() == 256` and 256 of them fail `get`. Both behaviours were reproduced
against the crate outside this repository. Every `u64` key set here that
spans a full byte range needs such a node, so the harnesses check the key
set's widest node first (`max_fanout`) and record `fast_radix_trie` as
`invalid` with the reason, without building it. This is a defect of that
version, recorded as `INVALID`, never as a loss or a win. It has not been
reported upstream.

Integer keys reach the twins as big-endian 8 bytes, so byte order is numeric
order. The claims are about these three implementations, not about Patricia
tries in general.

## 2. Math-first envelope (AGENTS.md §8.8 commit 1)

`scripts/patricia_envelope.py` (run by `scripts/gate.sh`) rebuilds the
compressed trie for a key set and derives:
- the exact bytes and node count `patricia_tree` allocates;
- the nodes a `patricia_tree` hit visits (root, descents, sibling steps);
- the nodes on the root-to-value path, which is what an inline child array
  visits;
- `qp-trie`'s branch points per key, over nybbles taken low nybble first, the
  order that crate branches in;
- `max_fanout`, which predicts `fast_radix_trie` validity.

It reproduces the harness key streams exactly for every distribution but
`zipfian`.

The byte census is exact for keys of at most 255 bytes. Longer labels are
split order-dependently, and `census` refuses them and the empty key. Its tests
pin hand-derived node sizes and small trees (including a key that is a prefix
of others), the nybble order, and three cells of different shape that the
`patricia_memory` allocator hook records exactly, in both build orders:

| Cell | Census bytes / nodes | Hook (requested bytes / allocations) |
|---|---|---|
| sequential, 10k | 240,664 / 10,042 | 240,664 / 10,042 |
| uniform_random, 10k | 323,720 / 10,945 | 323,720 / 10,945 |
| path, 35-byte prefix, 10k | 379,864 / 13,746 | 379,864 / 13,746 |

`python3 scripts/patricia_envelope.py --report` gives (derived; path keys only
up to 100k, the script's budget):

| n | key set | `patricia_tree` B/key | `patricia_tree` nodes / hit | nodes on path | `qp-trie` branches | max fanout |
|---|---|---|---|---|---|---|
| 10k | sequential | 24.07 | 150.34 | 4.00 | 4.00 | 256 |
| 10k | clustered | 24.08 | 138.33 | 5.00 | 4.00 | 256 |
| 10k | uniform_random | 32.37 | 148.95 | 3.14 | 4.04 | 256 |
| 10k | sparse_stride | 24.07 | 150.34 | 4.00 | 4.00 | 256 |
| 10k | path, prefix 8 / 35 / 128 / 240 | 37.98 / 37.99 / 38.00 / 38.01 | 28.97 | 6.04 | 6.33 | 16 |
| 100k | sequential | 24.06 | 239.35 | 5.00 | 5.00 | 256 |
| 100k | clustered | 24.08 | 176.29 | 5.32 | 5.32 | 256 |
| 100k | uniform_random | 30.54 | 232.04 | 3.79 | 4.85 | 256 |
| 100k | sparse_stride | 24.06 | 239.35 | 5.00 | 5.00 | 256 |
| 100k | path, every prefix length | 37.94 | 35.99 | 6.85 | 7.74 | 16 |
| 1M | sequential | 24.06 | 265.52 | 5.00 | 5.00 | 256 |
| 1M | clustered | 24.08 | 263.15 | 5.99 | 6.16 | 256 |
| 1M | uniform_random | 25.51 | 266.45 | 4.06 | 5.65 | 256 |
| 1M | sparse_stride | 24.06 | 265.52 | 5.00 | 5.00 | 256 |

What the envelope says:
1. **`patricia_tree` pays for sibling-list walks.** It visits 138–266 nodes per
   `u64` hit and 29–36 per path hit, while an array- or nybble-indexed trie
   passes 3–8 nodes. Expanse descends at most one level per key byte, so at
   most 8 for `u64`. Visits are node counts, not cache misses. The 256-node
   top list is about 6 KB and should stay cached; that part is not derived.
2. **`patricia_tree` memory is not a floor.** `u64` bytes/key follow the leaf
   label length. Uniform keys go 32.37 → 30.54 → 25.51 B/key from 10k to 1M as
   leaves shrink from 7- to 6-byte labels, while dense key sets sit at 24.
3. **A shared prefix costs a radix tree nothing per key.** `patricia_tree`
   bytes/key and every visit count are flat across prefix lengths 8–240, since
   the prefix is one label. `ExpanseStrMap` descends it eight bytes per level.
   So Expanse's relative position should worsen as the prefix grows. That is
   the regime where a twin can win (§8.3, C-b), and the prefix sweep is how a
   crossover would show.
4. **`fast_radix_trie` is invalid wherever max fanout is 256:** every `u64`
   key set here except, possibly, `zipfian`, which is not reproduced. Path keys
   have fanout 16, so it is valid there.

Not derived: time per operation on any arm; memory for `fast_radix_trie`,
`qp-trie` and Expanse; the effect of build order on layout.

## 3. Pre-registration (AGENTS.md §8.8 commit 2)

**This revision supersedes the first pre-registration** (commit `docs(patricia): pre-registration, runner and suite README`),
before any reference-host run. That version had one twin and a miss generator
that put sequential and sparse misses above every present key. Its text stays
in git history.

**What was seen before these predictions were written:**
- Local `--quick` runs of the first harness version, on a laptop with no load
  snapshot and contended: `patricia_tree` against Expanse, for `u64` lookup,
  insert and 1k/10k memory, and 35-byte path lookup.
- An internal synthetic-persona review, which ran the same smoke harnesses and
  a build-order layout diagnostic on `patricia_tree`.
- For this revision: `patricia_tree` byte counts on the three pinned 10k cells,
  and `fast_radix_trie`'s validity status. No timing and no memory figure of
  `fast_radix_trie`, `qp-trie` or the new harnesses was observed.

The `patricia_tree` predictions are therefore not blind. The `fast_radix_trie`
and `qp-trie` predictions and the prefix-length trend (T1) are.

**Evaluated cells:** the reference host, N ∈ {100k, 1M}, both build orders
unless stated. The 10k cells, and anything run with `--quick`, are
`NOT_PREREGISTERED`.

**Verdict labels.** A timing statement concerns the geometric-mean ratio
(Expanse ns ÷ twin ns) and its BCa 95% interval over per-round log ratios.
- `PASS`: the interval lies wholly on the predicted side of 1.0.
- `REFUTED`: the interval lies wholly on the other side.
- `BOUNDARY_RESULT`: the interval spans 1.0.
- `INVALID`: the twin failed validation, so the cell was not timed.
- `UNPREDICTED_LOSS`: any cell with no prediction whose interval lies wholly
  above 1.0.

A memory statement is an exact comparison of requested bytes, `PASS` or
`REFUTED`. No result here has had independent peer review. The internal review
used synthetic personas and is not peer review.

| ID | Cells | Prediction |
|---|---|---|
| P1 | `patricia_lookup_hit`, every `u64` distribution | Expanse faster than `patricia_tree` |
| P2 | `patricia_lookup_miss`, every `u64` distribution | Expanse faster than `patricia_tree` |
| P3 | `patricia_insert`, every `u64` distribution, both orders | Expanse faster than `patricia_tree` |
| P4 | `patricia_scan` full traversal, every `u64` distribution | Expanse faster than `patricia_tree` |
| P5 | `patricia_lookup_hit`, `patricia_insert`, `patricia_memory`, `patricia_scan`, `u64` `sequential` / `clustered` / `uniform_random` / `sparse_stride` | `fast_radix_trie` `INVALID` (max fanout 256) |
| P6 | every harness, every path cell | `fast_radix_trie` valid |
| P7 | `patricia_memory`, `sequential` / `clustered` / `uniform_random` / `sparse_stride` at 100k and 1M, and every path cell at 100k | `patricia_tree` requested bytes equal the census exactly, in both orders |
| P8 | `patricia_memory`, `sequential` and `clustered` | Expanse fewer requested bytes than `patricia_tree`, both orders |
| T1 | `patricia_string`, generator order, per valid twin | the ratio rises with prefix length. `PASS` iff the 240-byte interval lies wholly above the 8-byte interval; `INTERMEDIATE` if the four point estimates rise monotonically but the intervals overlap; `REFUTED` otherwise |

**Amendment A1 (2026-09-22, after the first local `--quick` smoke run at
the commit `docs(patricia): re-register predictions for three twins before any run`; no reference-host data).** P5 originally read "every harness". But
`patricia_lookup_miss` builds a random half of a 2n draw, not the key sets the
envelope derived max fanout for. In the smoke run, `fast_radix_trie` was valid
on that harness's half-split `clustered` set. P5 is narrowed to the harnesses
whose key sets the envelope covers. The half-split cells carry no validity
prediction; their status is reported as measured. No other prediction changed.

**Amendment A2 (2026-09-24, after the reference-host runs at `c9f1600d` and
`668c28d7`; refs #1096).** The prefix-scan cells change the Expanse surface
they time. The subject arm uses `ExpanseStrMap::cursor_prefix`, which ends each
walk at the prefix boundary without a per-key comparison. Before, it used
`cursor_at_or_after` with a `starts_with` per yielded key. The twins still use
their own prefix reads, so the Expanse arm now does what `iter_prefix` does
(§8.3). The earlier walk stays in the same cells as an `expanse_unbounded` arm
on its own map, so the effect of the surface change is measured in the same
rounds and not inferred across runs. Every row records `expanse_surface`. The
prefix scan carries no directional prediction (above), so no prediction
changes, and cells from runs before this amendment are not comparable with
later ones on the Expanse arm. The §4 *Scans* item describes the subject arm
as before A2.

**Amendment A3 — diagnostic D1 (2026-09-25, registered before any counter
data; refs #1096).** At n = 1,000,000 the prefix-scan cell takes 41.4 ns per
yielded entry when the map is built in generator order and 14.7 ns when it is
built sorted: 161,445 and 57,177 ns per prefix over 3,901.1 entries per prefix
(measured: i9-12900F reference host, 6000b4a1; run 36092319875). Both builds
hold the same key set, so the same node census
(`test_mem_used_order_invariant`) and the same walk over it; that the two
retire the same instructions is derived, not measured. D1 asks whether the
26.7 ns per entry between them is memory placement: a generator-order build
allocates nodes and suffix leaves in insertion order, so a scan in key order
visits them scattered across the heap.

*Instrument.* `scripts/perf_counters.py`, arms `strmap_prefix_scan` and
`strmap_prefix_scan_sorted` of `examples/perf_point_lookup.rs` (the suite's
path keys, its 64 prefixes, `cursor_prefix`), `--pops 1000000 --hit-pcts 100
--passes 200 --runs 10`, the P-core PMU, the driver's default event set. Counts
are `probe − build` per run, divided by the entries the passes yield
(200 × 249,670). Each order's per-entry figure carries a BCa 95% interval over
the 10 runs.

*Predictions*, generator order against sorted:

| # | Counter per entry | Prediction | Falsified if |
|---|---|---|---|
| D1a | `instructions` | the two orders agree within 2% | the point estimates differ by more than 2%: the builds do not run the same walk, and nothing below is attributable to placement |
| D1b | `mem_load_retired.l3_miss` + `dTLB-load-misses` | generator exceeds sorted, intervals disjoint | the generator interval of either counter is not above the sorted one, for both counters |
| D1c | `cycle_activity.stalls_l3_miss` | generator − sorted is at least half of generator − sorted `cycles` | below half: L3-miss stalls do not carry most of the gap, whatever D1b says |

The 2% in D1a is a tolerance for the counter's run-to-run spread, set before
the data; the instruction streams are expected to be identical. D1b predicts
only a direction because the number of misses the gap implies depends on the
host's memory latency and on how many misses overlap, neither measured here.
At an assumed 80–100 ns per DRAM access (not measured on this host), fully
serialised misses would account for the gap at 0.27–0.33 extra misses per
entry; overlapped misses would need more. That range is context for reading
D1b, not a threshold.

If D1b is falsified the gap is reported as unexplained, and no placement
remedy (co-allocation, prefetch, a packed-suffix default) is proposed on its
strength. If D1b and D1c hold, placement is the measured cause, and a remedy
is a separate, ablated change.

**Amendment A4 — diagnostic D2 (2026-10-04, registered before any huge-page counter
data; refs #1096).** Diagnostic D1 established that the 1M prefix scan's
build-order gap (41.4 vs 14.7 ns per entry; workload: patricia_scan) is driven by memory placement
rather than instruction path disparity: `instructions` agreed to 0.06% (D1a PASS),
while `dTLB-load-misses` showed 0.8203 vs 0.0097 misses per entry (85× difference)
and `mem_load_retired.l3_miss` showed 0.2360 vs −0.0000 misses per entry (D1b PASS).
In D1 stalls (`counters_prefix_scan_d1_stalls.json` at commit `c3e54ec2`), L3 miss stalls
accounted for 52.50 cycles per entry of the 85.94 cycle gap (61.1%).

### §3.4.1 Precedent from #782 (Masstree Comparison) & Why D2 is Distinct
In #782 (commit `ee6b4290`), transparent huge pages were evaluated for random string point lookup (`strmap_get` vs integer `map_get` at N = 10⁶):
> "Translation misses go to **zero** on both arms and the gap does not close:
> 3.44× → 3.29×. The misses were real and removing them is worth about 11% of
> the string arm, but **they were not the cost** — 96% of the gap survives their
> complete elimination. An `madvise(MADV_HUGEPAGE)` change to the engine's
> arenas would buy a real, small win and would not address what #724 is about. [...]
> With translation eliminated as a variable, the surviving memory signal is unambiguous:
> `LLC-load-misses` 2.899 against `map_get`'s 0.107, and `L1-dcache-load-misses` 8.556 against 3.918,
> both under huge pages. Read with `strmap_get_counter` — no suffix leaf, still 1.156 `LLC-load-misses`
> — the remaining target is the **string tree's descent and node layout**, neither the leaf nor the allocator."
> (`docs/benchmarks/masstree_comparison/README.md:644-658`)

Diagnostic D2 is not a repetition of #782 for four structural reasons:
1. **Workload Traversal Structure**: #782 evaluated single-key random point lookup (`strmap_get`),
   where each probe traverses arbitrary tree depth across disparate nodes globally across the trie.
   D2 evaluates sequential prefix range iteration (`strmap_prefix_scan`), where a single initial
   seek descends to the prefix root and subsequent iterations walk horizontally across adjacent sibling
   leaves and child branches within a localized subtree.
2. **Comparison Arms**: #782 compared string keys (`strmap_get`) against u64 integer keys (`map_get`).
   D2 evaluates generator draw order (`strmap_prefix_scan`) against sorted build order
   (`strmap_prefix_scan_sorted`) within the identical string map structure holding the identical 1M path keys.
3. **Spatial Locality Hypothesis**: In D1, generator-order build exhibited 0.8203 dTLB load misses per
   yielded entry (vs 0.0097 sorted). A sequential prefix walk touching adjacent entries might have
   suffered from virtual page dispersal across 4 KiB boundaries that 2 MiB contiguous physical pages
   resolve; alternatively, dispersal across disparate 64-byte cache lines might persist. #782 did not
   and could not answer whether prefix-scan range traversal's build-order gap survives 2 MiB page backing.
4. **Treatment Verification Precondition**: #782 established the mandatory verification requirement:
   verifying that `AnonHugePages` / `thp_fault_alloc` actually increased before interpreting the result.
   D2 adopts this exact protocol as a required precondition.

### §3.4.2 Instrument Preconditions & Protocol
- **Instrument Precondition**: `GLIBC_TUNABLES=glibc.malloc.hugetlb=1` only invokes `madvise(MADV_HUGEPAGE)`.
  Whether 2 MiB pages actually back the tree depends on host THP mode (`/sys/kernel/mm/transparent_hugepage/enabled`)
  and fragmentation. The run must record `/sys/kernel/mm/transparent_hugepage/{enabled,defrag}` and non-zero
  `AnonHugePages` (from `smaps_rollup`) or positive `thp_fault_alloc` deltas (from `/proc/vmstat`).
  Any 2m arm without verified huge-page allocation is **VOID**, not a measurement.
- **Pin & Protocol (quoted from D1 section & artifact provenance)**:
  - From `METHODOLOGY.md` §3 Amendment A3: *"the P-core PMU, the driver's default event set. Counts are `probe − build` per run, divided by the entries the passes yield (200 × 249,670). Each order's per-entry figure carries a BCa 95% interval over the 10 runs."* (`METHODOLOGY.md:192-195`)
  - From `counters_prefix_scan_d1.json` provenance: `cpu_core_cpus: "0-15"`, `scaling_governor_pin_set: "0-15"`, `scaling_governor_pin_source: "EXPANSE_BENCH_PIN_APPLIED"`, `columns: "mean over paired runs of (probe phase - build phase) counts, BCa 95% interval over those per-run differences"`.
  - The driver (`scripts/perf_counters.py`) selects the performance-core PMU (`cpu_core`) and applies `pin_for("cpu_core")` (`taskset -c 0-15`, confining the workload to the P-core PMU's CPU set as recorded in D1).
- **Two-Run Protocol**: Two independent sequential runs on the quiet reference host (`loadavg <= 1.0`,
  `foreign_busy_cpus == 0`), evaluated over BCa 95% bootstrap confidence intervals (2,000 resamples).
- **Harness**: `scripts/perf_counters.py`, running 4 cells (baseline and `--hugetlb` for both `strmap_prefix_scan`
  and `strmap_prefix_scan_sorted`) of `crates/expanse/examples/perf_point_lookup.rs` (the suite's path keys,
  64 prefixes, `cursor_prefix`), `--pops 1000000 --hit-pcts 100 --passes 200 --runs 10`. Counts are `probe − build`
  per run divided by yielded entries (200 × 249,670 = 49,934,000 ops).
- **Maintainer Dispatch**:
  `/benchmark patricia_d2_paging` (runs on `main` after merge)
  or
  `gh workflow run bench_baremetal.yml --ref main -f suite=patricia_d2_paging`

### §3.4.3 Pre-Registered Thresholds (Derived in `scripts/patricia_d2_bounds.py`)
All thresholds are derived mathematically in committed Python (`scripts/patricia_d2_bounds.py`) from D1 baseline
artifacts (`counters_prefix_scan_d1.json` and `counters_prefix_scan_d1_stalls.json` at commit `c3e54ec2`),
with pinned unit tests (Rule 12 / §1.3). Every clause is evaluated strictly on BCa 95% bootstrap confidence interval bounds (Rule 1 / §1.1):

| # | Clause | Metric | Evaluated On | Condition | Derivation Source | Falsified if |
|---|---|---|---|---|---|---|
| D2a | dTLB Elimination | `dTLB-load-misses` | generator_2m | $\text{BCa}_{\text{upper}} \le 0.0410$ | ≥ 95% reduction from D1 baseline 0.8203 | $\text{BCa}_{\text{upper}} > 0.0410$: 2 MiB paging failed to eliminate address translation |
| D2b | L3 Miss Invariance | `mem_load_retired.l3_miss` delta | generator_2m − sorted_2m | $\text{BCa}_{\text{lower}} \ge 0.1416$ | ≥ 60% retention of D1 delta 0.2360 | $\text{BCa}_{\text{lower}} < 0.1416$: huge pages eliminated LLC cache-line misses |
| D2c | Surviving Cycle Gap | `cycles` delta | generator_2m − sorted_2m | $\text{BCa}_{\text{lower}} \ge 52.50$ | Pins D1 L3 stalls delta 52.50 cycles (61.1% of cycle gap, target) | $\text{BCa}_{\text{lower}} < 52.50$: paging alone resolved the majority of the cycle gap |

### §3.4.4 Decision Criteria & Scope Boundary
- **If D2 PASS (D2a + D2b + D2c)**: Translation misses are confirmed as an orthogonal overlay. 2 MiB pages
  successfully eliminate dTLB misses, but spatial cache-line dispersal across 45 MB of memory persists and
  accounts for $\ge 52.50$ cycles of the gap. This indicates that an OS/allocator paging configuration
  cannot resolve the performance gap. Co-allocation of child nodes with leaves remains a candidate
  architectural remedy for v0.12 (alongside software prefetching or packed suffixes), which will require its
  own pre-registered gate and ablated evaluation prior to any implementation in v0.12.
- **If D2c is falsified**: 2 MiB huge-page backing eliminates the bulk of the cycle gap: huge-page backing
  is the documented remedy for this workload; layout work is deprioritised.

**No directional prediction:**
- Any `fast_radix_trie` or `qp-trie` timing. The envelope gives them 3–8
  dependent node loads, the same range as Expanse, so nothing derived separates
  them.
- `patricia_tree` string lookup, where a smoke run was seen.
- Every usable-byte comparison, and every Expanse vs `fast_radix_trie` /
  `qp-trie` memory cell.
- Each arm's generator/shuffled insert ratio. A visit-count argument favours
  shuffled order and locality favours ascending order.
- The prefix scan.

These are reported with the labels above.

**Expected losses.** None predicted. The string cells at long prefixes (§2
item 3) against `fast_radix_trie` and `qp-trie` are where losses are most
likely.

**Claims ceiling.** At most:
- On `u64` keys, Expanse beats `patricia_tree` 0.10.2 on lookup, insert and
  traversal in the measured cells. That crate walks sibling lists linearly.
  Whether that walk is the cause is tested by the two other twins, not asserted.
- Against `fast_radix_trie` and `qp-trie`, whatever the intervals show, cell by
  cell.
- `fast_radix_trie` 1.2.0 cannot store the `u64` key sets here at all.

Nothing generalises to Patricia tries as a class.

## 4. Harnesses and procedure

| Harness | `workload_id` | Measures |
|---|---|---|
| `benches/patricia_lookup_hit.rs` | `patricia_lookup_hit` | `u64` lookup, 100% hit, generator and shuffled builds |
| `benches/patricia_lookup_miss.rs` | `patricia_lookup_miss` | `u64` lookup, 50/50, in-range misses (`split_half`) |
| `benches/patricia_insert.rs` | `patricia_insert` | cold build, both orders timed in the same rounds |
| `benches/patricia_memory.rs` | `patricia_memory` | requested and usable live bytes, every arm, both orders |
| `benches/patricia_string.rs` | `patricia_string_lookup` | path lookup, 50/50, four prefix lengths, generator and sorted builds |
| `benches/patricia_scan.rs` | `patricia_scan` | `u64` full traversal; 64 fixed path prefixes scanned |

Shared by all harnesses (`benches/patricia_common/mod.rs`):
- **Keys.** Each cell's keys come from a fresh `SHARED_SEED` stream, so a cell
  depends on its distribution and n alone, and the envelope can reproduce it.
  `zipfian` names a key set (distinct ranks of n Zipf(0.99) draws), not skewed
  access.
- **In-range misses (§8.6).** The 50/50 `u64` cell draws 2n keys, splits the
  distinct keys at random, keeps one half in generator order as the
  population, and uses the other half as misses. Its population is therefore a
  random half of a 2n draw, recorded in `raw_draws`. Hits are a random half of
  the population. String misses come from the same generator on an independent
  stream, rejected on membership.
- **Validation.** Each arm is built on its own and checked before any timing:
  `count()`, every key reads back its value, and every arm returns the same hit
  count or scan sum. An arm that fails is recorded with its reason and not
  timed. An Expanse failure panics.
- **Timing.**
  - One discarded warm-up pass per arm calibrates the repetitions, so every
    timed pass is at least `MIN_WINDOW` (20 ms). A 6 ns arm and a 1 µs arm are
    then timed over comparable windows.
  - Arm order rotates every round, over 15 rounds.
  - Construction, validation, key encoding and drop stay outside the window,
    and every result reaches `black_box`.
  - A zero-length window panics.
- **Estimator.**
  - Per-arm medians.
  - Ratios are geometric means of per-round ratios, with a BCa 95% interval on
    the log ratios. The interval's construction is labelled `bca`, `clamped`,
    `bc` or `degenerate` (`art_common::bca_ci_labeled`, the
    `scripts/bca_bootstrap.py` vocabulary).
  - Insert order effects are paired within rounds.
- **Memory.** The hook records requested `Layout::size` and the allocator's
  usable size (`malloc_usable_size` on Linux, `malloc_size` on macOS), because
  small nodes round up more than large ones. Every arm owns its key bytes.
  Usable bytes depend on the host's allocator, and neither column is the full
  resident cost of a small node. glibc's `malloc_usable_size` reports the chunk's
  payload but excludes its 8-byte header. A 24-byte `patricia_tree` node
  therefore reads 24 usable bytes while it occupies a 32-byte chunk (derived from
  glibc's chunk layout, not measured here).
- **Scans.** `qp-trie`'s traversal is unordered, so its full-traversal cell is
  a traversal, not an ordered scan. The only public prefix read on
  `patricia_tree` and `fast_radix_trie` materialises an owned key per entry,
  and that API cost is part of what their prefix-scan cells measure.
  `ExpanseStrMap` seeks once and steps a borrowing cursor.

## 5. Reproduction

On a pull request, comment `/benchmark patricia_comparison`. The bare-metal
workflow:
1. takes the host-wide benchmark lock and the P-core pin;
2. runs `scripts/run_all.py` with the anonymized host description and the run
   URL;
3. posts the summary and uploads `results/baseline_*.json` as a run artifact.

Commit those files into `results/` in the PR that publishes figures.
`run_all.py` builds every harness first. It then runs each population in its
own harness process with a load snapshot before it (§8.17), and records host
facts and the pin.

On the host directly:

```bash
docs/benchmarks/patricia_comparison/run.sh           # full run
docs/benchmarks/patricia_comparison/run.sh --quick   # smoke, writes results/quick/ (gitignored)
```

---

## Errata to Amendment A4 (appended 2026-10-05; the amendment above is not edited)

| Where | As written | Correction |
|---|---|---|
| §3.4.2, maintainer dispatch | `gh workflow run bench_baremetal.yml --ref main -f suite=patricia_d2_paging` | The workflow input is `benchmark_suite`: `gh workflow run bench_baremetal.yml --ref main -f benchmark_suite=patricia_d2_paging`. |
| §3.4.3 | "(Rule 12 / §1.3)" and "(Rule 1 / §1.1)" | `AGENTS.md` §8.8 commit 1 (bounds as committed, unit-tested code) and `AGENTS.md` §8.4 (a claim passes on its BCa bound, not its point). |
| §3.4.3, clause D2c | bound 52.50 "Derived in `scripts/patricia_d2_bounds.py`" | The registered value is unchanged. In the script it was computed in `main()`, outside any tested function; `registered_bounds()` now returns all three bounds and is pinned by a test. The same script's `evaluate_d2()` is the evaluator for a run artifact. |
| §3.4.2, two-run protocol | "the quiet reference host (`loadavg <= 1.0`, `foreign_busy_cpus == 0`)" | Not met as written by either run: the artifacts record a one-minute load average of up to 1.47 and 1.52, and foreign CPU of up to 0.02 core-equivalents. The condition is mis-specified, since the measured workload is one thread and reads a load average near 1 by itself. It is not redefined here. `evaluate_d2()` reads it from the artifact's load snapshots and returns `INTERMEDIATE`, not PASS, when the clauses are met and it is not. A PASS needs an amendment that registers a host condition the workload can satisfy, and fresh runs. |
| §3.4.3, clause D2b title | "L3 Miss Invariance" | The clause is a lower bound on the L3-miss difference (60% of D1's), and that is what is evaluated. The count is not invariant: against the 4 KiB arms of the same dispatches it falls by about 30%. |
| §3.4.4, "If D2 PASS" | "accounts for ≥ 52.50 cycles of the gap" and "an OS/allocator paging configuration cannot resolve the performance gap" | D2 measures the size of the remaining gap, not what fills it: the L3-stall counter is withheld in this event set. Against the 4 KiB arms of the same dispatches, 2 MiB pages remove 26% and 22% of the cycle gap; they do not close it. |
| `evaluate_d2()` | evaluated any artifact with both arms | It now returns VOID for an artifact that is not the registered measurement: an arm without 10 runs, a per-entry denominator other than 49,934,000, an arm recorded twice, a huge-page fault that fell back, or a counter a clause reads whose status is not `ok`. Neither committed run is affected. |

---

## Errata A5 to Amendment A4: the host condition (appended 2026-10-12, registered before any further D2 data; refs #1096; nothing above is edited)

This errata changes one thing: the host condition of §3.4.2's two-run protocol. A4's condition, `loadavg <= 1.0` and `foreign_busy_cpus == 0`, is **superseded by §A5.3** for dispatches made after this errata merges. Clauses D2a, D2b and D2c, their bounds, the estimator, the arms, the pin, the event set and the sample sizes are exactly as §3.4.2 and §3.4.3 register them. It contains no new measurement.

### A5.1 What the condition is read from

| Quantity | Where it is computed | What it is |
|---|---|---|
| `load1` | `scripts/bench_provenance.py:261` (`load_snapshot`, `os.getloadavg()`) | The kernel's one-minute load average at the instant of the snapshot. |
| `busy_cpus_since_prev` | `scripts/bench_provenance.py:193` | Non-idle time of the aggregate `cpu` line of `/proc/stat` between two snapshots, in core-equivalents over all 24 logical CPUs, rounded to 0.01. |
| `own_busy_cpus_since_prev` | `scripts/bench_provenance.py:229` | CPU time of the driver's reaped children (`getrusage(RUSAGE_CHILDREN)`) over the same window, rounded to 0.01. |
| `foreign_busy_cpus_since_prev` | `scripts/bench_provenance.py:250` | The first minus the second, rounded to 0.01. It covers every CPU of the host, pinned or not. Only reaped children are subtracted: the driver process itself and the host watcher are not children of the driver, so their CPU time is in it. |

`scripts/perf_counters.py` takes one snapshot before each cell and one at the end (`:1169`, `:1212`). A D2 artifact therefore holds three snapshots and two windows, and each window contains one arm's cell whole: its build-phase and probe-phase processes for all 10 runs. The first snapshot has no window. The 2 MiB artifact's first snapshot is taken as the 4 KiB invocation of the same dispatch ends.

`scripts/patricia_d2_bounds.py:219` (`host_precondition`) applies A4 to every snapshot: each `load1` at most 1.0, each `foreign_busy_cpus_since_prev` equal to 0. When the clauses are met and it is not, `evaluate_d2` returns `INTERMEDIATE` (`:321`).

Separately from the artifact, the bare-metal workflow runs `scripts/bench_host_guard.py`: `check` before any build (`.github/workflows/bench_baremetal.yml:959`), `watch` for the whole job (`:993`) and `summarize` after the benchmarks (`:1854`). The guard judges the CPU of every task outside the job's process tree plus interrupt time, host-wide and on the pinned CPUs, per window. A window over a void boundary fails the job with `host_contaminated`. Its record is in the run log and is not in the artifact.

### A5.2 Why A4's condition cannot be met

**`loadavg <= 1.0`.** The kernel updates the figure at a fixed interval as `load = load * e + active * (1 - e)`, with `e = 1/exp(5 s / 1 min)` (`include/linux/sched/loadavg.h`, `calc_load` and `EXP_1`). Over a window with a constant number `n` of runnable tasks its closed form is `load(t) = n + (load0 - n) * exp(-t / 60 s)`: the figure moves toward `n` and does not cross it. The measured workload keeps one thread runnable, so `n >= 1` and every snapshot after the first reads at least `min(load0, 1.0)`. The condition then holds only when the dispatch starts below 1.0, and in that case it bounds how long the dispatch has run, not what else ran. `expected_load1` in `scripts/patricia_d2_bounds.py` is that closed form; replayed with `n = 1` over the 12 windows of the committed D1 and D2 counter artifacts it lands within 0.03 of every recorded `load1` (`test_expected_load1_against_committed_snapshots`). That agreement is observational.

In the D2 dispatches the first 4 KiB snapshot follows the release build and reads 1.93 and 2.02. From 2.02 with one runnable task the closed form reaches 1.05 only at `t = ln(1.02 / 0.05) * 60 s = 181 s` and never reaches 1.0; the last snapshot of each dispatch is at `t = 75 s` (derived from the snapshots' `at` fields). D1's two artifacts started at 0.66 and 0.59 and ended at 0.83 and 0.79, still rising toward 1.

**`foreign_busy_cpus == 0`.** The figure is recorded on a 0.01 grid from three roundings, so its rounding error alone reaches 0.015, and by §A5.1 it includes the driver's and the watcher's own CPU time. An exact zero is one grid value inside a band the instrument cannot narrow. What the remainder consists of on this host is unmeasured: the artifact carries no attribution.

**Recorded values** (observational; committed artifacts on the reference host, 12th Gen Intel(R) Core(TM) i9-12900F, 24 threads; the counter artifacts are workload: example_perf_point_lookup):

| Artifact (`results/`) | Commit | Windows | `load1`, all snapshots | `foreign_busy_cpus_since_prev` | A4 met |
|---|---|---|---|---|---|
| `counters_prefix_scan_d1.json` | `c3e54ec2` | 2 | 0.66 to 0.83 | 0.01, 0.00 | no (foreign) |
| `counters_prefix_scan_d1_stalls.json` | `c3e54ec2` | 2 | 0.59 to 0.79 | 0.01, 0.00 | no (foreign) |
| `counters_prefix_scan_d2_4k_run1.json` | `7243adab` | 2 | 1.52 to 1.93 | 0.02, 0.02 | no (both) |
| `counters_prefix_scan_d2_2m_run1.json` | `7243adab` | 2 | 1.26 to 1.47 | 0.02, 0.02 | no (both) |
| `counters_prefix_scan_d2_4k_run2.json` | `052b0a2a` | 2 | 1.52 to 2.02 | 0.02, 0.01 | no (both) |
| `counters_prefix_scan_d2_2m_run2.json` | `052b0a2a` | 2 | 1.29 to 1.52 | 0.02, 0.01 | no (both) |
| `baseline_*.json` (six files, one shared load series) | `6000b4a1` | 18 | 1.00 to 2.04 | 0.00 to 0.03 | no (both) |

No committed artifact of this suite meets A4, D1's included. Other suites' artifacts on the same host record the same fields, mostly over multi-threaded cells, where the remainder ranges from −4.87 (`docs/benchmarks/masstree_comparison/results/`) to 16.9 (`docs/benchmarks/concurrency/results/`). Those are differences of a tick-sampled clock and an exact one over many threads; they are not comparable to a one-thread window and are not used here.

Two further dispatches ran at `d2843607` and neither artifact is committed. Run `38153339742` concluded `success`; its log records the guard's start gate as quiet (foreign 0.02, on the pinned CPUs 0.03) and its in-run record as 60 samples, worst foreign 0.15 and worst on the pinned CPUs 0.11. Run `38157345006` concluded `failure`: the guard voided it on two samples, with on-pin foreign load 0.389 and 0.998 against the 0.25 boundary. The snapshot readings of `38153339742` were not read for this errata.

### A5.3 The replacement condition

A dispatch is **admissible** when all three parts hold on **both** of its artifacts, the 2 MiB one the clauses read and the 4 KiB control. Judging the control too is the stricter reading: the two invocations share one job and one host state.

| Part | Quantity | Sampled | Condition |
|---|---|---|---|
| H0 | The host guard's verdicts (`bench_host_guard.py check` and `summarize`), as recorded in each artifact's `provenance` (§A5.7) | `check`: before any build. `watch`: every window of the job, from CPUs outside the pin | The start gate was quiet, the in-run record is complete, and no in-run window crossed a void boundary. Constants in §A5.6. |
| H1 | `foreign_busy_cpus_since_prev` of each artifact | Each of its two cell windows (snapshots 2 and 3), so each arm's cell is covered whole: four windows per dispatch | A number, and `-0.02 <= f <= 0.06` core-equivalents in every window. |
| H2 | `load1` of each artifact | All three of its snapshots | Each at most `8.0` (half the host's 16 physical cores), and within an artifact the largest minus the smallest at most `2.0`. |

- H0 is `AGENTS.md` §8.17's void rule as the workflow already enforces it: in any one window, a non-target process at 100% of a CPU, foreign load of 1.0 core-equivalent, or of 0.25 on the pinned CPUs.
- H1 is the condition on the non-lagging busy-CPU delta. "Foreign" in H1 is the snapshot's definition (§A5.1): all 24 logical CPUs, the driver's reaped children subtracted and nothing else. It therefore bounds foreign load on the pinned CPUs from above, up to the instrument's error.
- H2 is §8.17's load-average floor and nothing more. `load1 <= 1.0` is withdrawn: by §A5.2 it reads the harness. No settle period is registered, because no part of the condition compares a load average with a level that an earlier job could still be decaying from.
- A reading of `None`, a missing snapshot, a snapshot count other than three, or an artifact without the guard's record leaves the condition not evaluable, which is not met.
- The start gate is the guard's existing check of measured CPU (three consecutive quiet windows). No rule on which processes are present is registered; an operator's look for known resident workloads before dispatching is a courtesy outside this registration and decides nothing.

**Void rule.** A dispatch that ran its benchmark and is not admissible is `VOID`. A job the start gate refused measured nothing: it is disclosed, and it is neither a dispatch nor a void. Its clauses are not evaluated, its counts are not published, and the record names the run and the reading that voided it (`AGENTS.md` §8.17, §8.1). Admissibility is decided from H0, H1, H2 and §3.4.2's huge-page precondition alone, before any clause is read.

### A5.4 Where the H1 bounds come from

**Upper bound, 0.06.** The principle: foreign load must be too small to carry, by itself, a result that D2c's falsifier describes over D2c's bound. The falsifier reads "paging alone resolved the majority of the cycle gap", so the nearest such result is a surviving gap of half D1's: `85.9445 / 2 = 42.9722` cycles per entry. The registered bound is 52.50, a distance of `9.5278`.

The worst case assumed is a hypothesis, unmeasured: every foreign CPU-second lands on the generator arm alone, and while it runs that arm's cycles per entry at most double (`slowdown = 2`, one foreign thread sharing the workload's physical core). A cell-mean foreign load `f` then inflates the generator-minus-sorted difference by at most `f * (2 - 1) * 158.6335` cycles per entry, with 158.6335 the generator arm's D1 figure from `counters_prefix_scan_d1_stalls.json`. Requiring that to stay below 9.5278:

`f < 9.5278 / 158.6335 = 0.06006`, rounded down to the snapshot's 0.01 grid: `0.06`.

`foreign_load_ceiling` and `a5_host_bounds` in `scripts/patricia_d2_bounds.py` compute it and `test_a5_host_bounds` pins it. Every input is a D1 figure or a registered bound; no D2 reading enters.

What this does and does not carry:
- The bound is only as good as `slowdown`. At 3 the same arithmetic gives 0.03, and at 4 it gives 0.02. No injected-load measurement of it exists on this host; the experiment meant to set the guard's on-pin boundary was inconclusive (`scripts/bench_provenance.py:129-135`).
- It is derived for D2c. Under the same assumption D2a is moved toward failing, not passing. For D2b no bound on what foreign load does to an L3-miss difference is derived here: D2b's protection is H0 and H1 as they stand, and its sensitivity to foreign load is unmeasured.
- The D2 readings in §A5.2 (0.01 to 0.02) were known when this principle was chosen. The value is not a function of them, but the choice of principle was not blind. The bound is registered with that stated. The alternative that needs no principle chosen after the data is H0 alone, whose constants were committed on 2026-09-27 (`8cda40fd4`), before A4. H0 alone admits 0.25 core-equivalents on the pinned CPUs in every window, which under the same worst case is `0.25 * 158.6335 = 39.7` cycles per entry and exceeds the 9.5278 distance. That is why H1 is registered in addition.
- Reachability is observational: all 30 windows in §A5.2's table read at most 0.03.

**Lower bound, −0.02.** The figure is a difference of two clocks and can read negative. Three roundings of half a grid step each give `3 * 0.005 = 0.015`, which the grid records as 0.02 (`foreign_reading_floor`). A reading below −0.02 is the two operands disagreeing by more than rounding, and a negative remainder can hide a foreign task of the same size (`scripts/bench_host_guard.py:34-37`).

**Resolution.** With `USER_HZ = 100`, one jiffy on one CPU over the shortest committed D2 cell window (`15.779 s`) is `1 / (100 * 15.779) = 0.0006` core-equivalents. The 0.01 rounding grid is the coarser limit, and the 0.06 bound is six grid steps.

### A5.5 What this change costs, and the verdict rule

Changing a registered condition after data was seen is a post hoc change (`AGENTS.md` §8.19). Its price is paid in full:

- Every D2 dispatch made so far keeps its A4 label and is **not re-read under A5**. Runs `37261900492` and `37476779161` stay `INTERMEDIATE` as recorded in the README. Run `38153339742` was dispatched under A4 and run `38157345006` was voided by the guard; their artifacts are not committed retroactively.
- The verdict comes only from **two new, independent dispatches** of `patricia_d2_paging`.
  - **Commit rule.** Both run at one commit `S` on `main`, given as a full 40-character SHA in the workflow's `ref` input. `S` contains this errata and the evaluator change of §A5.7. All four artifacts' `provenance.commit` equal `S`.
  - **Independence.** Each is its own workflow run with its own build, start gate and processes. The second is dispatched after the first has concluded.
  - **PASS** when both dispatches are admissible (§A5.3) and each meets D2a, D2b and D2c on its own BCa bounds. Nothing is pooled across the two.
  - **FAIL** when an admissible dispatch does not meet a clause. That is final: an admissible dispatch is never replaced.
  - **VOID** dispatches are replaced at the same `S`, and each is disclosed. At most `3` replacement dispatches are made in total across the pair. A further void after the third replacement ends D2 as `INTERMEDIATE`, and the record says that the cap was reached and which readings voided each dispatch.
- **Direction** (`docs/BENCHMARKING.md` rule 18). A statement that a counter's generator-minus-sorted difference moved between 4 KiB and 2 MiB pages is made only for a counter that moved the same way in both dispatches, and levels are quoted as two values.
- Unchanged from A4: 10 runs per cell, 200 passes, 49,934,000 entries per run, BCa 95% with 2,000 resamples, the three bounds 0.0410, 0.1416 and 52.50, and the void conditions `evaluate_d2` already applies.

### A5.6 Constants

Values as committed at `ed59e8838`. None is changed by this errata.

| Constant | Value | Where |
|---|---|---|
| H1 upper bound | `0.06` core-equivalents | `a5_host_bounds()["foreign_cell_max"]` |
| H1 lower bound | `-0.02` core-equivalents | `a5_host_bounds()["foreign_cell_min"]` |
| Assumed worst-case slowdown | `2.0` | `foreign_load_ceiling(slowdown=...)` |
| H1 windows per artifact | `2`, one per arm cell | `scripts/perf_counters.py:1169`, `:1212` |
| H2 level, spread | `8.0`, `2.0` | `a5_host_bounds()["load1_max"]`, `["load1_spread_max"]`; `AGENTS.md` §8.17 |
| Load-average time constant | `60.0 s` | `LOAD1_TAU_S` |
| Snapshot rounding grid | `0.01` | `FOREIGN_GRID`; `scripts/bench_provenance.py:213`, `:247`, `:258` |
| Shortest window a busy figure is computed over | `MIN_WINDOW_S = 0.1 s` | `scripts/bench_provenance.py:113` |
| Start gate: foreign, on-pin, one process | `0.5`, `0.1`, `50 %` | `START_FOREIGN_MAX`, `START_ON_PIN_MAX`, `START_PROCESS_MAX_PCT` (`scripts/bench_provenance.py:138-140`) |
| Start gate: window, consecutive quiet windows | `START_WINDOW_S = 1.0 s`, `3` | `scripts/bench_provenance.py:142-143` |
| Start gate: longest wait for quiet | `EXPANSE_BENCH_QUIET_WAIT = 300 s` | `.github/workflows/bench_baremetal.yml:753` |
| In-run void: foreign, on-pin, one process | `1.0`, `0.25`, `100 %` | `RUN_FOREIGN_VOID`, `RUN_ON_PIN_VOID`, `RUN_PROCESS_VOID_PCT` (`scripts/bench_provenance.py:127-135`) |
| In-run sampling interval, longest gap | `WATCH_INTERVAL_S = 2.0 s`, `3.0` intervals | `scripts/bench_provenance.py:145`; `scripts/bench_host_guard.py:767` |
| Replacement dispatches across the pair | at most `3` | `a5_host_bounds()["void_replacements_max"]` |
| Artifacts judged per dispatch | `2` (2 MiB and 4 KiB), four H1 windows | §A5.3 |
| Settle period | none registered | §A5.3, H2 |

### A5.7 Required before dispatch (not in this change)

1. **Evaluator.** `scripts/patricia_d2_bounds.py` (`host_precondition`, `evaluate_d2`) applies A4. It must apply §A5.3 to both artifacts of a dispatch and return `VOID` when the condition is unmet, and decide the pair by §A5.5, the cap included. A4's check stays available by name only so that the README's `INTERMEDIATE` record of the committed artifacts remains reproducible.
2. **H0 in the artifact.** The guard's verdict is in the run log only, and the artifact is uploaded whether or not the job failed. The workflow must write the guard's record into each artifact's `provenance` after `summarize` in `.github/workflows/bench_baremetal.yml`: start-gate verdict, sample count, worst foreign and on-pin readings, the number of windows over a boundary, and whether the record is complete. An artifact without it is not evaluable under A5.
3. **Start gating is already in code.** `check` refuses to start until three consecutive windows are quiet and gives up after `EXPANSE_BENCH_QUIET_WAIT`. It judges measured CPU, never the presence of a process, so a resident but idle workload passes it and can become busy mid-run: run `38157345006` passed the gate and was voided later.

---

## A5 outcome: D2 is `FAIL` on D2b (appended after the A5 dispatches; refs #1096; nothing above is edited)

Both dispatches ran at `256e7736a3267e516539817d05f690efdaea1736`, a commit that contains §A5 and the §A5.7 evaluator and guard-record changes. Artifacts: `results/counters_prefix_scan_d2_{2m,4k}_a5_run{1,2}.json`, each with `provenance.host_guard`. Figures, intervals, the 4 KiB control and the host readings are in `README.md`, section "D2 under errata A5".

| Dispatch | Run | §A5.3 | Result |
|---|---|---|---|
| 1 | `38424662601` | host condition H0 to H2 met; §3.4.2 huge-page precondition not met | `VOID`: `strmap_prefix_scan: 12 huge-page fault(s) fell back to small pages`. First of at most 3 replacements used. |
| 2 | `38446328899` | admissible | `FAIL`: D2a met, D2b not met, D2c met. |

D2b, the clause that decides: BCa 95% lower bound 0.1400 against the registered 0.1416, point 0.1482 (`ci_method: bca`, 10 runs). Per §A5.5 an admissible dispatch that does not meet a clause is final and is never replaced, so the pair verdict by `scripts/patricia_d2_bounds.py --verdict` is `FAIL` and no further dispatch is made. The bounds, the estimator and the condition were not changed after the data; the gate is not reopened (`AGENTS.md` §8.19). The void dispatch is disclosed, with its artifacts (§A5.3, `AGENTS.md` §8.17); its counts are not published. The A4-era dispatches keep the labels the README records.

## Start-gate note: huge-page probe (appended 2026-10-13; refs #1096; nothing above is edited)

This adds one refusal to the start gate that A5.3's H0 reads. It changes no clause, bound, estimator, arm, event set, sample size, void rule or the replacement cap of §A5.5, and no D2 dispatch has been judged under it.

- **Rule.** `bench_host_guard.py check` refuses a suite that declares `"huge_pages": true` in `.github/bench-suites.json` (here `patricia_d2_paging` only) when the THP mode in `/sys/kernel/mm/transparent_hugepage/enabled` is `never` or unreadable, or when its probe fails. The probe maps `required_huge_pages()` private anonymous 2 MiB extents (plus one of slack for alignment), requests huge pages with `madvise(MADV_HUGEPAGE)`, touches one byte per extent, and reads that mapping's `AnonHugePages` from `/proc/self/smaps`. Any touched extent not backed by a huge page, an unreadable smaps, or a probe that cannot run refuses; the probe needs Python 3.8 or later (`mmap.madvise`), and an older interpreter refuses by name. The mapping is released before `check` returns. The refusal exits 1 with `fail_reason=no_huge_pages`, is recorded in `host-guard.json` under `hugepages`, and is copied into `provenance.host_guard.start.hugepages` by `attach`. The guard changes nothing on the host.
- **Status of a refusal.** Under A5.3's existing rule a job the start gate refused measured nothing: it is disclosed, and it is neither a dispatch nor a void. It does not consume a replacement of §A5.5.
- **Size of the probe.** `required_huge_pages()` = `ceil(1638 / 20) * 2` = 164 extents (328 MiB). 1638 is the largest per-cell `thp_delta.thp_fault_alloc` of the committed 2 MiB artifacts (`docs/benchmarks/patricia_comparison/results/counters_prefix_scan_d2_2m_run1.json` and `..._run2.json`; the other cells read 1620, 1638 and 1618). A cell is 10 runs of a build and a probe process, 20 sequential processes, and both phases build the same structure from fixed seeds, so one process maps about 82 huge pages. The factor 2 is a stated margin and not a measurement. `--self-test` re-derives 1638 and 20 from the artifacts. 164 sizes the probe; it is not a free-memory level that separates good hosts from bad ones.
- **Recorded, not judged.** Free 2 MiB pages from `/proc/buddyinfo` (Normal zones, summed over nodes, order 10 counting two; all zones beside it), `MemFree`, `Cached`, the THP `enabled` and `defrag` modes, `thp_fault_fallback`, and the compaction counters `compact_stall`, `compact_fail`, `compact_success`. The `thp_fault_alloc` / `thp_fault_fallback` deltas across the probe are system-wide, so another process moves them; they are context. Free pages are not a clause because the one reading taken after a voided dispatch, 166 free 2 MiB pages (operator observation after the run, in no artifact), sat beside a run that had fallen to 4 KiB pages: a count at one instant does not bound what the kernel can supply.
- **Limits.** A probe at the start does not guarantee supply for the whole run, since the suite's builds change memory and page-cache state; it shows that the kernel could supply the suite's own need at gate time. An in-run fallback still voids the run by the existing rule. Whether the probe would have refused the voided dispatch's host is unknown: no probe was run there and its state during the run was not recorded.
