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
| Lookup attribution | landed (`examples/lookup_profile.rs`) | sampling profile of a `get`-only loop — *where* time goes, not how long; sample distribution inside one process is far less load-sensitive than a cross-binary ratio |
| Concurrent read scaling (1..N threads) | landed (`examples/concurrent_scaling.rs`) | Read-only and write-churn mixes; the per-node-OCC go/no-go instrument — first numbers below |
| Full libjudy + ART comparison | Phase 8 remainder | Headline table, dedicated-host runs, driven through the capi surface |

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

### Attribution findings (measured: M1 MacBook Pro under load — attribution only, no timing claim; commit with this section)

Profiling a 1M-key random `get` loop (`examples/lookup_profile.rs`, macOS `sample`) attributed **~6% of lookup samples to `memcmp` reached through a dynamic-linker stub**: the linear-leaf key comparison used a runtime-width slice compare, which lowers to a libc call rather than inline code. Monomorphizing the scan over the key width (`leaf::search_fixed::<KB>`) removed the call — the re-profiled binary contains **zero** `memcmp` references on that path. This is a *structural* claim (the call no longer exists), independent of machine load; the resulting ns/op change is unclaimed until a quiet-host run.

Also visible: `EdgeTag::from_u8` not fully inlined (~1.7% of samples), which is the measured part of the "tag-dispatch overhead" hypothesis — small next to the leaf comparison, and worth revisiting only with the vs-stock harness on a quiet host.

Method note: attribution is done inside a single process, so co-resident load shifts *how many* samples land, not *where* they land. A/B wall-clock ratios do not share that property and stay deferred (rule 2).

### Concurrent read scaling, `SyncExpanseMap` (measured: M1 MacBook Pro 8-core, load ≈ 3.7 with two ~25% background processes — magnitudes far beyond that noise; commit with this section; 1M random keys, 500 ms windows)

| readers | reads/s idle | scale | churn (tree-level, before) | churn (per-node, after) |
|---|---|---|---|---|
| 1 | 10.1 M | 1.0× | **3.4 K** | **2.4 M** |
| 2 | 15.4 M | 1.5× | 3.4 M | 6.2 M |
| 4 | 32.8 M | 3.2× | 10.0 M | 10.0 M |
| 8 | 39.8 M | 3.9× | 21.3 M | 12.5 M† |

Before: with one tree-level seqlock, a full-speed writer (~2.5 M op/s) collapsed optimistic reads — the version changed faster than a walk completed, so nearly every validation failed into the mutex fallback (the 3.4 K/s cell). After the **per-node refinement** (writers bracket each node's in-place mutations with that node's version; readers validate hand-over-hand): single-reader churn throughput rose **~700×** to 2.4 M/s and writer throughput rose to ~3.3 M op/s (fewer mutex handoffs). The remaining churn-vs-idle gap is the walk-start gate (the tree version still brackets whole ops for the root snapshot); † higher reader counts under a saturating writer shift toward retry pressure along the written path — the next refinement target if a real workload needs it. Single-threaded trees skip the version brackets entirely (`NodeAlloc::occ_enabled`), so the classic engine pays nothing.

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
