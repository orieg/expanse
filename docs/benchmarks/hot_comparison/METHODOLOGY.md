# HOT (Height Optimized Trie) vs. Expanse: Pre-Registration & Comparative Methodology

**Status: pre-registration locked. No suite measurement has been taken.**

This document is commit 2 of the three-commit cadence (AGENTS.md §8.8): hypothesis,
claims ceiling, expected-losses matrix and gate taxonomy, committed *before* the
harness and before any measurement. Delivers the pre-registration half of
[#660](https://github.com/orieg/expanse/issues/660).

---

## 1. Problem Statement

The repo has no measured comparison against HOT. [#387](https://github.com/orieg/expanse/issues/387)
landed the ART arm (`docs/benchmarks/art_comparison/`) and left HOT and Masstree as
mechanism-level argument carrying no numbers. This suite is the HOT half.

HOT ([Binna, Zangerle, Pichl, Specht & Leis, SIGMOD 2018](https://dl.acm.org/doi/10.1145/3506692))
dynamically varies the number of discriminative bits considered per node to hold fanout
roughly constant, which bounds tree height independently of key distribution. Expanse
partitions by key expanse at a fixed 8-bit span per level, so its descent depth on 64-bit
keys is a function of how much of the key is discriminating. **The two designs differ in
exactly the axis that decides sparse-key point-lookup depth, which is why HOT is the
sharper test of the architecture than ART was.**

Implementation under test: [`speedskater/hot`](https://github.com/speedskater/hot) `96bf6fb`,
ISC licensed, header-only C++14, single-threaded variant, reached through the C++ FFI
foundation built in #660.

---

## 2. Prior Observations at Lock Time (mandatory disclosure)

This pre-registration is **not** blind. A Step 0 feasibility gate ran before it and its
results are recorded in
[#660 (Step 0 comment)](https://github.com/orieg/expanse/issues/660#issuecomment-5545881067).
Stating what was already seen is a precondition for the pre-registration being honest;
predictions below that were informed by these observations are marked **(informed)**.

Observed before lock, build-only census, sequential keys, `IdentityKeyExtractor<uint64_t>`
*(measured: x86_64 build host — Intel Xeon E5-2697 v4, Linux; HOT `96bf6fb`;
`g++ -O3 -std=c++17 -march=haswell -DNDEBUG`; Step-0 gate program, not a registered
harness; workload: `step0_hot_gate`)*:

| N | HOT B/key |
|---:|---:|
| 1k | 21.72 |
| 10k | 9.56 |
| 100k | 10.23 |
| 1M | 10.54 |

No HOT latency figure was observed. No valid HOT figure on any distribution other than
sequential was observed — the uniform-random pass of the gate was **invalid** (see §3.1)
and is discarded rather than carried forward.

---

## 3. Locked Constraints

### 3.1 63-bit keyspace, applied symmetrically

`HOTSingleThreadedChildPointer` tags leaves in bit 0 (`isLeaf() { return mPointer & 1; }`)
and recovers the stored value with `getTid() { return mPointer >> 1; }`. HOT's inline
value payload is therefore **63 bits**. Measured at the gate: keys with bit 63 set are
accepted by `insert()` (returns `true`) and then not found by `lookup()` — 0/1000 found,
against 1000/1000 for bit 62. A uniform-random 64-bit key stream silently loses about half
its population while the harness observes a fully successful build (AGENTS.md §8.1).

**Locked:** every arm draws keys from a 63-bit space. The restriction is applied to the
Expanse side identically, declared in each harness's workload shape, and stated in the
suite README. It is never applied to the HOT side alone. This is an encoding capacity
limit of the same tagged-machine-word class as this project's own `ValueSlot` (§2.1), not
a defect, and the suite describes it that way.

### 3.2 Memory census definition: bytes held from the allocator

HOT allocates nodes with `posix_memalign` inside a pooled allocator; nothing routes through
`operator new`. The census interposes the C allocator symbols at link time
(`-Wl,--wrap=posix_memalign,--wrap=malloc,--wrap=free`) with a pointer→size side table, and
validates against a known-size control allocation before any figure is recorded.

`MemoryPool::returnToPool` retains freed nodes on per-size-class free lists (≤199 entries
each, `MAXIMUM_NODE_SIZE_IN_LONGS = 60`), bounding retention at ~2.9 MB regardless of N.
During a build-only census HOT calls `free()` zero times, so bytes-allocated equals
bytes-held and retained-but-dead nodes sit inside the number.

**Locked:** the published metric is **bytes held from the allocator after a build-only
population**, applied symmetrically to the Expanse arena. The bound on free-list retention
is published alongside it.

### 3.3 Build-flag symmetry

HOT requires AVX2 and BMI2 and its authors specify `-march=haswell`. Expanse's published
figures are built at the default `x86-64` baseline unless a v2/v3 package is named. Comparing
an AVX2-targeted C++ arm against a baseline-targeted Rust arm is an ISA asymmetry, not a
design result.

**Locked:** both arms are built for the same ISA target for every published cell — HOT with
`-march=haswell`, Expanse with the equivalent `-C target-cpu=haswell`. The flags are recorded
in the suite README. If a baseline-Expanse cell is also reported, it is labelled as a
separate, explicitly non-comparable row.

---

## 4. The Two Symmetric Pairings

HOT's key extractor decides its value model, and the two available models correspond to two
different Expanse types. Rather than choosing one and inheriting its asymmetry, both are
measured.

### Arm A — HOT-set vs `ExpanseSet`

`HOTSingleThreaded<uint64_t, IdentityKeyExtractor>` against `ExpanseSet`. HOT stores the key
inline in the tagged child pointer; `ExpanseSet` stores membership in bitmap leaves. Neither
side carries a separate payload, so no heap indirection is introduced on either arm.

This is the harder test for Expanse and the arm that can produce a loss.

### Arm B — HOT-map vs `ExpanseMap`

`HOTSingleThreaded<std::pair<uint64_t,uint64_t>*, PairPointerKeyExtractor>` against
`ExpanseMap`. Both are key → 8-byte-value maps. HOT reaches its value through a
heap-allocated pair per entry; Expanse packs the value into a `ValueSlot`.

This is where every existing published Expanse figure lives, and it is structurally the same
shape as the blart comparison, whose 32-byte `LeafNode` imposed the analogous per-entry floor.

### Pillars (both arms)

1. Point lookup, 100% hit.
2. Point lookup, 50% hit / 50% miss, misses rejection-sampled from the **same** generator as
   the population (§8.6 — never a fixed transform of a present key).
3. Dynamic insertion into a cold structure.
4. Ordered range scan (k = 10, 100, 1000) and full in-order iteration.
5. Live memory census per §3.2, across N ∈ [1k, 10k, 100k, 1M].

Distributions: sequential, clustered, sparse stride, uniform random, Zipfian — matching
`art_comparison/` so the two suites' *Expanse* columns are directly relatable.

---

## 5. Expected-Losses Matrix

Registered before measurement. A suite that predicts only wins is not pre-registered.
Confidence is the pre-registration's own, not a measured quantity.

### 5.1 Where Expanse is expected to LOSE

| Arm | Pillar / regime | Prediction | Confidence | Reasoning |
|---|---|---|---|---|
| A | Memory, sparse stride | **HOT wins** | High | `ExpanseSet` sparse stride is 16.31 B/key *(measured: deterministic `NodeAlloc` accounting, `example_bytes_per_key`)* — one 16-byte edge per isolated key, the structural floor. HOT held 10.54 B/key on sequential at 1M **(informed)**. If HOT's height optimization holds its footprint roughly flat across distributions, it clears the Expanse sparse floor. *(workloads differ: `example_bytes_per_key` vs `step0_hot_gate`)* |
| A, B | Point lookup, uniform random | **HOT wins** | Medium-high | Uniform random is Expanse's worst point-lookup distribution: 43.32 ns at 1M against 17.79 ns sequential *(measured: reference host, harness 07b8413e, `art_lookup_hit`)*. Random 64-bit keys discriminate late, forcing deep descent at a fixed 8-bit span, while HOT's variable discriminative-bit selection is designed to bound height regardless of distribution. This is the mechanism ART lacked, which is why the analogous ART prediction was refuted in Expanse's favour and this one is registered again anyway. Hypothesis, unmeasured. |
| A, B | Range scan, k = 10 and k = 100 | **HOT wins** | Medium-high | `art_comparison/` recorded this as an unpredicted loss against blart — ART 2.09× at k=10 and 1.65× at k=100 *(measured: reference host, harness 07b8413e, `art_scan`)*. Expanse pays a per-scan descent and cursor setup that short scans cannot amortize. Nothing about HOT removes that cost, so the loss is carried forward as a prediction rather than rediscovered. |
| A, B | Point lookup, clustered | **BOUNDARY_RESULT or HOT win** | Low-medium | Clustered is Expanse's second-worst lookup distribution at 26.47 ns *(measured: reference host, harness 07b8413e, `art_lookup_hit`)*. Registered as genuinely uncertain rather than assigned a winner. |

### 5.2 Where Expanse is expected to WIN

Registered so that a win is a confirmed prediction rather than a headline discovered after
the fact, and so the low-information cells are visible as such.

| Arm | Pillar / regime | Prediction | Confidence | Reasoning |
|---|---|---|---|---|
| A | Memory, sequential and clustered | **Expanse wins, large margin** | High | `ExpanseSet` is 0.07 B/key sequential and 0.12–0.36 B/key clustered *(measured: `example_bytes_per_key`)* — bitmap leaves hold 256 keys per descriptor. HOT allocates per-entry regardless of density. This cell is low-information: it measures the presence of bitmap compression, not a contest. |
| B | Memory, all distributions | **Expanse wins** | High | Arm B hands HOT a heap-allocated pair per entry on top of its node memory, the same structural floor blart's 32-byte `LeafNode` imposed at 40.13 B/key *(measured: reference host, harness 07b8413e, `art_memory`)*. `ExpanseMap` packs the value in a `ValueSlot`. Predicting this is not a claim of architectural advantage — it is a consequence of the value model, and the README will label it `PASS_categorical_by_design` if it lands as predicted. |
| A, B | Full in-order iteration | **Expanse wins** | Medium | Expanse iterates contiguous leaf arrays; the ART arm measured 10.98× on random at 1M *(measured: reference host, harness 07b8413e, `art_scan`)*. HOT's iterator walks a shallower but pointer-rich structure. Lower confidence than against blart. |
| A, B | Point lookup, sequential and sparse stride | **Expanse wins** | Medium | Expanse's strongest lookup regimes, 17.79 ns and 14.81 ns *(measured: reference host, harness 07b8413e, `art_lookup_hit`)*, where the trie skips empty expanses outright. |
| A, B | Insertion | **Expanse wins** | Low-medium | Expanse beat blart 1.71×–4.84× *(measured: reference host, harness 07b8413e, `art_insert`)*, but HOT's insert path is more optimized than blart's and performs height-directed restructuring. Registered as a weak prediction. |

### 5.3 Explicitly not predicted

Zipfian on either arm, and the 50%-miss pillar. No mechanism argument distinguishes the two
designs there in advance. Outcomes will be reported as `not pre-registered`.

---

## 6. Gate Taxonomy

Every published cell carries exactly one label.

| Label | Condition |
|---|---|
| `CONFIRMED` | Pre-registered direction, and the BCa 95% CI excludes parity. |
| `REFUTED` | Opposite direction to the pre-registration, CI excludes parity. Reported as prominently as a confirmation, in whichever direction it falls. |
| `BOUNDARY_RESULT` | CI includes 1.00×. No winner is claimed. |
| `PASS_categorical_by_design` | Pre-registered direction, but the losing arm is denied a winning regime by its value model rather than out-competed (§C-b). Applies by default to Arm B's memory cells. |
| `not pre-registered` | Outside §5.1/§5.2; reported with its number and no verdict framing. |
| `UNPREDICTED LOSS` | A loss in a cell §5.2 predicted as a win. Reported in the executive scorecard, not a footnote. |

Statistical gating per §8.4: wall-clock and continuous metrics pass on the **BCa 95%
bootstrap CI lower bound**, ≥1,000 resamples, never a point estimate. The memory census is a
deterministic byte count and carries no interval — an interval on an exact integer is wrong,
not missing.

---

## 7. Claims Ceiling

What this suite will be permitted to claim when it lands, stated before the numbers exist:

1. **Single-threaded only.** The ROWEX concurrent arm is separate and conditional; no
   concurrency claim follows from this suite.
2. **63-bit integer keys only.** No claim about full 64-bit keyspaces, and no string-key
   claim — the string arms are separate scope in #660.
3. **x86-64 with AVX2 and BMI2 only.** No aarch64 claim of any kind; HOT does not build there.
4. **One HOT implementation at one commit.** Claims attach to `speedskater/hot` `96bf6fb`
   built as documented, not to "HOT" as a design in the abstract, and not to the figures in
   the SIGMOD paper, which were measured on different hardware with a different harness.
5. **No cross-suite ratio.** A HOT-vs-Expanse ratio from this suite is never placed beside an
   ART-vs-Expanse ratio from `art_comparison/` as though the two were commensurable (§8.12).
   The shared Expanse column is the only legitimate bridge, and only where the workload IDs
   match.
6. **No peer review.** This is internal work. Every verdict carries that qualifier.

---

## 8. What Would Falsify the Suite Itself

Recorded so the harness cannot quietly fail into a flattering result:

- Any arm where the HOT side's final population differs from the intended population is void.
  Population is asserted after build on both arms, not assumed from `insert()` return values —
  the gate proved `insert()` returns `true` on keys it then cannot find.
- Any census where the control allocation does not move the counter by its known size is void.
- Any latency cell whose paired arms did not consume identical key streams from the same
  seed is void.
- Any cell where the two arms were built for different ISA targets is void (§3.3).
