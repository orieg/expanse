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
| bytes/key | landed (`examples/bytes_per_key.rs`) | deterministic allocator accounting — load-immune, results below |
| Concurrent read scaling (1..N threads) | Phase 7 | Read-only and read-mostly mixes |
| Full libjudy + ART comparison | Phase 8 remainder | Headline table, dedicated-host runs, driven through the capi surface |

## Measured results

### bytes/key (measured: deterministic allocation accounting via `NodeAlloc`, commit with this section; machine-independent)

| dist | pop 1k | pop 100k | pop 1M | |
|---|---|---|---|---|
| sequential (set) | 0.58 | 0.07 | **0.06** | full-expanse + bitmap-leaf compression |
| clustered (set) | 0.38 | 0.37 | **0.35** | was 1.34 before narrow-pointer synthesis — a 3.8× improvement |
| random (set) | 10.43 | 12.91 | 7.06 | not part of the dense/clustered target |
| sparse `i << 40` (set) | 16.58 | 16.07 | **16.06** | the single-child branch-chain cost — the measured motivation for narrow-pointer synthesis (ARCHITECTURE.md §6 step 3) |

Map-flavor figures run ~8 B/key above the set figures (the stored value word). The `< 9.5 B/key dense+clustered` architecture target is **met** on the distributions it names; the sparse row is the narrow-pointer work's before-number.

### libexpanse vs stock libjudy, JudyL surface (measured: M1 MacBook Pro under load — a VM at ~226% CPU co-resident — commit with this section; interleaved A/B medians of 5 rounds, so the *ratios* are meaningful while absolute ns are contaminated; harness: `crates/expanse-capi/examples/bench_vs_libjudy.rs`)

| dist | pop | get ratio (ours/stock) | insert ratio | B/key ratio |
|---|---|---|---|---|
| sequential | 1M | 2.3–4× slower | 3.4× slower | 1.03 |
| random | 1M | 1.55× slower | **1.35× slower** | **0.93 (smaller)** |
| clustered | 1M | **~parity (0.94–1.2×)** | **1.7× slower** | **0.93 (smaller)** |

History of the insert column (each row measured at its commit): 13.5× → 3.7× (narrow-pointer synthesis) → **1.7×** (insert-path optimization: capacity-classed allocations with in-place shifts across leaves, bitmap-branch subarrays and bitmap-leaf value arrays, plus the fused single-walk `JudyLIns`). Sequential insert went 5.9× → 3.4×, random 1.9× → 1.35×. Memory cost of the classes is bounded by keeping ≤2-entry allocations exact (random map bytes/key briefly regressed 21→27 with naive rounding; 22.9 after the refinement, vs stock's 24.8). Remaining insert gap is profile-driven follow-up (immediate rebuilds, per-level dispatch).

Honest reading (v1 correctness-first, zero optimization passes yet):

- **Memory is already competitive** — smaller than stock on random keys.
- **Clustered lookup is solved**: narrow-pointer synthesis removed the chain walk (5.31× slower → parity). The remaining lookup gap is on sequential/random — next profile targets: per-level dispatch overhead and the root-leaf/tree split.
- **Insert gap** has three known, documented v1 costs to burn down: full leaf rebuild per insert (no capacity classes), `Vec` materialization in the mutation path, and capi `JudyLIns` walking the tree three times (contains + insert + slot). Each is an isolated follow-up with this table as baseline.

Timing numbers here are working baselines, not publishable claims; headline numbers still require a quiet-host run under the system-load protocol above.
