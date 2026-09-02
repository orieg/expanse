# Embedded memtable shapes (host suite)

The host-side half of the embedded lane: the `embedded_memtable` criterion
bench (`crates/expanse/benches/embedded_memtable.rs`, #556) — sensor time-series
ingest and flush, CAN-ID dispatch lookup, BLE tracker point lookup and TTL
eviction in the bulk and steady shapes — against `std::BTreeMap` and
`hashbrown::HashMap`, run on the dedicated benchmark host through the
`embedded_memtable` workflow dispatch.

- **Results and their reading**: `docs/DATABASE.md` §5.4 (the pre-registered
  losses on ingest and point lookup, the steady-state eviction win, the bulk
  eviction loss, and the `remove_range` correction record), with the derived
  chart `docs/assets/bench_embedded.svg`.
- **Artifact**: [`results.json`](results.json) — `meta` (population, lookups
  per iteration, the memory model used for panel 1) and `wallclock_ns` with
  BCa 95% bootstrap intervals per arm, harvested from the dispatched run
  ([run 33567182415](https://github.com/orieg/expanse/actions/runs/33567182415),
  commit `13ee3d92`; read it per `docs/BENCHMARKING.md` §4a, "Reading a
  dispatched run honestly").
- **Regenerate the chart**: `python3 scripts/generate_embedded_svg.py
  --from-baseline <baseline-embedded_memtable.json>` rewrites this file and
  `docs/assets/bench_embedded.svg`; every rendered number is derived from the
  JSON (AGENTS.md §8.2).
- **Pre-registration**: the hypothesis and expected-loss matrix are in #556
  (§3.2) and are quoted, not reconciled, in `docs/DATABASE.md` §5.4.
- **On-target counterpart**: the same fixtures on real Cortex-M cores are the
  [`stm32h747/`](../stm32h747/README.md) suite.

This directory has no `run.sh`: the suite runs on the benchmark host via the
`/benchmark embedded_memtable` PR comment or `workflow_dispatch`
(`docs/BENCHMARKING.md` §3–4).
