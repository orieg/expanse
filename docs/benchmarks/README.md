# Comparative benchmark suites

One directory per suite, each self-contained: `README.md` (results),
`METHODOLOGY.md` (pre-registered hypotheses, claims ceiling, expected losses),
`run.sh` (one-command reproduction on the reference host), `scripts/`
(JSON → tables and charts) and `results/` (raw artifacts and dual-theme charts).

Start here to find a benchmark. The table below is generated from
`.github/bench-suites.json`, so a `/benchmark` token and the suite that owns its
results cannot drift apart.

**Where the numbers live.** Each suite's `README.md` is the authority on its own
cells; `docs/BENCHMARKING.md` carries the methodology every suite obeys and the
core engine instruments. Top-level `results/baseline_*.json` is reserved for CI
regression-gate baselines and is not suite output.

**Timed harnesses that are not benchmark targets.** Eleven harnesses under
`crates/expanse/examples/` are `fn main` drivers rather than criterion targets —
built for direct execution under `perf stat` (`perf_point_lookup`), Valgrind
probes (`avx512_probe`) and deterministic memory accounting (`bytes_per_key`,
which gates the `memory-budget` CI job). `scripts/check_bench_shapes.py`
classifies them as *Group 5: Standalone Examples & Profile Drivers* and holds
them to the same workload-shape declarations. The split is intentional: they are
not reachable as `/benchmark` tokens and so do not appear below.

**Cross-language binding harnesses** are documented separately in
[`../BINDINGS_BENCHMARKS.md`](../BINDINGS_BENCHMARKS.md).

<!-- BEGIN GENERATED: bench-suites -->
<!-- Generated from `.github/bench-suites.json` by
     `python3 scripts/check_bench_suites.py --write`. Do not hand-edit:
     the `lint` CI job fails when this block and the manifest disagree. -->

| Suite | `/benchmark` tokens | Instrument | What it covers |
|---|---|---|---|
| [`art_comparison/`](art_comparison/README.md) | `art_insert`, `art_lookup_hit`, `art_lookup_miss`, `art_memory`, `art_scan`, `art_small_payload` | wall-clock | Expanse vs. Adaptive Radix Tree (ART): Empirical Benchmark Suite |
| [`avx512/`](avx512/README.md) | `avx512_bitmap` | wall-clock | AVX-512 `vpopcntq` for `Bitmap256` cardinality |
| [`embedded/`](embedded/README.md) | `embedded`, `embedded_memtable` | wall-clock | Embedded memtable shapes (host suite) |
| [`hashbrown_comparison/`](hashbrown_comparison/README.md) | `hashbrown_container_dists`, `hashbrown_memory_alloc`, `hashbrown_native_suite`, `hashbrown_tail_latency`, `hashbrown_ycsb` | wall-clock | Expanse vs. Hashbrown vs. BTreeMap: Empirical Comparative Benchmark Suite |
| [`hot_comparison/`](hot_comparison/README.md) | `hot_concurrent`, `hot_latency`, `hot_memory_curve`, `hot_string_latency`, `hot_string_memory` | wall-clock | Expanse vs. HOT (Height Optimized Trie): Empirical Benchmark Suite |
| [`llm_inference/`](llm_inference/README.md) | `bench_grammar_masks`, `bench_llm_datastore` | wall-clock | LLM Inference & Speculative Decoding Benchmark — Expanse vs Industry Baselines |
| [`redis_zset_engine/`](redis_zset_engine/README.md) | `zset_memory`, `zset_range`, `zset_rank`, `zset_zadd` | wall-clock | Redis ZSET Engine: Expanse dual-trie sorted set vs SkipList + Dict |
| [`rocksdb_memtable/`](rocksdb_memtable/README.md) | `rocksdb` | wall-clock | RocksDB MemTable suite: results and how to read them |
| [`search_inverted_index/`](search_inverted_index/README.md) | `search_boolean`, `search_instructions`, `search_memory`, `search_wand` | Callgrind + wall-clock | Search / Inverted-Index Benchmark: ExpanseSet vs Roaring |
| [`set_algebra/`](set_algebra/README.md) | `domain` | wall-clock | Set algebra — engine kernels and the interned set domain |
| [`wasm/`](wasm/README.md) | `wasm_fuel` | wasm fuel | WebAssembly suite: exact fuel on wasm32 and wasm64, and a Node wall-clock harness |

### Core engine instruments (no comparative suite directory)

These run from the engine crates and publish into `docs/BENCHMARKING.md`
rather than a suite directory. They are listed so that a token without a
suite is visible rather than silently absent from this index.

| Token | Instrument | What it runs |
|---|---|---|
| `all` | Callgrind | Default for a bare `/bench`: dual-pass Callgrind over `instructions` and `vs_stock`, the two B/key examples, the paired `bench_vs_libjudy` wall-clock comparison, and the fast comparative sweep. |
| `batch_lookup` | wall-clock | Interleave-width sweep for the batched descent, on a cold-DRAM population and a cache-resident control. |
| `comparative` | wall-clock | Wall-clock head-to-head against hashbrown / BTreeMap, with the `bench_report.py --quick` markdown table. |
| `compare` | wall-clock | Standing container comparison harness across the core map and set types. |
| `concurrency` | wall-clock | `Sync*` wall-clock scaling instrument on a reduced thread/workload sweep; report-only, never gating. |
| `domain_aarch64` | wall-clock | Interned set domain on aarch64-apple-darwin, wall clock. Indicative cross-architecture check on the ingestion and resolution results; gates nothing. |
| `extended` | Callgrind | Everything in `all`, plus the multi-population scaling sweep and the microarchitecture target-CPU matrix (`bench_report.py --extended --arch-sweep`). |
| `instructions` | Callgrind | Core deterministic Callgrind instruction counters plus the 64-bit and 32-bit B/key examples, dual-pass against the base ref. |
| `large_values` | wall-clock | Blob-arena storage paths for values above the immediate capacity. |
| `point_lookup_counters` | `perf stat` | Hardware performance counters (`perf stat`) over the random point-lookup path — `probe` minus `build`, with a BCa 95% interval per counter. Diagnostic only: it gates nothing, and it is the instrument the Callgrind `Ir` gate structurally cannot be. |
| `python_concurrency` | wall-clock | Python multi-core read scaling across the pyo3 `py.detach` GIL-releasing path, against a GIL-serialised `dict` twin (`bindings/python/bench_concurrency.py`). |
| `smoke_instructions` | Callgrind | Scaled-down Callgrind smoke counters, dual-pass against the base ref. The same instrument as the `callgrind-smoke` CI job, on the reference host. |
| `vs_libjudy` | wall-clock | Paired wall-clock comparison of `libexpanse` against a dlopen'd stock libjudy through the identical C surface, arms interleaved per round (`bench_vs_libjudy`). |
| `vs_stock` | Callgrind | C ABI drop-in parity against the stock oracle (`expanse-capi`), dual-pass against the base ref. |
| `ycsb` | wall-clock | YCSB core workloads on the 64-bit map. |
<!-- END GENERATED: bench-suites -->
