# Benchmarking Guidelines

> Canonical benchmarking doc. Design targets: [ARCHITECTURE.md](ARCHITECTURE.md) §6 · Testing: [TESTING.md](TESTING.md)

Performance claims are this project's reason to exist, so they follow the strictest discipline in the repo: **no claim without a measurement, no measurement without methodology.**

## Harness and metrics

- Criterion benches live in `crates/expanse/benches/`; C-comparison benches drive both sides through the C surface once `expanse-capi` exports it.
- Key distributions: the same matrix as [TESTING.md](TESTING.md) (sequential, random, clustered, sparse/pathological, boundary) — performance work that only measures random keys is rejected.

| Metric | Definition |
|---|---|
| Lookup latency | ns/op, hit and miss measured separately, per distribution × population size |
| Insert/delete throughput | Mops/s, cold-build and steady-state (interleaved ins/del) measured separately |
| Memory footprint | bytes/key at population checkpoints, via allocator instrumentation (counting `GlobalAlloc` wrapper), cross-checked against `MemUsed` |
| Scan throughput | keys/s for full-order iteration |

## Comparison targets

1. **C libjudy** — the headline comparison ("faster than the original, or explain why").
2. `std::collections::BTreeMap` / `HashMap` — the "why not just use std" baseline.
3. An ART (adaptive radix tree) implementation — the modern-SOTA trie baseline, when a maintained crate is selected.

## Methodology rules (binding)

0. **State the measured region, and have a reviewer check it.** Every
   benchmark's doc comment says exactly what is inside the timed window
   and what is in `setup`/`teardown`. This rule exists because the
   vs-stock lookup arms silently measured the tree *build* (and then, on
   the first attempt to fix that, still measured the *teardown*), which
   produced published ratios that had to be retracted twice. Every other
   rule below governs how a number is interpreted; this one governs
   whether it measures what its name says.

   **Four** separate violations have now been found and fixed, across
   both harness files: the build inside the vs-stock lookup arms; the
   teardown inside every vs-stock arm; **key generation inside the insert
   arms**; and finally the same teardown bug again in
   `benches/instructions.rs`, the file that produces the vs-base column on
   every PR, where the arms take the container by value so `Drop` ran
   inside the timed window.

   Two lessons, both paid for:

   - **Symmetric contamination is not harmless contamination.** Key
     generation was charged to both libraries equally, which is exactly
     why it survived review — and it still corrupts the comparison, by
     pulling every ratio toward 1.00.
   - **Every one of the four was found by an instrument, never by a
     reviewer** — despite this rule explicitly asking a reviewer to check
     the measured region. The rule is necessary and has not once been
     sufficient. Treat "a reviewer checked it" as the weakest evidence
     available and get per-function output instead; `free_subtree`
     appearing inside a benchmark named `map_get` is unmissable, while the
     same fact stated in prose was missed four times.

   The same discipline applies to a number's **provenance**: B0 below was
   labelled with a commit it was not measured at, because the branch had
   been cut from other in-flight work. A mislabelled commit misleads
   exactly as much as a mismeasured region.

0b. **Both arms must be the same shape.** A comparison is only valid
   between binaries built and reached the same way. Ours was an LTO'd
   rlib called directly while stock was a PIC shared object reached
   through `dlopen`, which handed us cross-object inlining and direct
   calls that stock structurally cannot have — a bias in our favour that
   sat unexamined behind a code comment claiming the opposite. Every
   vs-stock arm now has an `*_expanse_dl` twin that loads our own
   `libexpanse.so` exactly as stock is loaded; the PR comment reports the
   `.so` ratio as the headline and the rlib−`.so` difference as an
   explicit correction factor.

