# Expanse vs. Masstree: Empirical Benchmark Suite

Head-to-head evaluation of `ExpanseMap`, `ExpanseStrMap`, `SyncExpanseMap` and
`SyncExpanseStrMap` against **Masstree**
([Mao, Kohler & Morris, EuroSys 2012](https://doi.org/10.1145/2168836.2168855)),
reached through a C++ FFI shim over the reference implementation
[`kohler/masstree-beta`](https://github.com/kohler/masstree-beta) at `1119842`.

> **Status: harness landed, not yet measured.** Pre-registration, locked
> constraints and the expected-losses matrix: [`METHODOLOGY.md`](METHODOLOGY.md).
> No Masstree figure exists on any registered harness; this page is filled by
> `scripts/tables.py` from `results/` once the reference-host run lands, and
> nothing in the methodology is edited to fit it (§8.7). Tracking issue:
> [#661](https://github.com/orieg/expanse/issues/661). Internal work; no external
> peer review.

## Reproduction

```bash
git submodule update --init --depth 1 third_party/hot third_party/masstree
docs/benchmarks/masstree_comparison/run.sh --concurrent        # everything
docs/benchmarks/masstree_comparison/run.sh --quick             # smoke, results/quick/
python3 docs/benchmarks/masstree_comparison/scripts/tables.py  # README tables from results/
python3 docs/benchmarks/masstree_comparison/scripts/generate_charts.py
```

The runner takes the host-wide benchmark lock and the P-core pin, runs the
validation gate (`masstree_validate`) first and fatally, then one process per
cell. Requires an x86-64 host with AVX2 and BMI2 (both arms are bound to one ISA
target, `METHODOLOGY.md` §3.5) and a C++17 toolchain; Masstree is compiled
without autoconf from `crates/expanse-hot-bench/cpp/masstree_config/config.h`.
