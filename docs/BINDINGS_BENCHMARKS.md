# Cross-Language Comparative Benchmark Suite

Expanse provides modern, clean-room Judy arrays and digital tries with cache-line-tuned node geometries, inline value packing, and O(1) rank/select/range queries across 9 major runtime environments.

This benchmark suite measures head-to-head lookup latency, insertion throughput, iteration speed, and memory density against native standard library collections in each ecosystem.

---

## Ecosystem Baseline Matrix

Every row below names the harness that backs it. Cells marked **(target)** are design goals, not measurements. No memory-advantage multipliers are quoted here: the harness estate shipped with silent-fallback and hardcoded-constant defects (#373, fixed on this branch), so all cross-language memory comparisons are **unverified until the next nightly baseline run** ([#382](https://github.com/orieg/expanse/issues/382)) completes on the repaired harnesses. Verified numbers will live in the nightly `bindings-baseline` artifact, not in this file.

| Runtime Environment | Expanse Target | Industry Baseline | Harness | Memory Comparison Status |
|:---|:---|:---|:---|:---|
| **Node.js / Bun / Deno** | `@orieg/expanse` (`ExpanseMap`, `ExpanseSet`) | Native V8 `Map`, `Set` | `crates/expanse-node/bench.js` | Expanse: native arena bytes (`memUsed()`); baseline: heap delta, `null` when unmeasurable (unverified until next baseline run) |
| **WebAssembly / Edge** | `@orieg/expanse-wasm` (`WasmExpanseMap`) | JavaScript `Map` / `Set` | `crates/expanse-wasm/tests/bench.js` | Throughput only; linear-memory density reporting (target) |
| **Go** | `github.com/orieg/expanse/bindings/go` | Go `map[uint64]uint64` | `bindings/go/expanse_bench_test.go` | `B/op`/`allocs/op` from `-benchmem`: Expanse's ~0 B/op means GC-invisible off-heap storage — the trie's native arena bytes are NOT reported by this harness |
| **Python** | `expanse_trie` (`ExpanseMap`, `ExpanseSet`) | Python `dict`, `set` | `bindings/python/bench.py` | Expanse: `mem_used()`; baseline: tracemalloc peak over a 10k-key sample (unverified until next baseline run) |
| **PHP** | `orieg/expanse` (`Expanse\Map`, `Expanse\Set`) | PHP `array` (Zend HashTable) | `bindings/php/bench.php` (requires `-d ffi.enable=1`; refuses the pure-PHP fallback driver) | Expanse: `memUsed()` via FFI; baseline: `memory_get_usage()` delta, `null` when unmeasurable (unverified until next baseline run) |
| **Ruby** | `expanse` gem (`Expanse::Map`, `Expanse::Set`) | Ruby `Hash`, `Set` | `bindings/ruby/bench.rb` | Expanse: `mem_used` (native arena); baseline: shallow `ObjectSpace.memsize_of`, flagged `bytes_per_key_estimated` (unverified until next baseline run) |
| **Java 22+** | `expanse-java` (Panama FFM) | `java.util.HashMap`, `TreeMap` | `bindings/java/.../ExpanseBenchmark.java` | Expanse: `memoryUsed()` (off-heap segment); baseline: GC'd heap delta, flagged approximate, `null` when unmeasurable (unverified until next baseline run) |
| **.NET 8/9** | `Expanse.NET` | `Dictionary<ulong, ulong>` | `bindings/dotnet/.../ExpanseBenchmark.cs` | Expanse: `MemoryUsed` (unmanaged); baseline: `GC.GetTotalMemory(true)` delta, flagged approximate, `null` when unmeasurable (unverified until next baseline run) |
| **RocksDB** | `ExpanseMemTableRepFactory` | `SkipListRepFactory` | `integrations/rocksdb/benches/` | See `integrations/rocksdb/README.md` — the single source for that comparison (being re-baselined under #372; no density number is duplicated here) |

Accounting is intentionally **asymmetric between the two columns of each row**: Expanse reports exact native-arena bytes while runtime baselines can only be observed via heap deltas or allocator probes. Each harness therefore labels how its baseline number was obtained (`bytes_per_key_estimated` / `bytes_per_key_approximate`) and emits `null` — never a constant — when the observation fails.

---

## Running Benchmarks

### 1. Unified Cross-Language Runner
Run all locally available binding benchmarks and generate a unified report. The
orchestrator discovers each ecosystem's toolchain (`go`, `mvn`, `dotnet`, `node`,
`python3`, `php`, `ruby`) independently, so any subset of the 8 runtimes present
on the host runs; the rest are skipped with a `[WARN]`.

Prerequisites per runtime:
- **go, java, dotnet, ruby, php**: the release-built C ABI artifact — `cargo build --release -p expanse-capi` (go/java/dotnet load it directly; the Ruby Fiddle binding and the PHP FFI driver resolve it from `target/release/`).
- **node**: the napi addon — `cargo build --release -p expanse-node` (the `index.js` loader falls back to `target/release/`).
- **python**: the PyO3 extension installed — `pip install .` from the repo root (maturin).
- **wasm**: a Node-requirable pkg — `wasm-pack build --release --target nodejs crates/expanse-wasm`. The wasm bench exits non-zero if `pkg/` is absent; it never falls back to a JS mock.
- **php**: the orchestrator passes `-d ffi.enable=1` itself; `bench.php` exits non-zero if only the pure-PHP fallback driver is available.

```bash
cargo build --release -p expanse-capi   # required by go, java, dotnet, ruby, php

python3 scripts/bench_bindings.py

# Quick mode (N = 10,000-20,000 depending on runtime, for fast feedback):
python3 scripts/bench_bindings.py --quick

# Emit machine-readable JSON:
python3 scripts/bench_bindings.py --json

# Only specific runtimes:
python3 scripts/bench_bindings.py --runtimes go java dotnet

# Orchestrator self-checks (parsing, coverage-loss detection, null-memory handling):
python3 scripts/bench_bindings.py --self-test
```

Baseline flags: `--check-baseline <json>` compares against a saved baseline and reports **coverage loss** (a runtime present in the baseline that produced no results tonight) as explicit ⚠️ rows; `--save-baseline <json>` re-saves the baseline, carrying forward the old entries of missing runtimes (marked `carried_forward_from_baseline`) unless `--prune-missing` is passed.

### 2. Individual Binding Harnesses

#### Node.js (`@orieg/expanse`):
```bash
cargo build --release -p expanse-node
cd crates/expanse-node
npm run bench
node bench.js --quick --json
```

#### WebAssembly (`@orieg/expanse-wasm`):
```bash
wasm-pack build --release --target nodejs crates/expanse-wasm
cd crates/expanse-wasm
node tests/bench.js --quick
```
The bench refuses to run (exit 1 with a build hint) when `pkg/` is missing — it never silently benchmarks a JS `Map` mock.

#### Go:
```bash
cd bindings/go
export LD_LIBRARY_PATH="$PWD/../../target/release:$LD_LIBRARY_PATH"   # DYLD_LIBRARY_PATH on macOS
export CGO_LDFLAGS_ALLOW=".*"
go test -run '^$' -bench . -benchmem -benchtime=1s .    # orchestrator default; --quick uses -benchtime=100ms
```
`scripts/bench_bindings.py` parses `iterations`/`ns/op`/`B/op`/`allocs/op` directly from this text
output (no `--json` flag exists for `go test`); see `_parse_go_bench_output()`. The Go harness
covers the `random` distribution only, and its `pop` field describes the fixed 100,000-key lookup
maps (the insert benches insert `b.N` keys, recorded as `*_iterations`) — the JSON marks this
`pop_configured_not_measured`.

#### Python:
```bash
pip install .          # from the repo root (maturin builds expanse_trie)
cd bindings/python
python3 bench.py --quick --json
```

#### PHP:
```bash
cd bindings/php
php -d ffi.enable=1 bench.php --quick --json
```
Without `ffi.enable=1` (or the native extension) the bench exits non-zero rather than measuring the pure-PHP array fallback shim.

#### Ruby:
```bash
cd bindings/ruby
ruby -Ilib bench.rb --quick --json    # needs target/release/libexpanse.* (cargo build --release -p expanse-capi)
```

#### Java 22+ Panama:
```bash
cd bindings/java
mvn -q test-compile exec:java \
  -Dexec.mainClass="io.github.orieg.expanse.ExpanseBenchmark" \
  -Dexec.classpathScope=test \
  -Dexec.args="--quick --json" \
  -Dexpanse.library.path="$PWD/../../target/release/libexpanse.dylib"  # .so on Linux, expanse.dll on Windows
```
`ExpanseBenchmark.main()` already accepts `--quick`/`--pop N`/`--json` and prints a
single JSON line (run unforked via `exec:java`, not `mvn test`'s Surefire harness, so
stdout isn't wrapped in a test report).

#### .NET 8/9:
```bash
cd bindings/dotnet
export EXPANSE_LIB_DIR="$PWD/../../target/release"
EXPANSE_BENCH_JSON=1 EXPANSE_BENCH_QUICK=1 dotnet test tests/Expanse.NET.Tests/Expanse.NET.Tests.csproj \
  -c Release -f net9.0 --filter "FullyQualifiedName~ExpanseBenchmark" --logger "console;verbosity=normal"
```
`RunComparativeBenchmark` is an xUnit `[Fact]`, which `dotnet test` gives no CLI-arg
passthrough into — `EXPANSE_BENCH_QUICK`/`EXPANSE_BENCH_JSON` are the equivalent knobs.
In JSON mode the result line is written via `Console.WriteLine` with an
`##EXPANSE_BENCH_JSON##` marker prefix so it survives VSTest's console logger
interleaving unrelated xUnit diagnostic lines with stdout.

---

## Experimental Discipline & CI Integration

1. **Deterministic Key Generation**: all eight harnesses (node, wasm, go, python, php, ruby, java, dotnet) generate keys with the same `XorShift64` PRNG — seed `0x0DDB_1A5E_5EED_0001`, shifts 13/7/17, logical right shift — verified to produce bit-identical streams across languages (first 5 values cross-checked in python/node/php/ruby/go on this branch, #373). Probe-shuffle *order* still differs per language (each uses its own seeded shuffle or reversal), so per-language probe sequences are deterministic but not cross-language identical.
2. **Distribution Sweeps**: node, python, php, ruby, java, and dotnet sweep `random` / `sequential` / `clustered`; wasm sweeps `random` / `sequential`; **the Go harness covers `random` only** (it is `go test -bench` based, with fixed lookup populations).
3. **CI & Nightly Gating — what actually runs**:
   - **Per-PR CI** (`ci.yml`): crash-check smoke runs of individual harnesses on a hosted runner (`node bench.js --quick`, `php -d ffi.enable=1 bench.php --quick`, `python3 bench.py --quick`). These verify the harnesses execute — they do **not** gate on performance numbers.
   - **Nightly** (`nightly.yml`, `bench-report` job): builds `libexpanse` (release), the node addon, the python extension, and the wasm pkg, then runs `scripts/bench_bindings.py --quick` against the rolling `bindings-baseline` artifact. Regressions beyond 25% throughput / 10% memory are reported; the baseline **re-saves every night**, so the threshold ratchets against the previous night, not a fixed anchor. Coverage loss (a runtime disappearing from the results) is reported as ⚠️ rows and the missing runtime's old baseline entry is preserved. Hosted-runner numbers are noisy — they gate for gross regressions and crashes, and are **not publishable performance figures** (see `docs/BENCHMARKING.md` for the bare-metal methodology).
