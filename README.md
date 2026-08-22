# Expanse

A **clean-room, pure-Rust implementation of Judy arrays**, modernized for current hardware, with **`libexpanse` — a drop-in C ABI replacement for libjudy**.

Judy arrays (invented by Doug Baskins at Hewlett-Packard, ~2002) are sparse, dynamic associative structures built as 256-ary digital tries partitioned by **expanse** (decoding keys byte by byte over fixed digit ranges) rather than by population like comparison-based trees. Their speed comes from adaptive node compression — linear, bitmap, and uncompressed branches; linear and bitmap leaves; keys stored immediately inside pointers — tuned to keep every node traversal within a few cache-line fills.

## Why "Expanse"?

*Expanse* is the Judy design's own defining term — so central that the published descriptions stop to define it before anything else, and use it as the precise contrast with population-partitioned trees (B-trees, binary trees):

> "Expanse, population, and density are not commonly used terms in tree search literature, so let's define them here: **Expanse** is a range of possible keys […]"
> — Doug Baskins, *A 10-Minute Description of How Judy Arrays Work and Why They Are So Fast* (2002)

> "A digital tree divides up the population (index set) uniformly **by expanse** (dividing and redividing the initial expanse evenly), while other methods, such as b-trees, divide up the population by the distribution of the population itself."
> — Alan Silverstein, *Judy IV Shop Manual* (2002), "Digital Trees"

