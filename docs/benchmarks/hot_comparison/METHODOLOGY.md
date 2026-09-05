# HOT (Height Optimized Trie) vs. Expanse: Pre-Registration & Comparative Methodology

**Status: pre-registration locked. No suite measurement has been taken.**

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
dense-distribution advantage** relative to the repo's own table. Any cell
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
