# Set algebra suite — methodology and pre-registration

Scope: the `domain` harness (`crates/expanse/benches/domain.rs`, workload
`domain_interned_set`). The four other algebra harnesses are pre-registered by
the suites that own them — [`search_inverted_index/METHODOLOGY.md`](../search_inverted_index/METHODOLOGY.md)
(Pillars 1 and 2), [`llm_inference/METHODOLOGY.md`](../llm_inference/METHODOLOGY.md),
and [`avx512/README.md`](../avx512/README.md) — and are not re-registered here.

## 1. Novelty tier

**Engineering.** The suite asserts no new mechanism. `algebra.rs` and the
interned domain already exist and are separately reviewed; this suite measures
what they cost. It therefore carries pre-registration and fair-twin obligations
(§8.8 commit 2, §8.3) without a novelty-tier declaration.

## 2. Pre-registered hypotheses and expected losses

| # | Arm | Hypothesis | Expected loser | Structural rationale |
|---|---|---|---|---|
| H1 | `domain_set_algebra_overhead` | `DomainSet` intersection is **indistinguishable** from raw `ExpanseSet` intersection | neither — this is a null result by design | The provenance check is a dictionary-identity comparison outside the descent loop; it cannot scale with population. A measurable gap would mean the check leaked into the kernel. |
| H2 | `domain_ingestion`, text keys | `batch_insert_text` beats `scalar_insert_text` | scalar | Chunk amortisation pays the dictionary lookup and slab growth once per chunk rather than once per key. |
| H3 | `domain_ingestion`, UUID keys | `batch_insert_uuid` beats `scalar_insert_uuid`, by a **smaller** margin than H2 | scalar, narrower | Order-preserving byte-stuffing for embedded NULs is per-key work that amortisation cannot remove, so the binary arm keeps a cost the text arm does not. |
| H4 | `domain_resolution` | `resolve_full_scan` performs **zero allocations** | — | Resolution borrows out of the stable slab; any allocation means a copy path was taken. |

**H1 is a null hypothesis and is reported as one.** A "win" for `DomainSet`
would be as much a defect signal as a loss — it would mean the twin arms are
not measuring the same work. The pass condition is that the BCa 95% intervals
overlap, not that either arm is faster.

## 3. Twin fairness (§8.3)

The `raw_expanse_set_*` arms are not strawmen: they are the production
`ExpanseSet` kernels the domain wraps, over identical key sequences, built by
the same generator. They have a winning regime by construction — any provenance
cost that reached the descent loop would show up as a raw-arm win, which is
precisely what H1 tests for. The ingestion twins are the same interning path
with and without chunking, so the only difference is the amortisation under
test.

## 4. Claims ceiling

- This suite makes **no claim about `ExpanseSet` against a third-party
  container.** The competitive Boolean-algebra comparison against `roaring` is
  `search_inverted_index` Pillar 1; nothing here extends or restates it.
- H2/H3 speedups are claims about **ingestion into the interned domain**, at the
  measured populations and key shapes, not about interning in general.
- H4 is a deterministic allocation-count claim and is gated as an exact integer
  (§8.4), not statistically.

## 5. Gating

| Arm class | Instrument | Gate |
|---|---|---|
| H1, H2, H3 | criterion wall clock, reference host | BCa 95% CI lower bound vs floor (§8.4); overlapping intervals on H1 are the **pass**, and are labelled as such rather than as a boundary result |
| H4 | allocator counter | exact integer, zero variance |

Development-machine runs are diagnostic only and gate nothing (§8.4): the arms
are single-threaded and small enough that frequency scaling and co-resident load
move them more than the effects under test.

## 6. Amendments after pre-registration

None. Sections §1-§5 are the locked pre-registration; any later change to the
harness shape is appended here with its rationale and does not edit the sections
above (§8.7).