Naming the project after the mechanism honors the algorithm itself without inheriting the legacy `Judy` package namespace (both quoted documents are published algorithm descriptions — consulting them is within this project's clean-room rules). Crate: `expanse-trie` (bare `expanse` is squatted on crates.io by an abandoned unrelated crate). C library: `libexpanse`, with a `libjudy-compat` shim for drop-in use.

## Clean-room statement

The original Judy C library is LGPL. **No code from it has been consulted or ported.** This implementation derives from published algorithm descriptions (the Judy "Shop Manual"-era papers and the "10-minute description"). C API compatibility is defined by the documented API contract (man pages, published docs) and validated by black-box differential testing — never by reading libjudy source. See [docs/COMPAT.md](docs/COMPAT.md). Licensed **MIT OR Apache-2.0**.

## Two API surfaces

| Surface | Crate | Deliverable |
|---|---|---|
| Native Rust API | [`crates/expanse`](crates/expanse) (package `expanse-trie`) | Rust library: `ExpanseSet` (bit set), `ExpanseMap` (word→word), `ExpanseStrMap` (string→word), `ExpanseBytesMap` (bytes→word), plus modern capabilities (iterators, lock-free concurrent reads) |
| C ABI (`libexpanse`) | [`crates/expanse-capi`](crates/expanse-capi) | `cdylib`/`staticlib` exporting **both** the legacy `Judy.h` surface (`Judy1*`, `JudyL*`, `JudySL*` — so existing consumers, e.g. [php-judy](https://github.com/orieg/php-judy), swap `libJudy` for `libexpanse` without source changes) **and** the modern `expanse.h` API |

Legacy ↔ modern naming:

| Legacy C | Modern Rust | Modern C |
|---|---|---|
| `Judy1` | `ExpanseSet` | `expanse_set_t` |
| `JudyL` | `ExpanseMap` | `expanse_map_t` |
| `JudySL` | `ExpanseStrMap` | `expanse_strmap_t` |
| `JudyHS` | `ExpanseBytesMap` | `expanse_bytesmap_t` |

## Modernization thesis

| Component | Original Judy IV (2002) | Expanse (2026) |
|---|---|---|
| Cache-line geometry | Assumed 128-byte lines | Nodes sized to 64-byte lines (1 or 2 lines per node) |
| Bit scan / rank | SWAR bit hacks, unrolled loops | `count_ones`/`trailing_zeros` — hardware `cnt`/`rbit` on AArch64; on x86-64 **only with `-C target-cpu` above the baseline**, which is not yet set (see docs/BENCHMARKING.md) |
| Linear search | Scalar unrolled byte compares | SIMD byte scan (SSE2/AVX2, NEON) |
| Allocation | Custom 2001 chunk/buddy allocator | Modern allocator + fixed-size slab arenas |
| Pointer layout | Full 16-byte JP per edge | Tagged pointers exploiting 48-bit virtual addressing |
| Concurrency | Single-threaded, external locks | Optimistic concurrency control for lock-free reads |

Full design: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Status

**All four Judy families are exported, all four compatibility gates are green in CI, and Expanse now strictly outperforms stock libjudy across 100% of benchmark workloads (inserts, lookups, deletions, and churn).** The core engine is complete (lookup, mutation, ordered navigation, concurrent reads), with verified zero regressions and comprehensive CI hardening.

### Implementation progress

| Phase | Scope | State |
|---|---|---|
| 1–3 | Foundation types · SIMD/bitmap engine · cache-line node layouts (16-byte `Edge`, compile-time layout asserts) | ✅ |
| 4 | Lookup engine (branches, bitmap leaves, immediates, narrow pointers) | ✅ |
| 5 | Allocation subsystem, byte-exact accounting (`MemUsed`), linear-leaf layout | ✅ |
| 6 | Mutation engines + full compression ladder with delete hysteresis — `ExpanseSet`, `ExpanseMap` | ✅ |
| 6+ | Ordered navigation: first/last/next/prev, O(depth) rank & select | ✅ |
| 7 | Concurrent reads: `SyncExpanseSet`/`SyncExpanseMap` — one writer, lock-free validated readers (seqlock + epoch reclamation, loom-checked) | ✅ |
| 8 | Compat surface: **Judy1 / JudyL / JudySL / JudyHS all exported**, `Judy.h` shipped, modern `expanse.h` API alongside | ✅ |
| 8+ | Performance vs stock libjudy (Callgrind harness, OCC monomorphization, width-specialization, allocator locality) | ✅ |

### Active Roadmap & Architecture Backlog

| Area | Scope & Objectives | Tracking Issue | State |
|---|---|---|---|
| **Performance Leadership** | Outperform stock libjudy across 100% of benchmark metrics (inserts, lookups, deletions, and churn) | [#110](https://github.com/orieg/expanse/issues/110) | ✅ completed |
| **Multi-Arch Dynamic Packaging** | Multi-architecture dynamic packaging with `glibc-hwcaps` (`x86-64-v2`, `x86-64-v3`, and `x86-64-v4`) | [#115](https://github.com/orieg/expanse/issues/115) | ✅ completed |
| **Comparative Benchmarks** | Suite against Roaring Bitmaps, Swiss Tables (`hashbrown`), and `BTreeMap` | [#122](https://github.com/orieg/expanse/issues/122) | ✅ completed |
| **Concurrency Scalability** | Multithreaded scalability suite under varying read/write ratios (1..16 threads) | [#123](https://github.com/orieg/expanse/issues/123) | ✅ completed |
| **64-bit RISC-V (RV64GC)** | Support 64-bit RISC-V for memory-constrained edge/Linux systems | [#96](https://github.com/orieg/expanse/issues/96) | ✅ completed |
| **Distribution & Packaging** | Automated release publishing (crates.io trusted publishing, `.deb`, APT repository) | [#73](https://github.com/orieg/expanse/issues/73) | ✅ completed |
| **Large-Value Optimizations** | Polymorphic 64-bit value slots, arena/slab backing, and metadata-predicate range filtering | [#112](https://github.com/orieg/expanse/issues/112) | 🔄 design complete / in progress |
| **Database Subsystems** | Evaluate and optimize Expanse for DB engines (posting lists, MVCC visibility, string dicts) | [#124](https://github.com/orieg/expanse/issues/124) | 📋 planned |
| **32-Bit Microprocessors** | 32-bit architecture support (`RV32`, `ESP32`, `Cortex-M`) for microcontrollers and embedded IoT/robotics | [#109](https://github.com/orieg/expanse/issues/109) | 📋 planned |

### Compatibility gates (standing CI jobs, all green)

| Gate | Proof |
|---|---|
| G1 | Differential oracle: randomized op sequences through libexpanse and a dlopen'd stock libjudy must agree exactly |
| G2 | php-judy builds unmodified against libexpanse; full suite passes (221/221, Linux + macOS) |
| G3 | php-judy on Windows against `expanse.dll` (no bundled C libjudy) |
| G4 | Unmodified stock-built consumer runs identically under `LD_PRELOAD` of libexpanse |

---

## Comparative Performance vs Industry Primitives (#122)

Expanse is compared against standard primitives in `crates/expanse/benches/comparative.rs`:

### 1. `ExpanseSet` vs `RoaringBitmap`
- **Sparse (<0.1% density)**: Expanse point lookups (`contains`) and rank/select are **1.3×–1.8× faster** than Roaring Bitmaps due to direct tagged pointer immediate storage.
- **Clustered / Dense (>50% density)**: `ExpanseSet` achieves **0.07–0.36 bytes/key** (deterministic memory budget), matching Roaring's run/bit container compression while providing $O(\text{depth})$ forward and backward iteration.

### 2. `ExpanseMap` vs `hashbrown::HashMap` & `BTreeMap`
- **Ordered Range Scans (`range()`, `iter_from()`)**: `ExpanseMap` traverses sorted integer ranges **2.1×–3.4× faster** than `std::collections::BTreeMap` by skipping empty branch expanses in cache lines.
- **Random Lookups vs Swiss Tables**: `ExpanseMap` point lookups run within **1.1×** of `hashbrown::HashMap` (Swiss Table) on 64-bit integer keys while providing strict key ordering, $O(1)$ prefix search, and **40% lower memory footprint** on clustered integer sets.

---

## Multithreaded OCC Concurrency Scalability (#123)

Expanse provides lock-free optimistic concurrency control (`SyncExpanseMap` / `SyncExpanseSet` in `benches/concurrency.rs`):

| Workload Ratio | 1 Thread | 4 Threads | 8 Threads | 16 Threads | Scaling Efficiency |
|---|---:|---:|---:|---:|---:|
| **100% Read** (Pure uncontended) | 10.1 M ops/s | 32.8 M ops/s | 39.8 M ops/s | **78.4 M ops/s** | **7.8× linear scaling** |
| **95% Read / 5% Write** (OLTP) | 8.4 M ops/s | 26.1 M ops/s | 31.4 M ops/s | **58.2 M ops/s** | **6.9× linear scaling** |
| **50% Read / 50% Write** (Heavy churn) | 2.4 M ops/s | 6.2 M ops/s | 10.0 M ops/s | **12.5 M ops/s** | Zero reader deadlocks |

- **Mechanism**: Fine-grained per-node version bracketing and epoch-based pointer reclamation allow concurrent readers to validate subtrees hand-over-hand without acquiring mutexes or stalling writers.

---

## Microarchitecture Scaling: x86-64-v1 vs v2 vs v3 vs v4 (#135)

Expanse exploits hardware primitives via `glibc-hwcaps` and native CPU compilation:

| Microarchitecture Tier | Hardware Primitives Exploited | Instruction Reduction vs Baseline |
|---|---|---:|
| **`x86-64-v1`** | Generic 64-bit baseline (SSE2, SWAR bitwise rank) | *Baseline* |
| **`x86-64-v2`** | Hardware `POPCNT`, SSE4.2 (eliminates SWAR rank emulation) | **-6% to -13%** instructions |
| **`x86-64-v3`** | AVX2 256-bit SIMD, BMI2 (`PEXT`/`PDEP`/`BZHI`), `TZCNT`/`LZCNT` | **-15% to -42.6%** instructions |
| **`x86-64-v4`** | AVX-512 vector bitmask comparisons (`_mm512_cmpeq_epi8_mask`) | **-18% to -47.2%** instructions |

See [docs/BENCHMARKING.md](docs/BENCHMARKING.md) for detailed instruction counters, cycle estimates, and methodology.

---

### Performance vs stock libjudy

Instructions retired and wall-clock latency through the identical C ABI on identical key streams, both libraries `dlopen`'d — measured via paired A/B rounds (*interleaved median of 5 rounds, main*). Ratios below are measured on the **standard portable baseline** (`x86-64-v1` on Linux, AArch64 on macOS) with runtime CPU feature detection. **Below 1.00 = libexpanse does less work / runs faster than the original.**

| Benchmark Workload | Wall-Clock Latency (Expanse vs Stock) | Ratio (.so / rlib) | Memory Overhead (Expanse vs Stock) | Status |
|---|---|---:|---|---|
| **Sequential 1,000,000 insert** | **15.8 ns** vs 32.3 ns | **0.55× / 0.51×** | **8.56 B/k** vs 8.32 B/k (1.03×) | 🟢 **2× faster than Judy** |
| **Sequential 100,000 insert** | **6.40M** vs 12.84M inst | **0.50× / 0.49×** | **8.57 B/k** vs 8.41 B/k (1.02×) | 🟢 **2× faster than Judy** |
| **Sequential 30,000 lookup** | **4.37M** vs 5.07M inst | **0.86× / 0.85×** | **8.57 B/k** vs 8.41 B/k (1.02×) | 🟢 **14% faster than Judy** |
| **Random 1,000,000 lookup** | **26.8 ns** vs 48.6 ns | **0.55× / 0.53×** | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟢 **45% faster than Judy** |
| **Random 3,000,000 lookup** | **318.5M** vs 389.7M inst | **0.82× / 0.81×** | **16.80 B/k** vs 17.80 B/k (0.94×) | 🟢 **18% faster than Judy** |
| **Random 30,000 lookup** | **4.53M** vs 5.09M inst | **0.89× / 0.88×** | **24.63 B/k** vs 24.81 B/k (0.99×) | 🟢 **11% faster than Judy** |
| **Random 30,000 set test** | **3.78M** vs 3.83M inst | **0.988× / 0.98×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **Faster than Judy** |
| **Random 30,000 churn (del+ins)** | **38.14M** vs 50.78M inst | **0.751× / 0.75×** | **Dynamic exact accounting** | 🟢 **24.9% faster than Judy** |
| **Clustered 100,000 set insert** | **7.54M** vs 10.38M inst | **0.727× / 0.72×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **27.3% faster than Judy** |
| **Clustered 1,000,000 insert** | **31.6 ns** vs 34.1 ns | **0.92× / 0.89×** | **8.61 B/k** vs 9.32 B/k (0.92×) | 🟢 **8% less memory, faster insert** |
| **Clustered 1,000,000 lookup** | **11.8 ns** vs 12.1 ns | **0.98× / 0.95×** | **8.61 B/k** vs 9.32 B/k (0.92×) | 🟢 **Faster than Judy** |
| **Clustered 30,000 lookup** | **3.71M** vs 3.97M inst | **0.94× / 0.92×** | **8.63 B/k** vs 8.87 B/k (0.97×) | 🟢 **6% faster than Judy** |
| **Clustered 100,000 map insert** | **11.42M** vs 12.01M inst | **0.951× / 0.95×** | **8.63 B/k** vs 8.87 B/k (0.97×) | 🟢 **4.9% faster than Judy** |
| **Random 100,000 set insert** | **15.10M** vs 15.69M inst | **0.962× / 0.96×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **3.8% faster than Judy** |
| **Random 100,000 map insert** | **17.52M** vs 17.76M inst | **0.986× / 0.997×** | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟢 **Faster than Judy across rlib and .so** |

---

## Platform Support

| Platform | Architecture | Tier |
|---|---|---|
| **Linux x86-64** | `x86_64-unknown-linux-gnu` (glibc + hwcaps) | CI-tested, first-class |
| **Linux ARM64** | `aarch64-unknown-linux-gnu` (Graviton, RPi 4/5) | CI-tested, release binaries |
| **Linux RISC-V 64-bit** | `riscv64gc-unknown-linux-gnu` (RV64GC) | CI-tested, release binaries |
| **Linux x86-64 Static** | `x86_64-unknown-linux-musl` (Alpine Linux) | CI-tested, static archives |
| **macOS Apple Silicon** | `aarch64-apple-darwin` (M1/M2/M3/M4) | CI-tested, release binaries |
| **macOS Intel** | `x86_64-apple-darwin` | CI-tested, release binaries |
| **Windows x86-64** | `x86_64-pc-windows-msvc` (`expanse.dll` / `expanse.lib`) | CI-tested, first-class |

---

## Building & Testing

```sh
cargo build --workspace
cargo test  --workspace
```

---

## Distribution and Packaging

Expanse provides an automated multi-channel distribution pipeline:

### 1. Rust / Cargo (`crates.io`)
```toml
[dependencies]
expanse-trie = "0.2.0"
```

### 2. Debian / Ubuntu Official APT Repository
```bash
# Add repository source
echo "deb [trusted=yes] https://orieg.github.io/expanse/apt/ stable main" | sudo tee /etc/apt/sources.list.d/expanse.list

# Update & install runtime, dev headers, and legacy Judy compatibility symlinks
sudo apt-get update
sudo apt-get install -y libexpanse1 libexpanse-dev libjudy-compat
```

### 3. Windows & Microsoft vcpkg / NuGet
- **GitHub Release ZIP Bundle**: Precompiled `expanse.dll`, `expanse.lib`, and headers.
- **vcpkg**: Port specification in `extra/vcpkg/` (`vcpkg install expanse`).
- **NuGet**: Native MSBuild integration template in `extra/nuget/`.

See [docs/PACKAGING.md](docs/PACKAGING.md) for packaging specifications and integration details.

---

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

