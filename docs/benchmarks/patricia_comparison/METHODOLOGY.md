# Patricia Trie vs. Expanse: Methodology and Pre-Registration

## 1. Question and twin

How do `ExpanseMap` (u64 keys) and `ExpanseStrMap` (string keys) compare with a
Patricia trie?

**Twin:** [`patricia_tree`](https://github.com/sile/patricia_tree) `=0.10.2` (MIT,
pinned in `crates/expanse/Cargo.toml`), `PatriciaMap<u64>`. It is a byte-labelled
Patricia tree after Morrison, *PATRICIA — Practical Algorithm To Retrieve
Information Coded in Alphanumeric*, J. ACM 15(4), 1968
([doi:10.1145/321479.321481](https://doi.org/10.1145/321479.321481)).
The layout below was read from `src/node.rs` at 0.10.2:

- each node is **one allocation** holding `flags: u8`, `label_len: u8`, the label
  bytes, and then an optional value, first child and next sibling, each present
  only when set (`new_with_boundary`, `Layout::extend` + `pad_to_align`);
- the children of a node are a **byte-sorted, singly linked sibling list**, and
  `Node::get` walks it linearly, stopping early when the probe's first byte is
  below a sibling's first label byte.

Integer keys go to the Patricia arm as their big-endian 8 bytes, so byte order
matches numeric order on both arms. Both arms get their keys already encoded,
outside the timed window.

**Scope of the claim.** This suite measures *this* implementation. A Patricia trie
that branches on single bits (crit-bit), or one that indexes its children by an
array, has a different descent cost. Nothing here generalises to "Patricia tries".
A crit-bit twin is a natural second arm and is not included.

## 2. Math-first envelope (AGENTS.md §8.8 commit 1)

`scripts/patricia_envelope.py` (run by `scripts/gate.sh`) rebuilds the exact node
set `patricia_tree` holds for a key set and sums each node's layout size. Its
tests pin hand-derived node sizes and small trees. They also pin one cell the
allocator hook measured: for sequential `n = 10,000` keys the census gives
**240,664 B across 10,042 nodes**, and `patricia_memory` records the same two
numbers (it was run locally; byte counts are deterministic and
host-independent). The same exact agreement holds for `sequential` and
`sparse_stride` at 1k and 10k.

`python3 scripts/patricia_envelope.py --report` (derived: census, commit this
suite lands in):

| n | distribution | Patricia B/key | Patricia nodes visited per hit |
|---|---|---|---|
| 10,000 | sequential | 24.07 | 150.34 |
| 100,000 | sequential | 24.06 | 239.35 |
| 1,000,000 | sequential | 24.06 | 265.52 |
| 1,000,000 | sparse_stride | 24.06 | 265.52 |
| 1,000,000 | uniform_random† | 25.51 | 266.45 |

† The report's uniform draw uses a fresh `SHARED_SEED` stream. The harness's
uniform keys come later in that stream, after `clustered`, so they are a
different draw of the same distribution.

What the envelope says:

1. **Memory floor.** A key needs at least one node carrying a value (16 B with a
   label of ≤ 6 bytes), and every node except the last in its sibling list also
   carries a sibling pointer (24 B). With 256-way lists, the census settles near
   24 B/key for every u64 distribution at large N.
2. **Descent cost.** A hit visits, on average, about half of each sibling list
   on its path. At 1M keys that is ~266 dependent node loads per hit. Expanse
   descends at most 8 levels for a u64 key (`docs/ARCHITECTURE.md` §2), and
   each level is a single indexed, SWAR-searched or bitmap-ranked step, not a
   list walk.
3. **Shared prefixes.** For `prefixed_path` keys (a 34-byte prefix shared by
   every key), a Patricia tree stores the prefix once, in one label.
   `ExpanseStrMap` descends it eight bytes per level. This is the regime where
   the twin can plausibly win, which §8.3 / C-b require one to exist.

The envelope derives no Expanse byte counts. Expanse memory is read from the
same allocator hook in the harness.

## 3. Pre-registration (AGENTS.md §8.8 commit 2)

**Disclosure.** Before these predictions were written, the harnesses were run
once locally in `--quick` mode (1k/10k memory cells; 10k/50k timing cells, on a
laptop with no load snapshot). The memory cells at 1k and 10k are therefore
*observed*, not predicted, and are excluded below. At 1k keys Expanse held more live bytes than Patricia on every u64 distribution; the cause of that fixed cost is unmeasured. The timing smoke run is
inadmissible as data (§8.17), but it was seen, so the timing predictions are
not blind.

Predictions are evaluated on the reference host at N ∈ {100k, 1M}. A timing
prediction PASSES iff the BCa 95% interval of the paired ratio (Expanse ns ÷
Patricia ns) lies wholly on the predicted side of 1.0. A memory prediction is
an exact comparison.

| ID | Cell | Prediction |
|---|---|---|
| P1 | `patricia_lookup_hit`, every u64 distribution | Expanse faster: CI upper < 1.0 |
| P2 | `patricia_lookup_miss`, every u64 distribution | Expanse faster: CI upper < 1.0 |
| P3 | `patricia_insert`, every u64 distribution, both orders | Expanse faster: CI upper < 1.0 |
| P4 | `patricia_insert`, Patricia arm, `sequential` and `sparse_stride` | generator (ascending) order slower than shuffled order for Patricia (point estimates; the tail append walks the full list) |
| P5 | `patricia_memory`, `sequential` and `clustered` | Expanse fewer live bytes |
| P6 | `patricia_memory`, `uniform_random` and `sparse_stride` | no prediction: at 10k they were within 7% of each other (observed), and the direction at 100k/1M is not derived |
| P7 | `patricia_memory`, every u64 distribution | Patricia byte count identical in generator and shuffled order (`patricia_order_invariant = true`) |
| P8 | `patricia_string` (`prefixed_path`) | no directional prediction; this is the regime the twin could win (§2 item 3) |
| P9 | `patricia_memory`, `prefixed_path` | no directional prediction |

**Expected losses.** None are predicted with confidence. P6, P8 and P9 are where
a loss would be expected to show up, and they are left open on purpose rather
than predicted as wins.

**Claims ceiling.** At most: *on u64 keys, `ExpanseMap` beats `patricia_tree`
0.10.2 on lookup and insertion in the measured cells, because that crate walks
sorted sibling lists linearly; on dense u64 keys, it holds fewer live bytes.*
This is a statement about one implementation's child layout, not about Patricia
tries.

## 4. Harnesses

| Harness | `workload_id` | What it times or counts |
|---|---|---|
| `benches/patricia_lookup_hit.rs` | `patricia_lookup_hit` | 100% hit, shuffled probe stream |
| `benches/patricia_lookup_miss.rs` | `patricia_lookup_miss` | 50/50, misses by same-distribution rejection (`gen_distribution_misses`) |
| `benches/patricia_insert.rs` | `patricia_insert` | cold build, generator and Fisher–Yates order |
| `benches/patricia_memory.rs` | `patricia_memory` | live heap after build, both orders for Patricia |
| `benches/patricia_string.rs` | `patricia_string_lookup` | shared-prefix string keys, 50/50 |

Key generators, PRNG, seeds, BCa and `rounds_raw` are shared with the ART suite
(`benches/art_common/mod.rs`), so both suites draw identical u64 keys. The string
generator and miss generator live in `benches/patricia_common/mod.rs`. Arm order
alternates per round, and every result reaches `black_box`. Construction and drop
sit outside the timed window, and string keys are NUL-validated once before any
timing starts.

**Not measured:** range scan and ordered iteration. `PatriciaMap::iter`
reconstructs every key into an owned `Vec<u8>`, so a scan cell would time that
allocation on one arm only. Scan stays out until a symmetric formulation exists.

## 5. Reproduction

On a pull request, comment `/benchmark patricia_comparison`. The bare-metal
workflow takes the host-wide benchmark lock and the P-core pin, runs
`scripts/run_all.py` with the anonymized host description and the run URL,
posts the summary table, and uploads `results/baseline_*.json` as a run
artifact. Commit those files into `results/` in the PR that publishes figures.

On the host directly:

```bash
docs/benchmarks/patricia_comparison/run.sh           # full run
docs/benchmarks/patricia_comparison/run.sh --quick   # smoke, writes results/quick/ (gitignored)
```

`run.sh` takes the same lock and pin. `run_all.py` records host facts, the pin and
a load snapshot before each harness (§8.17).