1. **Interleaved A/B arms.** Any A-vs-B comparison (regression check, libjudy comparison, before/after a change) alternates arms per benchmark group over several rounds — never suite-A-then-suite-B. Runner/thermal drift then hits both arms and cancels in the paired ratio. (Learned the hard way in php-judy — back-to-back suites reported false regressions; see php-judy issue #87 and its `bench-compare` harness.)
2. **System-load hygiene.** Before the first run and between comparison runs, snapshot load (`ps -A -o %cpu,%mem,command | sort -rn | head`; load average vs core count). A non-target process above ~100% CPU, or a load-average shift > 2 between arms, contaminates the run: discard it, don't reinterpret it. Laptops running concurrent sessions are shared infrastructure.
3. **CI benches detect changes, not truths.** CI runners produce paired ratios good for regression alarms. Publishable absolute numbers (README/claims) come from a dedicated quiet host with the hardware named.
4. **Tag every number.** Performance figures in docs are tagged `(measured: <machine, commit>)` or `(target)`. Untagged numbers are lint errors in review. The two architecture KPIs (<15 ns random point lookup; <9.5 B/key dense) are `(target)` until measured.
5. **Report distributions, not just means.** Criterion medians + CIs; a claim that A beats B requires the CI bounds to separate, not the point estimates.
6. **Fixed seeds, recorded populations.** Every bench names its RNG seed and population sizes so runs are reproducible bit-for-bit.
7. **No time estimates, no projections-as-results** — per repo-wide discipline (CLAUDE.md).

## Bench matrix

| Bench | Status | Notes |
|---|---|---|
| Lookup latency grid (hit/miss × distribution × population) | landed (`benches/compare.rs`) | vs `BTreeSet`/`BTreeMap`, `HashSet`/`HashMap`; timing numbers unpublished until a quiet-host run |
| Insert throughput (cold build per distribution) | landed (`benches/compare.rs`) | same caveat |
| bytes/key | landed (`examples/bytes_per_key.rs`) | deterministic allocator accounting — load-immune, results below; **gates CI** via the `memory-budget` job |
| Instruction/cache counts | landed (`benches/instructions.rs`, iai-callgrind) | deterministic via callgrind — load-immune and resolves ~1% changes; **posted as a PR comment with head-vs-base deltas** by the `instruction-counts` job |
| Lookup attribution | landed (`examples/lookup_profile.rs`) | sampling profile of a `get`-only loop — *where* time goes, not how long; sample distribution inside one process is far less load-sensitive than a cross-binary ratio |
| Concurrent read scaling (1..N threads) | landed (`examples/concurrent_scaling.rs`) | Read-only and write-churn mixes; the per-node-OCC go/no-go instrument — first numbers below |
| Full libjudy + ART comparison | Phase 8 remainder | Headline table, dedicated-host runs, driven through the capi surface |

## Reading perf results in a PR

Every pull request gets a single updating comment from the
`instruction-counts` job, with two comparisons that answer different
questions — reading one as the other is the mistake the comment is
designed to prevent:

1. **vs stock libjudy** (leads the comment): identical C ABI calls and
   key streams through libexpanse and through a `dlopen`'d stock
   libjudy, in instructions retired (`crates/expanse-capi/benches/vs_stock.rs`).
   This is the drop-in question — is the replacement competitive — and
   it is the project's headline claim. Ratio below 1.00 means we do
   less work.
2. **vs the merge base**: the same engine measured against its own
   previous commit (`crates/expanse/benches/instructions.rs`). This is
   the regression question — did this change make the engine do more
   work — and it says nothing about stock.

Both are callgrind counts, plus collapsed sections for cache/RAM traffic
and the bytes/key table.

Interpretation rules, so the comment is not over-read:

- **Instruction and cache counts are exact.** The same commit produces
  the same numbers on any runner, so a delta above 0.1% is a real change
  in work done, not measurement noise. That is why the comment leads
  with deltas and flags regressions ≥ 1%.
- **They are cost, not time.** Fewer instructions is strictly better
  work, but the wall-clock effect depends on how well the machine hides
  the remaining latency. A speed claim still needs a quiet host.
- **Wall-clock vs stock libjudy lives in the nightly `bench-report`
  job**, and remains a regression alarm rather than a publishable
  number (rule 3, and the retracted `memcmp` claim below). The
  instruction-count vs-stock table in the PR comment does not replace
  it: instructions say how much work each library does, wall-clock says
  how long the machine takes to do it, and the gap between those two is
  cache behaviour — which is exactly what the allocator-locality work
  (issue #1 item 4) is about.
- **The two vs-stock arms are not built alike, and the asymmetry
  favours us.** Stock is reached through `dlsym` — it exports the same
  symbols libexpanse does, and loading it privately is what keeps them
  from colliding — so it pays an indirect call per operation. Larger
  than that: stock is a PIC shared object with no cross-object inlining,
  while our arm is an LTO'd rlib linked straight into the harness, and
  stock's `dlopen` still sits inside the measured region. Every one of
  those points the same way. **Treat every vs-stock ratio as a floor on
  the gap, not an estimate of it**, until the same-shape comparison
  (our own `libexpanse.so`, `dlopen`'d identically) lands — checkpoint
  B2. An earlier version of this bullet claimed the indirection biased
  *against* stock; that was wrong, and it was printed on every PR
  comment.

## Measured results

### bytes/key (measured: deterministic allocation accounting via `NodeAlloc`, commit with this section; machine-independent)

| dist | pop 1k | pop 100k | pop 1M | |
|---|---|---|---|---|
| sequential (set) | 0.32 | 0.07 | **0.06** | full-expanse + bitmap-leaf compression |
| clustered 256-run (set) | 0.38 | 0.37 | **0.36** | was 1.34 before leaf-targeted narrow pointers — a 3.7× improvement |
| clustered 4096-run (set) | 0.32 | 0.12 | **0.12** | was 0.64 / 0.20 / 0.19 before **branch-targeted** narrow pointers (divergence-level branch placement + `split_skip`) |
| random (set) | 12.27 | 13.69 | 7.65 | not part of the dense/clustered target |
| sparse `i << 40` (set) | 16.58 | 16.07 | **16.06** | one 16-byte edge per isolated key — the structural floor, not a chain cost (immediates absorb the remainders) |

Map-flavor figures run ~8 B/key above the set figures (the stored value word). The `< 9.5 B/key dense+clustered` architecture target is **met** on the distributions it names. Unit-level anchor for the branch-targeted work: two 512-key clusters cost 192 structural bytes vs 960 under per-level chains (`branch_skip_clusters` tests, both flavors).

### Instruction counts: issue #1 items 1-3 (measured: callgrind via the `instruction-counts` CI job; deterministic, so these are exact — commit with this section)

Controlled A/B against a branch with all three items reverted, same harness and job. Instructions retired, 50k keys per benchmark; negative is less work.

| benchmark | instructions | est. cycles |
|---|---:|---:|
| `set_insert/random` | **−16.95%** | −17.62% |
| `map_iterate/random` | −14.47% | −15.42% |
| `set_contains/random` | −14.11% | −14.95% |
| `map_ins_slot/random` | −12.28% | −12.62% |
| `map_insert/random` | −12.06% | −12.49% |
| `set_insert/sequential` | −10.73% | −11.42% |
| `map_get/random` | −10.43% | −10.98% |
| `map_insert/small` | −8.90% | −9.95% |
| `map_insert/sequential` | −8.07% | −8.18% |
| `map_remove/random` | −7.44% | −8.36% |
| `set_insert/clustered` | −6.88% | −7.83% |
| `map_get/sequential` | −6.02% | −6.28% |
| `map_insert/clustered` | −2.98% | −3.95% |
| `map_get/clustered` | −2.26% | −3.08% |

All 14 benchmarks improved; nothing regressed. The three changes were: the per-level OCC check hoisted into a const generic (removing ~10 atomic loads per mutation), immediate-edge key handling moved from `Vec` to a stack buffer (removing a malloc/free per insert into an immediate), and width-monomorphized packed key access (`read_packed`/`write_packed`/`lower_bound`).

Two things this table is **not**. It is not a wall-clock claim — instructions are cost, and how much of it a machine hides is a separate question needing a quiet host. And it is not comparable to the vs-stock ratios: those are wall-clock on different hardware. What it *is*: reproducible evidence that the engine does measurably less work, at a resolution (0.1%) neither available environment can reach with a timer.

Contrast with the `memcmp` episode below, where wall-clock at n=1 suggested a 7-11% lookup win that a second run erased. Same class of change, same magnitude — but here the measurement can actually resolve it.

### Attribution findings (measured: M1 MacBook Pro under load — attribution only, no timing claim; commit with this section)

Profiling a 1M-key random `get` loop (`examples/lookup_profile.rs`, macOS `sample`) attributed **~6% of lookup samples to `memcmp` reached through a dynamic-linker stub**: the linear-leaf key comparison used a runtime-width slice compare, which lowers to a libc call rather than inline code. Monomorphizing the scan over the key width (`leaf::search_fixed::<KB>`) removed the call — the re-profiled binary contains **zero** `memcmp` references on that path. This is a *structural* claim (the call no longer exists), independent of machine load; the resulting ns/op change is unclaimed until a quiet-host run.

Also visible: `EdgeTag::from_u8` not fully inlined (~1.7% of samples), which is the measured part of the "tag-dispatch overhead" hypothesis — small next to the leaf comparison, and worth revisiting only with the vs-stock harness on a quiet host.

**Did removing the `memcmp` call speed lookups up? Not measurably — verdict: NO DETECTABLE EFFECT.** A controlled A/B was run through the `bench-report` job: identical code and job on both arms, with only `leaf::search` reverted to the runtime-width compare on a throwaway branch. The first pair looked like a 7–11% get-ratio win in all three distributions; a second pair of runs refuted it.

| get ratio @1M | with monomorphized search | with `memcmp` |
|---|---|---|
| sequential | 1.60, 1.75 | 1.77, 1.62 |
| random | 1.47, 1.47 | 1.66, **1.34** |
| clustered | 1.55, 1.73 | 1.65, 1.65 |

The arms overlap; the pre-change branch produced both the worst and the best random-key ratio observed. Within-arm spread (up to 0.32 on a ~1.5 ratio) is as large as the between-arm difference, so any real effect is buried. The change is kept — it strictly removes work (a PLT-indirected libc call from the innermost loop) and cannot be slower — but **no speedup is claimed**.

**Methodological consequence — the CI runner's noise floor.** Across four runs the same ratio varied by up to ±10%, and absolute ns by ±40%, on freshly booted runners at load < 1.7. That puts the minimum detectable effect for this setup somewhere around **15–20% at n=2** — fine for catching a structural regression, useless for validating incremental optimization. Incremental perf work therefore needs a dedicated quiet host with many rounds and reported confidence intervals; adding runs to CI buys little because the variance is between runner instances, not within a run. Until such a host exists, prefer changes justified *structurally* (work removed, allocations avoided, cache lines not touched) over changes justified by a measured delta this environment cannot resolve.

Method note: attribution is done inside a single process, so co-resident load shifts *how many* samples land, not *where* they land. A/B wall-clock ratios do not share that property and stay deferred (rule 2).

### Concurrent read scaling, `SyncExpanseMap` (measured: M1 MacBook Pro 8-core, load ≈ 3.7 with two ~25% background processes — magnitudes far beyond that noise; commit with this section; 1M random keys, 500 ms windows)

| readers | reads/s idle | scale | churn (tree-level, before) | churn (per-node, after) |
|---|---|---|---|---|
| 1 | 10.1 M | 1.0× | **3.4 K** | **2.4 M** |
| 2 | 15.4 M | 1.5× | 3.4 M | 6.2 M |
| 4 | 32.8 M | 3.2× | 10.0 M | 10.0 M |
| 8 | 39.8 M | 3.9× | 21.3 M | 12.5 M† |

Before: with one tree-level seqlock, a full-speed writer (~2.5 M op/s) collapsed optimistic reads — the version changed faster than a walk completed, so nearly every validation failed into the mutex fallback (the 3.4 K/s cell). After the **per-node refinement** (writers bracket each node's in-place mutations with that node's version; readers validate hand-over-hand): single-reader churn throughput rose **~700×** to 2.4 M/s and writer throughput rose to ~3.3 M op/s (fewer mutex handoffs). The remaining churn-vs-idle gap is the walk-start gate (the tree version still brackets whole ops for the root snapshot); † higher reader counts under a saturating writer shift toward retry pressure along the written path — the next refinement target if a real workload needs it. Single-threaded trees skip the version brackets entirely (`NodeAlloc::occ_enabled`), so the classic engine pays nothing.

### Checkpoint B0 — first vs-stock baseline on the corrected harness (measured: GitHub `ubuntu-latest` runner, callgrind via the `instruction-counts` job, **the issue #1 items-1-3 branch, not `af21e02`**; deterministic)

The first vs-stock instruction baseline taken after the harness stopped
measuring its own setup. Both the tree build **and** the teardown were inside
the timed region of every arm before `af21e02`; the lookup arms were therefore
reporting insert work. **The earlier 2.09× / 2.11× lookup ratios are retracted.**

⚠️ **Provenance correction.** This table was originally labelled `af21e02`. It
was not measured there — the numbers come from the branch carrying issue #1
items 1-3, so they are `main` *plus* that engine work. B2 below has the true
`main` baseline alongside it. Kept here as the rlib-only measurement it is, and
as a record that a mislabelled commit is exactly as misleading as a
mismeasured region.

Ratio = ours ÷ stock, instructions retired through the identical C ABI on
identical key streams. 30k keys except `random_big`, which is 1.5M.

| operation | ours | stock | ratio | est. cycles ratio |
|---|---:|---:|---:|---:|
| `judy1_set/clustered` | 12,561,871 | 10,910,533 | **1.15×** | 1.20× |
| `judy1_set/random` | 32,633,176 | 16,350,887 | 2.00× | 2.02× |
| `judy1_test/random` | 7,160,539 | 3,970,257 | 1.80× | 1.69× |
| `judyl_get/clustered` | 6,730,957 | 4,238,479 | 1.59× | 1.52× |
| `judyl_get/random` | 7,576,697 | 5,095,457 | 1.49× | 1.45× |
| `judyl_get/random_big` (1.5M) | 441,381,294 | 403,155,522 | **1.09×** | **1.04×** |
| `judyl_get/sequential` | 9,300,109 | 5,072,393 | 1.83× | 1.68× |
| `judyl_insert/clustered` | 20,710,296 | 12,532,068 | 1.65× | 1.69× |
| `judyl_insert/random` | 40,152,643 | 18,377,126 | 2.18× | 2.20× |
| `judyl_insert/sequential` | 23,217,209 | 13,089,262 | 1.77× | 1.77× |

**The gap is population-dependent, and that is the most useful thing in this
table.** The same random-key lookup is 1.49× at 30k keys and 1.09× at 1.5M —
and its estimated-cycles ratio (1.04×) is *below* its instruction ratio, the
only arm where that happens, which is what a denser structure taking
proportionally fewer misses looks like. Every prior "the gap is instructions,
not memory" statement was made from small-population arms only.

Read it as a hypothesis, not a result. It is one arm at one population; the
estimated-cycles figure is callgrind's `Ir + 10·L1m + 100·LLm` model, which
assumes zero mispredicts and zero dependent-load latency; and confirming that
the narrowing is real needs wall-clock at 1.5M with bootstrap CIs (checkpoint
B5). What it does establish is that the small-population arms are not
representative of the whole curve, so no single ratio should be quoted as
"the gap" without its population.

**Every ratio above is optimistic**, and the correction has not been applied
yet. Our arm is an LTO'd rlib reached by direct calls; stock is a PIC shared
object reached through `dlopen`/`dlsym`, and its `dlopen` is still inside the
measured region. Both biases favour us. The honest drop-in number needs our
own `libexpanse.so` measured the same way stock is (checkpoint B2), and until
that lands these are a floor on the gap, not an estimate of it.

Operational note: `instruction-counts` runs in **~2 minutes against its
45-minute timeout**, including the 1.5M arms. The timeout risk that argued for
moving the big arms to nightly does not exist — they stay in the required
check. `miri` is the only expensive job at ~25 minutes, and it is now
conditional on the diff touching Rust sources.

### Checkpoint B2 — the same-shape comparison, and the shared-library correction factor (measured: GitHub `ubuntu-latest` runner, callgrind via `instruction-counts`, commit with this section; deterministic)

B0 above measured our rlib against stock's shared object, on a branch that
also carried issue #1 items 1-3. This measures our own `libexpanse.so`,
`dlopen`'d and called through resolved symbols exactly as stock is — the shape
a drop-in consumer actually gets — at **two code states**, so the drop-in cost
and the engine work are separable. **Quote the `.so` columns.**

| operation | `.so` on `main` | `.so` with items 1-3 | rlib on `main` |
|---|---:|---:|---:|
| `judy1_set/clustered` | 1.35× | **1.25×** | 1.26× |
| `judyl_get/random_big` (1.5M) | 1.29× | **1.15×** | 1.23× |
| `judyl_get/random` | 1.66× | 1.58× | 1.57× |
| `judyl_get/clustered` | 1.71× | 1.71× | 1.59× |
| `judyl_insert/clustered` | 1.89× | 1.78× | 1.78× |
| `judyl_get/sequential` | 1.94× | 1.94× | 1.83× |
| `judy1_test/random` | 1.95× | 1.88× | 1.87× |
| `judyl_insert/sequential` | 1.97× | 1.71× | 2.03× |
| `judy1_set/random` | 2.18× | 1.93× | 2.26× |
| `judyl_insert/random` | 2.40× | 2.16× | 2.44× |

**Correction factor: 1.05× median, range 0.96–1.07× on `main`** (1.06×,
0.95–1.08× with items 1-3). It barely moves between two different code states,
which is what a ratio between two builds of the *same* code should do — the
one number here that is robust rather than provisional.

**The prediction behind it was wrong in an instructive way.** The expectation
on record was that being a shared object would cost us uniformly, so every
ratio would widen. It does not. The cost is real but **confined to the lookup
arms** (1.05–1.07×), where a PLT-mediated entry is a meaningful fraction of a
short operation; on the insert arms the `.so` is *cheaper* than the LTO'd rlib
(0.96–0.98×). Both builds get `lto = "thin"` and `codegen-units = 1`; what the
rlib arm additionally gets is cross-crate inlining with the harness, and on a
long insert that apparently costs more instructions than it saves. So
"cross-object inlining is an advantage stock cannot have" was the right shape
of argument and the wrong magnitude — worth ~5–7% on lookups, nothing on
inserts.

**Two lookup arms are byte-identical across the two code states** —
`judyl_get/clustered` at 7,227,854 and `judyl_get/sequential` at 9,840,110,
unchanged to the instruction while every other arm moved. That is an
independent confirmation of the terminal-form census below, which predicted
that items 1-3 cannot reach those arms: they build only bitmap leaves, so
neither the immediate scan nor the linear-leaf population read is on their
path. A prediction made from structure, confirmed by an instrument that did
not know about it.

Both arms bind their symbols in `setup`, so neither measures its own dynamic
linking, and key generation has moved out of the insert arms' measured region —
see rule 0 for why that mattered even though it was symmetric.

### First per-function attribution (measured: GitHub `ubuntu-latest` runner, `callgrind_annotate` over the `instruction-counts` job, commit with this section; deterministic)

Until now the job kept per-benchmark totals only, so a delta could be observed
but not attributed. The first run with per-function output found three things
in one pass, two of which are the largest actionable items on record.

**1. `EdgeTag::from_u8` is not inlined, and it is 8-19% of every lookup.**

| arm | `from_u8` | share |
|---|---:|---:|
| `set_contains/random` | 2,948,168 | **19.4%** |
| `map_get/random` | 2,413,688 | 14.2% |
| `map_get/sequential` | 1,602,072 | 9.6% |
| `map_get/clustered` | 1,008,776 | 8.1% |

It appears as its own symbol — a real call — at ~8 instructions each, once per
level descended. The call count cross-checks exactly against the terminal-form
census below: sequential lookups touch 4 edges (`BranchL3`@8, `BranchL3`@7,
`BranchU`@2, `LeafB1`@1) and the profile shows 200,259 calls over 50,000
probes, i.e. 4.005 per lookup. Earlier macOS sampling put this at ~1.7% and it
was treated as a minor item; that was a sampling profile of a different
distribution. Inlining it is now a measured opportunity, not a hypothesis.

**2. ~16% of `map_insert/random` is inside the allocator** — `_int_malloc`
7,685,357 (11.2%) plus `_mid_memalign` 3,226,000 (4.7%). The `memalign` path
is what 64-byte alignment costs: it takes glibc off its fast `malloc` route.
This is the measured case for alignment classes.

**3. Rule 0, violated a fourth time — in the other harness file.**
`free_subtree` appeared *inside the lookup benchmarks*: 1,654,204 (9.7%) of
`map_get/random`, 1,264,288 (8.3%) of `set_contains/random`. The arms take the
container by value, so `Drop` ran inside the measured region. `vs_stock.rs` had
been fixed; `instructions.rs` — the file that produces the vs-base column on
every PR — had not. Fixed by leaking, matching `vs_stock.rs`, along with
`keys()` moving into `setup` for the insert arms.

The pattern is worth naming: **every one of the four violations was found by an
instrument, never by reading the harness.** Rule 0 asks a reviewer to check the
measured region, and four times a reviewer did not catch it.

### What the bench matrix structurally covers (measured: terminal-form census over the tree each benchmark builds, 50k keys, map flavor; commit with this section; machine-independent)

A benchmark named after a distribution does not tell you which *forms* it
exercises, and the answer turned out to be narrower than assumed:

| distribution | terminal forms |
|---|---|
| `sequential` | 196 × `LeafB1` at level 1 — no linear leaves, no immediates |
| `clustered` | 196 × `LeafB1` at levels 6-7, behind narrow pointers |
| `random` | 23,290 immediates (6-byte) + 11,675 × `Leaf6` at level 6 |

Two consequences, both load-bearing:

- **No benchmark exercises a linear leaf on a dense distribution.** Linear-leaf
  search is reachable only through `random`, and only at width 6. Any claim
  about leaf-width monomorphization or leaf search is currently measured on one
  arm at one width — the `dense_leaf` arm is a known coverage hole, not a
  precaution.
- **Immediates are ~2/3 of random-key terminals** (23,290 of 34,965),
  confirming the assumption behind prioritizing `immed_find`.

This census is also what attributed a +1.50% regression that per-benchmark
totals could not: the two arms that regressed reach *none* of the code paths
the change touched, which rules out the obvious explanations rather than
ranking them. Worth re-running whenever a distribution or population changes —
promoting it out of a throwaway probe is tracked in issue #1.

### libexpanse vs stock libjudy, JudyL surface (measured: GitHub `ubuntu-latest` runner, 2 cores, load 0.42 at start — the standing reference environment; commit with this section; interleaved A/B medians of 5 rounds; harness: `crates/expanse-capi/examples/bench_vs_libjudy.rs`, nightly `bench-report` job)

| dist | pop | get ratio (ours/stock) | insert ratio | B/key ratio |
|---|---|---|---|---|
| sequential | 1M | 1.60× slower | 2.88× slower | 1.03 |
| random | 1M | 1.47× slower | 2.38× slower | **0.93 (smaller)** |
| clustered | 1M | 1.55× slower | 2.48× slower | **0.92 (smaller)** |

**This table is the reference, not the M1 one below.** A CI runner is not a quiet host either, but it is freshly booted, unshared with a desktop session, and reproducible — whereas the development laptop runs VMs, browsers and indexers mid-measurement (the "Reproducibility correction" note below is what that costs). Ratios, not absolute ns, are what transfers between machines: the paired interleaved arms normalize away machine speed. Absolute numbers for publication still require a dedicated host with the hardware named.

Cross-machine sanity check: bytes/key columns are byte-identical to the local run (deterministic accounting), which is what makes the timing columns' disagreement attributable to hardware rather than to the build.

### Earlier: same harness on the development laptop (measured: M1 MacBook Pro under load — a VM at ~226% CPU co-resident — commit with this section; interleaved A/B medians of 5 rounds, so the *ratios* are meaningful while absolute ns are contaminated; harness: `crates/expanse-capi/examples/bench_vs_libjudy.rs`)

| dist | pop | get ratio (ours/stock) | insert ratio | B/key ratio |
|---|---|---|---|---|
| sequential | 1M | 2.3–4× slower | 3.4× slower | 1.03 |
| random | 1M | 1.55× slower | **1.35× slower** | **0.93 (smaller)** |
| clustered | 1M | **~parity (0.94–1.2×)** | **1.7× slower** | **0.93 (smaller)** |

History of the insert column (each row measured at its commit): 13.5× → 3.7× (narrow-pointer synthesis) → **1.7×** (insert-path optimization: capacity-classed allocations with in-place shifts across leaves, bitmap-branch subarrays and bitmap-leaf value arrays, plus the fused single-walk `JudyLIns`). Sequential insert went 5.9× → 3.4×, random 1.9× → 1.35×. **Reproducibility correction (2026-08-18):** re-measuring three historical commits back-to-back on a quieter host put the random-insert ratio in a **1.8–2.3 band at every one of them** — including the commit the 1.35 was recorded at. The 1.35 reading was distorted by the co-resident VM load of its session (interleaving cancels drift, not cache-pressure asymmetry); treat the band, not the point, as the working baseline until a quiet-host run. Clustered insert (~1.7–1.8×) and all bytes/key columns reproduce exactly. Memory cost of the classes is bounded by keeping ≤2-entry allocations exact (random map bytes/key briefly regressed 21→27 with naive rounding; 22.9 after the refinement, vs stock's 24.8). Remaining insert gap is profile-driven follow-up (immediate rebuilds, per-level dispatch).

Honest reading (v1 correctness-first, zero optimization passes yet):

- **Memory is already competitive** — smaller than stock on random keys.
- **Clustered lookup is solved**: narrow-pointer synthesis removed the chain walk (5.31× slower → parity). The remaining lookup gap is on sequential/random — next profile targets: per-level dispatch overhead and the root-leaf/tree split.
- **Insert gap** has three known, documented v1 costs to burn down: full leaf rebuild per insert (no capacity classes), `Vec` materialization in the mutation path, and capi `JudyLIns` walking the tree three times (contains + insert + slot). Each is an isolated follow-up with this table as baseline.

Timing numbers here are working baselines, not publishable claims; headline numbers still require a quiet-host run under the system-load protocol above.
> **Correction (2026-08-19): the first vs-stock lookup figures were wrong.**
> The `*_get` / `*_test` benchmarks built their 30k-key array *inside* the
> measured region, so the reported "lookup" ratios were a blend dominated
> by insert. Two independent reviews caught it; it is fixed with
> `setup =` so only the probe loop is counted. Ratios published before
> that fix (`judyl_get/random 2.09x`, `judy1_test/random 2.11x`, and the
> clustered/sequential lookup rows) are **retracted**. Insert ratios were
> never affected — those benchmarks always measured only inserts.

