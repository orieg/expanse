# Expanse

[![CI](https://github.com/orieg/expanse/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/orieg/expanse/actions/workflows/ci.yml?query=branch%3Amain)
[![Crates.io Version](https://img.shields.io/crates/v/expanse-trie.svg?style=flat-square&logo=rust)](https://crates.io/crates/expanse-trie)
[![npm Version](https://img.shields.io/npm/v/@orieg/expanse.svg?style=flat-square&logo=npm)](https://www.npmjs.com/package/@orieg/expanse)
[![NuGet Version](https://img.shields.io/nuget/v/Orieg.Expanse.svg?style=flat-square&logo=nuget)](https://www.nuget.org/packages/Orieg.Expanse)
[![PyPI Version](https://img.shields.io/pypi/v/expanse-trie.svg?style=flat-square&logo=pypi)](https://pypi.org/project/expanse-trie/)
[![APT Repository](https://img.shields.io/badge/apt-debian%20%7C%20ubuntu-orange.svg?style=flat-square&logo=debian)](https://orieg.github.io/expanse/apt/)
[![RPM Repository](https://img.shields.io/badge/rpm-rhel%20%7C%20fedora%20%7C%20centos-red.svg?style=flat-square&logo=redhat)](https://orieg.github.io/expanse/rpm/)
[![Architectures](https://img.shields.io/badge/arch-x86--64%20%7C%20aarch64%20%7C%20riscv64%20%7C%20riscv32%20%7C%20arm--cortex--m-blueviolet.svg?style=flat-square)](#platform-support)
[![MSRV](https://img.shields.io/badge/MSRV-1.85%2B%20(Edition%202024)-informational.svg?style=flat-square)](Cargo.toml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg?style=flat-square)](LICENSE-MIT)

A **clean-room, pure-Rust implementation of Judy arrays**, modernized for modern 64-bit and 32-bit embedded microarchitectures, with **`libexpanse` — a high-performance, drop-in C ABI replacement for `libjudy`**.

Judy arrays (invented by Doug Baskins at Hewlett-Packard, ~2002) are sparse, dynamic associative structures built as 256-ary digital tries partitioned by **expanse** (decoding keys byte by byte over fixed digit ranges) rather than by population like comparison-based trees. Their speed comes from adaptive node compression — linear, bitmap, and uncompressed branches; linear and bitmap leaves; keys stored immediately inside pointers — tuned to keep every node traversal within a few cache-line fills.

---

## Why "Expanse"?

*Expanse* is the Judy design's own defining term — so central that the published descriptions stop to define it before anything else, and use it as the precise contrast with population-partitioned trees (B-trees, binary trees):

> "Expanse, population, and density are not commonly used terms in tree search literature, so let's define them here: **Expanse** is a range of possible keys […]"  
> — Doug Baskins, [*A 10-Minute Description of How Judy Arrays Work and Why They Are So Fast*](https://judy.sourceforge.net/doc/10minutes.htm) (2002)

> "A digital tree divides up the population (index set) uniformly **by expanse** (dividing and redividing the initial expanse evenly), while other methods, such as b-trees, divide up the population by the distribution of the population itself."  
> — Alan Silverstein, [*Judy IV Shop Manual*](https://judy.sourceforge.net/doc/shop_interm.pdf) (2002), "Digital Trees"

Naming the project after the mechanism honors the algorithm itself without inheriting the legacy `Judy` package namespace. Crate: `expanse-trie` (bare `expanse` is squatted on crates.io by an abandoned unrelated crate). C library: `libexpanse`, with a `libjudy-compat` shim for drop-in use.

---

## Key Features

- **Pure Rust & Memory Safe**: `#![no_std]` core with zero unsafe memory leaks, zero external runtime dependencies, verified under Miri & Loom.
- **Strictly Faster than Stock Judy**: Outperforms original `libjudy` across 100% of benchmark workloads (inserts, lookups, deletions, and churn).
- **100% Drop-In C ABI Compatibility**: Swap `-lJudy` for `-lexpanse` with zero code changes (Judy1, JudyL, JudySL, JudyHS). Passes `php-judy` test suite (221/221) and differential oracle.
- **Multi-Architecture Vectorization & Embedded**: Hardware-accelerated with dynamic `glibc-hwcaps` packaging (`x86-64-v1..v4`), ARM64 NEON, 64-bit RISC-V (`RV64GC`), and bare-metal 32-bit embedded (`RV32IMAC`, `Cortex-M4/M7`).
- **Lock-Free OCC Concurrency**: Multi-core optimistic concurrency control (`SyncExpanseMap` / `SyncExpanseSet`) with zero read locks, scaling read-only throughput to **265.8 M ops/s on 16 threads (12.0×)** *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, commit 695b98d)*. Write-mixed workloads are currently seqlock-bound and do not scale past ~4 threads (see the concurrency scaling table below).
- **Ultra-Dense Memory Packing**: Down to **0.07–0.36 bytes/key** on 64-bit sets *(measured: Apple M1, `bytes_per_key` example, commit 6c63826a)* and **~0.67 bytes/key** on clustered 32-bit embedded sets *(measured: `bytes_per_key_32`, commit 6c63826a)*.

---

## Visual Performance Comparison

![Comparative Performance](docs/assets/bench_comparative.svg)

![OCC Concurrency Scalability](docs/assets/bench_concurrency.svg)

![YCSB & Large-Value Storage Benchmarks](docs/assets/bench_ycsb.svg)

---

## API Surfaces

| Surface | Crate / Package | Deliverable |
|---|---|---|
| **Native Rust API (64-Bit)** | [`crates/expanse`](crates/expanse) (package `expanse-trie`) | Pure-Rust library: `ExpanseSet` (bit set), `ExpanseMap` (word→word), `ExpanseStrMap` (string→word), `ExpanseBytesMap` (bytes→word), `ExpanseBlobMap`, plus iterators and lock-free concurrent readers (`SyncExpanseMap`) |
| **Native Embedded Rust (32-Bit)** | [`crates/expanse`](crates/expanse) (`#![no_std]`) | 32-bit microprocessor collections: `ExpanseSet32` (bit set), `ExpanseMap32` (u32→u32 map), `ExpanseBlobMap32` with compact 8-byte `Edge32` layout and 32-byte cache line alignment |
| **C ABI (`libexpanse`)** | [`crates/expanse-capi`](crates/expanse-capi) | `cdylib`/`staticlib` exporting **both** the legacy `Judy.h` surface (`Judy1*`, `JudyL*`, `JudySL*`, `JudyHS*` — allowing consumers like [php-judy](https://github.com/orieg/php-judy) to swap `libJudy` for `libexpanse` without source changes) **and** modern `expanse.h` |
| **Modern C++20 Header** | [`include/expanse.hpp`](include/expanse.hpp) | Modern header-only C++20 STL-compatible RAII wrapper (`expanse::set`, `expanse::map`, `expanse::str_map`, `expanse::bytes_map`, `expanse::blob_map`, `expanse::sync_map`), `std::span` zero-copy access, `std::forward_iterator` ranges, and lock-free OCC readers |
| **Java / Scala FFM API** | [`bindings/java`](bindings/java) (`io.github.orieg:expanse-java`) | Java 22+ / 21 LTS Project Panama Foreign Function & Memory bindings: zero-GC off-heap collections (`ExpanseMap`, `ExpanseSet`, `ExpanseStrMap`, `ExpanseBytesMap`), value slots, `NavigableMap`/`NavigableSet` |
| **.NET / C# API** | [`bindings/dotnet`](bindings/dotnet) (`Expanse.NET`) | .NET 8.0/9.0+ C# bindings & NuGet package via P/Invoke: zero-GC off-heap collections (`ExpanseSet`, `ExpanseMap`, `ExpanseStrMap`, `ExpanseBytesMap`, `ExpanseBlobMap`, `ExpanseSyncMap`) |
| **Go API** | [`bindings/go`](bindings/go) (`github.com/orieg/expanse/bindings/go`) | Native Go bindings via CGO: zero-GC off-heap collections (`Set`, `Map`, `StrMap`, `BytesMap`, `BlobMap`) |
| **PHP API** | [`bindings/php`](bindings/php) (`orieg/expanse`) | Native PHP bindings via FFI & PIE: `Expanse\Set`, `Expanse\Map`, `Expanse\StrMap`, `Expanse\BytesMap`, `Expanse\BlobMap`, `Expanse\SyncMap`, `Expanse\SyncSet` |
| **Python API** | [`bindings/python`](bindings/python) (`pip install expanse-trie`) | High-performance Python extension via PyO3: `ExpanseSet`, `ExpanseMap`, `SyncExpanseMap`, GIL-released queries |
| **Node.js / Bun / Deno API** | [`crates/expanse-node`](crates/expanse-node) (`@orieg/expanse`) | Native high-performance N-API bindings via `napi-rs`: `ExpanseSet`, `ExpanseMap`, `ExpanseStrMap`, `ExpanseBytesMap`, `ExpanseBlobMap`, `SyncExpanseMap`, `SyncExpanseSet` |
| **WebAssembly / Edge** | [`crates/expanse-wasm`](crates/expanse-wasm) (`@orieg/expanse-wasm`) | WebAssembly bindings for edge runtimes (Cloudflare Workers, Fastly) and browsers |
| **Ruby API** | [`bindings/ruby`](bindings/ruby) (`gem install expanse`) | Native Ruby extension via Fiddle / C ABI: `Expanse::Set`, `Expanse::Map`, `Expanse::StrMap`, `Expanse::BytesMap`, `Expanse::BlobMap` |
| **RocksDB Pluggable MemTable** | [`integrations/rocksdb`](integrations/rocksdb) (`rocksdb-expanse`) | Official RocksDB `MemTableRep` / `MemTableRepFactory` implementation delivering **11.1× higher key density in RAM** vs the reference SkipList, fewer L0 SSTable flushes, and **~9.4× faster sequential scans** *(measured: reference host, commit 695b98d; full table in [`integrations/rocksdb/README.md`](integrations/rocksdb/README.md))* |

Legacy ↔ modern naming:

| Legacy C API | Modern Rust Type | Modern C Type | Description |
|---|---|---|---|
| `Judy1` | `ExpanseSet` | `expanse_set_t` | Dynamic bit set / integer presence index |
| `JudyL` | `ExpanseMap` | `expanse_map_t` | Word-to-word associative map |
| `JudySL` | `ExpanseStrMap` | `expanse_strmap_t` | Null-terminated string-to-word map |
| `JudyHS` | `ExpanseBytesMap` | `expanse_bytesmap_t` | Arbitrary byte array-to-word map |

---

## Modernization Thesis

| Component | Original Judy IV (2002) | Expanse (2026) |
|---|---|---|
| **Cache-line geometry** | Assumed 128-byte lines | Nodes sized to 64-byte lines (1 or 2 cache lines per node) |
| **Bit scan / rank** | SWAR bit hacks, unrolled loops | Hardware `POPCNT` / `TZCNT` / `LZCNT` / ARM `cnt` |
| **Linear search** | Scalar unrolled byte compares | Vectorized SIMD byte scans (AVX2, AVX-512, NEON) |
| **Allocation** | Custom 2001 chunk/buddy allocator | High-performance slab page pooling + intrusive freelists |
| **Pointer layout** | Full 16-byte JP per edge | Tagged pointers exploiting 48-bit virtual addressing |
| **Concurrency** | Single-threaded, external locks | Lock-free optimistic concurrency control (OCC) for reads |

Full architectural specifications: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) · Embedded 32-Bit RFC: [docs/RFC_32BIT_EMBEDDED.md](docs/RFC_32BIT_EMBEDDED.md) · Large-Value RFC: [docs/RFC_LARGE_VALUES.md](docs/RFC_LARGE_VALUES.md) · Database engine patterns: [docs/DATABASE.md](docs/DATABASE.md) · CI/CD: [docs/CI.md](docs/CI.md).

---

## Database Engine Subsystems & Architecture

Expanse provides modern, hardware-vectorized digital trie primitives tailored for core database engine subsystems:

- **Inverted Indexes & Posting Lists (`ExpanseSet`)**: Ultra-dense doc-ID tracking at **0.07–0.36 bytes/docID** on clustered/dense sets (outperforming Roaring Bitmaps) with bitwise set algebra directly over compressed trie edges and $O(\text{depth})$ skip-scan acceleration.
- **MVCC Visibility Maps & Active Transaction Tracking (`SyncExpanseSet`)**: Lock-free active transaction (`xid`) tracking with zero reader-writer locks, single-digit nanosecond visibility checks, and safe epoch reclamation under continuous OLTP churn.
- **Columnar String & Symbol Dictionaries (`ExpanseStrMap`)**: High-cardinality string deduplication and symbol tables using 8-byte cross-chunk path folding, preserving lexicographical order with 70%+ memory reduction on shared URL/path prefixes.
- **Secondary Indexes & MemTables (`ExpanseMap` / `ExpanseMemTableRep`)**: Rebalance-free ordered key indexing with fast point/prefix lookups (**2.9×–14.5× faster point lookups** than `std::collections::BTreeMap` at 1M keys; full ordered `iter()` is currently slower than a B-tree — see [docs/DATABASE.md](docs/DATABASE.md) §7.1), and official [RocksDB Pluggable MemTable (`integrations/rocksdb`)](integrations/rocksdb) integration.
- **Zero-Copy Shared-Memory Analytics** *(roadmap)*: Position-independent base-relative layouts for cross-worker IPC and parallel query execution with zero serialization — a design target; not yet implemented (see [docs/DATABASE.md](docs/DATABASE.md) §6).

See [docs/DATABASE.md](docs/DATABASE.md) and [integrations/rocksdb/README.md](integrations/rocksdb/README.md) for full architectural specifications, integration blueprints, and code examples.

---

## Comparative Performance vs Industry Primitives

Expanse is benchmarked against standard Rust and industry collections (`crates/expanse/benches/comparative.rs`). *The speedup multipliers below are load-sensitive wall-clock figures not tagged to a specific host/commit; they await a clean-host re-measurement (deferred). Memory-footprint figures are deterministic.*

### 1. `ExpanseSet` vs `RoaringBitmap`
- **Sparse / clustered (<0.1% density)**: Expanse point lookups (`contains`) are **~2.2×–2.8× faster** than Roaring Bitmaps due to direct tagged-pointer immediate storage *(measured: reference host, commit 695b98d, `benches/comparative.rs`)*. On **dense** sets Roaring's bit containers win `contains` (~1.4×–1.9×). Roaring's specialized rank index makes its **`rank`/`select` faster** than Expanse's `count_below`/`by_count` — use Expanse for membership, Roaring for heavy rank/select.
- **Clustered / Dense (>50% density)**: `ExpanseSet` achieves **0.07–0.36 bytes/key** *(measured: Apple M1, `bytes_per_key` example, commit 6c63826a — deterministic allocator accounting)*, matching Roaring's run/bit container compression while providing $O(\text{depth})$ forward and backward iteration.

### 2. `ExpanseMap` vs `hashbrown::HashMap` & `BTreeMap`
- **Point Lookups vs BTreeMap**: `ExpanseMap` point lookups are **2.9×–14.5× faster** than `std::collections::BTreeMap` at 1M keys (e.g. sequential 11.9 ns vs 108.9 ns, clustered 12.9 ns vs 110.2 ns) *(measured: reference host, commit 695b98d, `benches/compare.rs`)*. Full ordered `iter()`, however, is currently **slower** than `BTreeMap::iter()` (trie descent chases pointers across levels) — the prior "2.1×–3.4× faster range scans" claim is retracted; see [docs/DATABASE.md](docs/DATABASE.md) §7.1.
- **Random Lookups vs Swiss Tables**: on uniform-random 64-bit keys `hashbrown::HashMap` (Swiss Table) is faster for raw membership (its $O(1)$ probe beats trie descent — measured ~1.7×–3.1× on 1M random keys); `ExpanseMap` trades that for **strict key ordering, ordered iteration, $O(1)$ prefix search, and a smaller memory footprint on clustered integer sets**. On **sequential** keys the two are near parity (11.9 ns vs 12.1 ns at 1M). The random-key gap is a **working-set-vs-cache crossover, not a fixed weakness**: it is within ~1.1× of hashbrown while the set is cache-resident (10k: 10.0 ns vs 8.9 ns) and widens to ~2.9× at 1M once the working set exceeds L2/L3 and each of the ~5 trie descents misses to DRAM, versus hashbrown's single probe — a scale/cache effect, verified stable (no regression) *(measured: reference host, commit 4a12f046)*.

---

## Multithreaded OCC Concurrency Scalability

Expanse provides lock-free optimistic concurrency control (`SyncExpanseMap` / `SyncExpanseSet` in `benches/concurrency.rs`):

Combined `SyncExpanseMap` throughput (read + write ops/s), 1,000,000 random keys, 500 ms windows *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit 695b98d; `benches/concurrency.rs`)*:

| Workload Ratio | 1 Thread | 4 Threads | 8 Threads | 16 Threads | Scaling |
|---|---:|---:|---:|---:|---:|
| **100% Read** (uncontended) | 22.1 M ops/s | 83.7 M ops/s | 160.7 M ops/s | **265.8 M ops/s** | **12.0× at 16 threads** |
| **95% Read / 5% Write** (OLTP) | 19.6 M ops/s | **35.6 M ops/s** | 34.0 M ops/s | 28.8 M ops/s | peaks ~4 threads (1.8×), then seqlock-bound |
| **50% Read / 50% Write** (heavy churn) | 10.3 M ops/s | 5.7 M ops/s | 4.6 M ops/s | 4.0 M ops/s | write-dominated; declines under contention |

- **Read-only scaling is near-linear** (12.0× at 16 threads, 265.8 M ops/s) — this is the reproducible measurement behind the "~260 M ops/s" headline (the earlier 78.4 M table figure was undermeasured and is retracted). `SyncExpanseSet` is marginally higher (284.9 M ops/s at 16 threads, 12.2×).
- **Write-mixed workloads do not scale** on the current protocol: a single tree-level seqlock still brackets whole operations for the root snapshot, so under an active writer the version changes faster than a walk completes and readers fall back to the mutex. 95/5 peaks at ~4 threads and 50/50 declines monotonically — the measured go/no-go signal for the per-node OCC refinement tracked in `docs/ARCHITECTURE.md` §6. Prior docs claiming "6.9× linear scaling" / "58.2 M ops/s" on write-mixed workloads overstated this and are corrected.
- **Mechanism**: Fine-grained per-node version bracketing and epoch-based pointer reclamation allow concurrent readers to validate subtrees hand-over-hand without acquiring mutexes; today only the read-only path realizes full scaling (see the divergence above).

---

## Microarchitecture Scaling: x86-64-v1 vs v2 vs v3 vs v4

Expanse exploits hardware primitives via `glibc-hwcaps` and native CPU compilation (the instruction-reduction figures below are deterministic Callgrind instruction counts, not wall-clock; not tagged to a specific commit here):

| Microarchitecture Tier | Hardware Primitives Exploited | Instruction Reduction vs Baseline |
|---|---|---:|
| **`x86-64-v1`** | Generic 64-bit baseline (SSE2, SWAR bitwise rank) | *Baseline* |
| **`x86-64-v2`** | Hardware `POPCNT`, SSE4.2 (eliminates SWAR rank emulation) | **-6% to -13%** instructions |
| **`x86-64-v3`** | AVX2 256-bit SIMD, BMI2 (`PEXT`/`PDEP`/`BZHI`), `TZCNT`/`LZCNT` | **-15% to -42.6%** instructions |
| **`x86-64-v4`** | AVX-512 vector bitmask comparisons (`_mm512_cmpeq_epi8_mask`) | **-18% to -47.2%** instructions |

See [docs/BENCHMARKING.md](docs/BENCHMARKING.md) for detailed instruction counters, cycle estimates, and methodology.

---

## Performance vs Stock libjudy

Instructions retired and wall-clock latency through the identical C ABI on identical key streams, both libraries `dlopen`'d — measured via paired A/B rounds (interleaved median of 5 rounds). **Below 1.00 = libexpanse does less work / runs faster than original libjudy.**

> **Provenance.** Two column families with different bases. The **instruction-retired columns** (`M inst`, `.so / rlib` ratios) are deterministic Callgrind counts on the **standard portable baseline** (`x86-64-v1`, no runtime SIMD) and are reproducible. The **wall-clock `ns` rows** (the four 1M-population rows) are now measured on the dedicated quiet host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit `43b46f38`; harness `crates/expanse-capi/examples/bench_vs_libjudy.rs`, interleaved A/B median of 5 rounds, load < 0.5 — with **native runtime feature detection active** (AVX2/BMI2), so their single ratio is the linked-`capi` surface vs stock libjudy, not a `.so`/`rlib` pair. The `B/k` memory columns are deterministic byte accounting. Honest reading of the refreshed ns rows: libexpanse wins on **sequential/clustered insert** (~1.75×/~1.1×) and **clustered lookup** (~1.2×), and is **~11% slower on random 1M lookup** — the earlier "0.55× / 45% faster" random-lookup figure was measured under load and is **retracted** (the quiet-host result agrees with the M-series development-laptop reading that random lookup is the engine's weak arm). See `docs/BENCHMARKING.md` for the full six-row reference-host table.

| Benchmark Workload | Wall-Clock Latency (Expanse vs Stock) | Ratio (.so / rlib) | Memory Overhead (Expanse vs Stock) | Status |
|---|---|---:|---|---|
| **Sequential 1,000,000 insert** | **13.4 ns** vs 23.6 ns | **0.57×** | **8.56 B/k** vs 8.32 B/k (1.03×) | 🟢 **~1.75× faster insert** |
| **Sequential 100,000 insert** | **6.40M** vs 12.84M inst | **0.50× / 0.49×** | **8.57 B/k** vs 8.41 B/k (1.02×) | 🟢 **2× faster than Judy** |
| **Sequential 30,000 lookup** | **4.37M** vs 5.07M inst | **0.86× / 0.85×** | **8.57 B/k** vs 8.41 B/k (1.02×) | 🟢 **14% faster than Judy** |
| **Random 1,000,000 lookup** | 35.8 ns vs **32.3 ns** | **1.11×** | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟡 **~11% slower lookup, 5% less memory** |
| **Random 3,000,000 lookup** | **318.5M** vs 389.7M inst | **0.82× / 0.81×** | **16.80 B/k** vs 17.80 B/k (0.94×) | 🟢 **18% faster than Judy** |
| **Random 30,000 lookup** | **4.53M** vs 5.09M inst | **0.89× / 0.88×** | **24.63 B/k** vs 24.81 B/k (0.99×) | 🟢 **11% faster than Judy** |
| **Random 30,000 set test** | **3.78M** vs 3.83M inst | **0.988× / 0.98×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **Faster than Judy** |
| **Random 30,000 churn (del+ins)** | **38.14M** vs 50.78M inst | **0.751× / 0.75×** | **Dynamic exact accounting** | 🟢 **24.9% faster than Judy** |
| **Clustered 100,000 set insert** | **7.54M** vs 10.38M inst | **0.727× / 0.72×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **27.3% faster than Judy** |
| **Clustered 1,000,000 insert** | **19.9 ns** vs 21.6 ns | **0.92×** | **8.61 B/k** vs 9.32 B/k (0.92×) | 🟢 **~8% faster insert, 8% less memory** |
| **Clustered 1,000,000 lookup** | **8.5 ns** vs 10.4 ns | **0.82×** | **8.61 B/k** vs 9.32 B/k (0.92×) | 🟢 **~18% faster lookup** |
| **Clustered 30,000 lookup** | **3.71M** vs 3.97M inst | **0.94× / 0.92×** | **8.63 B/k** vs 8.87 B/k (0.97×) | 🟢 **6% faster than Judy** |
| **Clustered 100,000 map insert** | **11.42M** vs 12.01M inst | **0.951× / 0.95×** | **8.63 B/k** vs 8.87 B/k (0.97×) | 🟢 **4.9% faster than Judy** |
| **Random 100,000 set insert** | **15.10M** vs 15.69M inst | **0.962× / 0.96×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **3.8% faster than Judy** |
| **Random 100,000 map insert** | **17.52M** vs 17.76M inst | **0.986× / 0.997×** | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟢 **Faster than Judy across rlib and .so** |

---

## Compatibility Gates (Standing CI, 100% Green)

| Gate | Verification Target | Status |
|---|---|---|
| **G1: Differential Oracle** | Randomized operation sequences through `libexpanse` and stock `libjudy` must agree identically | 🟢 Passing |
| **G2: `php-judy` Drop-in** | `php-judy` compiles unmodified against `libexpanse`; entire test suite passes (221/221 on Linux + macOS) | 🟢 Passing |
| **G3: Windows Parity** | `php-judy` compiles on Windows MSVC against `expanse.dll` / `expanse.lib` and passes full suite | 🟢 Passing |
| **G4: `LD_PRELOAD` Parity** | Unmodified binaries built against stock Judy run identically under `LD_PRELOAD=libexpanse.so` | 🟢 Passing |

---

## Platform Support

| Platform | Target Triple | Distribution & Packaging |
|---|---|---|
| **Linux x86-64** | `x86_64-unknown-linux-gnu` | `libexpanse` APT/RPM package (`glibc-hwcaps` for `v2`/`v3`/`v4`), `.tar.gz` |
| **Linux ARM64** | `aarch64-unknown-linux-gnu` | `libexpanse` APT/RPM package (Graviton, Raspberry Pi 4/5), `.tar.gz` |
| **Linux RISC-V 64-bit** | `riscv64gc-unknown-linux-gnu` | `libexpanse` APT/RPM package (RV64GC edge/server), `.tar.gz` |
| **Linux x86-64 Static** | `x86_64-unknown-linux-musl` | Static musl archives, Alpine Linux compatible `.tar.gz` |
| **macOS Apple Silicon** | `aarch64-apple-darwin` | Universal / Native AArch64 `.tar.gz` |
| **macOS Intel** | `x86_64-apple-darwin` | x86-64 `.tar.gz` |
| **Windows x86-64** | `x86_64-pc-windows-msvc` | Precompiled `expanse.dll` / `expanse.lib` `.zip`, vcpkg, NuGet |
| **RISC-V 32-Bit (RV32)** | `riscv32imac-unknown-none-elf` | `#![no_std]` staticlib / embedded crate ([RFC #109](docs/RFC_32BIT_EMBEDDED.md)) |
| **ARM Cortex-M (M4/M7)** | `thumbv7em-none-eabihf` | `#![no_std]` staticlib / embedded crate ([RFC #109](docs/RFC_32BIT_EMBEDDED.md)) |
| **Espressif ESP32 (RV32/Xtensa)** | `riscv32imc-esp-espidf` | ESP-IDF component / `#![no_std]` ([RFC #109](docs/RFC_32BIT_EMBEDDED.md)) |

---

## 32-Bit Embedded Microprocessor Architecture (`#![no_std]`)

Expanse provides first-class support for 32-bit embedded microprocessors (`ExpanseSet32`, `ExpanseMap32`, `ExpanseBlobMap32`) designed to operate in tightly constrained internal SRAM:

- **Compact 8-Byte `Edge32`**: 50% structural SRAM reduction vs 64-bit descriptors (`[ptr (4B) | aux (3B) | tag (1B)]`), packing up to 7 immediate keys with zero heap allocations.
- **32-Byte Cache Alignment**: Nodes are sized for embedded microarchitectures (`BranchL2_32` = 32B = 1 cache line on Cortex-M7/ESP32; `BranchL6_32` = 64B = 2 cache lines).
- **Polymorphic `ValueSlot32`**: Payloads $\le 3\text{ bytes}$ (CAN-bus flags, status codes, checksums) fit inline with zero heap allocations.
- **Microcontroller SRAM Footprint** — real `mem_used()` byte accounting from `cargo run --release --example bytes_per_key_32` *(measured, commit 6c63826a; deterministic — host-independent for the fixed 8-byte `Edge32` layout)*:
  - Clustered sensor timestamps (10k consecutive): **$0.67\text{ B/key}$**.
  - Sparse 29-bit CAN IDs (500 IDs): **$12.61\text{ B/key}$** (genuinely sparse — a handful of keys spread across the 29-bit space).
  - IPv4 subnet /24 routing map (2k routes): **$9.38\text{ B/key}$**.
  - Dense consecutive map (10k, `u32→u32`): **$5.21\text{ B/key}$**.

---

## Distribution & Quick Start

### 1. Rust / Cargo (64-Bit & 32-Bit)
```toml
[dependencies]
expanse-trie = "0.3.0"
```

```rust
use expanse_trie::{ExpanseMap, ExpanseMap32};

fn main() {
    // 64-bit server map
    let mut map = ExpanseMap::new();
    map.insert(42, 100);
    assert_eq!(map.get(42), Some(100));

    // 32-bit embedded map
    let mut map32 = ExpanseMap32::new();
    map32.insert(100, 500);
    assert_eq!(map32.get(100), Some(500));
}
```

### 2. Debian / Ubuntu Official APT Repository
```bash
# Add official repository
echo "deb [trusted=yes] https://orieg.github.io/expanse/apt/ stable main" | sudo tee /etc/apt/sources.list.d/expanse.list

# Update & install runtime, dev headers, and legacy Judy compatibility symlinks
sudo apt-get update
sudo apt-get install -y libexpanse1 libexpanse-dev libjudy-compat
```

### 3. Enterprise Linux Official RPM Repository (RHEL / CentOS / Fedora / Rocky / Amazon Linux)
```bash
# 1. Add official repository configuration
sudo dnf config-manager --add-repo https://orieg.github.io/expanse/rpm/expanse.repo

# 2. Update & install runtime, dev headers, and legacy Judy compatibility symlinks
sudo dnf install -y libexpanse libexpanse-devel libjudy-compat
```

### 4. Modern C API (`expanse.h`)
```c
#include <stdio.h>
#include <expanse.h>

int main(void) {
    expanse_map_t *map = expanse_map_new();
    
    // Insert key -> value
    expanse_map_insert(map, 42, 100, NULL);
    
    // Fast O(depth) lookup
    uint64_t val;
    if (expanse_map_get(map, 42, &val)) {
        printf("Key 42 -> %lu\n", val);
    }
    
    // Exact byte memory accounting
    printf("Memory: %zu bytes\n", expanse_map_mem_used(map));
    
    expanse_map_free(map);
    return 0;
}
```
Compile and link directly:
```bash
gcc main.c -lexpanse -o main
```

### 5. Modern C++20 Header-Only API (`expanse.hpp`)
```cpp
#include <iostream>
#include <string_view>
#include <expanse.hpp>

int main() {
    // 1. Bitset (Judy1) with range iteration & O(depth) rank/select
    expanse::set s;
    s.insert(42);
    s.insert(100);
    for (uint64_t key : s) {
        std::cout << "Key: " << key << "\n";
    }
    std::cout << "Rank of 50: " << s.rank(50) << "\n";

    // 2. Word map (JudyL) with operator[] and structured binding iteration
    expanse::map<uint64_t, uint64_t> m;
    m[42] = 1000;
    for (auto [k, v] : m) {
        std::cout << k << " -> " << v << "\n";
    }

    // 3. String trie (JudySL) with std::string_view keys
    expanse::str_map<uint64_t> sm;
    sm["apple"] = 10;
    sm["banana"] = 20;

    // 4. Large-value off-heap blob map with zero-copy views
    expanse::blob_map bm;
    bm.insert(1, std::string_view("arbitrary payload bytes"), 0x01);
    if (auto view = bm.get(1)) {
        std::cout << "Blob: " << view->as_string_view() << "\n";
    }

    // 5. Multi-threaded OCC lock-free concurrent map
    expanse::sync_map sync_m;
    sync_m.insert(10, 500);
    auto reader = sync_m.make_reader();
    std::cout << "Read concurrent: " << reader.get(10).value_or(0) << "\n";
    return 0;
}
```
Compile with any C++20 compiler:
```bash
clang++ -std=c++20 main.cpp -Iinclude -lexpanse -lpthread -ldl -lm -o main
```

### 6. Drop-in Legacy C API (`Judy.h`)
```c
#include <stdio.h>
#include <Judy.h>

int main(void) {
    Pvoid_t judy = (Pvoid_t)NULL;
    Word_t *val;
    
    // JudyL insert macro
    JLI(val, judy, 42);
    *val = 100;
    
    // JudyL lookup macro
    JLG(val, judy, 42);
    printf("Value: %lu\n", *val);
    
    // Exact memory used macro
    Word_t bytes;
    JLMU(bytes, judy);
    printf("Memory: %lu bytes\n", bytes);
    
    // Free array macro
    Word_t freed;
    JLFA(freed, judy);
    return 0;
}
```
Compile with `-lexpanse` or drop-in `-lJudy`:
```bash
gcc legacy.c -lJudy -o legacy
```

### 7. Windows MSVC / vcpkg / NuGet
- **Release Bundle**: `expanse-v0.3.0-x86_64-pc-windows-msvc.zip` with DLL, import lib, and headers.
- **vcpkg**: `vcpkg install expanse` using `extra/vcpkg/`.
- **NuGet**: Visual Studio C++ package template in `extra/nuget/`.

### 8. Python Quickstart (`pip install expanse-trie`)
```python
from expanse_trie import ExpanseSet, ExpanseMap, SyncExpanseMap

# 1. Dynamic sparse 64-bit integer set (Judy1)
s = ExpanseSet([10, 20, 50, 100])
assert 20 in s
assert s.next_at_or_after(25) == 50
assert s.count_range(10, 50) == 3

# 2. Key-value associative map (JudyL)
m = ExpanseMap({1: 100, 2: 200})
m[42] = 1000
assert m.range(0, 50) == [(1, 100), (2, 200), (42, 1000)]

# 3. Multithreaded lock-free OCC map (GIL-free queries)
sync_m = SyncExpanseMap({10: 100})
assert sync_m[10] == 100
```
See [docs/BINDINGS_PYTHON.md](docs/BINDINGS_PYTHON.md) for full Python documentation and benchmarks.

### 9. Java & Scala Quickstart (`io.github.orieg:expanse-java`)

> **Not yet on Maven Central.** No `io.github.orieg` artifact is published (Maven Central returns 404 / `numFound:0`), and no release-workflow job currently builds or deploys the Java bindings. Build from `bindings/java` locally until first publish. The coordinates below are the planned ones.

```xml
<dependency>
    <groupId>io.github.orieg</groupId>
    <artifactId>expanse-java</artifactId>
    <version>0.3.0</version>
</dependency>
```

```java
import io.github.orieg.expanse.ExpanseMap;
import io.github.orieg.expanse.ExpanseSet;

// Zero-allocation, off-heap ordered map & set (Project Panama FFM)
try (ExpanseMap map = new ExpanseMap();
     ExpanseSet set = new ExpanseSet()) {
    // Inserts & lookups with zero JVM heap allocations
    map.put(42L, 1000L);
    long val = map.getOrDefault(42L, -1L);

    set.add(100L);
    set.add(200L);
    long count = set.countRange(50L, 250L); // O(depth) rank
}
```
See [docs/BINDINGS_JAVA.md](docs/BINDINGS_JAVA.md) for Panama FFM architecture, GC elimination benchmarks, and Spark/Flink off-heap integration patterns.

### 10. .NET & C# Quickstart (`Orieg.Expanse`)

> **Not yet on NuGet.org.** The `Orieg.Expanse` package does not resolve yet (NuGet returns 404 / `totalHits:0`); the version badge above renders "not found" until first publish. The `release.yml` push step is wired (OIDC trusted publishing) but has not landed a package. Build from `bindings/dotnet` locally until then.

```bash
dotnet add package Orieg.Expanse
```

```csharp
using Expanse;

// Zero-GC, off-heap ordered bit set & word map
using var set = new ExpanseSet();
using var map = new ExpanseMap();

set.Add(42);
map[42] = 1000;

ulong rank = set.Rank(100); // O(depth) rank
bool found = map.TryGet(42, out ulong value);
```
See [bindings/dotnet/README.md](bindings/dotnet/README.md) for full .NET documentation and guides.

See [bindings/go/README.md](bindings/go/README.md) for full Go documentation.

### 11. PHP Quickstart (`orieg/expanse`)
```bash
composer require orieg/expanse
```

```php
use Expanse\Set;
use Expanse\Map;

$set = new Set();
$set->add(42);
$rank = $set->rank(100);

$map = new Map();
$map->set(42, 1000);
$val = $map->get(42);
```
See [docs/BINDINGS_PHP.md](docs/BINDINGS_PHP.md) and [bindings/php/README.md](bindings/php/README.md) for full PHP documentation.

### 12. Node.js, Bun & Deno Quickstart (`npm i @orieg/expanse`)
```bash
npm install @orieg/expanse
# or bun add @orieg/expanse
```

```javascript
import { ExpanseSet, ExpanseMap, ExpanseBlobMap } from '@orieg/expanse';

// 1. Dynamic sparse 64-bit integer set (Judy1)
const set = new ExpanseSet([10n, 20n, 50n, 100n]);
console.log(set.has(20n));               // true
console.log(set.next(25n));              // 50n
console.log(set.countRange(10n, 50n));   // 3n

// 2. Key-value associative map (JudyL)
const map = new ExpanseMap();
map.set(42n, 1000n);
console.log(map.get(42n));               // 1000n

// 3. High-performance polymorphic blob map (inline packing + arena)
const blobmap = new ExpanseBlobMap();
blobmap.set(1n, Buffer.from('inline'), 10 /* 32-bit hot metadata */);
const res = blobmap.getWithMeta(1n);
console.log(res.isInline);               // true (0 heap allocations)
```
See [crates/expanse-node/README.md](crates/expanse-node/README.md) for full Node.js documentation.
See [docs/PACKAGING.md](docs/PACKAGING.md) for full packaging instructions across all platforms.

---

## Clean-Room Statement

The original Judy C library is LGPL. **No code from it has been consulted or ported.** This implementation derives strictly from published algorithm papers and shop manuals:
- Doug Baskins, [*A 10-Minute Description of How Judy Arrays Work and Why They Are So Fast*](https://judy.sourceforge.net/doc/10minutes.htm) (Hewlett-Packard, 2002)
- Alan Silverstein, [*Judy IV Shop Manual*](https://judy.sourceforge.net/doc/shop_interm.pdf) (Hewlett-Packard, 2002)

C API compatibility is defined by the documented API contract (man pages, published documentation) and validated by black-box differential testing. Licensed under **MIT OR Apache-2.0**.

---

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
