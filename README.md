# Expanse

[![CI](https://github.com/orieg/expanse/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/orieg/expanse/actions/workflows/ci.yml?query=branch%3Amain)
[![Crates.io Version](https://img.shields.io/crates/v/expanse-trie.svg?style=flat-square&logo=rust)](https://crates.io/crates/expanse-trie)
[![npm Version](https://img.shields.io/npm/v/@orieg/expanse.svg?style=flat-square&logo=npm)](https://www.npmjs.com/package/@orieg/expanse)
[![NuGet Version](https://img.shields.io/nuget/v/Orieg.Expanse.svg?style=flat-square&logo=nuget)](https://www.nuget.org/packages/Orieg.Expanse)
[![PyPI Version](https://img.shields.io/pypi/v/expanse-trie.svg?style=flat-square&logo=pypi)](https://pypi.org/project/expanse-trie/)
[![APT Repository](https://img.shields.io/badge/apt-debian%20%7C%20ubuntu-orange.svg?style=flat-square&logo=debian)](https://orieg.github.io/expanse/apt/)
[![RPM Repository](https://img.shields.io/badge/rpm-rhel%20%7C%20fedora%20%7C%20centos-red.svg?style=flat-square&logo=redhat)](https://orieg.github.io/expanse/rpm/)
[![Architectures](https://img.shields.io/badge/arch-x86--64%20%7C%20aarch64%20%7C%20riscv64%20%7C%20riscv32%20%7C%20arm--cortex--m-blueviolet.svg?style=flat-square)](#platform-support)
[![MSRV](https://img.shields.io/badge/MSRV-1.88%2B%20(Edition%202024)-informational.svg?style=flat-square)](Cargo.toml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg?style=flat-square)](LICENSE-MIT)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.22152112.svg)](https://doi.org/10.5281/zenodo.22152112)

A **clean-room, pure-Rust implementation of Judy arrays**, modernized for modern 64-bit and 32-bit embedded microarchitectures, with **`libexpanse` — a high-performance, drop-in C ABI replacement for `libjudy`**.

Judy arrays (invented by Doug Baskins at Hewlett-Packard, ~2002) are sparse, dynamic associative structures built as 256-ary digital tries partitioned by **expanse** (decoding keys byte by byte over fixed digit ranges) rather than by population like comparison-based trees. Their speed comes from adaptive node compression — linear, bitmap, and uncompressed branches; linear and bitmap leaves; keys stored immediately inside pointers — tuned to keep every node traversal within a few cache-line fills.

**Jump to:** [Install](#install) · [Key features](#key-features) · [Benchmarks](#comparative-performance-vs-industry-primitives) · [Quick starts by language](#distribution--quick-start) · [Platforms](#platform-support) · [Packaging guide](docs/PACKAGING.md)

---

## Install

The C library (`libexpanse`, headers, man pages, and the `libJudy` drop-in links):

```bash
# macOS, or Homebrew on Linux (formula ships from v0.7.0)
brew install orieg/tap/expanse
# The formula conflicts with Homebrew's stock `judy` (both install Judy.h and libJudy);
# if judy is installed, `brew unlink judy` first. The judy keg stays at `$(brew --prefix judy)`.

# Debian / Ubuntu
echo "deb [trusted=yes] https://orieg.github.io/expanse/apt/ stable main" | sudo tee /etc/apt/sources.list.d/expanse.list
sudo apt-get update && sudo apt-get install -y libexpanse1 libexpanse-dev libjudy-compat

# Fedora / RHEL / Rocky / Amazon Linux
sudo dnf config-manager --add-repo https://orieg.github.io/expanse/rpm/expanse.repo
sudo dnf install -y libexpanse libexpanse-devel libjudy-compat
```

Prebuilt archives for Linux (glibc, musl), macOS and Windows, with `SHA256SUMS`, are on the [Releases page](https://github.com/orieg/expanse/releases/latest); vcpkg, NuGet and MacPorts are covered in the [packaging guide](docs/PACKAGING.md).

Language packages:

| Language | Install | Quick start |
|---|---|---|
| Rust | `cargo add expanse-trie` | [Rust](#1-rust--cargo-64-bit--32-bit) |
| Python | `pip install expanse-trie` | [Python](#8-python-quickstart-pip-install-expanse-trie) |
| Node.js / Bun / Deno | `npm i @orieg/expanse` | [Node.js](#12-nodejs-bun--deno-quickstart-npm-i-oriegexpanse) |
| .NET | `dotnet add package Orieg.Expanse` | [.NET](#10-net--c-quickstart-oriegexpanse) |
| Java / Scala | Maven `io.github.orieg:expanse-java` | [Java](#9-java--scala-quickstart-iogithuboriegexpanse-java) |
| PHP | `composer require orieg/expanse` | [PHP](#11-php-quickstart-oriegexpanse) |
| Ruby | `gem install expanse` | [docs/bindings/ruby.md](docs/bindings/ruby.md) |
| Go | `go get github.com/orieg/expanse/bindings/go` | [bindings/go](bindings/go/README.md) |
| WebAssembly | `npm i @orieg/expanse-wasm` | [crates/expanse-wasm](crates/expanse-wasm) |
| ESP-IDF | component `components/expanse` | [ESP-IDF](#13-espressif-esp-idf-component-esp32-c2c3c6h2p4) |

Then link with `-lexpanse` (or keep `-lJudy`), or see the [C](#4-modern-c-api-expanseh), [C++](#5-modern-c20-header-only-api-expansehpp) and [legacy `Judy.h`](#6-drop-in-legacy-c-api-judyh) examples.

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

- **Pure Rust & Memory Safe**: `#![no_std]` core on 32-bit embedded targets (`std` by default on 64-bit) with zero unsafe memory leaks, zero external runtime dependencies, verified under Miri & Loom.
- **Fewer Instructions than Stock Judy**: Lower Callgrind instruction counts than original `libjudy` on every measured arm (inserts, lookups, set tests, churn). Wall clock wins on insert and on sequential and clustered 1M lookup; the one measured loss is random 1M `get` at **1.031×, BCa 95% CI [1.024, 1.038]** *(measured: reference host, [`results/baseline_vs_libjudy.json`](results/baseline_vs_libjudy.json))*. [Full table](#performance-vs-stock-libjudy).
- **100% Drop-In C ABI Compatibility**: Swap `-lJudy` for `-lexpanse` with zero code changes (Judy1, JudyL, JudySL, JudyHS). Passes `php-judy` test suite (221/221) and differential oracle.
- **Multi-Architecture Vectorization & Embedded**: Hardware-accelerated with runtime CPUID dispatch on x86-64, ARM64 NEON, 64-bit RISC-V (`RV64GC`), and bare-metal 32-bit embedded (`RV32IMAC`, `Cortex-M4/M7`). `glibc-hwcaps` variants (`x86-64-v2`/`v3`/`v4`) are a build recipe, not part of the released packages ([docs/COMPAT.md](docs/COMPAT.md)).
- **Concurrent Readers and Writers**: Every `Sync*` wrapper serves readers that **take no lock on the common path** and concurrent writers through optimistic lock coupling (a blocking protocol, not lock-free). `SyncExpanseMap` serves **348–354 M reads/s at 16 threads** *(measured: reference host; workload: `core_concurrency`)*. [Tables, losses and competitors](#multithreaded-occ-concurrency-scalability).
- **Dense Memory Packing**: Down to **0.07–0.36 bytes/key** on dense and clustered 64-bit **sets** (`ExpanseSet`, key presence only) *(measured: `bytes_per_key` example)*. A **map** stores an 8-byte value word per key, so it cannot go below 8 B/key: `ExpanseMap` measures **8.56–8.61 bytes/key** on the same dense and clustered distributions at 1M keys *(measured: deterministic `mem_used()` accounting, host-independent; `bytes_per_key` example, workload `example_bytes_per_key`; gated copy `docs/visualizer_data.json` → `memory_budget`)* and **~0.31 bytes/key** on clustered 32-bit embedded sets *(measured: `bytes_per_key_32` example)*. On uniform random keys per-key set cost is a sawtooth in key density, **7.08–21.02 bytes/key** *(measured: `keyspace_density` example; [docs/ARCHITECTURE.md §3.5](docs/ARCHITECTURE.md#35-per-key-memory-is-a-sawtooth-in-expanse-occupancy-and-leaf_cap-sets-the-tooth))*.

---

## Visual Performance Comparison

![Comparative Performance](docs/assets/bench_comparative.svg)

![OCC Concurrency Scalability](docs/assets/bench_concurrency.svg)

![YCSB Workloads A–F: ExpanseMap / ExpanseBlobMap vs BTreeMap and SkipMap](docs/assets/bench_ycsb.svg)

![Memory density across expanse occupancy: ExpanseSet bytes/key is a sawtooth in λ = N / 2¹⁶, with the LEAF_CAP cascade and both memory-budget cells marked](docs/assets/bench_density_sawtooth.svg)

---

## API Surfaces

| Surface | Crate / Package | Deliverable |
|---|---|---|
| **Native Rust API (64-Bit)** | [`crates/expanse`](crates/expanse) (package `expanse-trie`) | Pure-Rust library: `ExpanseSet` (bit set), `ExpanseMap` (word→word), `ExpanseStrMap` (string→word), `ExpanseBytesMap` (bytes→word), `ExpanseBlobMap`, plus iterators and optimistic concurrent readers (`SyncExpanseMap`) |
| **Native Embedded Rust (32-Bit)** | [`crates/expanse`](crates/expanse) (`#![no_std]`) | 32-bit microprocessor collections: `ExpanseSet32` (bit set), `ExpanseMap32` (u32→u32 map), `ExpanseBlobMap32` with compact 8-byte `Edge32` layout and 32-byte cache line alignment |
| **C ABI (`libexpanse`)** | [`crates/expanse-capi`](crates/expanse-capi) | `cdylib`/`staticlib` exporting **both** the legacy `Judy.h` surface (`Judy1*`, `JudyL*`, `JudySL*`, `JudyHS*` — allowing consumers like [php-judy](https://github.com/orieg/php-judy) to swap `libJudy` for `libexpanse` without source changes) **and** modern `expanse.h` |
| **Modern C++20 Header** | [`include/expanse.hpp`](include/expanse.hpp) | Modern header-only C++20 STL-compatible RAII wrapper (`expanse::set`, `expanse::map`, `expanse::str_map`, `expanse::bytes_map`, `expanse::blob_map`, `expanse::sync_map`), `std::span` zero-copy access, `std::forward_iterator` ranges, and optimistic OCC readers |
| **Java / Scala FFM API** | [`bindings/java`](bindings/java) (`io.github.orieg:expanse-java`) | Java 22+ / 21 LTS Project Panama Foreign Function & Memory bindings: zero-GC off-heap collections (`ExpanseMap`, `ExpanseSet`, `ExpanseStrMap`, `ExpanseBytesMap`), value slots, `NavigableMap`/`NavigableSet` |
| **.NET / C# API** | [`bindings/dotnet`](bindings/dotnet) (`Orieg.Expanse`) | .NET 8.0/9.0+ C# bindings & NuGet package via P/Invoke: zero-GC off-heap collections (`ExpanseSet`, `ExpanseMap`, `ExpanseStrMap`, `ExpanseBytesMap`, `ExpanseBlobMap`, `ExpanseSyncMap`) |
| **Go API** | [`bindings/go`](bindings/go) (`github.com/orieg/expanse/bindings/go`) | Native Go bindings via CGO: zero-GC off-heap collections (`Set`, `Map`, `StrMap`, `BytesMap`, `BlobMap`) |
| **PHP API** | [`bindings/php`](bindings/php) (`orieg/expanse`) | Native PHP bindings via FFI & PIE: `Expanse\Set`, `Expanse\Map`, `Expanse\StrMap`, `Expanse\BytesMap`, `Expanse\BlobMap`, `Expanse\SyncMap`, `Expanse\SyncSet` |
| **Python API** | [`bindings/python`](bindings/python) (`pip install expanse-trie`) | High-performance Python extension via PyO3: `ExpanseSet`, `ExpanseMap`, `SyncExpanseMap`, GIL-released queries |
| **Node.js / Bun / Deno API** | [`crates/expanse-node`](crates/expanse-node) (`@orieg/expanse`) | Native high-performance N-API bindings via `napi-rs`: `ExpanseSet`, `ExpanseMap`, `ExpanseStrMap`, `ExpanseBytesMap`, `ExpanseBlobMap`, `SyncExpanseMap`, `SyncExpanseSet` |
| **WebAssembly / Edge** | [`crates/expanse-wasm`](crates/expanse-wasm) (`@orieg/expanse-wasm`) | WebAssembly bindings for edge runtimes (Cloudflare Workers, Fastly) and browsers |
| **Ruby API** | [`bindings/ruby`](bindings/ruby) (`gem install expanse`) | Native Ruby extension via Fiddle / C ABI: `Expanse::Set`, `Expanse::Map`, `Expanse::StrMap`, `Expanse::BytesMap`, `Expanse::BlobMap` |
| **RocksDB Pluggable MemTable** | [`integrations/rocksdb`](integrations/rocksdb) (`rocksdb-expanse`) | Official RocksDB `MemTableRep` / `MemTableRepFactory` implementation. Against a fair variable-height skiplist baseline: **1.42× higher key density in RAM** (13.2 vs 18.7 B/entry, deterministic accounting), point lookup **1.49×** [1.4901, 1.5073], range seek **1.53×** [1.5225, 1.5414], sequential scan **3.07×–3.14×** [3.0480, 3.2269], batch scan **2.13×** [2.0452, 2.3111] — each interval spans both runs *(measured: reference host, two runs; [`baseline_rocksdb.json`](docs/benchmarks/rocksdb_memtable/results/baseline_rocksdb.json))*. Fewer L0 flushes is inferred (target). See [`docs/benchmarks/rocksdb_memtable/`](docs/benchmarks/rocksdb_memtable/README.md) and [`integrations/rocksdb/`](integrations/rocksdb/README.md) |

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
| **Bit scan / rank** | SWAR bit hacks, unrolled loops | Hardware `POPCNT` / `TZCNT` / `LZCNT` / ARM `cnt` (runtime CPUID dispatch on hot read paths; SWAR fallback on generic baseline builds; native in `x86-64-v2`/`v3` packages) |
| **Linear search** | Scalar unrolled byte compares | Vectorized SIMD byte scans (SSE2 on x86-64, NEON on ARM64; AVX2/AVX-512 not yet implemented) |
| **Allocation** | Custom 2001 chunk/buddy allocator | High-performance slab page pooling + intrusive freelists |
| **Pointer layout** | Full 16-byte JP per edge | 16-byte `Edge`: word 0 is the raw untruncated 64-bit pointer, tag and metadata live in word 1 — zero upper-bit stealing, so it stays correct under 57-bit LA57 and 52-bit ARM64 LVA ([encoding reference](docs/ARCHITECTURE.md#10-bit-level-encoding-reference)) |
| **Concurrency** | Single-threaded, external locks | Optimistic concurrency control (OCC) for reads |

Full architectural specifications: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) · Embedded 32-Bit design: [docs/design/32-bit-embedded.md](docs/design/32-bit-embedded.md) · Large-Value design: [docs/design/large-values.md](docs/design/large-values.md) · Database engine patterns: [docs/DATABASE.md](docs/DATABASE.md) · CI/CD: [docs/CI.md](docs/CI.md).

---

## Database Engine Subsystems & Architecture

Expanse provides modern, hardware-vectorized digital trie primitives tailored for core database engine subsystems:

- **Inverted Indexes & Posting Lists (`ExpanseSet`)**: Doc-ID tracking at **0.07–0.36 bytes/docID** (presence only, no value) on clustered/dense sets — denser than Roaring Bitmaps on those distributions — with bitwise set algebra directly over compressed trie edges and $O(\text{depth})$ skip-scan acceleration.
- **MVCC Visibility Maps & Active Transaction Tracking (`SyncExpanseSet`)**: Optimistic active transaction (`xid`) tracking with no reader-side lock on the common path, and safe epoch reclamation under continuous OLTP churn.
- **Columnar String & Symbol Dictionaries (`ExpanseStrMap`)**: High-cardinality string deduplication and symbol tables using 8-byte chunk decomposition and tail collapse, preserving lexicographical order while sharing common prefix nodes.
- **Secondary Indexes & MemTables (`ExpanseMap` / `ExpanseMemTableRep`)**: Rebalance-free ordered key indexing, **2.9×–14.5× faster point lookups** than `std::collections::BTreeMap` at 1M keys, and full ordered `iter()` faster than `BTreeMap::iter()` for dense keys — sparse-key iteration is still slower, see [docs/DATABASE.md](docs/DATABASE.md) §7.1. Ships an official [RocksDB Pluggable MemTable (`integrations/rocksdb`)](integrations/rocksdb) integration.
- **Zero-Copy Shared-Memory Analytics** *(roadmap)*: Position-independent base-relative layouts for cross-worker IPC and parallel query execution with zero serialization — a design target; not yet implemented (see [docs/DATABASE.md](docs/DATABASE.md) §6).

See [docs/DATABASE.md](docs/DATABASE.md) and [integrations/rocksdb/README.md](integrations/rocksdb/README.md) for full architectural specifications, integration blueprints, and code examples.

---

## Comparative Performance vs Industry Primitives

Where Expanse wins, where it loses, and where to read the full tables. Losses are listed as plainly as wins; every suite publishes BCa 95% intervals per cell. Decision matrix: [docs/DATABASE.md §7](docs/DATABASE.md). Methodology: [docs/BENCHMARKING.md](docs/BENCHMARKING.md). "Reference host" throughout is a quiet Intel Core i9-12900F; the commit, run and interval behind each figure are in the linked docs.

### 1. `ExpanseSet` vs `RoaringBitmap`

- **Sparse and clustered sets — Expanse.** `contains` is **1.24×–2.09× faster**: sparse 1.77× [1.76, 1.77] at 10k and 2.09× [2.08, 2.09] at 100k, clustered 1.59× [1.59, 1.60] and 1.24× [1.24, 1.24].
- **Dense sets and rank/select — Roaring.** Its bit containers win dense `contains` by **7.86× [7.84, 7.89]** at 10k and **3.11× [3.11, 3.13]** at 100k, and its rank index beats `count_below`/`by_count` in every cell measured.
- **Memory — parity on dense data.** `ExpanseSet` holds **0.07–0.36 bytes/key** on clustered and dense sets *(measured: `bytes_per_key` example, deterministic allocator accounting)*; that is key presence only — an `ExpanseMap` holding an 8-byte value per key measures 8.56–8.61 bytes/key on the same keys at 1M (same instrument), with ordered forward and backward iteration.

Rule of thumb: Expanse for membership on sparse and clustered sets, Roaring for dense sets and heavy rank/select *(measured: reference host, 50% hit rate; [`results/baseline_comparative.json`](results/baseline_comparative.json); workload: `core_comparative`)*.

### 2. `ExpanseMap` vs `hashbrown::HashMap` & `BTreeMap`

- **vs `BTreeMap` — Expanse on lookups.** Point lookups are **2.9×–14.5× faster** at 1M keys (sequential 11.9 ns vs 108.9 ns, clustered 12.9 ns vs 110.2 ns; workload: `core_compare`) *(measured: reference host, `benches/compare.rs`)*. Ordered `iter()` is also faster on sequential, clustered and random keys (0.5×–0.8× the time of `BTreeMap::iter()`), but **sparse-key iteration is ~4.7× slower** ([#270](https://github.com/orieg/expanse/issues/270)) *(measured: reference host, `benches/compare.rs`)*. Details: [docs/DATABASE.md §7.1](docs/DATABASE.md).
- **vs `hashbrown` — hashbrown on random keys.** A Swiss Table's single probe beats trie descent by ~1.7×–3.1× on 1M uniform-random keys. The gap is small while the set is cache-resident (10k: 10.0 ns vs 8.9 ns; workload: `core_compare`), and sequential keys are near parity (11.9 ns vs 12.1 ns at 1M; workload: `core_compare`) *(measured: reference host, `benches/compare.rs`)*. What Expanse offers in exchange: strict key ordering, ordered iteration, prefix search, and a smaller footprint on clustered integer keys.

### 3. Trie competitors: ART, HOT and Masstree

Measured at 1M keys unless noted, ratios above 1 = Expanse faster. Full suites: [ART](docs/benchmarks/art_comparison/README.md) · [HOT](docs/benchmarks/hot_comparison/README.md) · [Masstree](docs/benchmarks/masstree_comparison/README.md).

| Competitor | Point lookup | Insert | Ordered scan | Memory |
|---|---|---|---|---|
| **ART** (integer keys) | **Expanse** 1.54×–3.21× | **Expanse** 4.84× (sequential) | **Expanse**: k = 10 by 1.50×–1.53× on structured keys, 2.67× on random | see suite |
| **HOT** (integer keys) | **Expanse** in most cells; **HOT** on uniform-random map lookup, 0.946 [0.934, 0.960] | mostly **Expanse** | **HOT**: 0.39–0.52 on random keys at k = 1000 (100k keys) | **HOT** flat 11.7–12.1 B/key (set); Expanse lower only for λ ∈ [8, 23] |
| **HOT** (string keys) | **Expanse** on `skewed` 1.22×–2.00×; **HOT** on `prefixed` 0.80× | **Expanse** on `prefixed` 1.34×–1.43× (`ExpanseStrMap`); **HOT** 0.74× (`ExpanseBytesMap`) | **HOT** in 72 of 72 cells, 0.646 [0.643, 0.650] down to 0.050 [0.049, 0.050] | **HOT** 36.2 vs 48.2 B/key (workload: `hot_str_ptr`) |
| **Masstree** (integer keys) | **Expanse** 2.9×–13.5× | **Masstree** 0.68×–0.75× on sorted `random`/`sparse`; **Expanse** 1.89× on shuffled `random` | **Expanse** 1.08×–2.54× on structured keys and `random` at 1M; **Masstree** 0.49×–0.67× on `random` at 10k–100k | **Masstree** flat 22.8 B/key; Expanse 17.6–20.0 for λ ∈ [8, 30], 23.8–24.7 past it |
| **Masstree** (string keys) | **Expanse** on `short` 1.33×, `skewed` 1.47×, `prefixed` 1.12×; **Masstree** on `counter` 0.95× | **Masstree** 0.43×–0.96× | **Masstree** in 33 of 36 cells, 0.09×–0.83×; **Expanse** on `prefixed` k = 10, 1.53×–2.27× | **Masstree** 33.9 vs 48.2 B/key (workload: `masstree_str_map`) |

Across the HOT integer suite, 112 of 144 latency cells go to Expanse, 31 to HOT (28 of them scans) and 1 to parity, identically in two runs.

- **Ordered scans are the systematic loss against HOT**, on integer and string keys alike.
- **Insert verdicts are sorted-order verdicts.** Ascending insertion is a B+-tree's best case; the Masstree `random` insert cell flips to Expanse on a shuffled permutation of the same keys.
- **Memory depends on density.** Expanse's bytes/key is a sawtooth in expanse occupancy λ = N / 2¹⁶ ([docs/ARCHITECTURE.md §3.5](docs/ARCHITECTURE.md#35-per-key-memory-is-a-sawtooth-in-expanse-occupancy-and-leaf_cap-sets-the-tooth)), while HOT and Masstree are flat, so the winner changes with λ.
- **Key-length limits.** HOT and Masstree cap string keys at 255 bytes (HOT drops longer keys silently, Masstree refuses them); the Expanse arms are not restricted to match.
- **Concurrent arms** (HOT-ROWEX, concurrent Masstree) are in the [next section](#against-tries-that-admit-concurrent-writers).

*(measured: reference host, two runs per suite agreeing on every winner quoted; hosts, commits, intervals and per-cell artifacts are in the three suite READMEs linked above; workloads: `art_scan`, `hot_latency`, `hot_memory_curve`, `hot_string_latency`, `hot_str_ptr`, `masstree_map_64bit`, `masstree_str_map`. Memory figures are deterministic allocator censuses.)*

---

## Multithreaded OCC Concurrency Scalability

Every `Sync*` wrapper (`SyncExpanseSet`, `SyncExpanseMap`, `SyncExpanseStrMap`, `SyncExpanseBytesMap`, `SyncExpanseBlobMap`) serves concurrent readers and concurrent writers.

- **Readers take no lock on the common path.** A reader samples a version, walks, and re-validates; retired memory is reclaimed through epochs.
- **Writers use [optimistic lock coupling](https://db.in.tum.de/~leis/papers/artsync.pdf)** (Leis, Scheibner, Kemper & Neumann, DaMoN 2016): a writer locks only the node it changes, so writers on disjoint subexpanses proceed in parallel. After a bounded number of restarts, or for a structural change the lock-coupled path does not cover, an operation falls back to an exclusive section.
- **The protocol is blocking — not lock-free and not obstruction-free.** Lock-free reads are a fast path, not a progress guarantee. Design, fallback paths and measured fallback rates: [docs/ARCHITECTURE.md §4.1–4.2](docs/ARCHITECTURE.md#41-concurrent-reads-occ--sync) and [docs/benchmarks/concurrency/](docs/benchmarks/concurrency/README.md).

### Throughput at 1 and 16 threads

Total operations per second (reads + writes), bounded keyspaces, two independent runs *(measured: reference host, 16 hardware threads on 8 P-cores; intervals, commit and run links in [concurrency §12](docs/benchmarks/concurrency/README.md) and `docs/benchmarks/concurrency/results/baseline_concurrent_mixed.json`; workload: `core_concurrency`)*.

| arm | keys → values | 1 Thread | 16 Threads, run 1 | 16 Threads, run 2 | Scaling, run 1 / run 2 |
|---|---|---:|---:|---:|---:|
| `SyncExpanseMap` (100% read) | u64 → u64, 1M draws | 38.4 M ops/s | **348 M ops/s** | **354 M ops/s** | **9.07× / 8.83×** |
| `SyncExpanseSet` (100% read) | u64, 1M draws | 79.2 M ops/s | 605 M ops/s | 602 M ops/s | 7.64× / 7.58× |
| `SyncExpanseMap` (50R/50W mixed) | u64 → u64, 1M draws | 28.4 M ops/s | **104 M ops/s** | **106 M ops/s** | **3.65× / 3.73×** |
| `SyncExpanseSet` (50R/50W mixed) | u64, 1M draws | 42.7 M ops/s | 226 M ops/s | 225 M ops/s | 5.30× / 5.27× |
| `SyncExpanseBlobMap` (100% read) | u64 → 128-byte payload, 200k draws | 33.9 M ops/s | 293 M ops/s | 304 M ops/s | 8.64× / 9.01× |
| `SkipMap` (100% read) | u64 → 128-byte payload, 200k draws | 3.47 M ops/s | 38.6 M ops/s | 38.5 M ops/s | 11.14× / 11.14× |
| `SyncExpanseBlobMap` (50R/50W mixed) | u64 → 128-byte payload, 200k draws | 17.6 M ops/s | 7.29 M ops/s | 6.83 M ops/s | 0.40× / 0.38× |
| `SkipMap` (50R/50W mixed) | u64 → 128-byte payload, 200k draws | 2.06 M ops/s | 17.5 M ops/s | 17.6 M ops/s | 8.53× / 8.55× |
| `SyncExpanseBytesMap` (100% read) | 37-byte string → u64, 100k draws | 11.5 M ops/s | 121 M ops/s | 116 M ops/s | 10.53× / 10.17× |
| `SyncExpanseStrMap` (100% read) | 37-byte string → u64, 100k draws | 7.09 M ops/s | 77.5 M ops/s | 77.8 M ops/s | 10.93× / 10.97× |
| `DashMap<Vec<u8>, u64>` (100% read) | 37-byte string → u64, 100k draws | 15.7 M ops/s | 131 M ops/s | 130 M ops/s | 8.36× / 8.38× |
| `SyncExpanseStrMap` (50R/50W mixed) | 37-byte string → u64, 100k draws | 5.78 M ops/s | **53.5 M ops/s** | **52.8 M ops/s** | **9.25× / 9.14×** |
| `SyncExpanseBytesMap` (50R/50W mixed) | 37-byte string → u64, 100k draws | 4.88 M ops/s | 33.6 M ops/s | 32.8 M ops/s | 6.92× / 6.75× |
| `DashMap` (50R/50W mixed) | 37-byte string → u64, 100k draws | 10.8 M ops/s | 83.6 M ops/s | 83.2 M ops/s | 7.76× / 7.79× |
| `Mutex<Expanse*>` baselines (100% read) | blob and string keys | 8.86–40.9 M ops/s | 2.73–5.39 M ops/s | 2.68–5.55 M ops/s | 0.13×–0.31× (collapse) |

How to read it:

- **Compare rows only within a key type** (u64 → u64, u64 → 128-byte payload, 37-byte string → u64). Populations and key widths differ between the three.
- **Read-only scaling holds to sixteen threads** on every wrapper, where a `Mutex` around the same structure falls below its single-thread rate.
- **The 50R/50W rows are a mixed-operation rate, not read scaling**: every thread picks a read or a write per operation. The integer and string wrappers scale there; `SyncExpanseStrMap` reaches 53 M ops/s against 2.3–2.4 M for the same map behind one mutex, and `SyncExpanseBytesMap` 33 M against 2.6–2.9 M (workload: `core_concurrency`).
- **Known losses at 50R/50W:** `SyncExpanseBlobMap` loses throughput as threads are added (0.38×–0.40×) where `SkipMap` scales 8.5× on the same keys, and `DashMap` serves 83 M ops/s on string keys against 53 M and 33 M for the two Expanse wrappers.

### Against tries that admit concurrent writers

| Competitor | Writers only | Eight readers alongside writers | Details |
|---|---|---|---|
| **HOT-ROWEX** (integer keys) | **Expanse** at one to eight writers, 1.11×–1.45× | **Expanse** in every cell, 1.200–2.094 | [hot_comparison §7](docs/benchmarks/hot_comparison/README.md) |
| **Masstree** (integer keys) | **Masstree** from two writers, 0.743–0.751 at eight; one writer is direction-only at 0.977 [0.956, 1.000] and 0.977 [0.958, 0.999] | **Expanse**, 2.355–2.625 | [masstree_comparison §7](docs/benchmarks/masstree_comparison/README.md) |
| **Masstree** (`short` string keys) | **Masstree** in every cell, 0.678–0.726 at one writer | **Expanse**, 1.189–1.299 | [masstree_comparison §7](docs/benchmarks/masstree_comparison/README.md) |

Ratios are competitor-relative throughput, above 1 = Expanse faster; each range covers two runs of 15 interleaved rounds, with BCa 95% intervals per cell in the linked sections *(measured: reference host; workloads: `hot_rowex_set_63bit`, `hot_rowex_map_64bit`, `masstree_conc_map_64bit`, `masstree_conc_str`)*. Neither arm carries hardware counters, so what sets these levels is unmeasured.

---

## Microarchitecture Scaling: x86-64-v1 vs v3

**Higher ISA tiers do not uniformly help.** On the measured arch sweep — run [33030463060](https://github.com/orieg/expanse/actions/runs/33030463060) on the idle reference host — clustered lookups gain **1.08×–1.14×** over the portable baseline, random is flat to slightly worse (**0.87×–0.95×**), and sequential regresses, including an unexplained **0.34×** `x86-64-v2` cell at N = 10k (cause unknown; published as measurement, not finding). Full table and caveats: [docs/BENCHMARKING.md](docs/BENCHMARKING.md).

Per-tier instruction counts are deterministic: [`docs/visualizer_data.json`](docs/visualizer_data.json) carries Callgrind counts for `x86-64-v1` and `x86-64-v3` across every instruction-benchmark routine — v1→v3 deltas span **−1.9% to −42.6%** (largest on `map_remove/random`).

---

## Performance vs Stock libjudy

Instructions retired and wall-clock latency through the identical C ABI on identical key streams, both libraries `dlopen`'d — measured via paired A/B rounds (interleaved median of 5 rounds). **Below 1.00 = libexpanse does less work / runs faster than original libjudy.**

> **Provenance.** The `M inst` rows (workload: `capi_vs_stock`) are deterministic Callgrind counts on the portable `x86-64-v1` baseline, and the `B/k` columns are deterministic byte accounting. The `ns` rows (workload: `capi_bench_vs_libjudy`) are wall clock: 15 paired rounds, arms interleaved, 50% hit rate, value slot dereferenced, ratios with BCa 95% intervals *(measured: reference host — Intel i9-12900F, [run 33151981386](https://github.com/orieg/expanse/actions/runs/33151981386); per-round data in [`results/baseline_vs_libjudy.json`](results/baseline_vs_libjudy.json))*.
>
> **Random 1M lookup is the one measured wall-clock loss: 1.031× slower than stock libjudy, BCa 95% CI [1.024, 1.038]** (workload: `capi_bench_vs_libjudy`). Full matrix and intervals: [docs/BENCHMARKING.md](docs/BENCHMARKING.md).

| Benchmark Workload | Wall-Clock Latency (Expanse vs Stock) | Ratio (.so / rlib) | Memory Overhead (Expanse vs Stock) | Status |
|---|---|---:|---|---|
| **Sequential 1,000,000 insert** | **12.2 ns** vs 22.4 ns (workload: `capi_bench_vs_libjudy`) | **0.545×** [0.544, 0.546] | **8.56 B/k** vs 8.32 B/k (1.03×) | 🟢 **~1.84× faster insert** |
| **Sequential 100,000 insert** | **6.40M** vs 12.84M inst (workload: `capi_vs_stock`) | **0.50× / 0.49×** | **8.57 B/k** vs 8.41 B/k (1.02×) | 🟢 **2× faster than Judy** |
| **Sequential 30,000 lookup** | **4.37M** vs 5.07M inst (workload: `capi_vs_stock`) | **0.86× / 0.85×** | **8.57 B/k** vs 8.41 B/k (1.02×) | 🟢 **14% faster than Judy** |
| **Random 1,000,000 lookup** | 41.0 ns vs **39.8 ns** (workload: `capi_bench_vs_libjudy`) | **1.031×** [1.024, 1.038] | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟡 **3% slower lookup, 5% less memory** |
| **Random 3,000,000 lookup** | **318.5M** vs 389.7M inst (workload: `capi_vs_stock`) | **0.82× / 0.81×** | **16.80 B/k** vs 17.80 B/k (0.94×) | 🟢 **18% faster than Judy** |
| **Random 30,000 lookup** | **4.53M** vs 5.09M inst (workload: `capi_vs_stock`) | **0.89× / 0.88×** | **24.63 B/k** vs 24.81 B/k (0.99×) | 🟢 **11% faster than Judy** |
| **Random 30,000 set test** | **3.78M** vs 3.83M inst (workload: `capi_vs_stock`) | **0.988× / 0.98×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **Faster than Judy** |
| **Random 30,000 churn (del+ins)** | **38.14M** vs 50.78M inst (workload: `capi_vs_stock`) | **0.751× / 0.75×** | **Dynamic exact accounting** | 🟢 **24.9% faster than Judy** |
| **Clustered 100,000 set insert** | **7.54M** vs 10.38M inst (workload: `capi_vs_stock`) | **0.727× / 0.72×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **27.3% faster than Judy** |
| **Clustered 1,000,000 insert** | **19.9 ns** vs 21.6 ns (workload: `capi_bench_vs_libjudy`) | **0.92×** | **8.61 B/k** vs 9.32 B/k (0.92×) | 🟢 **~8% faster insert, 8% less memory** |
| **Clustered 1,000,000 lookup** | **8.5 ns** vs 10.4 ns (workload: `capi_bench_vs_libjudy`) | **0.82×** | **8.61 B/k** vs 9.32 B/k (0.92×) | 🟢 **~18% faster lookup** |
| **Clustered 30,000 lookup** | **3.71M** vs 3.97M inst (workload: `capi_vs_stock`) | **0.94× / 0.92×** | **8.63 B/k** vs 8.87 B/k (0.97×) | 🟢 **6% faster than Judy** |
| **Clustered 100,000 map insert** | **11.42M** vs 12.01M inst (workload: `capi_vs_stock`) | **0.951× / 0.95×** | **8.63 B/k** vs 8.87 B/k (0.97×) | 🟢 **4.9% faster than Judy** |
| **Random 100,000 set insert** | **15.10M** vs 15.69M inst (workload: `capi_vs_stock`) | **0.962× / 0.96×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **3.8% faster than Judy** |
| **Random 100,000 map insert** | **17.52M** vs 17.76M inst (workload: `capi_vs_stock`) | **0.986× / 0.997×** | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟢 **Faster than Judy across rlib and .so** |

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
| **Linux x86-64** | `x86_64-unknown-linux-gnu` | `libexpanse` APT/RPM package, `.tar.gz` |
| **Linux ARM64** | `aarch64-unknown-linux-gnu` | `libexpanse` APT/RPM package (Graviton, Raspberry Pi 4/5), `.tar.gz` |
| **Linux RISC-V 64-bit** | `riscv64gc-unknown-linux-gnu` | `libexpanse` APT/RPM package (RV64GC edge/server), `.tar.gz` |
| **Linux x86-64 Static** | `x86_64-unknown-linux-musl` | Static musl archives, Alpine Linux compatible `.tar.gz` |
| **macOS Apple Silicon** | `aarch64-apple-darwin` | Universal / Native AArch64 `.tar.gz`, Homebrew tap (`orieg/tap/expanse`), MacPorts Portfile |
| **macOS Intel** | `x86_64-apple-darwin` | x86-64 `.tar.gz`, Homebrew tap (`orieg/tap/expanse`), MacPorts Portfile |
| **Windows x86-64** | `x86_64-pc-windows-msvc` | Precompiled `expanse.dll` / `expanse.lib` `.zip`, vcpkg, NuGet |
| **RISC-V 32-Bit (RV32)** | `riscv32imac-unknown-none-elf` | `#![no_std]` staticlib / embedded crate ([design #109](docs/design/32-bit-embedded.md)) |
| **ARM Cortex-M (M4/M7)** | `thumbv7em-none-eabihf` | `#![no_std]` staticlib / embedded crate ([design #109](docs/design/32-bit-embedded.md)); C ABI measured on-target on an STM32H747I-DISCO Cortex-M7 and Cortex-M4 ([harness](integrations/stm32h747/README.md), [results](docs/benchmarks/stm32h747/README.md)); executed on every PR on an emulated Cortex-M3 (`thumbv7m-none-eabi`, QEMU `mps2-an385`, [smoke](integrations/qemu-cortex-m3/README.md)) |
| **Espressif RISC-V (ESP-IDF)** | `riscv32imc-unknown-none-elf` (C2/C3 — RV32IMC, no A extension), `riscv32imac-unknown-none-elf` (C6/H2), `riscv32imafc-unknown-none-elf` (P4 — hard-float ilp32f, matching ESP-IDF) | ESP-IDF Component (`components/expanse/`), `#![no_std]`. RISC-V parts only — the Xtensa ESP32/S2/S3 have no mainline rustc target. No `Judy*` symbols at 32-bit ([docs](components/expanse/README.md)). Per-part ISA, HP/LP core counts and CAS soundness are sourced to the Espressif datasheets/TRMs in [docs/HARDWARE.md §4.3](docs/HARDWARE.md#43-espressif-risc-v-per-part-core-inventory--cas-soundness--validated-567) |
| **WebAssembly (wasm32)** | `wasm32-unknown-unknown` | npm `@orieg/expanse-wasm` (`WasmExpanseMap32`, `WasmExpanseSet32`) |
| **WebAssembly Memory64 (wasm64)** | `wasm64-unknown-unknown` | 64-bit engine (`ExpanseMap`, `ExpanseSet`), Node.js Memory64 (`--experimental-wasm-memory64`) |

---

## 32-Bit Embedded Microprocessor Architecture (`#![no_std]`)

Expanse provides first-class support for 32-bit embedded microprocessors (`ExpanseSet32`, `ExpanseMap32`, `ExpanseBlobMap32`) designed to operate in tightly constrained internal SRAM:

- **Compact 8-Byte `Edge32`**: 50% structural SRAM reduction vs 64-bit descriptors (`[ptr (4B) | aux (3B) | tag (1B)]`), packing up to 7 immediate keys with zero heap allocations.
- **32-Byte Cache Alignment**: Nodes are sized for embedded microarchitectures (`BranchL2_32` = 32B = 1 cache line on Cortex-M7/ESP32; `BranchL6_32` = 64B = 2 cache lines).
- **Polymorphic `ValueSlot32`**: Payloads $\le 3\text{ bytes}$ (CAN-bus flags, status codes, checksums) fit inline with zero heap allocations.
- **Microcontroller SRAM Footprint** — real `mem_used()` byte accounting from `cargo run --release --example bytes_per_key_32` *(measured; deterministic — host-independent for the fixed 8-byte `Edge32` layout)*:
  - Clustered sensor timestamps (10k consecutive): **$0.31\text{ B/key}$** (0.3120 B/key).
  - Sparse 29-bit CAN IDs (500 IDs): **$9.86\text{ B/key}$** (9.8560 B/key — genuinely sparse, keys spread across 29-bit space).
  - IPv4 subnet /24 routing map (2k routes): **$8.42\text{ B/key}$** (8.4160 B/key).
  - Dense consecutive map (10k, `u32→u32`): **$4.42\text{ B/key}$** (4.4240 B/key).

---

## Distribution & Quick Start

### 1. Rust / Cargo (64-Bit & 32-Bit)
```toml
[dependencies]
expanse-trie = "0.7.1"
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

### 3b. macOS: Homebrew & MacPorts
```bash
brew install orieg/tap/expanse   # macOS, or Homebrew on Linux
brew test orieg/tap/expanse      # compiles and runs one program against expanse.h and one against Judy.h
```
Installs `libexpanse` (dylib and static), `expanse.h` / `expanse.hpp` / `Judy.h`, the manual pages, pkg-config files and the `libJudy` compatibility links; it also works under Homebrew on Linux. The formula declares `conflicts_with "judy"`, since both install `Judy.h` and a `libJudy` library: if Homebrew's stock `judy` is installed, run `brew unlink judy` first. The `judy` keg stays in the Cellar and remains reachable at `$(brew --prefix judy)`, which is where the `oracle` tests and `bench_vs_libjudy` look for stock libjudy on macOS. Upgrade with `brew update && brew upgrade expanse`. A MacPorts `Portfile` ships with every release — see [docs/PACKAGING.md §2.15](docs/PACKAGING.md).

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

    // 5. Multi-threaded OCC concurrent map
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
- **Release Bundle**: `expanse-v0.7.1-x86_64-pc-windows-msvc.zip` with DLL, import lib, and headers.
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

# 3. Multithreaded optimistic OCC map (GIL-free queries)
sync_m = SyncExpanseMap({10: 100})
assert sync_m[10] == 100
```
See [docs/bindings/python.md](docs/bindings/python.md) for full Python documentation and benchmarks.

### 9. Java & Scala Quickstart (`io.github.orieg:expanse-java`)

> **Not yet on Maven Central.** No `io.github.orieg` artifact is published yet (Maven Central returns 404 / `numFound:0`); publication is wired into `.github/workflows/release.yml` (`package-maven`) to deploy on release tags. Build from `bindings/java` locally until first publish. The coordinates below are the planned ones.

```xml
<dependency>
    <groupId>io.github.orieg</groupId>
    <artifactId>expanse-java</artifactId>
    <version>0.7.1</version>
</dependency>
```

```java
import io.github.orieg.expanse.ExpanseMap;
import io.github.orieg.expanse.ExpanseSet;

// Zero-allocation, off-heap ordered map & set (Project Panama FFM, Java 22+)
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
**JDK Baseline**: Java 22+ (finalized Project Panama FFM - [JEP 454](https://openjdk.org/jeps/454)). Requires `--enable-native-access=ALL-UNNAMED`. Java 21 LTS supported for source builds with `--enable-preview`. See [docs/bindings/java.md](docs/bindings/java.md) for Panama FFM architecture, bundled multi-arch native platform matrix, GC elimination benchmarks, and Spark/Flink off-heap integration patterns.

### 10. .NET & C# Quickstart (`Orieg.Expanse`)

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
See [docs/bindings/php.md](docs/bindings/php.md) and [bindings/php/README.md](bindings/php/README.md) for full PHP documentation.

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

### 13. Espressif ESP-IDF Component (ESP32-C2/C3/C6/H2/P4)

Add `expanse` to your ESP-IDF project's `main/idf_component.yml`:
```yaml
dependencies:
  expanse:
    version: "^0.7.1"
```
Or clone directly into your project's `components/` directory:
```bash
git clone https://github.com/orieg/expanse.git components/expanse
```

```c
#include "expanse.h"
#include "expanse_esp_idf.h"
#include "esp_log.h"

void app_main(void) {
    // 32-bit digital map (compact 8-byte Edge32, 32-byte aligned nodes).
    // Keys and values are expanse_word_t — one machine word, uint32_t here.
    expanse_map_t *map = expanse_map_new();
    expanse_map_insert(map, 0x18FF50E5 /* CAN ID */, 42 /* value */, NULL);

    expanse_word_t val = 0;
    if (expanse_map_get(map, 0x18FF50E5, &val)) {
        ESP_LOGI("expanse", "Found CAN ID 0x18FF50E5 -> Value %u", (unsigned int)val);
    }
    expanse_map_free(map);
}
```

The 32-bit library exports the ordered `expanse_set_*` / `expanse_map_*` core
and **no `Judy*` symbols** — the drop-in ABI is a 64-bit-only guarantee. See the
[surface matrix](docs/COMPAT.md#build-configuration-surface-matrix).
See [components/expanse/README.md](components/expanse/README.md) for full ESP-IDF component documentation and `Kconfig` options.
See [docs/PACKAGING.md](docs/PACKAGING.md) for full packaging instructions across all platforms.

---

## Clean-Room Statement

The original Judy C library is LGPL. **No code from it has been consulted or ported.** This implementation derives strictly from published algorithm papers and shop manuals:
- Doug Baskins, [*A 10-Minute Description of How Judy Arrays Work and Why They Are So Fast*](https://judy.sourceforge.net/doc/10minutes.htm) (Hewlett-Packard, 2002)
- Alan Silverstein, [*Judy IV Shop Manual*](https://judy.sourceforge.net/doc/shop_interm.pdf) (Hewlett-Packard, 2002)

C API compatibility is defined by the documented API contract (man pages, published documentation) and validated by black-box differential testing. Licensed under **MIT OR Apache-2.0**.

---

## Citation

Expanse is archived on Zenodo. Machine-readable metadata is in [`CITATION.cff`](CITATION.cff); GitHub renders it under **Cite this repository**.

Two DOIs are minted. Cite the **concept DOI** for the project as a whole — it always resolves to the latest release — or a **version DOI** to pin the exact release you used:

| Scope | DOI |
|---|---|
| Concept (all versions) | [`10.5281/zenodo.22152112`](https://doi.org/10.5281/zenodo.22152112) |
| v0.6.0 | [`10.5281/zenodo.22569440`](https://doi.org/10.5281/zenodo.22569440) |
| v0.5.0 | [`10.5281/zenodo.22152113`](https://doi.org/10.5281/zenodo.22152113) |

```bibtex
@software{brousse_expanse,
  author  = {Brousse, Nicolas},
  title   = {{Expanse: clean-room, pure-Rust Judy arrays with a
             drop-in libjudy-compatible C ABI}},
  year    = {2026},
  version = {0.7.1},
  doi     = {10.5281/zenodo.22152112},
  url     = {https://github.com/orieg/expanse}
}
```

If your claim depends on a measured number, cite the version DOI rather than the concept DOI: figures are re-measured between releases, and several changed in v0.5.0.

---

## Contributing

[`CONTRIBUTING.md`](CONTRIBUTING.md) covers what a mergeable change looks like: the clean-room rule, `scripts/gate.sh`, the pull request flow, and the evidence standard any performance number has to meet. [`AGENTS.md`](AGENTS.md) is the full engineering guide behind it, for humans and coding agents alike. Bug, performance, and feature reports have [issue forms](.github/ISSUE_TEMPLATE) that ask for the evidence triage needs; suspected vulnerabilities go through the private channel in [`SECURITY.md`](SECURITY.md), never a public issue. Participation is covered by the [Code of Conduct](CODE_OF_CONDUCT.md).

---

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
