# Benchmarking Guidelines

> Canonical benchmarking doc. Design targets: [ARCHITECTURE.md](ARCHITECTURE.md) §6 · Testing: [TESTING.md](TESTING.md)

Performance claims are this project's reason to exist, so they follow the strictest discipline in the repo: **no claim without a measurement, no measurement without methodology.**

## Harness and metrics

- Criterion benches live in `crates/judy/benches/`; C-comparison benches drive both sides through the C surface once `judy-capi` exports it.
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

## Bench matrix (populated as phases land)

| Bench | Lands with | Notes |
|---|---|---|
| `bits` microbenches (popcount/rank/SIMD find vs fallback) | Phase 2 | Criterion-only; guards against fallback regressions |
| Lookup latency grid | Phase 4 | First numbers vs BTreeMap/HashMap |
| Insert/delete throughput, bytes/key | Phase 6 | Adds allocator instrumentation |
| Concurrent read scaling (1..N threads) | Phase 7 | Read-only and read-mostly mixes |
| Full libjudy + ART comparison | Phase 8 | Headline table, dedicated-host runs |
