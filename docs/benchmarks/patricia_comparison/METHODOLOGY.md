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
