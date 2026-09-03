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

## On-device ESP32 harvest (`esp32.json`)

[`esp32.json`](esp32.json) is a different experiment from `results.json` above
and the two must not be read against each other: it records **CPU cycles on a
microcontroller**, where the host suite records **wall-clock nanoseconds on a
desktop**. It is produced by `integrations/esp32/`, which runs the component's
Unity cases as a gate and then the benchmark runner in
`components/expanse/test/bench_esp32_memtable.c`, against the §8.3 comparison
twins in `components/expanse/test/twin_containers.h`.

- **Provenance**: ESP32-D0WD-V3 rev v3.1, 2 cores, 160 MHz; ESP-IDF
  `v6.0-dev-2980-gab149384e1`; Xtensa Rust 1.97.0.0; `-O2`; engine
  `0.5.0-dev (v0.5.0-91-ge310a9e7)`, commit `e310a9e7`; 10 repetitions per arm
  in a single boot. The `provenance` and `stack` objects in the file carry the
  same facts as the board reported them, and every published number derives
  from them (§8.2).
- **Serial capture can drop a line.** The harvester reports each arm's
  `sample_count`; check it reads 10 before quoting an arm. In the #622 control
  capture one line was lost, leaving
  `esp32_tsdb_aggregate_500/hash_open_addressing` at n=500, pop=500 with 9
  repetitions (median 62.15, unaffected at the 0.03% level, and on an arm
  published as unattributed). Nothing in the committed treatment artifact is
  short.
- **The figure to quote is the `median`.** Each arm records `min`, `median`,
  `mean`, `max`, a `spread_ratio` and a `contaminated` flag alongside the BCa
  95% interval on the mean. A single repetition whose timed window catches a
  FreeRTOS tick or a flash-cache miss storm moves the mean by more than any
  code change this suite measures — one observed arm read
  `mean 2,830` against `median 1,355` on C source that had not changed — so
  the mean is reported for §8.4 continuity but is not the headline. An arm
  flagged `contaminated` (slowest repetition more than 2× its median) should
  not have its mean compared against anything.
- **Run-to-run drift is per arm, not global, and on some arms it is the whole
  signal.** The twin containers are byte-identical C across builds and still
  move between two flashes -- 0.07% at the median, up to ~20% on the smallest
  count arms -- because binary layout changes what this part's flash cache
  holds. Compare an Expanse delta against the largest twin drift **on its own
  arm**. Necessary, but not sufficient: two links of the engine with
  byte-identical engine source moved `esp32_ble_ttl_eviction` at pop=500 by
  **+57%** while the twins beside it barely moved, so those arms cannot carry a
  regression verdict without controlling layout. An arm's result is credible
  when it reproduces across *several* links; the ingest arm has, the pop=500
  eviction arms have not. Details and numbers: `docs/design/32-bit-embedded.md`
  §8.1.3.
- **A missing arm is a finding, not a gap.** If the harness cannot construct an
  arm -- a twin whose table needs a contiguous block the fragmented heap no
  longer has, say -- it prints a `{"skipped": ...}` record with the free heap
  and largest block at that moment, and the harvester surfaces it as a warning
  and keeps it in `esp32.json` under `skipped`. A table with a hole and nothing
  to say why is the fail-quiet outcome §8.1 forbids.
- **The engine must be rebuilt before it is measured.** The component's
  `libexpanse.a` custom command lists its Rust sources as dependencies so a
  code change re-invokes cargo; before that was fixed, CMake saw the archive
  file and skipped the rebuild, so a harvest taken after a Rust change could
  silently measure the previous engine. The app prints `expanse_version()` in
  its provenance line for exactly this reason -- check it names the commit you
  meant to measure before harvesting.
- **Regenerate**: capture a monitor log to a gitignored scratch path (§8.5),
  then
  `python3 scripts/esp32_bench_harvest.py --input <log> --out <report.md> --emit-json docs/benchmarks/embedded/esp32.json`
  followed by `python3 scripts/generate_embedded_svg.py --on-device`, which
  writes `docs/assets/bench_esp32_ondevice.svg`. Replace the file wholesale;
  never splice new arms into a previous run's twin numbers (§8.3).
- **Coverage**: `esp32` only. The RISC-V parts (C3, C6, H2, P4) have not been
  run on hardware; #579 tracks that.

This directory has no `run.sh`: the suite runs on the benchmark host via the
`/benchmark embedded_memtable` PR comment or `workflow_dispatch`
(`docs/BENCHMARKING.md` §3–4).
