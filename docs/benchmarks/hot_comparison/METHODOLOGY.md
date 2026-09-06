# HOT (Height Optimized Trie) vs. Expanse: Pre-Registration & Comparative Methodology

**Status: §1–§9 (single-threaded arms) and §11 (HOT-ROWEX concurrent arm,
[#692](https://github.com/orieg/expanse/issues/692)) measured on the reference
host and published in [`README.md`](README.md) (§1–§5 and §6 respectively).
§11's text is the locked pre-registration; outcomes are recorded in the README
and are never reconciled into it (§8.7).**

This document is commit 2 of the three-commit cadence (AGENTS.md §8.8): hypothesis,
claims ceiling, expected-losses matrix and gate taxonomy, committed *before* the
harness and before any measurement. Delivers the pre-registration half of
[#660](https://github.com/orieg/expanse/issues/660). The status line above is
the integer arms' locked text; their measurement is in the README. The
string-key arms ([#693](https://github.com/orieg/expanse/issues/693)) are
pre-registered in §10, under their own status line.

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

> **Amended by §9.5.** Every memory prediction below is stated at a population.
> Population alone does not identify a memory cell for this engine — expanse
> occupancy does — so the memory rows are re-expressed in §9.5 and the
> sparse-stride row's confidence is downgraded there. The latency rows stand.


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
2. **Integer keys, full 64-bit domain.** *(Amended by §9.4 — the 63-bit exclusion
   originally recorded here was inherited from the superseded §3.1, not derived from the
   question.)* Arm A additionally reports the portion of that domain HOT's inline payload
   cannot represent, as a finding about HOT. No string-key claim — the string arms are
   separate scope in #660.
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

---

## 9. Amendments after Pre-Registration

Recorded as amendments rather than edited into §3, so the locked text stays
readable as what was locked (§8.7 forbids reconciling a pre-registration in
place). Both entries below are measurement constraints found while building the
harness; neither is a result.

### 9.1 One instrument for both arms, not two (supersedes the §3.2 method)

§3.2 locked allocator-symbol interposition for the HOT side and implied the
Expanse side would be accounted separately. Building it showed the stronger
option: Rust's allocator bottoms out in the same C symbols HOT uses, so a single
link-time interposition (`-Wl,--wrap=malloc,--wrap=calloc,--wrap=realloc,`
`--wrap=free,--wrap=posix_memalign,--wrap=aligned_alloc`) measures **both** arms
under one definition. The symmetry §8.3 requires then holds by construction
rather than by arguing two counters into agreement. Validated by a control
allocation on every run: `+1,052,656 B` observed on a `1,048,576 B` request,
residual `0 B` after free.

The counters are `std::atomic` and the shim is compiled `-fno-builtin-malloc`
`-fno-builtin-calloc -fno-builtin-realloc -fno-builtin-free`. Both are
load-bearing rather than stylistic: the compiler knows the allocator family as
builtins and may assume the calls do not touch globals, which lets it cache a
plain counter across them. That is the defect the Step 0 gate program hit, where
`free` was observed running while the byte total did not move.

### 9.2 Every census cell runs in its own process (new constraint)

HOT's node pool is a function-local `static` inside
`HOTSingleThreadedNodeBase::getMemoryPool()`, so it is **process-global and
outlives every trie instance**. A trie that is built and dropped leaves its nodes
on the pool's free lists, and the next trie reuses them without calling
`posix_memalign` — invisible to any allocator-level census.

Measured on the identical 100k uniform-random build *(measured: x86_64 build host
— Intel Xeon E5-2697 v4; `-C target-cpu=haswell`, `-march=haswell`; foundation
validation binary; workload: `hot_validate_census`)*:

| Pool state at census entry | HOT | `ExpanseSet` |
|---|---:|---:|
| Cold (0 prior allocations) | **11.76 B/key** | 13.38 B/key |
| Warm (31,093 prior allocations) | 3.61 B/key | 13.39 B/key |

A **3.3× understatement** of HOT, from measurement order alone. The Expanse arm
is unchanged to two decimals because it has no equivalent process-global pool,
which makes the error **asymmetric and in HOT's favour** — the shape of mistake
that reads as a competitor win.

**Locked:** one arm, one population, one process. The shim exposes HOT's own pool
counter and the harness asserts it is zero before a census, so a violation fails
loudly instead of depending on the runner's ordering. A census cell taken on a
warm pool is void.

### 9.3 The Expanse cells here will not equal the repo's `bytes/key` table

Two independent reasons, both measured rather than argued. Reconciled by
`crates/expanse-hot-bench/src/bin/instrument_bridge.rs`, which reads both
instruments on the same build of the same key stream *(measured: x86_64 build
host — Intel Xeon E5-2697 v4; `-C target-cpu=haswell`; workload:
`hot_instrument_bridge`)*. That probe reproduces the committed
`example_bytes_per_key` figures exactly — sequential 1M `0.07`, clustered 1M
`0.36`, sparse 1M `16.31`, random 100k `14.78`, random 1M `7.92` B/key — so the
generator and the engine accounting are confirmed identical before anything is
compared.

**Reason 1 — different instrument.** The repo's table publishes `mem_used()`,
the engine's byte-exact node accounting. This suite must publish bytes held from
the C allocator, the only definition HOT can also be measured under (§3.2). The
gap is allocator chunk headers, size-class rounding, and arena capacity the
engine does not count as used, and it is **not a constant factor**:

| Distribution | N | `mem_used` B/key | allocator B/key | ratio |
|---|---:|---:|---:|---:|
| sequential (set) | 10k | 0.09 | 6.62 | 72.6× |
| sequential (set) | 100k | 0.07 | 0.81 | 11.9× |
| sequential (set) | 1M | 0.07 | 0.14 | 2.1× |
| sparse (set) | 1M | 16.31 | 16.42 | 1.007× |
| random 64-bit (set) | 1M | 7.92 | 12.62 | 1.59× |

The ratio collapses toward 1.0 as either the population or the per-key footprint
grows, because fixed arena capacity amortizes. It is largest exactly where
Expanse is most compact, so **the allocator instrument understates Expanse's
dense-distribution advantage** relative to the repo's own table. *(Amended by
§9.10.6: the `random` row's 1.59× is a generator-order figure; the same
structure built from sorted keys — which is how every allocator-instrument cell
of this suite is built — measures 1.03×. The instrument gap has an
insertion-order term this table did not name.)* Any cell
published here states which instrument produced it, and no cell from this suite
is set beside a `mem_used` cell as though the two were the same quantity
(§8.12).

**Reason 2 — different keyspace.** The repo's `random` row draws full 64-bit
keys. This suite is locked to 63 bits (§3.1), and that is not cosmetic on this
distribution:

| Keyspace | N | `ExpanseSet` `mem_used` B/key |
|---|---:|---:|
| 64-bit random | 1M | 7.92 |
| 63-bit random | 1M | 13.60 |

**+72% against Expanse**, on the engine's own instrument, from the constraint
HOT's encoding imposes. The suite's random-distribution memory cells are
therefore not comparable to the repo's published random cells in either
direction, and the README says so next to them rather than in a footnote.

*(Reason 2 is withdrawn by §9.4: the 63-bit restriction it describes is no
longer in force, and the "unexplained" direction it recorded is now measured.
The instrument gap of Reason 1 stands unchanged.)*


### 9.4 The 63-bit lock is withdrawn; §3.1 was locked on a false premise

§3.1 locked a 63-bit keyspace **for both arms and both systems**, on the reading
that §8.3 symmetry means identical key bytes. That was wrong in two independent
ways, and the constraint is withdrawn in full.

**It was factually over-broad.** HOT's 63 bits are the width of its *inline
value payload* (`getTid() { return mPointer >> 1; }`), not of its keys. That
binds the keyspace only where the stored value *is* the key — Arm A's
`IdentityKeyExtractor`. Arm B stores a heap pointer and takes the whole domain.
Measured on keys spanning bit 63 (`1`, `42`, `2^62+7`, `2^63`, `2^63+99`,
`u64::MAX`) *(measured: x86_64 build host — Intel Xeon E5-2697 v4; HOT
`96bf6fb`; workload: `hot_arm_capability`)*:

| Arm | Found | Correct value | Population by walk |
|---|---:|---:|---:|
| A — `IdentityKeyExtractor` | 3/6 | — | — |
| B — `PairPointerKeyExtractor` | **6/6** | **6/6** | **6/6** |

Arm B never needed the restriction, and the shim was rejecting keys it handles.

**It was not symmetric in effect, which is what §8.3 actually asks.** Identical
key bytes are a *mechanism* for symmetry, not its definition; the definition is
that both systems are asked the same question. HOT selects discriminative bits
dynamically to hold fanout roughly constant, so removing one top bit is close to
a no-op for it. Expanse partitions by key *expanse* at a fixed 8-bit span, and
for such a structure the only parameter that matters is occupancy per expanse,
`d = N / 2^w`. **Halving the keyspace is therefore arithmetically identical to
doubling the population.** The lock silently doubled Expanse's effective load
factor and left HOT's workload materially unchanged.

That equivalence is measured, not argued — `ExpanseSet`, `mem_used()`, same PRNG
and seed *(measured: x86_64 build host — Intel Xeon E5-2697 v4; workload:
`hot_keyspace_density_probe`)*:

| N | 64-bit | 63-bit | 62-bit |
|---:|---:|---:|---:|
| 100k | 14.78 | 12.60 | 10.42 |
| 200k | 12.59 | 10.41 | 8.42 |
| 400k | 10.41 | 8.42 | 8.47 |
| 600k | 9.17 | 7.64 | 19.25 |
| 800k | 8.41 | 8.49 | 21.02 |
| 1M | 7.92 | 13.60 | 19.66 |
| 1.2M | 7.64 | 19.30 | 18.50 |
| 2M | 13.60 | 19.67 | 15.58 |

63-bit at N reproduces 64-bit at 2N, and 62-bit at 4N, to two decimals:
`12.60 ≈ 12.59`, `8.42 ≈ 8.41`, `8.42 ≈ 8.41`. One curve in one variable.

**And the effect has no stable sign.** The restriction *helps* Expanse below
600k (−15% to −19%), is at parity near 800k (+1%), and costs +72% at 1M and
+153% at 1.2M. A workload parameter chosen for a reason internal to the other
system, whose sign and magnitude depend on N, is a free variable, and the
pre-registered headline population is where it happens to be worst.

**Each curve also crosses a density cliff**, positioned by keyspace width and
halving per bit removed: 64-bit in (1.2M, 2M], 63-bit in (800k, 1M], 62-bit in
(400k, 600k]. At the headline N=1M the 64-bit arm is pre-cliff and the 63-bit
arm is post-cliff, so the lock did not shift Expanse along a smooth curve — it
carried it across a discontinuity.

**The cliff is `LEAF_CAP`, and that is measured rather than inferred.**
`LEAF_CAP = 32` (`crates/expanse/src/types.rs:99`) is the linear-leaf population
cap at levels 2..=7; overflow cascades into a branch whose children are
single-key immediates, at one 16-byte edge per key. For uniform random keys the
top two bytes saturate, so the controlling parameter is the occupancy of a
2-byte-prefix expanse, `λ = N / 2^16` at 64 bits and `N / 2^15` at 63 bits — the
top byte taking 128 values rather than 256. At N=1M that is λ=15.26 (48% of
`LEAF_CAP`, a packed `Leaf6` at ~6 B/key) against λ=30.52 (**95% of
`LEAF_CAP`**, Poisson σ=5.5, so ~39% of expanses overflow and pay ~17 B/key).
The mixture is the observed 13.60.

Single-variable causal test — `LEAF_CAP` changed alone, everything else
identical *(measured: x86_64 build host — Intel Xeon E5-2697 v4; workload:
`hot_keyspace_density_probe`)*:

| `LEAF_CAP` | 64-bit @1M | 63-bit @1M | 64-bit @2M |
|---|---:|---:|---:|
| 32 (shipped) | 7.92 | **13.60** | 13.60 |
| 48 | 7.92 | **6.99** | 7.00 |

Raising the cap abolishes the inversion — 63-bit becomes *cheaper* than 64-bit,
the intuitive direction — and leaves the 64-bit 1M cell unchanged to the
decimal. No other explanation survives that. This is **not** a recommendation to
change `LEAF_CAP`: a larger linear leaf trades memory against scan cost on the
read path, which this suite has not measured and which belongs to the engine's
own instruments, not to a comparison harness.

**Retraction (§8.10).** The `13.60 B/key` figure published for `ExpanseSet`
uniform random at 1M in the first revision of §9.3 is **formally retracted as an
artifact of a withdrawn constraint**. It is dead, not withheld for a later run —
no measurement is outstanding against it. It is
not a property of `ExpanseSet`; it is a measurement of the harness. The
corresponding un-restricted figure is `7.92 B/key`, which is the value the
repo's committed `bytes/key` table already carries. The retracted number is not
silently replaced — the correction is the point.

Two consequences are logged for follow-up outside this suite, because they
concern the engine and its gates rather than the comparison:

1. The committed `memory-budget` ceiling in
   `crates/expanse/examples/bytes_per_key.rs` — `("random", 1_000_000, 9.00,
   18.00)` — is calibrated at λ=15.26, **48% of `LEAF_CAP`, near a trough**. Its
   headroom is ~2× *in N*, not the 12% *in bytes* the numbers suggest, and
   nothing records that. The same workload at 2M measures 13.60 B/key and would
   breach the ceiling by 51%, so a legitimate population change reads as a
   regression. The withdrawn 63-bit figures would have breached it in both
   flavours (set 13.60 > 9.00, map 19.38 > 18.00).
2. The density behaviour itself — bytes/key as a function of λ, and the
   `LEAF_CAP` overflow cascade that shapes it — is undocumented. It is a
   property of the node ladder, not of this comparison, and belongs in
   `docs/ARCHITECTURE.md` whether or not this suite ever ships.

**Standing rule this episode earns.** *A twin's limitation is recorded as a
predicate on the twin and evaluated against the workload; the workload is never
edited to accommodate it. If the remedy touches a path the incumbent also reads,
it is in the wrong place.* §3.2 already did this correctly for HOT's
process-global `MemoryPool` — instrumenting on HOT's side rather than changing
what both arms measure — so the precedent was in this file before §3.1 was
written.


### 9.5 Memory predictions are re-expressed at a density, not a population

§9.4 established that per-key cost for this engine is governed by expanse
occupancy, `λ = N / (populated 2-byte-prefix expanses)`, and that `LEAF_CAP`
sets a cascade the curve crosses. A memory prediction stated at a population is
therefore under-specified: two cells with the same declared `population` can sit
on opposite sides of the cascade, and two with different populations can be the
same measurement. §5's memory rows are re-expressed accordingly.

**Density is a required workload-shape field for this suite.** Every memory cell
declares `λ` alongside `population` and `keyspace_width`, and no two memory cells
are compared unless their `λ` is stated. This is a §8.12 field for the suite's
harnesses, not a repo-wide change.

**Which anchors move and which do not.** The distinction is derivable from the
generators, not a matter of measurement luck:

| Distribution | Generator | λ | Density-dependent? |
|---|---|---|---|
| `sequential` | `i` | dense, contiguous | No — fully packed regardless of N |
| `clustered` | `base + (i % 256)`, `base` random | 256 per cluster | No — cluster size fixes it |
| `sparse` | `i << 40` | **256 exactly** (top two bytes are `i >> 8`, so each expanse holds 256 consecutive keys) | No — 8× above `LEAF_CAP`, permanently cascaded |
| `random` | full-width draw | `N / 2^w` | **Yes** — this is the only one that moves |

That also explains the `sparse` anchor's stability at `16.83 / 16.32 / 16.31`
across three decades of N: at λ=256 every expanse is past the cascade, so the
cost is one 16-byte edge per key at any population. It is the **saturated**
regime, not a lower bound on the structure — `random` measures 7.92 B/key below
it when λ sits under `LEAF_CAP`.

The derivation was checked past the committed range rather than assumed
*(measured: x86_64 build host — Intel Xeon E5-2697 v4; `-C target-cpu=haswell`;
`mem_used()`; workload: `hot_keyspace_density_probe`)*:

| N | `sequential` | `clustered` | `sparse` `i << 40` |
|---:|---:|---:|---:|
| 100k | 0.07 | 0.37 | 16.32 |
| 400k | 0.07 | 0.37 | 16.31 |
| 1M | 0.07 | 0.36 | 16.31 |
| 2M | 0.06 | 0.36 | **16.31** |

All three are flat to two decimals across a 20× population range, including past
the point where the `random` curve has already crossed its cascade (13.60 at 2M).
Construction-fixed occupancy behaves as derived; only `random` moves.

**§5.1's sparse-stride row is downgraded from High to Low confidence**, for a
reason unrelated to density. It compares `ExpanseSet` on *sparse stride*
(16.31 B/key) against HOT's Step-0 figure of 10.54 B/key, which was measured on
*sequential* keys — the paragraph carries the `(workloads differ: …)` tag, but
the prediction's confidence was set as though the two were comparable. It rests
on an untested assumption that HOT's footprint is roughly distribution-invariant.
No HOT figure on sparse keys exists yet. The row stands as a registered
prediction; its confidence does not.

**§5.1's and §5.2's uniform-random memory rows** are re-expressed at λ rather
than N. The registered direction is unchanged; what changes is that a cell is
only evaluable against a prediction at the same λ. The pre-registered headline
N=10^6 at 64 bits is λ=15.26, 48% of `LEAF_CAP` — deliberately recorded, because
it is a trough, and a suite that published only that point would be sampling the
curve at its most flattering position for Expanse.

The latency, insert and scan predictions are untouched: nothing measured here
shows those to be density-governed, and asserting it without measurement is the
error §8.9 forbids.


### 9.6 The memory pillar publishes a curve across λ, and Arm A declares its domain

Two design decisions follow from §9.4 and §9.5. Both are locked here before the
harnesses exist.

#### Memory is a curve, not a cell

§9.4 measured a sawtooth: the same `ExpanseSet` spans 7.64–21.02 B/key under
density alone, with a cascade at `λ ≈ LEAF_CAP`. **A single-population memory
cell is therefore a cherry-pick whichever side of the cascade it lands on**, and
the pre-registered N=10^6 at 64 bits lands at λ=15.26 — 48% of `LEAF_CAP`, near a
trough, which is the most flattering point available to Expanse.

The memory pillar publishes **bytes/key as a function of λ**, swept across the
cascade on both arms, rather than a point per distribution. This departs from
`art_comparison/`'s one-population-per-cell shape deliberately: that suite's
competitor has no equivalent discontinuity, so a point cell was defensible there
and is not here.

A verdict is stated over an interval of λ or not at all. "Expanse uses less
memory than HOT" is not a claim this suite can make without saying at which
occupancy, and the honest answer may be "below the cascade yes, above it no".

#### Arm A restricts its generator and says so in its name

HOT's inline payload cannot represent keys with bit 63 set (§9.4), so Arm A —
`HOTSingleThreaded<uint64_t, IdentityKeyExtractor>` — cannot hold a full 64-bit
uniform stream. Rather than let its population silently differ from Arm B's, Arm
A **draws from a 63-bit domain on both of its sides**, under its own workload ID,
titled so the restriction and its cause are visible:

> `hot_set_63bit` — HOT set arm vs `ExpanseSet`, 63-bit keys, restricted because
> HOT's inline value payload is 63 bits wide.

This is not the withdrawn §3.1 lock returning. The difference is scope and
labelling: §3.1 imposed 63 bits on **both arms and the whole suite**, and
published the result under names — "`ExpanseSet`, uniform random, 1M" — that
collide with the repo's own committed cells. Arm A's restriction is confined to
one pairing, is named for the reason it exists, and its cells never appear beside
Arm B's or the repo's (§8.12). Arm B remains at the full 64-bit domain.

#### Why the two decisions compose

Plotted against λ rather than N, **the restriction stops being a confound**.
Halving the keyspace is exactly a doubling of density (§9.4), so Arm A's 63-bit
Expanse curve and Arm B's 64-bit Expanse curve are the *same curve* — Arm A's
points simply sit at 2× the λ of Arm B's at equal N. The two arms therefore
share a comparable axis despite differing domains, which no choice of population
could have given them.

That is a testable prediction, not a convenience. **Corrected by §9.9** — the
paragraph originally continued by claiming the harness asserts the two arms'
Expanse curves superimpose on λ. They cannot: Arm A pairs a *set* and Arm B a
*map*, so they hold different payloads. What the arms can check is the payload
delta, and the density falsifier is same-flavour and cross-keyspace. See §9.9.


### 9.7 The census also has to replace `operator new`

`-Wl,--wrap=malloc` rewrites symbol resolution **only for the objects being
linked**. libstdc++'s `operator new` lives in `libstdc++.so` and reaches malloc
through the dynamic linker at runtime, so nothing it allocates is wrapped.

Arm B allocates one heap `std::pair` per entry through `operator new`. Measured
at N=100,000 *(measured: x86_64 build host — Intel Xeon E5-2697 v4;
`-C target-cpu=haswell`; workload: `hot_memory_curve`)*:

| Arm B, N=100k | counted allocations | HOT B/key |
|---|---:|---:|
| malloc interposition alone | 4,611 | 11.77 |
| plus replaced `operator new` | **104,612** | **35.77** |

A **3× understatement**, and in HOT's favour on the arm where its value model is
most expensive — the per-entry heap pair is the whole reason Arm B exists, and it
was the part the census could not see.

The shim now defines the replaceable global allocation functions
(`operator new`/`new[]`/`delete`/`delete[]`, sized and unsized), which the
standard makes program-wide, routing every `new` through the counted path.

This is the mirror image of the Step 0 finding, and the pair is worth stating
together: there, an `operator new` counter missed HOT's `posix_memalign` nodes;
here, a malloc counter missed the C++ `operator new` pairs. Neither instrument
alone sees the arm. `hot_validate` now asserts the count directly — 50,000 heap
pairs must produce at least 50,000 counted allocations — so a regression fails
loudly rather than returning a flattering number.

Three census defects have now been found in this integration, all of which
would have published a plausible wrong figure: the process-global node pool
(§9.2, 3.3×), the unwrapped `operator new` (here, 3×), and the free path that
never subtracted (§9.1). Every one understated somebody's memory.


### 9.8 Probe symmetry, checked before the latency pillars were written

The latency pillars rest on an assumption worth testing rather than asserting:
that both arms do the *same work* per probe and that the work is consumed. This
repo has already retracted absolute figures from a harness whose read path never
dereferenced the payload, billing index traversal only (`docs/DATABASE.md` §7),
and every census defect in this integration was found by running something
rather than by reading it.

`hot_probe_symmetry` checks four properties, all deterministic (§8.4 — no
wall-clock, so no intervals apply). Result *(measured: x86_64 build host — Intel
Xeon E5-2697 v4; `-C target-cpu=haswell`; workload: `hot_probe_symmetry`)*:

| Arm | Probes | Hit rate | Disagreements | Values | Sinks |
|---|---:|---:|---:|---|---|
| A — set, 63-bit | 200,000 | 50.0% | 0 | n/a | equal |
| B — map, 64-bit | 200,000 | 50.0% | 0 | identical | fed |

1. **Answer agreement** — HOT and Expanse return the same result on every probe.
   A disagreement voids the arm and would otherwise surface as a latency
   difference rather than as the correctness failure it is.
2. **Value consumption** — on Arm B both sides produce the stored *value*, not a
   presence bit, and both sinks are fed from it. HOT reaches its value through a
   heap pointer; Expanse reads it from an inline `ValueSlot`. **That cost
   asymmetry is the architectural difference Arm B exists to measure, and it is
   only a fair measurement because both arms are made to fetch.** A probe that
   stopped at `mIsValid` would hand HOT the traversal without the dereference.
3. **Miss shape** — misses are rejection-sampled from the same generator as the
   population, never a transform of a present key (§8.6).
4. **Probe order** — the stream is shuffled with Fisher-Yates from the same
   PRNG. This was a **defect found while writing the check**: the population is
   sorted, so hits drawn by index arrived in ascending key order, handing an
   ordered trie cache and prefetch behaviour no real point-lookup workload
   provides. `art_comparison/` had to amend for exactly this after its first
   reference-host run; inheriting the amendment cost nothing, rediscovering it
   would have cost a sweep.

Arm A compares membership against membership with no value on either side, so
consumption there is the presence bit itself.

### 9.9 The two arms cannot superimpose — §9.6's falsifier was mis-stated

§9.6 claimed the harness would assert that Arm A's and Arm B's Expanse curves
superimpose when plotted against λ, and treat a failure as voiding the memory
pillar. Running the sweep showed the claim is structurally impossible: **Arm A
pairs `ExpanseSet` and Arm B pairs `ExpanseMap`**, so the two hold different
payloads and their curves are separated by a value word at every occupancy. The
underlying density reasoning is unaffected; the falsifier was pointed at the
wrong pair of curves.

Measured on the quick sweep *(measured: x86_64 build host — Intel Xeon E5-2697
v4; `-C target-cpu=haswell`; `mem_used()`; workload: `hot_memory_curve`;
development host, not the reference host — smoke, not a published cell)*:

| λ | `ExpanseSet` | `ExpanseMap` | ratio | difference |
|---:|---:|---:|---:|---:|
| 1 | 14.06 | 22.66 | 1.61× | 8.60 |
| 2 | 13.76 | 24.09 | 1.75× | 10.33 |
| 4 | 11.83 | 22.28 | 1.88× | 10.45 |
| 8 | 9.56 | 19.05 | 1.99× | 9.49 |

The **ratio** drifts 1.61 → 1.99 and looks like a violation; the **difference**
stays within about 2 B/key of one value word and is the quantity that should be
flat. The ratio moves only because the set's base cost falls with density while
the added word does not.

**What each check is actually for:**

- *Payload delta* (`payload_delta_check`) — map minus set at matched occupancy,
  expected ≈ one value word and flat. This is what the two arms can tell you
  about each other, and it is a check on the harness, not on the engine.
- *Density model* — the real falsifier is same-flavour and cross-keyspace:
  `ExpanseSet` at 63 bits and λ must equal `ExpanseSet` at 64 bits and the same
  λ. `keyspace_density_probe` measures exactly that and it holds to two decimals
  across three width/N pairings (§9.4). It is not re-run inside the suite
  because it needs a keyspace the arms do not vary independently.

Recorded rather than quietly fixed because §9.6 is a locked decision and this
changes what one of its stated guarantees means (§8.7). The memory pillar's
design — a curve across λ, Arm A labelled for its restricted domain — is
unchanged.

---

### 9.10 The sweep extended: second tooth, node census, seed sensitivity, the cap-48 control's read path, and the two instruments reconciled

Extension of §9.4 and §9.5, measured after those sections were written and
not reconciled into them. Every deterministic cell below is the engine's own
`mem_used()` accounting, host-independent, from
`crates/expanse/examples/keyspace_density.rs --json` *(measured: deterministic
byte accounting; workload: `example_keyspace_density`; committed as
`docs/assets/data/bench_assets.json` → `density_sweep`, commit 86daaddf;
`crates/expanse/tests/test_visualizer_sync.rs` recomputes every 64-bit and
every second-tooth cell from the engine)*. The wall-clock cells are in their
own table (§9.10.5) with their instrument, host and intervals; no
deterministic figure shares a table with them.

#### 9.10.1 The first tooth at finer resolution (64-bit, item 2 and item 9)

The 1.2M → 2M gap of §9.4 is filled so the trough and the knee are located
between measured neighbours. Set and map flavors are both given (item 8).

| N @64 | λ | λ / `LEAF_CAP` | `ExpanseSet` B/key | `ExpanseMap<u64,u64>` B/key |
|---:|---:|---:|---:|---:|
| 1,000,000 | 15.26 | 48% | 7.92 | 16.70 |
| 1,100,000 | 16.78 | 52% | 7.76 | 16.46 |
| 1,200,000 | 18.31 | 57% | 7.64 | 16.28 |
| 1,300,000 | 19.84 | 62% | 7.59 — trough | 16.15 |
| 1,400,000 | 21.36 | 67% | 7.66 | 16.11 |
| 1,600,000 | 24.41 | 76% | 8.46 | 16.44 |
| 1,800,000 | 27.47 | 86% | 10.51 — knee | 17.58 |
| 2,000,000 | 30.52 | 95% | 13.60 | 19.38 |
| 2,200,000 | 33.57 | 105% | 16.83 | 21.31 |
| 2,400,000 | 36.62 | 114% | 19.26 | 22.77 |
| 2,600,000 | 39.67 | 124% | 20.67 | 23.64 |

The trough is at N = 1.3M (λ = 19.84, 7.59 B/key), bracketed by
7.64 at 1.2M and 7.66 at 1.4M, not at the 1.2M cell §9.4 happened to
end on. The curve is flat within 0.4 B/key from λ = 15 to 21 and then climbs
13 B/key by λ = 49; the knee — where the Poisson model says the cascade is 10%
on, λ = 25.9 — is the 1.8M cell at 10.51. The exact-λ cells at 27, 40
and 58 (63- and 62-bit equivalents of 1.77M, 2.62M and 3.8M @64, item 10) sit
on the same curve: 10.12, 20.79 and 20.00 B/key at λ = 27.00, 40.00 and 58.00.

#### 9.10.2 The second tooth (items 1 and 11)

Below a cascaded 2-byte expanse the level-5 sub-expanses hold λ / 256 keys
each, in `Leaf5` leaves under the same `LEAF_CAP`, so the prediction was a
second trough before λ ≈ 256 × 32 = 8,192 and a second rise across it.
`BranchU` is the branch form a cascaded expanse takes once more than
`BITMAP_TO_UNCOMPRESSED_THRESHOLD` = 192 of its 256 sub-expanses are populated.

| N @bits | λ | λ / 256 | `ExpanseSet` B/key | `ExpanseMap` B/key | `BranchU` nodes / level-6 expanses | level-5 sub-expanses cascaded |
|---:|---:|---:|---:|---:|---:|---:|
| 2,000,000 @58 | 1,953.1 | 7.63 | 8.80 | 18.51 | 1,028 / 1,024 | 0 of 262,144 (0.0%) |
| 2,000,000 @57 | 3,906.2 | 15.26 | 6.89 | 15.68 | 514 / 512 | 4 of 131,072 (0.0%) |
| 1,200,000 @56 | 4,687.5 | 18.31 | 6.71 | 15.35 | 257 / 256 | 81 of 65,536 (0.1%) |
| 1,700,000 @56 | 6,640.6 | 25.94 | 8.44 | 16.03 | 257 / 256 | 6,572 of 65,536 (10.0%) |
| 2,000,000 @56 | 7,812.5 | 30.52 | 12.98 | 18.74 | 257 / 256 | 22,894 of 65,536 (34.9%) |
| 2,700,000 @56 | 10,546.9 | 41.20 | 20.98 | 23.76 | 257 / 256 | 60,081 of 65,536 (91.7%) |
| 2,000,000 @55 | 15,625.0 | 61.04 | 19.66 | 23.04 | 128 / 128 | 32,768 of 32,768 (100.0%) |

`BranchU` nodes exceed the level-6 expanse count by the level-7 nodes where
those are uncompressed too (4, 2 and 1 at 58, 57 and 56 bits; the level-8
node is a linear branch); at 55 bits level 7 is a 128-child bitmap branch and
the 128 `BranchU` are all level 6. So every level-6 expanse is a `BranchU` in
all seven cells (workload: `example_keyspace_density`). The predictions of
items 1 and 11 both hold, and the shape is the first tooth's. The second trough is at λ = 4,688 (6.71 B/key, 1.2M @56) against
"near λ ≈ 4,700", bracketed by 6.89 at 3,906 and 8.44 at 6,641; the ramp
runs 8.44 → 12.98 → 20.98 across λ = 6,641 → 7,812 → 10,547 against
"between λ ≈ 6,600 and 10,400". The four 2M cells at
58–55 bits sit at sub-expanse occupancies 7.6, 15.3, 30.5 and 61.0 — the λ of
the 400k, 1M and 2M @64 cells and of the 1M @62 cell one level up — and
measure 8.80, 6.89, 12.98 and 19.66 B/key against 10.41, 7.92, 13.60 and
19.66 there. The 56-bit ladder reads 6.71 at
λ = 4,688, 8.44 at 6,641, 12.98 at 7,812 and 20.98 at 10,547, with the
cascaded sub-expanse fraction at 10.0%, 34.9% and 91.7% at
the last three — the Poisson model's 10%→90% ramp for this tooth runs from
λ = 6,627 to 10,379 (`scripts/density_poisson.py`), and at
λ / 256 = 30.52 the model's 35.0% is the first tooth's 2M @64 fraction
reproduced one level down. **The second trough is lower than the first** (6.71
against 7.59) because the 4,160-byte `BranchU` at level 6 is amortised over
thousands of keys and `Leaf5` packs 5 bytes per key where `Leaf6` packs 6;
**the second peak is the first peak's number** (19.66 at λ = 15,625, 19.66 at
λ = 61.04) because every sub-expanse has cascaded into a bitmap branch of
single-key immediates, which is exactly the level-6 structure at λ = 61.
`BranchU` nodes appear at level 6 in all seven cells — from λ ≳ 192 a cascaded
expanse has more populated sub-expanses than the bitmap threshold — whereas at
the first-tooth cells the only uncompressed branches are the 257 (64-bit) or
65 (62-bit) of levels 8 and 7.

#### 9.10.3 Node census at three cells (item 3), against the Poisson model (item 4)

`ExpanseSet::stats()` / `ExpanseMap::stats()` on the built structure. `NodeBytes`
is the per-form attribution added for this measurement; it is a decomposition
of `mem_used()`, not an estimate beside it — the walk charges every allocation
to the form that owns it and the total is asserted equal to `mem_used()` in
`validate::tests::node_bytes_sum_to_mem_used` and again by the sync test on
these committed cells. "Cascaded" is `branch_depth_histogram[6]`, the number of
2-byte expanses whose slot holds a branch rather than a leaf or immediate.

| cell | flavor | B/key | immed | `Leaf` (linear) | `BranchB` | `BranchU` | bytes in leaves | bytes in `BranchB` (+ edge subarrays) | bytes in `BranchU` | cascaded expanses |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1,000,000 @64 (λ 15.26) | set | 7.92 | 226 | 65,526 | 7 | 257 | 6,845,840 | 5,456 | 1,069,120 | 7 of 65,536 |
| 1,000,000 @64 (λ 15.26) | map | 16.70 | 211 | 65,541 | 7 | 257 | 15,628,944 | 5,456 | 1,069,120 | 7 of 65,536 |
| 2,000,000 @64 (λ 30.52) | set | 13.60 | 780,905 | 42,639 | 22,970 | 257 | 7,465,376 | 18,669,616 | 1,069,120 | 22,970 of 65,536 |
| 2,000,000 @64 (λ 30.52) | map | 19.38 | 727,745 | 95,799 | 22,970 | 257 | 19,012,704 | 18,669,616 | 1,069,120 | 22,970 of 65,536 |
| 800,000 @62 (λ 48.83) | set | 21.02 | 724,745 | 315 | 16,269 | 64 | 28,560 | 16,518,640 | 266,240 | 16,268 of 16,384 |
| 800,000 @62 (λ 48.83) | map | 23.90 | 657,851 | 67,209 | 16,269 | 64 | 2,332,976 | 16,518,640 | 266,240 | 16,268 of 16,384 |

No `BranchL3`/`BranchL7`, bitmap leaves or map immediate value arrays occur at
these cells (all zero, omitted). Against the model, with the expanse count of
each cell:

| cell | λ | P(X > 32) | predicted cascaded | census cascaded | residual | key share in cascaded expanses (model) |
|---|---:|---:|---:|---:|---:|---:|
| 1,000,000 @64 | 15.26 | 0.0001 | 3.6 | 7 | +3.4 (+93.45%) | 0.0001 |
| 2,000,000 @64 | 30.52 | 0.3501 | 22,944.6 | 22,970 | +25.4 (+0.11%) | 0.4180 |
| 800,000 @62 | 48.83 | 0.9931 | 16,271.4 | 16,268 | -3.4 (-0.02%) | 0.9957 |

The pinned figure in the request, 0.3503 × 65,536 ≈ 22,955, uses the share at
λ = 30.52 rounded to four places; the model at the cell's exact λ = 30.5176 gives
22,944.7, and the census counts 22,970: a residual of +25 expanses,
+0.11%, within what one XorShift64 stream at a fixed seed deviates from
the Poisson mean (the seed cells below move the 2M figure by 0.03 B/key). At the
2M @64 cell the census also says where the 13.60 B/key comes from: 18.67 MB of
the 27.20 MB (68.6%) is the 22,970 cascaded bitmap branches with their
780,905 single-key immediate edges, holding the model's 41.8% of the keys at
about 22.3 B/key (model-derived split); 7.47 MB (27.4%) is the 42,639 packed
linear leaves holding the rest at about 6.4 B/key; the 257 uncompressed
branches of levels 8 and 7 are a fixed 1.07 MB. The map flavor pays the same
branch bytes and about twice the leaf bytes (8-byte values), which is why its
tooth is shallower in ratio and identical in position.

#### 9.10.4 Seed sensitivity (item 5)

The three census cells re-drawn with a second XorShift64 seed, same generator,
same masks:

| cell | λ | seed A set / map | seed B set / map | Δ set | Δ map |
|---|---:|---:|---:|---:|---:|
| 1,000,000 @64 | 15.26 | 7.92 / 16.70 | 7.92 / 16.70 | +0.00 | +0.00 |
| 2,000,000 @64 | 30.52 | 13.60 / 19.38 | 13.58 / 19.37 | -0.02 | -0.01 |
| 800,000 @62 | 48.83 | 21.02 / 23.90 | 21.00 / 23.89 | -0.02 | -0.01 |

Seed A is `0xddb1a5e5eed0001` (every committed random cell in the repository),
seed B `0x5eedb0b5c0ffee02`. The largest move is 0.03 B/key, on the cell in the
cascade's mixture regime; the two below and above it move by 0.02 or less.
Cells that differ by more than that differ in λ, not in the draw.

#### 9.10.5 The `LEAF_CAP = 48` control (items 6 and 10)

A build-time patch of `crates/expanse/src/types.rs` (`pub const LEAF_CAP: usize
= 48;`), nothing else changed, the same sweep re-run, and the patch reverted —
a control, not a shipped configuration. The existing cap-48 figures (7.92 at
λ = 15.26, 6.99–7.00 at λ = 30.52) reproduce; the rest is new *(measured:
deterministic byte accounting; `density_sweep.leaf_cap_48_control`)*:

| cell | λ | cap 32 set / map | cap 48 set / map |
|---|---:|---:|---:|
| 1,000,000 @64 | 15.26 | 7.92 / 16.70 | 7.92 / 16.70 |
| 884,736 @63 | 27.00 | 10.12 / 17.36 | 7.09 / 15.53 |
| 2,000,000 @64 | 30.52 | 13.60 / 19.38 | 7.00 / 15.38 |
| 2,200,000 @64 | 33.57 | 16.83 / 21.31 | 7.04 / 15.33 |
| 2,600,000 @64 | 39.67 | 20.67 / 23.64 | 8.27 / 16.00 |
| 1,310,720 @63 | 40.00 | 20.79 / 23.71 | 8.38 / 16.06 |
| 800,000 @62 | 48.83 | 21.02 / 23.90 | 14.45 / 19.81 |
| 950,272 @62 | 58.00 | 20.00 / 23.33 | 18.78 / 22.57 |
| 1,000,000 @62 | 61.04 | 19.66 / 23.16 | 19.12 / 22.82 |
| 1,200,000 @62 | 73.24 | 18.50 / 22.62 | 18.49 / 22.62 |

Under cap 48 the tooth moves and does not flatten. Its trough is 6.99 B/key at
λ = 30.52 (1,000,000 @63; 0.64 × 48, as the cap-32 trough sits at
0.62 × 32), against the item-10 prediction of "near λ ≈ 27": the λ = 27 cell
measures 7.09, 0.10 B/key above the trough and on its descending side, so
the prediction lands one cell early on a floor that is flat to 0.1 B/key
between λ = 27 and 34. The ramp is where predicted: the Poisson model puts the cap-48
cascade at 10% on at λ = 40.3 and 90% at 58.2, and the cells read
8.38 at λ = 40.00, 14.45 at 48.83, 18.78 at 58.00 and 19.12 at 61.04. The
second tooth moves with it: 5.99 at λ = 7,812 (12.98 under cap 32) and
19.06 at 15,625.

**The read path, Callgrind.** `crates/expanse/benches/instructions.rs`
(workload `core_instructions`, 50k keys) at both caps, deterministic exact
integers *(measured: x86_64 build host — Intel Xeon E5-2697 v4, `rust:1.98`
container with valgrind 3.24.0 and `iai-callgrind-runner` 0.16.1;
`results/leaf_cap48_callgrind.json`)*:

| arm | Ir, cap 32 | Ir, cap 48 | Δ |
|---|---:|---:|---:|
| `map_get/sequential` | 6,950,212 | 6,950,212 | +0.000% |
| `map_get/random` | 6,262,259 | 6,262,259 | +0.000% |
| `map_get/clustered` | 5,619,908 | 5,619,908 | +0.000% |
| `map_get/dense_leaf` | 10,811,452 | 10,811,452 | +0.000% |
| `map_get/linear_leaf` | 9,750,101 | 9,748,994 | -0.011% |
| `set_contains/random` | 6,139,088 | 6,139,088 | +0.000% |
| `map_get_batch/random` | 8,705,024 | 8,705,024 | +0.000% |
| `set_contains_batch/random` | 8,500,035 | 8,500,035 | +0.000% |
| `map_insert/sequential` | 10,096,741 | 10,098,044 | +0.013% |
| `map_insert/random` | 26,644,387 | 28,714,426 | +7.769% |
| `map_insert/clustered` | 18,993,138 | 19,322,024 | +1.732% |
| `map_insert/small` | 25,520,589 | 25,602,712 | +0.322% |
| `map_insert/dense_leaf` | 23,894,188 | 25,665,586 | +7.414% |
| `map_insert/linear_leaf` | 24,770,247 | 26,543,306 | +7.158% |
| `set_insert/sequential` | 3,509,806 | 3,512,436 | +0.075% |
| `set_insert/random` | 21,440,033 | 23,069,689 | +7.601% |
| `set_insert/clustered` | 11,519,588 | 11,957,789 | +3.804% |
| `set_insert/dense_leaf` | 18,858,587 | 20,225,344 | +7.247% |
| `set_insert/linear_leaf` | 19,289,260 | 20,691,362 | +7.269% |
| `map_ins_slot/random` | 25,716,424 | 27,808,124 | +8.134% |

Every lookup arm is identical to the instruction except `map_get/linear_leaf`,
which moves by 1,107 instructions in 9.75 million (−0.011%). That is the
expected result and a weak one: at 50k keys per distribution (λ = 0.76 on `random`) no linear
leaf reaches either cap, so the pair shows only that the constant alone does
not change the descent. The cascaded regime, where a 48-key linear scan
replaces a bitmap-branch descent, is not reached by this harness; it is
measured by the dedicated `leaf_cap_cascaded` arm below. The write path moved: 18 of 53 arms
changed, and the insert arms retire 7.2–8.1% more instructions under cap 48 at
a population where no leaf ever holds 33 keys — the constant sizes something on
the mutation path whose cost is paid below the cap. Cause unmeasured here; a
per-function ranking on the insert arm is the next instrument.

**The read path, wall clock.** `crates/expanse/benches/compare.rs`
`set_lookup/random/1000000/expanse` (workload `core_compare`: 1M random 64-bit
keys, 4,096 probes each of present keys and of rejection-sampled absent keys),
two builds interleaved A/B/A/B with a rebuild between arms, on the bare-metal
reference host *(measured: 12th Gen Intel Core i9-12900F, benchmark shell
pinned to the P-cores by `scripts/bench_pin.sh`, host-wide bench lock held;
criterion, n = 100 samples per arm, BCa 95% intervals from
`scripts/bench_baseline.py`; `results/leaf_cap_contains_cap{32,48}_{a,b}.json`;
load average sampled before every arm: 0.59, 1.26, 2.34 and 2.76 on 24
threads, the rebuild between arms being the load)*:

| round | arm | cap 32 ns/probe [BCa 95%] | cap 48 ns/probe [BCa 95%] | cap 48 / cap 32 speedup [BCa 95%] |
|---|---|---:|---:|---:|
| a | hit | 34.39 [34.10, 34.71] | 36.24 [36.21, 36.28] | 0.9490× [0.9406, 0.9572] |
| a | miss | 36.62 [36.58, 36.68] | 36.58 [36.56, 36.61] | 1.0010× [0.9997, 1.0027] |
| b | hit | 36.43 [36.41, 36.47] | 36.45 [36.42, 36.50] | 0.9994× [0.9981, 1.0006] |
| b | miss | 36.36 [36.33, 36.42] | 36.58 [36.45, 36.64] | 0.9941× [0.9922, 0.9976] |

Verdict: **no read-path cost of the constant is detectable at this cell.** In
round b the two builds agree on the hit arm within 0.1% (interval spanning
1.0) and differ by 0.6% on the miss arm; in round a the cap-32 hit arm is 5.4%
faster than everything else. That round-a figure is the same binary that
measured 36.43 ns in round b — cap 32 against itself across rounds is 0.944×
[0.936, 0.952] on the hit arm — so the between-round drift of one build is
larger than any difference between the two builds, and it is the ceiling on
this claim. Cause of the drift unknown; the arm ran first in the driver,
seconds after a full bench build. This is also structurally the weak cell for
the question: at λ = 15.26 the two builds hold the same nodes but for 7 of
65,536 expanses (the census above), so what the pair measures is code layout,
not a longer leaf scan. The cell where cap 48 changes the structure is λ ≥ 25,
and the committed `compare` harness has no such population; a 1.8M–2.6M-key
arm at 64 bits, or its 63-bit equivalent, is the measurement that would decide
it, and this control does not stand in for it. Both halves of that
measurement follow (#715).

**The read path in the cascaded regime, Callgrind (#715).**
`crates/expanse/benches/leaf_cap_cascaded.rs` (workload
`core_leaf_cap_cascaded_instructions`): 1,000,000 uniform keys masked to 63
bits — λ = N / 2^(63−48) = 30.52, the 2M @64 cell of the sweep at half the
build cost, same generator and seed as the density cells — probed once per
key by a shuffled hit vector and once by 1,000,000 absent keys
rejection-sampled from the same generator at the same width (§8.6). The
population was chosen so that the two builds hold *different* structures:
the Poisson model (`scripts/density_poisson.py`) puts 35.0% of the 32,768
expanses past a cap of 32 at this λ (the 2M @64 census counts 35.05%) and
0.1% past a cap of 48, so under the shipped constant a third of the
expanses are a bitmap branch of single-key immediates and under the patch
all of them are a linear leaf of up to 48 keys. Both builds of the same
commit, deterministic exact integers, no interval *(measured: x86_64 dev
host — Intel Xeon E5-2697 v4, `rust:1.98` container with valgrind 3.24.0 and
`iai-callgrind-runner` 0.16.1, `--cache-sim=yes`; commit c81eaf5d;
`results/leaf_cap_cascaded_callgrind.json`)*:

| arm | Ir, cap 32 | Ir / probe | Ir, cap 48 | Ir / probe | Δ Ir |
|---|---:|---:|---:|---:|---:|
| `cascaded_set_contains/hit` | 171,709,610 | 171.71 | 200,945,920 | 200.95 | +17.03% |
| `cascaded_set_contains/miss` | 168,692,815 | 168.69 | 200,086,695 | 200.09 | +18.61% |
| `cascaded_map_get/hit` | 176,844,750 | 176.84 | 207,936,634 | 207.94 | +17.58% |
| `cascaded_map_get/miss` | 172,016,225 | 172.02 | 205,080,859 | 205.08 | +19.22% |

The trade at this λ, both columns from the same N, width, generator and seed
(workloads differ: `example_keyspace_density` vs
`core_leaf_cap_cascaded_instructions` — the memory cells are the
`density_sweep` byte accounting, the cost cells are the Callgrind arm above):

| flavor | B/key, cap 32 | B/key, cap 48 | Δ memory | Ir / probe, cap 32 (hit / miss) | Ir / probe, cap 48 (hit / miss) | Δ Ir |
|---|---:|---:|---:|---:|---:|---:|
| `ExpanseSet` | 13.60 | 6.99 | −48.6% | 171.71 / 168.69 | 200.95 / 200.09 | +17.0% / +18.6% |
| `ExpanseMap<u64,u64>` | 19.38 | 15.37 | −20.7% | 176.84 / 172.02 | 207.94 / 205.08 | +17.6% / +19.2% |

Verdict: **the read path is not free of the constant where the constant
changes the structure.** Every one of the four arms retires 17.0–19.2% more
instructions under cap 48, misses more than hits, the map slightly more than
the set; the λ = 15.26 null above was a property of that cell (same nodes
under both caps), not of the read path. This is the losing cell the memory
win has to be read against: at λ = 30.52 cap 48 halves the set's bytes per
key and costs it a sixth more instructions per lookup. Ir is the cost column,
not the verdict — `docs/BENCHMARKING.md` rule 16 decides random point
lookups by wall clock and hardware counters with intervals — and the two
builds do not walk the same memory: Callgrind's simulated hierarchy reports
the set hit arm's `RAM Hits` at 701,697 under cap 32 and 274,055 under cap 48
(the miss arm 303,736 and 293,743; the map arms 1,426,810 → 1,480,965 and
353,949 → 398,323), so on the set's hit path the smaller structure trades
instructions for simulated memory traffic. Which of the two the reference
host pays for is unmeasured here; that is what the wall-clock pair decides.
**The read path in the cascaded regime, wall clock (#715, item 3).**
`crates/expanse/benches/leaf_cap_cascaded_wallclock.rs` (workload
`core_leaf_cap_cascaded_wallclock`; the same 1,000,000 keys at 63 bits, hit and miss
arms, criterion n = 100 per arm) driven by
`scripts/leaf_cap_cascaded_wallclock.sh`: both binaries built before any run,
then five runs under the bench lock and the P-core pin — cap 32, cap 32 again
back to back (the same-build A/A repeat), then cap 48 / cap 32 / cap 48 —
each harvested by `scripts/bench_baseline.py` into BCa 95% intervals, with
`/proc/loadavg` recorded before every run (1.4 → 1.1 → 1.1 → 1.1 → 2.0, one
minute) *(measured: hybrid desktop, Intel Core i9-12900F, 8 P + 8 E cores,
30 MiB L3, Linux 6.8, benchmark shell pinned to the P-cores; commit
fa1704d4; `results/leaf_cap_cascaded_contains_cap{32,48}_{a,repeat,b}.json`)*.

The drift floor first — the same cap-32 binary against itself, ratio of
run 2 over run 1 (higher is faster), then run 4 over run 1 across an
intervening cap-48 run:

| arm | run 1 mean (ns) | run 2 / run 1 | run 4 / run 1 |
|---|---:|---:|---:|
| `set_lookup_cascaded … hit` | 39.02 | 1.0050× [1.0007, 1.0099] | 1.0053× [1.0019, 1.0106] |
| `set_lookup_cascaded … miss` | 38.61 | 1.0034× [1.0017, 1.0054] | 0.9996× [0.9973, 1.0014] |
| `map_lookup_cascaded … hit` | 41.63 | 1.0030× [1.0013, 1.0045] | 0.9953× [0.9928, 0.9974] |
| `map_lookup_cascaded … miss` | 39.33 | 0.9992× [0.9951, 1.0017] | 0.9990× [0.9965, 1.0002] |

One build reproduces itself to within 0.5% here, against the 5.6% drift
#712 saw between rounds that had a rebuild between them. The between-build
rows, cap 48 over cap 32 within each round (workload: `core_leaf_cap_cascaded_wallclock`):

| arm | cap 32 (ns) | cap 48 (ns) | cap 48 / cap 32, round a | round b |
|---|---:|---:|---:|---:|
| `set_lookup_cascaded … hit` | 39.02 · 38.81 | 45.93 · 46.07 | 0.8496× [0.8467, 0.8538] | 0.8426× [0.8396, 0.8447] |
| `set_lookup_cascaded … miss` | 38.61 · 38.62 | 46.36 · 46.24 | 0.8328× [0.8311, 0.8347] | 0.8352× [0.8335, 0.8375] |
| `map_lookup_cascaded … hit` | 41.63 · 41.83 | 48.29 · 48.48 | 0.8621× [0.8607, 0.8635] | 0.8629× [0.8597, 0.8656] |
| `map_lookup_cascaded … miss` | 39.33 · 39.37 | 47.71 · 47.81 | 0.8243× [0.8232, 0.8253] | 0.8234× [0.8223, 0.8254] |

Verdict: **cap 48 is slower on the wall clock at this λ, by 14–18% on every
arm, in both rounds, with every interval clear of both the 1.0 floor and the
same-build drift band.** The wall clock follows the instruction count
(+17–19% Ir above): the simulated `RAM Hits` advantage Callgrind gave the
cap-48 set on its hit path did not appear as time on this host. The whole
structure fits its last-level cache at this population (13.60 B/key × 1M =
13.6 MB against 30 MiB of L3 under cap 32, 7.0 MB under cap 48), so the
memory-traffic mechanism had no room to pay here; whether it does at a
population that overflows the cache is unmeasured, and hardware counters
(`LLC-load-misses`, `branch-misses` per probe) were not collected in this
pair. The decision the constant sets, at λ = 30.52 on this host: cap 48 buys
48.6% of the set's bytes per key (20.7% of the map's) for 14–18% of lookup
time. `LEAF_CAP` stays 32 by this measurement; nothing here rules on other λ
or on the insert path.

#### 9.10.6 The two memory instruments at the same cells (item 7)

`crates/expanse-hot-bench/src/bin/instrument_bridge.rs`, both instruments on
the same build of the same key stream, both flavors, at the three census
cells — and under **two insertion orders** *(measured: x86_64 build host —
Intel Xeon E5-2697 v4; `-C target-cpu=haswell`; workload:
`hot_instrument_bridge`; `results/baseline_instrument_bridge.json`)*:

| cell | order | flavor | `mem_used()` B/key | allocator-held B/key | ratio | C allocs / frees |
|---|---|---|---:|---:|---:|---:|
| 1,000,000 @64 (λ 15.26) | random | set | 7.92 | 12.62 | 1.593× | 12,150 / 8,318 |
| 1,000,000 @64 (λ 15.26) | random | map | 16.70 | 23.66 | 1.416× | 105,972 / 64,705 |
| 1,000,000 @64 (λ 15.26) | sorted | set | 7.92 | 8.14 | 1.028× | 11,821 / 9,077 |
| 1,000,000 @64 (λ 15.26) | sorted | map | 16.70 | 16.67 | 0.998× | 104,934 / 65,464 |
| 2,000,000 @64 (λ 30.52) | random | set | 13.60 | 20.27 | 1.490× | 96,999 / 72,664 |
| 2,000,000 @64 (λ 30.52) | random | map | 19.38 | 26.09 | 1.347× | 332,298 / 281,347 |
| 2,000,000 @64 (λ 30.52) | sorted | set | 13.60 | 13.99 | 1.029× | 94,472 / 73,241 |
| 2,000,000 @64 (λ 30.52) | sorted | map | 19.38 | 19.76 | 1.020× | 329,355 / 281,560 |
| 800,000 @62 (λ 48.83) | random | set | 21.02 | 27.01 | 1.285× | 40,163 / 34,709 |
| 800,000 @62 (λ 48.83) | random | map | 23.90 | 29.38 | 1.229× | 106,196 / 100,131 |
| 800,000 @62 (λ 48.83) | sorted | set | 21.02 | 21.72 | 1.033× | 39,219 / 34,766 |
| 800,000 @62 (λ 48.83) | sorted | map | 23.90 | 24.63 | 1.031× | 105,052 / 99,932 |

**Reconciliation of §9.3 and README §1.** §9.3 gives `random` 1M allocator
bytes as 12.62 B/key; README §1 lists `ExpanseSet` at 8.27 B/key at λ = 15
(N = 491,520). Both are the census instrument — bytes held from the C
allocator through the same `--wrap` shim, HOT commit `96bf6fb`, same build
host — and both are the set flavor at essentially the same λ (15.26 and 15.00),
so neither the instrument nor the density explains a 1.5× gap. What does is
**insertion order** (workloads differ: `hot_instrument_bridge` vs
`hot_memory_curve`): the 12.62 B/key cell is `instrument_bridge` inserting in
XorShift64 generator order at 64 bits; the 8.27 B/key cell is
`hot_memory_curve` on the `hot_set_63bit` arm, which sorts and deduplicates its
key vector before inserting, at 63 bits. Measured on the same cell here, the
same 1M @64 keys give 12.62 B/key in generator order and 8.14 in sorted
order, at 7.92 `mem_used()` either way — the engine's accounting is
order-independent, the allocator-held figure is not, and the sorted-order ratio
of 1.028× is what `hot_memory_curve` reports at its λ = 15 cell
(8.27 / 7.95 = 1.040×). The engine's node allocator keeps freed class-sized
blocks on per-class free lists and carves small classes from 4 KiB slab pages,
so what it holds from the C allocator depends on how many leaves were
mid-growth at once, which is a property of the order keys arrive in, not of the
final structure; that is the mechanism the code makes visible, consistent
with the counts (fewer C-level frees per allocation in generator order), and
the free lists themselves were not instrumented. So: **the two figures are not
comparable as published**, and the discrepancy is not a measurement error on
either side. §9.3's "1.59×" ratio is a generator-order figure; every
allocator-instrument cell this suite publishes is a sorted-order figure; the
repo's `bytes/key` table is `mem_used()` and unaffected. The consequence for
the suite is that its Expanse memory cells carry the *smaller* of the two
allocator overheads, and a reader who rebuilds the same structure from an
unsorted stream will hold up to 1.6× more from the allocator at the same
`mem_used()`. HOT's cells were always sorted-order too (same key vector), so
the comparison inside the suite is symmetric; what was asymmetric was the
cross-document comparison, and §9.3 is amended below to say so.

#### 9.10.7 Map flavor (item 8)

Every table above carries `ExpanseMap<u64,u64>` beside `ExpanseSet`; the
committed `density_sweep.cells` block has `map_bpk` for all 45 cells and the
census has both flavors at every cell. The map's tooth is in the same place
and shallower in ratio: 16.15 at the trough, 23.90 at the peak, and
15.35 / 23.76 at the second trough and peak.



## 10. String-Key Arms (#693): Pre-Registration

**Status: locked before any string-arm harness code exists. No string-key
measurement has been taken.** This section is the commit-2 pre-registration
(§8.8) for the string half of the HOT comparison, carried from #660 to
[#693](https://github.com/orieg/expanse/issues/693). It is appended rather
than edited into §3–§8 so the integer arms' locked text stays readable as what
was locked (§8.7).

### 10.1 Scope, and what was read before lock

The repo has no string-key comparative suite: `ExpanseStrMap` and
`ExpanseBytesMap` have never been measured against a competitor. This is also
the regime HOT was designed for — long and skewed string keys are where varying
the discriminative bits per node should pay most, and where a fixed 8-byte chunk
per level descends deepest.

Nothing string-keyed has been *measured*. The following was *read* from the
sources before lock, and every design decision below traces to one of these
facts:

| Fact | Source |
|---|---|
| HOT's shipped C-string configuration is `HOTSingleThreaded<const char*, IdentityKeyExtractor>`: the stored value **is** the key pointer. | `apps/benchmarks/string/hot-single-threaded-string-benchmark/src/main.cpp` |
| HOT's own string benchmark allocates every key individually (`new char[line.length() + 2]`) and hands HOT the pointer. | `libs/idx/benchmark-helpers/.../StringBenchmarkConfiguration.hpp` |
| `MAX_STRING_KEY_LENGTH = 255`; `toFixSizedKey<char const*>` copies the key with `strncpy` into a **255-byte** `std::array` (the issue text says 256; the array is `getMaxKeyLength<char const*>()` = 255 bytes); `getKeyLength = min(strlen + 1, 255)`; the discriminative-bit search compares `getMaxKeyLength()` = 255 bytes. | `libs/idx/content-helpers/.../KeyUtilities.hpp` |
| Every HOT lookup, insert and `lower_bound` on a C-string key builds that 255-byte fixed key on the stack (`strncpy` zero-fills the remainder) and confirms the leaf with `strcmp` through the stored pointer. | `HOTSingleThreaded.hpp`, `ContentEquals.hpp` |
| `ExpanseStrMap` is an ordered map from NUL-free byte strings to `u64`, keyed one 8-byte big-endian chunk per level, with the unbranched remainder of a key held in a `StrSuffix` leaf (`Box<[u8]>` + value). | `crates/expanse/src/strmap.rs` |
| `ExpanseStrMap`'s ordered navigation is JudySL-shaped: `next_at_or_after` / `next_after` re-descend from the root on every step and return a fresh `Vec<u8>` key. There is no cursor iterator. | `crates/expanse/src/strmap.rs` |
| `ExpanseBytesMap` is **hash-indexed and unordered** (JudyHS shape): a 64-bit hash into an `ExpanseMap` whose value points at a collision bucket holding byte-exact key copies. Under `std` the hasher is `RandomState`. | `crates/expanse/src/bytesmap.rs` |

### 10.2 Pairings (issue constraint 3: what counts as a comparable payload)

The value model is decided by HOT's key extractor, as it was for the integer
arms. Both available models are measured, under their own workload IDs.

**Arm C — `hot_str_ptr`: HOT C-string, identity extractor, vs `ExpanseStrMap`
storing the key's pointer.** HOT's leaf value is the harness's `const char*`.
The Expanse side stores the *same pointer* as its `u64` value. Both structures
are then string → 8-byte-word maps, neither allocates a per-entry payload
object, and both lookups return the word, which both timed loops fold into a
sink. Because the two words are the same pointer, **the two sinks must be
equal at the end of every round**; the harness asserts it, and a cell where they
differ is void. This is HOT's shipped string configuration and the pairing that
can produce an Expanse loss.

**Arm D — `hot_str_map`: HOT C-string through `PairPointerKeyExtractor` over a
heap `std::pair<const char*, uint64_t>`, vs `ExpanseStrMap` string → `u64`.**
The only way HOT carries a value distinct from its key is a heap pair per entry.
This is Arm B's shape again; its memory cells are labelled
`PASS_categorical_by_design` by default (§6), for the same reason.

**Arm E — `hot_bytes_ptr`: HOT C-string, identity extractor, vs
`ExpanseBytesMap` storing the key's pointer.** The Expanse side here is a
hash-indexed, unordered structure. The arm is measured because
`ExpanseBytesMap` is what this engine offers for byte-string keys where order is
not needed, and it has never been measured against anything. **It is not a
trie-against-trie comparison and no cell from it is described as one.** It has
no ordered-scan pillar.

### 10.3 Key ownership and the census (issue constraint 2)

HOT stores a pointer; the bytes it points at belong to the harness. Expanse
copies key bytes into its own nodes. A census that ignores this measures the
harness's string table on one side and not the other.

**Locked:**

1. **The harness owns every string as its own NUL-terminated heap allocation**,
   one `malloc` per key, exactly as HOT's own string benchmark does. Not a
   contiguous arena: an arena would give HOT's per-probe `strcmp` a locality
   no real key store provides.
2. **The instrument counts the strings on neither side.** The census is armed
   only around the index build; population and probe strings are allocated
   before it is armed. The `--wrap` interposition sees both indexes under one
   definition (§9.1) and neither string table.
3. **Three columns are published per memory cell**, never one:
   - `index B/key` — bytes held from the allocator by the index alone, both
     arms, from the instrument.
   - `external key storage B/key` — the harness's string table, reported
     twice: exact `Σ(len_i + 1) / N`, and as the allocator holds it (a
     separately armed census over the table's allocation, since a 13-byte
     string costs a 24- or 32-byte allocator chunk). HOT's leaves point into
     this storage and HOT cannot answer a lookup without it (`strcmp` through
     the pointer), so it is part of HOT's cost of ownership. Expanse's index
     does not reference it.
   - `ownership B/key` — HOT: `index + external key storage (as allocated)`;
     Expanse: `index` alone.
4. **Expanse's independence from the string table is demonstrated, not
   asserted**: `hot_string_validate` builds an `ExpanseStrMap` from one copy of
   the strings, frees that copy, and probes with a second, byte-identical copy;
   every probe must hit. The same demonstration is impossible for HOT by
   construction — freeing the strings invalidates its leaves — which is the
   asymmetry the ownership column records.

**Registered consequence.** On long keys the `index` column favours HOT by
construction: HOT stores an 8-byte pointer plus node bits per key while Expanse
stores the key bytes. If that lands, the `index` column is labelled
`PASS_categorical_by_design` **in HOT's favour** — the mirror of Arm B's memory
label — and the contest is the `ownership` column.

### 10.4 The 255-byte capability predicate (issue constraint 1)

HOT discriminates C-string keys on their first 255 bytes only (§10.1). Two keys
that agree on those bytes are one key to HOT: the second `insert` reports a
duplicate, and a `lookup` of either returns the first's value.

**Locked:** `hot_can_key(len) := len ≤ 254` (a NUL-terminated string of at
most 254 bytes fits the 255-byte fixed key with its terminator). Following the
rule §9.4 earned — *a twin's limitation is a predicate on the twin, evaluated
against the workload; the workload is never edited to accommodate it* —

- the predicate is **evaluated per key, per cell**, and the cell records the
  fraction of its population HOT can represent;
- a cell with fraction `< 1.0` publishes the Expanse figure and, in the HOT
  column, the finding (`not representable: n keys > 254 B`) — never a HOT
  number over a silently smaller population (§8: population is asserted by
  walking, and a mismatch voids the arm);
- **the Expanse side is never restricted**, and no generator parameter is set
  for HOT's benefit;
- what HOT does with such keys is *measured* by `hot_string_validate` —
  population by walk, `insert` return values, and what `lookup` returns for a
  key that differs from a stored key only past byte 255 — and reported as a
  capability finding about HOT.

One workload shape below (`beyond`) is designed to fail the predicate for its
whole population, so the finding is exercised where it is the point rather than
discovered inside a latency cell.

### 10.5 Workload shapes

All strings are NUL-free. The random alphabet is the 62 ASCII alphanumerics
(5.95 bits per byte). Every draw comes from the suite's `XorShift64` at
`XorShift::SEED`; the generator is shared by every pillar and both arms.

| Shape | Generator | Length | HOT-representable |
|---|---|---|---|
| `short` | random alphanumeric bytes | uniform 8..=16 | yes |
| `counter` | `k` + 11-digit zero-padded decimal of `i` | 12 | yes |
| `prefixed` | one 96-byte alphanumeric prefix drawn once from the PRNG, then 24 random alphanumeric bytes | 120 | yes |
| `skewed` | random alphanumeric bytes; length Pareto(α = 1.2, x_min = 4), truncated at 192 | 4..=192, heavy right tail | yes |
| `beyond` | one 256-byte alphanumeric prefix drawn once, then 16 random alphanumeric bytes | 272 | **no — every key exceeds 254 B; all discriminating bytes lie past HOT's window** |

`skewed`'s truncation at 192 is a design choice made here, before measurement,
so that the skewed cell measures the two mechanisms rather than the predicate;
the predicate is exercised by `beyond`. The truncation is a generator parameter
(`max_len`), and a cell run with a longer tail must report its representable
fraction per §10.4 rather than trim the tail.

**Misses** (§8.6): rejection-sampled from the same generator as the population
and rejected on membership, never a transform of a present key. `counter`
misses draw their index from `[N, 4N)` (the §8.6 offset rule). **Probe order**:
Fisher–Yates from the same PRNG (§9.8). **Hit rates**: 100% for `lookup_hit`,
50% for `lookup_miss`.

**Populations.** Latency cells at N ∈ {10⁴, 10⁵, 10⁶}, as the integer arms.
The memory pillar is a **population sweep**, N ∈ {1k, 2k, 5k, 10k, 20k, 50k,
100k, 125k, 150k, 200k, 500k, 1M}, with the reason stated as §9.6 requires:
string keys have no single density axis equivalent to λ. The candidate is the
occupancy of the *discriminating chunk map* — for the random-alphanumeric
shapes the first two bytes of the first random chunk take 62² = 3,844 values,
so `λ_chunk = N / 3,844` and the `LEAF_CAP = 32` cascade of §9.4 would sit near
N ≈ 1.23 × 10⁵. **That is a hypothesis, unmeasured**; the sweep is dense around
it (100k, 125k, 150k) so it can be confirmed or refuted, and if it holds the
memory rows are re-expressed against `λ_chunk` in an amendment rather than
edited here. `counter` has construction-fixed occupancy (dense in the digit
expanse) and is not expected to move.

### 10.6 Pillars

1. Point lookup, 100% hit — both arms return the stored word and fold it.
2. Point lookup, 50% hit / 50% same-generator miss.
3. Insertion into a cold structure.
4. Ordered range scan, k ∈ {10, 100, 1000}, starts drawn from the probe
   stream — **Arms C and D only**; `ExpanseBytesMap` is unordered.
5. Memory census per §10.3, one process per cell (§9.2).

**Scan surface disclosure, stated before the number exists.** The Expanse scan
pillar drives the shipped `ExpanseStrMap` navigation surface:
`next_at_or_after(start)` then `next_after(key)` per element, each a fresh root
descent returning a heap-allocated key. HOT's side is `lower_bound` plus an
incremental iterator. The pillar therefore measures the API surface each system
ships, and the Expanse cost includes one allocation per visited element. That
is what a JudySL caller pays today and it is published as such; a cursor
iterator for `ExpanseStrMap` would be an engine change outside this suite.

### 10.7 Expected-losses matrix

Registered before measurement; confidence is the pre-registration's own.
Losses first, because a suite that predicts only wins is not pre-registered.

#### Where Expanse is expected to LOSE

| Arm | Pillar / shape | Prediction | Confidence | Reasoning (hypothesis, unmeasured) |
|---|---|---|---|---|
| C, D | Ordered scan, every k, every shape | **HOT wins** | High | API-surface shape (§10.6): one root re-descent and one allocation per visited element against an incremental iterator. Not a statement about the trie's descent. |
| C, D | Point lookup (hit and miss), `prefixed` | **HOT wins** | Medium-high | This is HOT's design regime. HOT selects discriminative bits and skips the 96 shared bytes; `ExpanseStrMap` descends twelve single-entry chunk levels through the same prefix before reaching the discriminating chunk. |
| C, D | Insert, `prefixed` | **HOT wins** | Medium | The same twelve-level descent, followed by a `StrSuffix` allocation per key on the Expanse side. |
| C, D, E | Memory, `index` column, `prefixed`, `skewed`, `beyond` | **HOT wins** | High — but `PASS_categorical_by_design` | HOT holds 8 bytes plus node bits per key; Expanse holds the key bytes. Decided by what each side stores, not by how well (§10.3). |
| C, D | Point lookup, `skewed` | **HOT wins** | Low | Registered because skewed lengths are the regime HOT is built for and the issue expects the loss. The mechanism reading in §10.1 does not strongly support it: a random-content key of any length is one chunk-map descent plus one suffix compare for Expanse, while HOT pays a 255-byte fixed-key fill per probe. Recorded as genuinely uncertain, with the direction the issue predicts. |
| C, D | Point lookup, `beyond` | **no HOT cell** | — | Fails the predicate for its whole population (§10.4). Expanse figures published alone. |

#### Where Expanse is expected to WIN

| Arm | Pillar / shape | Prediction | Confidence | Reasoning (hypothesis, unmeasured) |
|---|---|---|---|---|
| C, D | Point lookup and insert, `counter` | **Expanse wins** | High | Twelve-byte keys sharing `k000…`: the first chunk is one path and the terminal chunk lands in a dense digit expanse that packs into bitmap leaves — the string analogue of `sequential`, where the integer arms measured their widest margins. HOT allocates a leaf per key regardless. Low-information as a contest; registered so a win is a confirmation and not a headline. |
| C | Point lookup, `short` | **Expanse wins** | Medium | One chunk-map descent over a 48-bit-entropy chunk plus one suffix compare, against a descent plus a 255-byte fixed-key fill plus `strcmp`. |
| C | Memory, `ownership` column, `counter` and `short` | **Expanse wins** | Low-medium | Expanse pays a chunk-map entry plus a `StrSuffix` (two allocations) per key not resolved in a terminal chunk; HOT pays node bits plus the externally allocated string (allocator chunk ≥ 24 B for a 13-byte key). Close enough that `BOUNDARY_RESULT` is a live outcome. |
| D | Memory, all columns, all representable shapes | **Expanse wins** | High — `PASS_categorical_by_design` | Arm D hands HOT a heap `std::pair` per entry on top of its index and the external string. A consequence of the value model, as for Arm B. |

#### Explicitly not predicted

`lookup_miss` on `short` and `skewed`; every latency cell of Arm E
(`ExpanseBytesMap` hashes the whole key, so its cost is a function of key
length rather than of prefix structure, and no mechanism argument places it
against HOT in advance); insert on `short` and `skewed`. Reported with their
numbers as `not pre-registered`.

### 10.8 Falsifiers and the silent-failure re-check

Every class §9.1–§9.9 found on the integer arms is re-checked on the string
path by `hot_string_validate`, which must pass before any cell is recorded:

| Class | Integer finding | String-path check |
|---|---|---|
| Payload width (§9.4) | 63-bit inline value | Arm C stores a pointer inline; every population pointer is asserted to have bit 63 clear, and every HOT lookup must return the pointer that was inserted (sink equality with Expanse, §10.2). |
| Process-global pool (§9.2) | 3.3× undercount on a warm pool | `require_cold_pool` before every census; one cell per process. |
| `operator new` (§9.7) | Arm B's pairs invisible | Arm D's build must count at least N allocations; Arm C's must count **fewer than N** — a per-entry heap object appearing on the identity arm means the shim stopped storing the pointer inline. |
| Counter cached across the call (§9.1) | `free` ran, total did not move | The `1 MiB` control, plus a **string-table control**: allocating N strings must count exactly N allocations and freeing them must return the live total to its prior value. |
| Key truncation (new, §10.4) | — | `beyond` is built into HOT deliberately: population by walk, `insert` return values, and the pointer `lookup` returns for a key differing only past byte 255 are recorded. |
| Probe symmetry (§9.8) | ordered probe stream | Answer agreement on every probe of every arm; identical shuffled stream; misses by rejection. |

Cells are void when: populations by walk differ from the intended population on
either side; the two arms disagree on any probe; Arm C's sinks differ; the
census control fails; the pool is warm; the ISA targets differ (§3.3).

### 10.9 Claims-ceiling amendments (§7)

§7 item 2 said *"No string-key claim — the string arms are separate scope."*
Amended: string-key claims are in scope for the arms above, under these limits:

1. Claims attach to HOT's **C-string configuration at `96bf6fb`** — `const
   char*` keys through `IdentityKeyExtractor` or `PairPointerKeyExtractor` —
   not to HOT with a length-prefixed or fixed-width key type it does not ship.
2. **No HOT claim on keys longer than 254 bytes.** Cells there publish Expanse
   alone and the finding.
3. **Arm E is not a trie comparison.** An `ExpanseBytesMap` cell is a
   hash-indexed structure against a trie and is described as such.
4. The scan pillar is a claim about the **shipped navigation surfaces**, not
   about descent cost (§10.6).
5. Memory claims name their column (`index`, `external`, `ownership`) and their
   population; the `index` column carries its categorical label where §10.3
   applies.
6. Single-threaded, x86-64 with AVX2 and BMI2, one HOT commit, no cross-suite
   ratio, no peer review — as §7.

### 10.10 Amendments found while building the string harness

Recorded as amendments rather than edited into §10.4 (§8.7). Both are
measurement constraints found by running `hot_string_validate` on the build
host; neither is a result.

**The window is 255 bytes, not 254.** §10.4 locked `hot_can_key(len) := len ≤
254`, reasoning that the terminator had to fit inside the 255-byte fixed key.
It does not: `strncpy` copies up to 255 content bytes and appends no terminator
when the key is that long, so a 255-byte key is fully visible. Measured with
pairs of keys differing only in their last byte *(measured: x86_64 build host —
Intel Xeon E5-2697 v4; HOT `96bf6fb`; `-march=haswell`; workload:
`hot_string_validate`)*:

| Key length | HOT entries after inserting both |
|---:|---|
| 254 | 2 — discriminated |
| 255 | 2 — discriminated |
| 256 | **1** |
| 300 | **1** |

The predicate is corrected to `hot_can_key(len) := len ≤ 255`
(`HOT_STRING_KEY_WINDOW`, asserted equal to HOT's `MAX_STRING_KEY_LENGTH` at
runtime). No workload shape changes: `beyond` is 272 bytes and still fails it
for every key; the other four shapes were inside both bounds. The issue text's
"256-byte fixed representation" is also off by one in the other direction — the
array is 255 bytes.

**HOT drops an over-window key; it does not return a false positive.** §10.4
predicted that a lookup of either colliding key would return the first's value.
Measured, it does not: `insert` of the second key returns `false` (reported as
a duplicate) and stores nothing, and `lookup` of that key returns *not found*,
because `contentEquals` confirms the leaf with a full `strcmp` against the
stored string. On `beyond` at N = 1,000: `insert` reported 1 key new, the trie
walked 1, `lookup` found 1 of 1,000 and returned a wrong pointer for 0. The
failure mode is therefore **silent population loss under a successful-looking
build** — the same class as the integer arms' `insert() == true` on
unretrievable keys (§3.1) — not wrong answers. It is the reason population is
asserted by walking on every cell.

**Populations are reported, not assumed.** `skewed` at N = 100,000 holds
99,976 distinct keys after deduplication (4-byte keys collide at that
population); every cell carries its actual population, and both sides are
asserted against it.

### 10.11 Outcome pointers (recorded after measurement, not reconciled above)

The string arms were measured on the reference host at harness commit
`d0149469`; the verdicts against §10.7 are in the README's §6 scorecard and are
not copied here, so this file stays readable as what was locked. Two amendments
follow from the outcome:

- **§10.5's conditional re-expression is not made.** The `λ_chunk` cascade
  hypothesis is *consistent with* the sweep — the Expanse index rises between
  N = 100k and 150k on the random-alphanumeric shapes and not on `counter` —
  but the single-variable test (changing the alphabet width and watching the
  step move) was not run, so the memory rows stay on a population axis.
  Hypothesis, partially supported; the falsifier is open.
- **§10.7's `ownership` prediction for `short` is refuted by a leaf-representation
  cost, not by HOT.** The gate's allocation counts show two allocations per
  `ExpanseStrMap` key not resolved in a terminal chunk (a `StrSuffix` shell and
  its byte buffer). That is a property of the engine's string leaf and is logged
  for the engine, as §9.4 logged the `LEAF_CAP` density behaviour, rather than
  amended into this suite.
## 11. HOT-ROWEX Concurrent Arm — Pre-Registration (#692)

**Status: locked before any harness code. No §11 measurement has been taken.**

This section is commit 1 of the three-commit cadence (AGENTS.md §8.8) for
[#692](https://github.com/orieg/expanse/issues/692): the concurrent arm that
#660 scoped and did not start. It extends this suite rather than opening a new
one — same FFI foundation, same workload generator, same census, same runner —
and it is written the way §2 was: everything already observed is disclosed,
and every prediction informed by an observation is marked **(informed)**.

### 11.1 The question, and the answer this arm is expected to give

HOT ships a **ROWEX** variant (`hot/rowex/HOTRowex.hpp`, Read-Optimized Write
EXclusion) that admits concurrent inserts, lookups and scans. `SyncExpanseMap`
and `SyncExpanseSet` serialize every writer on one mutex and validate readers
optimistically; the protocol is optimistic lock coupling and is **blocking by
design** (AGENTS.md §2.2). It is never described here as lock-free.

**Hypothesis, stated as the loss it is: Expanse LOSES on write concurrency.**
Aggregate writer throughput of the Expanse arm cannot exceed its single-writer
rate — every insert takes the same mutex — and is expected to fall below it as
writers are added, while ROWEX's per-node write exclusion is expected to scale
writer throughput with writer count. The arm exists to measure that loss and
to find the writer count at which it begins.

Two other quantities are measured alongside, because a write-path loss has a
read-path shadow: reader throughput while writers are active, and the
Expanse protocol's health under that write load (§11.3, decision 5).

### 11.2 Prior observations at lock time (mandatory disclosure)

A Step 0 feasibility gate ran before this section was written. It is a
standalone C++ program, not a registered harness, and nothing in it is a
published figure. Observed *(measured: x86_64 build host — Intel Xeon
E5-2697 v4, 72 threads, Linux; HOT `96bf6fb`; TBB `4c73c3b` (the `tbb_2018`
branch HOT pins as `third-party/tbb`); `g++ 11.4 -O3 -std=c++17
-march=haswell -DNDEBUG`; workload: `step0_rowex_gate`)*:

| Check | Result |
|---|---|
| ROWEX compiles against HOT's pinned TBB 2018 | yes — `libtbb.so.2` only; `tbbmalloc` fails to build under C++17 (dynamic exception specifications in `proxy.cpp`) and is not needed |
| Correctness, map arm, 1 / 8 / 16 concurrent writers, N = 200,000 | 200,000/200,000 found with correct values and 200,000 walked, verified from a thread that never inserted |
| Correctness, set arm (63-bit), 1 / 8 writers | 200,000/200,000 found and walked |
| Inline payload width, identity extractor | key `2^63` inserted and not found — the same 63-bit predicate as Arm A (§9.4) |
| Census control with TBB linked, before any ROWEX use | `+1,052,656 B` on a `1,048,576 B` request, residual `0 B` |
| Census control after threaded ROWEX use | `+1,048,584 B`, residual `0 B` |
| Counted allocations, single-writer map build of N = 200,000 | 418,289 ≥ 2N (one heap pair plus at least one node per insert); 208,967 frees observed during the build |
| Bytes held after build-only census, map arm | 37.24 B/key single writer; 36.74 B/key with 8 writers |
| Census counters **armed** during a 16-writer build | 4.61× the disarmed build time — diagnostic, local box, not a figure |
| Build time 1 / 8 / 16 writers, map arm, N = 200,000 | 117.5 / 42.0 / 30.5 ms — diagnostic on a shared 72-thread host, **not a figure**, disclosed because it informs §11.5 |

No reader-throughput, no Expanse-side and no reference-host observation of any
kind was made.

### 11.3 Locked constraint decisions

The four constraints #692 lists as blocking are settled here, with the
evidence each decision rests on. None of them is revisited by the harness.

**Decision 1 — TBB, the allocator, and what the census may publish.**

- *TBB.* ROWEX uses TBB for exactly one thing: `tbb::enumerable_thread_specific`
  holding each thread's epoch-based-reclamation state. No system package is
  installed for it on any host. The harness builds `libtbb` from HOT's own
  pinned nested submodule (`third_party/hot/third-party/tbb`, TBB 2018,
  `4c73c3b`) into the cargo build directory and links it with an rpath, so the
  competitor runs the TBB its authors built against and the reference host is
  not modified. `tbbmalloc` is not built: it does not compile under C++17 and
  ROWEX does not reference it.
- *Allocator.* HOT's CMake links tcmalloc **if found**. Neither the build host
  nor the reference host has a linkable tcmalloc and no package is installed for
  a benchmark, so **both arms allocate from glibc 2.35 `malloc`**, exactly as
  the single-threaded suite does. This is disclosed as a risk against ROWEX,
  not against Expanse: ROWEX copies a node per insert and frees it through
  EBR, with no pool, so it is more allocator-sensitive than Expanse's
  `NodeAlloc`, which recycles through size-class free lists before touching
  `malloc`. A tcmalloc sensitivity cell is **not run** and no claim about
  ROWEX under tcmalloc is made (§11.6).
- *Census validity under TBB.* Re-validated as #692 requires: the control
  allocation moves the counter by its size and returns to zero after free,
  with TBB linked, before and after threaded ROWEX use (§11.2); and the
  per-arm allocation-count assertion holds — a map build of N keys must count
  at least 2N allocations, a set build at least N. ROWEX nodes come from
  `posix_memalign` in header code instantiated in the shim's own translation
  unit, so the `--wrap` reaches them; its free lists are `std::vector`s
  through the replaced `operator new`. **Residual blind spot, disclosed:**
  `libtbb.so` allocates its own per-thread state through the dynamic linker,
  which `--wrap` cannot see (§9.7's mechanism). That cost is paid once per
  registering thread, is independent of N, and is therefore bounded to O(threads)
  bytes that a build-only single-writer census omits.
- *The census never runs inside a throughput cell.* The counters are
  process-global atomics; armed under 16 writers they slowed the build 4.61×
  (§11.2). A throughput cell runs with the census disarmed; a census cell runs
  its own process with no timing claim.
- *What memory is published.* A **secondary, deterministic** pillar only:
  bytes held from the allocator after a **build-only, single-writer**
  population, ROWEX against the `Sync*` wrapper, on the §9.6 λ targets — a
  curve across occupancy, never a single cell (§9.6). The concurrent cells
  carry no memory figure. A ROWEX memory cell whose census control fails, or
  whose allocation count falls below the floor above, is void.

**Decision 2 — No deletion.** ROWEX supports insert, lookup and scan; it has
no remove. Per the standing rule of §9.4, that is a predicate on the twin,
evaluated against the workload, never an edit to the workload. **The arm's
workload is therefore defined as insert and point lookup only.** The
`benches/concurrency.rs` 50/50 insert/remove churn mix that produced the
published `0.19×` read-scaling figure is a *different workload* and is
declared **out of scope for this arm**, not quietly dropped — a ROWEX cell
on it cannot exist. Ordered scan under concurrency is also not measured here:
single-threaded scan is published in README §3, and the concurrent question
this arm is filed for is the write path. Nothing is removed from the Expanse
arm for symmetry; the workload simply contains no removals.

**Decision 3 — Thread placement.** The runner sources `scripts/bench_pin.sh`,
which confines the benchmark shell and everything it spawns to the reference
host's performance cores (`cpu_core`, CPUs 0–15: 8 physical cores, 2 SMT
siblings each; `docs/BENCHMARKING.md` rule 2). Every cell keeps
**writers + readers ≤ 16** so no thread can be scheduled off the pin, and the
harness asserts it. Each cell records the process's `Cpus_allowed_list` and
`EXPANSE_BENCH_PIN_APPLIED` in every emitted row, so placement is part of the
artifact rather than of the run log. Two disclosures follow from the topology:
16 threads occupy both SMT siblings of every P-core, so the W = 16 cell
measures SMT sharing as well as the protocol (the sibling exposure was
measured under #680); and no cell can say anything about scaling past 8
physical cores.

**Decision 4 — Lock symmetry (AGENTS.md §8.16).** Both arms are measured
**below any external lock layer**, each through its native concurrent API:
ROWEX through `insert`/`lookup` with its internal per-node spin locks, CAS
root replacement and epoch-based reclamation; Expanse through
`SyncExpanseMap::insert` / `SyncExpanseSet::insert` and per-thread
`owned_reader()` handles, with its internal writer mutex, version brackets
and epoch collector. Neither is wrapped in a `Mutex` or `RwLock`. This is the
only symmetric choice that answers the question: each system's own
concurrency protocol *is* the object under measurement, and wrapping both in
an external lock would measure the wrapper.

**Decision 5 — The protocol-health number.** `benches/concurrency.rs` reports
a **Busy rate** for the 32-bit protocol, where `try_get` returns `Busy`
instead of waiting. The 64-bit protocol has no `Busy`: a reader that observes
a moved version **restarts**, up to `MAX_RETRIES = 64`, then **falls back to
the writer mutex**. The honest counterpart is therefore two event ratios from
the engine's `occ_stats` counters — the **restart share**
`(read_attempts − read_ops) / read_attempts` and the **fallback share**
`read_fallbacks / read_ops` — plus `sample_spins`, the iterations burnt
waiting for a tree-level bracket to close. These counters exist only under the
`occ-stats` cargo feature, which the engine documents as diagnostic-only and
never enabled for a published benchmark. So: the health cells are a
**separate build** of the same harness with `occ-stats` on; they publish
event ratios and nothing measured in time; and every throughput figure comes
from the default build with the counters compiled out.

**Decision 6 — One cell, one process.** ROWEX's reclamation strategy is a
function-local `static` singleton and its free lists live in thread-local
storage, both process-global and both outliving every trie instance — the
same class of hazard as §9.2. Every cell, throughput or census, is its own
process invocation, driven by the runner.

### 11.4 Workload

Shared with the rest of the suite through `workload.rs`, extended rather than
duplicated (§8.3 symmetry by construction):

- **Pairings.** Set arm: `HOTRowex<uint64_t, IdentityKeyExtractor>` against
  `SyncExpanseSet`, 63-bit domain on both sides, named for the reason (§9.6:
  `hot_rowex_set_63bit`). Map arm: `HOTRowex<std::pair*, PairPointerKeyExtractor>`
  against `SyncExpanseMap`, full 64-bit domain (`hot_rowex_map_64bit`). The map
  arm is the headline #692 names; the set arm is the harder test for Expanse,
  because it removes ROWEX's per-entry heap pair from the write path.
- **Distribution.** Uniform random only. Random keys spread concurrent writers
  across the trie, which is ROWEX's *favourable* case (few conflicting node
  locks) and Expanse's indifferent one (one mutex regardless). A contended
  distribution is not measured and no claim is made about one (§11.6).
- **Population.** Prefill N₀ = 2²⁰ keys (λ = 16 at 64 bits), inserted
  single-threaded outside the timed window, identical on both arms. Then
  M = 2²⁰ **fresh** keys, rejection-sampled from the same generator against
  the prefill (§8.6), split into W contiguous slices, one per writer.
- **Fixed work, not a fixed window.** Each writer inserts its whole slice;
  the timed region runs from a barrier release to the last writer's
  completion. Both arms therefore do *identical* work per round and the
  structure grows by exactly M on both, which a fixed-duration window cannot
  guarantee — the faster arm would grow more and face a larger trie.
  Writer throughput = M / elapsed.
- **Readers.** R readers probe a 50% hit / 50% same-generator miss stream
  drawn against the prefill, shuffled (§9.8), from the same barrier until the
  writers finish; reader throughput = reads completed / elapsed. Both sides
  fetch the stored value on the map arm and sink it; a hit's value is checked
  against its key-derived expectation on both arms, and a wrong value voids
  the cell. Some misses become hits as writers land — identically on both
  arms, since the streams are identical.
- **Cells.**
  - **C1 — write scaling:** W ∈ {1, 2, 4, 8, 16}, R = 0.
  - **C2 — readers alongside writers:** R = 8, W ∈ {0, 1, 2, 4, 8}. W = 0 is
    the reader-only reference; at W = 0 each reader makes one fixed pass over
    the stream.
  - **H — protocol health:** the C2 cells re-run on the `occ-stats` build,
    Expanse side only (ROWEX has no counterpart counter), 5 rounds, median
    with range. Event ratios, no time.
  - **M — memory:** build-only single-writer census at the §9.6 λ targets,
    both arms, one process per cell.
- **Rounds and statistics.** 15 rounds per throughput cell, arms interleaved
  per round (ROWEX, Expanse, ROWEX, …; `docs/BENCHMARKING.md` rule 1), fresh
  structures per round built and dropped outside the timed window. Verdicts
  are gated on the BCa 95% bootstrap interval of the throughput ratio over
  ≥ 2,000 resamples (§8.4). **The ratio is Expanse ÷ ROWEX throughput, so —
  as everywhere in this suite — above 1.000 means Expanse is faster.**
- **Build flags.** As §3.3: `-C target-cpu=haswell` and `-march=haswell` for
  every published cell.

### 11.5 Expected-losses matrix

Registered before measurement; confidence is the pre-registration's own.
Losses first, because the loss is the reason the arm exists.

#### 11.5.1 Where Expanse is expected to LOSE

| Cell | Prediction | Confidence | Reasoning |
|---|---|---|---|
| C1, W ≥ 4, both arms | **ROWEX wins** | High | One writer mutex bounds Expanse's aggregate insert rate at its single-writer rate and lock hand-off is expected to pull it below that; ROWEX admits concurrent writers. **(informed)** — the Step 0 build ran 2.8× faster at 8 writers and 3.9× at 16 than at 1 on the shared build host (§11.2), which says ROWEX scales, not by how much on the reference host. |
| C1, W = 2, both arms | **ROWEX wins or `BOUNDARY_RESULT`** | Medium | Expanse's single-writer insert is 2.520× (set) and 3.553× (map) faster than HOT's on random 1M *(measured: reference host, README §2, `hot_latency`)*, so ROWEX must roughly triple its single-writer rate to overtake. The crossover writer count W\* is registered as **W\* ∈ [2, 4]**; a crossover at W = 1 or none by W = 16 both count as refutations of *this row*. |
| C2, W ≥ 1, R = 8, both arms — reader throughput | **ROWEX wins** | Medium-high | Expanse readers restart whenever a bracket moves the version under them and fall back to the writer mutex after 64 restarts; ROWEX readers never wait on writers. The published `0.19×` read scaling under churn *(measured: reference host, `docs/BENCHMARKING.md`, `core_concurrency`; different workload — 50/50 insert/remove, not comparable to C2)* is the same mechanism observed on another workload. |
| M, set arm, λ outside [8, 23] | **ROWEX wins** | Medium | README §1 measured HOT's single-threaded set arm winning below λ = 8 and above λ = 23; ROWEX's node layout is the same family. No ROWEX set-arm memory was observed at Step 0, so the confidence stays medium. |

#### 11.5.2 Where Expanse is expected to WIN

| Cell | Prediction | Confidence | Reasoning |
|---|---|---|---|
| C1, W = 1, both arms | **Expanse wins** | Medium-high | The single-threaded insert win (2.520× / 3.553×, README §2) carried into the concurrent wrappers: Expanse adds a mutex acquire, a version bracket and one epoch advance per 32 writes; ROWEX adds a `MemoryGuard` and CAS/spin-lock traffic on every insert. Medium-high rather than high because both wrappers are unmeasured. |
| C2, W = 0, R = 8, map arm | **Expanse wins** | Medium | Single-threaded map lookup on random 1M: 1.399× hit, 1.393× 50/50 *(measured: README §2)*. Reader handles add a pin per lookup on the Expanse side, a `MemoryGuard` on ROWEX's. |
| C2, W = 0, R = 8, set arm | **`BOUNDARY_RESULT`** | Low-medium | The single-threaded set lookup on random 1M was itself `BOUNDARY_RESULT` at 0.998 hit and a HOT win at 0.940 on 50/50 (README §2). Registered as no-winner rather than assigned one. |
| M, map arm, all λ | **Expanse wins, `PASS_categorical_by_design`** | High | ROWEX carries the same heap `std::pair` per entry as Arm B; Step 0 held 37.24 B/key **(informed)** against `ExpanseMap`'s 16.26–24.71 B/key across λ (README §1). A value-model consequence, not an architectural claim — labelled as §6 requires. |
| M, set arm, λ ∈ [8, 23] | **Expanse wins** | Medium | The README §1 band, carried forward for the reason given in the loss row above. |

#### 11.5.3 Protocol health (H) — hypothesis with a falsifier

The **restart share** is expected to rise monotonically with W and the
**fallback share** to stay **below 1%** at every W ≤ 8. This is a hypothesis
about the protocol, not a comparison, and it carries its own falsifier: a
fallback share of 1% or more at any W is reader starvation and is reported
as a protocol-health finding in the README scorecard, whatever the throughput
cells say. `sample_spins` is reported without a prediction.

#### 11.5.4 Explicitly not predicted

The W = 16 cells (SMT sharing confounds the protocol effect; reported with
their numbers and `not pre-registered`), and every quantity of ROWEX at
writer counts beyond the pin.

### 11.6 Gate taxonomy and claims ceiling

The §6 labels apply unchanged, including `UNPREDICTED LOSS` for any §11.5.2
row that lands the other way. In addition, this arm may claim at most:

1. **x86-64 with AVX2 and BMI2**, at `speedskater/hot` `96bf6fb` with TBB
   `4c73c3b`, built as documented, under glibc `malloc`. **No claim about
   ROWEX under tcmalloc or any other allocator.**
2. **Insert and point-lookup concurrency on uniform random 64-bit / 63-bit
   integer keys only.** No deletion claim of any kind, no contended-key
   claim, no scan-under-concurrency claim, no string-key claim.
3. **Up to 16 threads on 8 physical performance cores with SMT.** Nothing
   about larger machines.
4. **The health ratios describe Expanse's protocol under this workload.**
   They are not a comparison; ROWEX has no counterpart counter.
5. **No cross-suite ratio** (§7 item 5) and **no peer review** (§7 item 6).

### 11.7 What would void a cell

- Either arm's population after the concurrent build differs from N₀ + M
  (ROWEX counted by walking, never from `insert()` return values — §8).
- A reader on either arm observed a hit whose value did not match its
  key-derived expectation.
- W + R > 16, or a recorded `Cpus_allowed_list` that is not the pin.
- A memory cell whose census control does not return to zero, or whose
  counted allocations fall below 2N (map) / N (set), or that ran in a process
  where any earlier ROWEX or HOT trie existed.
- Any throughput figure from a binary built with `occ-stats`, or any health
  ratio quoted as a timing.
- Any cell whose two arms were built for different ISA targets (§3.3).

### 11.8 Amendment: a health falsifier that can fire (#734)

Recorded after measurement. §11.5.3 is not edited — the hypothesis it locked is
the one that was measured, and reconciling a pre-registration in place is what
§8.7 forbids. This says what should be registered next and why the one above
could not decide anything.

**The fallback half of §11.5.3 cannot fire at these writer counts.** A reader
falls back to the writer mutex only after **64 consecutive failed optimistic
walks**. At the bracket lengths a single writer holds, the probability of 64 in
a row is negligible by construction, so the measured zero is a property of the
construction and not of the protocol: the falsifier would have read zero
whatever the engine did. Its honest label is `PASS_categorical_by_design`
(AGENTS.md §8, C-b), and README §7.3 and the §7.5 scorecard carry it. The
Masstree arm's §6.3 row was relabelled for the same reason.

**What the next concurrent arm should register instead.** A falsifier that can
fire has to be a quantity the protocol could plausibly move past a threshold
under this workload. The candidate is the one the reader collapse actually
concerns:

> **Reader nanoseconds per probe under one writer, against the same readers
> alone.** Register a ceiling before the run — the reader-only figure times a
> stated factor — and report the cell `REFUTED` when the measured ratio exceeds
> it. On this arm the fall was eight-fold on the set arm and five-fold on the
> map arm (README §7.2); on the Masstree arm it was seven-fold. A ceiling
> anywhere below those is a falsifier the measurement can and does cross, which
> is the property §11.5.3's fallback row lacks.

Two constraints on that registration, both learned here:

1. **The reader window is the writers' fixed work**, so the two arms' windows
   differ in length by the writer ratio and the population grows at different
   rates inside them (README §7.2). A ratio of reader nanoseconds is therefore
   between two windows of different length unless the registration fixes the
   window instead of the writer work.
2. **The health build perturbs what it counts.** Every restart and spin is a
   `fetch_add` on one shared counter line across nine threads; per-thread
   counters are [#721](https://github.com/orieg/expanse/issues/721).

**The mechanism stays unmeasured either way.** A nanosecond ratio is a
measurement of the effect, not of its cause; attributing the collapse to a
cache-line transfer, a futex or a bracket wait needs hardware counters
(§8.9 principle 1), which is
[#737](https://github.com/orieg/expanse/issues/737)'s shared `perf stat`
wrapper and [#568](https://github.com/orieg/expanse/issues/568)'s counter plan.

---

## 12. Harness Amendments Before the Re-Run (#731, #733, #735)

Review of the published measurement (harness commit `5232af74`) found three
things in the harness and one in the reporting, none in the predictions of §5,
§10.7 or §11.5. Each changes how a cell is taken, so the suite is re-run at the
amended commit and every refreshed number carries the new commit; nothing is
patched in place (§8.10). The registered rows are not edited, and §12.4 says
where the verdicts moved.

### 12.1 Arm order alternates, and scan starts scale with 1/k (#731)

**Arm order.** Every round of `hot_latency` and `hot_string_latency` timed the
HOT arm first and Expanse second, so whatever the first timed loop left behind
— a warmed cache, a raised clock — was inherited by Expanse alone, in one
direction, in every latency cell the suite publishes. The arm timed first now
alternates per round through `workload::ordered`, and every raw row records
`first_arm`, so a reader can check from the artifact that the alternation
happened rather than take it on trust. The same defect and the same fix are
recorded for the Masstree arm in that suite's METHODOLOGY §10.6.

**Scan starts.** Both binaries fixed the ordered-scan start count at 1,000 for
every k (`take(1_000)`). A k = 10 round therefore visited 10⁴ elements and
repeated the same thousand starts fifteen times, which made the smallest scan
cells the shortest timed windows in the suite and their numbers warm-start
numbers; a k = 1000 round visited 10⁶. Starts are now
`workload::scan_starts(k)` = `max(1000, 10⁶ / k)`, cycled from the probe stream
when it is shorter, so every k visits about 10⁶ elements per round. The helper
is pinned by a unit test in `crates/expanse-hot-bench/src/workload.rs`.

The Masstree arm made both changes and re-measured: with a hundred times more
distinct starts the per-element cost at k = 10 rose on both arms — Masstree
14.5 → 17.4 ns and Expanse 9.4 → 11.1 on `random` at 10⁶ *(measured: reference
host, harness commits `82966aae` → `2ce92b7f`,
`docs/benchmarks/masstree_comparison/results/baseline_latency.json`)*. The HOT
k = 10 scan cells published at `5232af74` are therefore the warm-start figures,
and README §3 compares the two runs in one sentence per arm. The cause of the
movement is **unmeasured**: no hardware counter was taken on either run, and an
aggregate wall-clock difference is not an observation of a cache or prefetch
mechanism (§8.9 principle 1).

**Also fixed, and not a measurement change:** the two arms' visited-element
counts were computed and never compared, so a divergence would have been
divided into both columns as if it were one quantity. Both binaries now void the
cell when the counts differ, as the Masstree binary already did.

### 12.2 Insertion order is a published dimension, and every registered cell is a sorted-order cell (#733)

The shared generators sort the population after drawing it, and every arm in
this suite builds in that order. That is not a neutral choice: it is a B+-tree's
best case, and it moves Expanse too. The Masstree arm's sensitivity set measured
`ExpanseMap`'s own insert cost at 27.4 → 65.2 ns and its allocator footprint at
16.67 → 23.63 B/key between the two orders on the same `random` 10⁶ keys, while
only its node census (`mem_used`, 16.70) was order-invariant *(measured:
reference host, `2ce92b7f`,
`docs/benchmarks/masstree_comparison/results/baseline_sensitivity.json`)*.

**Every insert verdict in §5, §10.7 and their scorecards is therefore a
sorted-order verdict**, and neither this file nor the README said so. The
registered rows stay locked on sorted order — that is the order they were
predicted and measured in, and reconciling them against a different workload in
place is what §8.7 forbids.

**Amended:** `hot_latency`, `hot_string_latency`, `hot_memory_curve` and
`hot_string_memory` take a trailing `sorted|shuffled` token, defaulting to
`sorted`, and every result row carries an `order` field. A **sensitivity pair**
re-runs both arms on a Fisher–Yates permutation of the same population from the
suite PRNG (`workload::shuffle_in_place`) at N = 10⁶ — `random` for the integer
arms, `short` for the string arms; memory on both instruments, 100%-hit lookup
and insert — and is published as its own table
(`results/baseline_sensitivity.json`, `results/baseline_string_sensitivity.json`),
never merged with the registered cells and never given a verdict against §5 or
§10.7.

**The invariant that makes the pair readable** is that the engine's own node
census does not move with build order while the allocator census does, so the
difference between the two columns is attributable to the allocator rather than
to the trie. It is pinned as a deterministic test —
`crates/expanse/tests/test_mem_used_order_invariant.rs` — over `ExpanseMap`,
`ExpanseSet`, `ExpanseStrMap` and `ExpanseBytesMap`, in the core crate because
`expanse-hot-bench` is detached from the workspace and no CI lane compiles it
(§8.12). `ExpanseBytesMap` needs its `BuildHasher` held fixed for the test to be
about order at all: it indexes an `ExpanseMap` by the key's hash and its default
`RandomState` is seeded per instance, so two maps built with `new()` hold two
different hash key sets. Asserted over `new()` the test failed at 4,311,568 B
against 4,310,768 B, which was two seeds and not two orders.

`docs/benchmarks/masstree_comparison/METHODOLOGY.md` §10.2 carries the same rule
for that suite; #726 scopes the generator-side change and the pre-registration
rule that a suite states its build order before locking predictions.

### 12.3 Concurrent cells are replicated, and C2 is cited as a direction and a range (#735)

The Masstree arm was run twice on the reference host and two C2 reader cells
moved past their own BCa 95% intervals between the runs — string W = 1 R = 8
from 0.114 [0.095, 0.129] to 0.228 [0.212, 0.241], integer W = 4 R = 8 from
0.697 [0.683, 0.710] to 0.472 [0.455, 0.494] — while every direction held
*(measured: reference host, `82966aae` and `2ce92b7f`,
`docs/benchmarks/masstree_comparison/README.md` §7)*. For those cells the
between-run spread exceeds the within-run interval, so a single run's level is
not a settled figure.

This suite's C2 cells (§11.4) were run once and are cited as levels in the root
README. **Amended:** the HOT-ROWEX concurrent sweep is run a second time under
the standing conditions, both runs' C2 cells are published side by side, and
every citation of a C2 cell outside this suite states a direction and a range
rather than a level. The replication rule itself — two runs for a concurrent
cell, the claim ceiling is the union of the two intervals, and a cell whose runs
do not overlap is reported as direction-only — is registered in
`docs/BENCHMARKING.md` so the next arm inherits it rather than rediscovering it.

Whether the spread is the host or the engine is **not measured here**: it is
#568's counter plan (`perf c2c`, `xsnp_hitm`, futex counts), and the shared
`perf stat` wrapper #737 adds is what would take it.

### 12.4 What the re-run moved

Recorded after measurement, never reconciled into §5, §10.7 or §11.5.

The suite was re-run on the reference host at harness commit `134a0471`, one
suite at a time under the host lock and the P-core pin, load average 0.15 at the
start and 1.00 or below through every single-threaded phase. Every count below
is recomputed by `scripts/integer_tables.py` and `scripts/string_tables.py` from
the artifacts, not typed.

**Integer arms (144 cells).** Expanse wins 109 — unchanged. HOT wins 30 → **29**,
`BOUNDARY_RESULT` 5 → **6**. Nine cells changed verdict, and eight of the nine sit
within 6% of parity. The exception is the finding:

* **`lookup_hit · map · random` at N = 10⁶ moved from an Expanse win at 1.399 to
  `BOUNDARY_RESULT` at 0.993 [0.977, 1.009]**, and at N = 10⁵ from 1.195 to
  0.959. These are the two cells where §12.1's fixed arm order helped Expanse
  most: HOT ran first in every round and Expanse inherited its warmed cache.
  Alternating the arm timed first removes that, and with it the suite's
  headline uniform-random map-lookup win. The §5.1 row is re-evaluated, not
  edited: it registered a HOT win on uniform-random point lookup, which is now
  **CONFIRMED on Arm A's miss path (0.961), REFUTED on Arm A's hit path (1.010)
  and on Arm B in both directions**.
* `set · lookup_hit · random` at 10⁶ moved the other way, 0.998 → 1.010, a narrow
  Expanse win; `set · lookup_miss · random` at 10⁶ stayed HOT's, 0.940 → 0.961.
* Two `map · scan · sparse` cells moved from `BOUNDARY_RESULT` to Expanse wins,
  and `set · scan · random` k = 10 from `BOUNDARY_RESULT` to a HOT win.

**Scan starts (§12.1).** With `max(1000, 10⁶ / k)` starts the k = 10 per-element
cost rose on both arms in every integer cell, as it did on the Masstree arm:
`map`/`random` HOT 13.82 → 22.16 ns and Expanse 9.91 → 12.95; `set`/`random` HOT
11.21 → 11.53 and Expanse 12.44 → 13.70. The `map`/`random`/10⁶ row's Expanse win
widened from 1.414 / 1.402 / 1.517 to 1.803 / 1.725 / 1.619. On the string arms
`short` k = 10 went HOT 27.44 → 37.52 and Expanse 117.27 → 129.76 (Arm C). **The
cause is unmeasured**: no hardware counter was taken on either run.

**String arms (225 cells, 180 with a HOT column).** HOT wins 97 — unchanged, and
still 72 of 72 scan cells. Expanse wins 78 → **75**, `BOUNDARY_RESULT` 5 → **8**.
Five cells changed verdict, all within 8% of parity, and four of the five moved
from a narrow Expanse win to `BOUNDARY_RESULT`. No registered direction changed.

**Insertion order (§12.2).** Published as README §4.1 with no verdict against §5
or §10.7. `mem_used` is identical in both orders on every arm; the allocator
census is not, and `ExpanseMap`'s moves 16.67 → 23.62 B/key on `random` at 10⁶ —
which reproduces the Masstree arm's independently measured 16.67 → 23.63 for the
same structure, shape and population.

**Concurrent replication (§12.3).** README §7.6. Four of the ten C2 reader cells
and five of the ten C1 writer cells do not overlap their own intervals between
the two runs; every direction and every verdict held.

**What did not move.** Every memory cell is within 0.02 B/key of its previous
value except where noted; the engine's own `mem_used` census is unchanged. The
concurrent §7.2–§7.5 levels are run 1's and are not restated.

*(measured: reference host, `134a0471`, `results/baseline_latency.json`,
`baseline_string_latency.json`, `baseline_memory_curve.json`,
`baseline_string_memory.json`, `baseline_sensitivity.json`,
`baseline_string_sensitivity.json`, `baseline_concurrent_run2.json`)*

---

## 13. The Re-Measurement at `0f4fd40c` and `d3bc49c0` (#732 follow-through)

Recorded after measurement. §5, §10.7, §11.5 and §12 are not edited: the
predictions they lock are the ones that were measured, and reconciling a
pre-registration in place is what §8.7 forbids.

### 13.1 Why the suite was re-run rather than re-tagged

The artifacts published at `134a0471` were taken before the shared provenance
module (#732) was in the tree. They carry no `provenance.host` — CPU model,
frequency driver and governor, transparent-huge-page mode, P-core/E-core and SMT
topology — no `provenance.estimators` statement of what each published column
is, and no busy-CPU delta between load snapshots, which is the figure that
catches a co-resident process while the one-minute load average is still lagging
it. The concurrent artifacts carried no `rounds_raw` at all, so a published
median and its BCa interval could not be recomputed from the artifact by anyone.

None of that is a defect in the numbers. It is a defect in what the artifact
lets a reader check, and §8.10 forbids re-tagging a figure with provenance it
was not measured under, so the suite was re-run.

### 13.2 What was run, and in what order

Four phases, each in its own invocation under the host lock and the P-core pin,
each gated on a load average below 0.6 at its start: the integer arms with the
sensitivity pair, the string arms with theirs, then two concurrent sweeps. The
runner still starts its concurrent sweep *before* the single-threaded phases
when both are asked for in one invocation, which is why they are driven
separately here: a single-threaded phase timed inside the sweep's load decay
reads as contamination under `docs/BENCHMARKING.md` rule 2 and is only the
sweep's own tail.

The single-threaded phases were measured at `0f4fd40c` at load 0.55–1.00 with
the host's busy CPU at 1.0 core-equivalents between every pair of snapshots —
the benchmark and nothing else. The concurrent pair was measured at `d3bc49c0`,
which adds `rounds_raw` to the concurrent and health cells and changes nothing
either arm executes; both sweeps started at load 0.40.

### 13.3 What the re-run moved

Every count below is recomputed by `scripts/integer_tables.py` and
`scripts/string_tables.py` from the artifacts, not typed.

**Integer arms (144 cells).** Expanse wins 109 → **111**, HOT 29 → **30**,
`BOUNDARY_RESULT` 6 → **3**. Five cells changed verdict, all of them narrow and
four at N = 10⁵: three moved off the parity line to Expanse, `lookup_hit · map ·
random` at 10⁵ moved from `BOUNDARY_RESULT` to a HOT win (0.938), and
`scan · map · sparse` at 10⁶ k = 1000 moved onto it.

**String arms (225 cells, 180 with a HOT column).** Expanse wins 75 → **77**,
HOT 97 → **96**, `BOUNDARY_RESULT` 8 → **7**. Seven cells changed verdict, all
within 8% of parity. HOT still takes 72 of 72 scan cells. No registered
direction changed on either arm.

**Concurrent.** Every direction and every verdict held across the pair; §7.6
carries the between-run table.

### 13.4 What this re-run settles that the last one could not

**The uniform-random map-lookup boundary is not one run's accident.** §12.4
reported `lookup_hit · map · random` at 10⁶ moving from an Expanse win of 1.399
to `BOUNDARY_RESULT` at 0.993 [0.977, 1.009] when the arm timed first began to
alternate. An independent run of the same harness puts it at 0.992
[0.977, 1.007] — intervals that agree to the third decimal. The suite's
headline uniform-random map-lookup win is gone, and it is gone reproducibly.

**The concurrent spread is run-to-run, not engine-to-engine.** The pair §12.3
published spanned two engine commits, so host variation and whatever changed in
the engine were confounded and §7.6 could only say so. This pair is two runs of
*identical binaries*: 4 of the 10 C2 reader cells and 2 of the 10 C1 writer
cells still fall outside each other's intervals, while all 20 concurrent memory
cells are byte-identical. A deterministic census taken by the same code on the
same host reproduces exactly; the wall-clock cells do not. The spread is the
host and the run, and the cause of it remains unmeasured.

### 13.5 The `ExpanseBytesMap` census is seeded per instance, and moves between processes

The string memory census reproduced exactly on Arms C and D and moved on
**168 figures of Arm E**, by up to 2.5 B/key at N = 2,000 and by less than
0.05 B/key at N = 10⁶. The cause is the one §12.2 records for the
order-invariance test: `ExpanseBytesMap` indexes an `ExpanseMap` by the key's
hash and its default `BuildHasher` is seeded per instance, so two processes
build two different key sets and land different bucket occupancies. The
published Arm E figures are quoted to one decimal at N = 10⁶, where the
movement is in the second, so no published number changes — but §9's statement
that memory "is deterministic and carries no interval" holds for Arms C and D
and for every integer arm, and holds for Arm E only within a process. A future
arm that wants Arm E's census to be reproducible across processes has to pin
the hasher, as `crates/expanse/tests/test_mem_used_order_invariant.rs` does.

*(measured: reference host, `0f4fd40c` and `d3bc49c0`,
`results/baseline_latency.json`, `baseline_string_latency.json`,
`baseline_memory_curve.json`, `baseline_string_memory.json`,
`baseline_sensitivity.json`, `baseline_string_sensitivity.json`,
`baseline_concurrent.json`, `baseline_concurrent_run2.json`)*

