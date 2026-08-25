# Cross-Language Comparative Benchmark Suite

Expanse provides modern, clean-room Judy arrays and digital tries with cache-line-tuned node geometries, inline value packing, and O(1) rank/select/range queries across 9 major runtime environments.

This benchmark suite measures head-to-head lookup latency, insertion throughput, iteration speed, and memory density against native standard library collections and industry baselines in each ecosystem.

---

## Ecosystem Baseline Matrix

| Runtime Environment | Expanse Target | Industry Baseline | Memory Advantage | Core Value Proposition |
|:---|:---|:---|:---|:---|
| **Node.js / Bun / Deno** | `@orieg/expanse` (`ExpanseMap`, `ExpanseSet`) | Native V8 `Map`, `Set` | **3× – 8× less RAM** (~8–22 B vs ~64–180 B) | Eliminates V8 heap bloat & GC pause lag during multi-million key sweeps. |
| **WebAssembly / Edge** | `@orieg/expanse-wasm` (`WasmExpanseMap`) | JavaScript `Map` / `Set` | **Linear Memory Density** | Fits within 128 MB Worker memory ceilings with zero GC overhead. |
| **Go** | `github.com/orieg/expanse/bindings/go` | Go `map[uint64]uint64` | **0 Heap Allocs** (0 B/op) | Off-heap storage completely invisible to the Go GC and scavenger. |
| **Python** | `expanse_trie` (`ExpanseMap`, `ExpanseSet`) | Python `dict`, `set` | **3× – 8× less RAM** (~8–22 B vs ~72 B) | GIL-released batch mutations and compressed integer trie indexing. |
| **PHP** | `orieg/expanse` (`Expanse\Map`, `Expanse\Set`) | PHP `array` (Zend HashTable) | **3× – 8× less RAM** (~8–20 B vs ~65 B) | Persistent process memory scaling under FrankenPHP, RoadRunner, Swoole. |
| **Ruby** | `expanse` gem (`Expanse::Map`, `Expanse::Set`) | Ruby `Hash`, `Set` | **3× – 7× less RAM** (~8–22 B vs ~64 B) | Dense integer indexing without Ruby GC object allocations. |
| **Java 22+** | `expanse-java` (Panama FFM) | `java.util.HashMap`, `TreeMap` | **Zero JVM Heap Overhead** | Panama off-heap memory segment storage without JNI marshalling cost. |
| **.NET 8/9** | `Expanse.NET` | `Dictionary<ulong, ulong>` | **0 Gen 0/1/2 GC Pressure** | Unmanaged memory backing without garbage collection pauses. |
| **RocksDB** | `ExpanseMemTableRepFactory` | `SkipListRepFactory` | **2.5× higher key density** | Drastically reduced L0 flush frequency and write stalls. |

---

## Running Benchmarks

### 1. Unified Cross-Language Runner
Run all locally available binding benchmarks and generate a unified report:
```bash
python3 scripts/bench_bindings.py

# Quick mode (N = 20,000 for fast feedback):
python3 scripts/bench_bindings.py --quick

# Emit machine-readable JSON:
python3 scripts/bench_bindings.py --json
```

### 2. Individual Binding Harnesses

#### Node.js (`@orieg/expanse`):
```bash
cd crates/expanse-node
npm run bench
node bench.js --quick --json
```

#### WebAssembly (`@orieg/expanse-wasm`):
```bash
cd crates/expanse-wasm
npm run bench
node tests/bench.js --quick
```

#### Go:
```bash
cd bindings/go
go test -v -bench=. -benchmem -benchtime=500ms .
```

#### Python:
```bash
cd bindings/python
python3 bench.py --quick --json
```

#### PHP:
```bash
cd bindings/php
php bench.php --quick --json
```

#### Ruby:
```bash
cd bindings/ruby
ruby -Ilib bench.rb --quick --json
```

#### Java 22+ Panama:
```bash
cd bindings/java
mvn test-compile exec:java -Dexec.mainClass="io.github.orieg.expanse.ExpanseBenchmark"
```

#### .NET 8/9:
```bash
cd bindings/dotnet
dotnet test --filter "FullyQualifiedName~ExpanseBenchmark" --logger "console;verbosity=normal"
```

---

## Experimental Discipline & CI Integration

All binding benchmark harnesses adhere to the global research discipline:
1. **Deterministic Key Generation**: Powered by identical `XorShift64` PRNG sequences across all languages to eliminate generator variance.
2. **Distribution Sweeps**: Measurements are partitioned across `random`, `sequential`, and `clustered` distributions to measure best-case bitmap compression and worst-case sparse tree traversal.
3. **CI Smoke Gating**: Each binding's benchmark harness is continuously executed in GitHub Actions CI to guarantee zero regressions in binding overhead and runtime interoperability.
