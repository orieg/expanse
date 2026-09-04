# Set algebra — engine kernels and the interned set domain

The entry point for every measurement of `crates/expanse/src/algebra.rs` and
`algebra_build.rs`: the native `AND` / `OR` / `AND-NOT` / `XOR` kernels that
descend two tries in lockstep by expanse, the $k$-way aggregate walk, direct
emission materialization, and the interned set domain built on top of them.

**Why this directory exists.** Set algebra is exercised by five harnesses that
publish into four different places. Nothing routed from the engine module to
its measurements, so "where are the algebra benchmarks?" had no answer. This
suite owns the one harness that had no home and **routes to the other four
where they already live** — it does not re-home them, and it does not restate
their numbers. Each owning suite remains the authority on its own cells.

---

## 1. Where each algebra measurement lives

| Harness | `/benchmark` token | Owner | What it measures |
|---|---|---|---|
| [`benches/domain.rs`](../../../crates/expanse/benches/domain.rs) | `domain` | **this suite** | Interned set domain: provenance-check overhead vs raw `ExpanseSet`, batched vs scalar ingestion, zero-copy slab resolution |
| [`benches/search_boolean.rs`](../../../crates/expanse/benches/search_boolean.rs) | `search_boolean` | [`search_inverted_index/`](../search_inverted_index/README.md) **Pillar 1** | Boolean posting-list algebra and materialization against `roaring` |
| [`benches/search_instructions.rs`](../../../crates/expanse/benches/search_instructions.rs) | `search_instructions` | [`search_inverted_index/`](../search_inverted_index/README.md) **Pillar 2** | Deterministic Callgrind counts for the same kernels — the noise-free cross-check |
| [`benches/bench_grammar_masks.rs`](../../../crates/expanse/benches/bench_grammar_masks.rs) | `bench_grammar_masks` | [`llm_inference/`](../llm_inference/README.md) | Grammar-constrained decoding mask cache and its set algebra vs roaring and dense bitmaps |
| [`benches/avx512_bitmap.rs`](../../../crates/expanse/benches/avx512_bitmap.rs) | `avx512_bitmap` | [`avx512/`](../avx512/README.md) | `Bitmap256::count_and`, the inner kernel of the intersection walk, across cache residency |

**Ownership does not move.** `search_boolean` and `search_instructions` are
Pillars 1 and 2 of the search suite's pre-registration; `bench_grammar_masks`
is an `llm_inference` arm; `avx512_bitmap` is the `avx512` suite's subject.
Reassigning any of them here would strip a suite of a pillar it pre-registered
and invalidate its claims ceiling. The table above is the routing this
repository lacked, not a transfer of ownership.

## 2. What this suite measures directly (`domain`)

`benches/domain.rs` (workload `domain_interned_set`) carries three groups:

| Group | Arms | Question |
|---|---|---|
| `domain_set_algebra_overhead` | `raw_expanse_set_intersection`, `domain_set_intersection`, `raw_expanse_set_intersection_len`, `domain_set_intersection_len` | Does the `DomainSet` provenance check cost anything over raw `ExpanseSet` algebra? |
| `domain_ingestion` | `scalar_insert_text`, `batch_insert_text`, `scalar_insert_uuid`, `batch_insert_uuid` | What does chunk amortisation buy over scalar interning, on text and on NUL-bearing binary keys? |
| `domain_resolution` | `resolve_full_scan` | What does a borrowed-slice scan out of the stable slab arena cost? |

Populations 10k / 50k / 100k; the parity group is symmetric by construction —
both arms intersect the same key sequences, and the domain arm differs only by
the provenance check under test.

## 3. Results

Numbers for the `domain` arms are published in
[`docs/DATABASE.md` §4.3](../../DATABASE.md), which carries the narrative for
the interned set domain. This suite does not duplicate them.

> **Provenance is unresolved for the §4.3 figure, and this suite restates no
> numbers from it.** `docs/assets/data/bench_domain_algebra.json` recorded one
> host field and one commit for five sections drawn from two harnesses, and its
> distinctive cells appear in no committed benchmark artifact, so no section's
> host is recoverable. Provenance is now stated **per section** in that dataset,
> with host and commit recorded as `unresolved` rather than asserted; the figure
> carries the same statement per panel. Assigning a host per section would be
> backfilling (§8.10). Re-measuring the `domain` arms on the reference host is
> what resolves it.
>
> Two claims that rode on that figure were withdrawn in the same pass: the
> parity badges asserted `+0.00 ns`, a precision 100× finer than the recorded
> values support, and are now stated as the resolution bound (`< 100 ns`,
> `< 10 ns`); and a footer line claiming an "identical instruction count" was
> removed, because `domain` is a wall-clock target with no arm in any Callgrind
> harness.

## 4. Reproduction

```bash
docs/benchmarks/set_algebra/run.sh          # full sweep
docs/benchmarks/set_algebra/run.sh --quick  # smoke, writes to a scratch path
```

Wall-clock arms are gated per §8.4 on the BCa 95% CI lower bound, on the quiet
reference host — never on a development machine's point estimates. The
deterministic cross-check for the shared kernels is `search_instructions`,
which needs valgrind and runs in the `instruction-counts` CI job.

## 5. Chart renderer

`scripts/generate_domain_algebra_svg.py` still writes to `docs/assets/` and has
not moved into this suite's `scripts/`; that move is tracked with the rest of
the chart-asset consolidation in
[#643](https://github.com/orieg/expanse/issues/643).
