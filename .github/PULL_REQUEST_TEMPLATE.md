<!--
Keep the prose impersonal (AGENTS.md §7): what changed, where (path / SHA /
issue), and status. No time estimates, no hostnames or home paths.
-->

## What & why

<!-- One paragraph. Reference the issue or design-doc section this implements. -->

## Verification (AGENTS.md §5 — paste evidence, not adjectives)

- [ ] `scripts/gate.sh` passed locally (fmt · clippy `-D warnings` · workspace tests with `PROPTEST_CASES=500` · repo scripts · docs hygiene; binding crates needing external runtimes are covered by CI)
- [ ] Every new `unsafe` block has a `// SAFETY:` rationale; new public items are documented
- [ ] Tests added/extended for the change (name them):
- [ ] Miri / loom / fuzz coverage considered (CI Tier-1 Miri runs on this PR; say if a new `unsafe` path needs a nightly Miri or loom model)

## Performance (AGENTS.md §6 / §8 — required for any change under `crates/expanse/src`)

- [ ] Callgrind instruction counts: no arm regressed > 0.1 % (paste the `perf_report.py` table from the `instruction-counts` job, or state "no hot-path change")
- [ ] Wall-clock claims carry `(measured: host, commit)` provenance resolving to a committed artifact, with a BCa 95 % CI where §8.4 applies; losing cells are published
- [ ] No pre-registered target was edited after the run (§8.7)

<!--
Intentional regression? Uncomment the next line and fill the reason (the CI
gate accepts ONLY this exact form), then add a "Performance Trade-off
Disclosure" section: regressed metric, load-bearing rationale, net win.

allow-regression: <reason>
-->

## Invariants (AGENTS.md §2 — tick only what applies, explain any "no")

- [ ] `Edge` stays 16 B / 8 B and `ValueSlot` one machine word; no fat slots, no metadata complected into index words
- [ ] Ordered digital-trie semantics, `Judy*` C ABI behaviour, and the JudyL slot-pointer contract unchanged
- [ ] No coarse locks; single-threaded paths stay zero-overhead

## Docs

- [ ] Canonical doc updated (`docs/ARCHITECTURE.md` / `ALGORITHMS.md` / `BENCHMARKING.md` / `CI.md` / `COMPAT.md` …) — no new standalone `.md`
