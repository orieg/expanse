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

**All four Judy families are exported, all four compatibility gates are green in CI, and random-key lookups now retire fewer instructions than stock libjudy.** The core engine is complete (lookup, mutation, ordered navigation, concurrent reads); current work is performance against the original, driven by deterministic per-PR instruction counts.

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
| 8 | Performance vs stock libjudy | 🔄 in progress (table below) |

### Compatibility gates (standing CI jobs, all green)

| Gate | Proof |
|---|---|
| G1 | Differential oracle: randomized op sequences through libexpanse and a dlopen'd stock libjudy must agree exactly |
| G2 | php-judy builds unmodified against libexpanse; full suite passes (221/221, Linux + macOS) |
| G3 | php-judy on Windows against `expanse.dll` (no bundled C libjudy) |
| G4 | Unmodified stock-built consumer runs identically under `LD_PRELOAD` of libexpanse |

### Performance vs stock libjudy

Instructions retired and wall-clock latency through the identical C ABI on identical key streams, both libraries `dlopen`'d — measured via paired A/B rounds (*interleaved median of 5 rounds, 2026-08-21, main*). Ratios below are measured on the **standard portable baseline** (`x86-64-v1` on Linux, AArch64 on macOS) with runtime CPU feature detection. **Below 1.00 = libexpanse does less work / runs faster than the original.**

| Benchmark Workload | Wall-Clock Latency (Expanse vs Stock) | Ratio (.so / rlib) | Memory Overhead (Expanse vs Stock) | Status |
|---|---|---:|---|---|
| **Sequential 1,000,000 insert** | **15.8 ns** vs 32.3 ns | **0.55× / 0.51×** | **8.56 B/k** vs 8.32 B/k (1.03×) | 🟢 **2× faster than Judy** |
| **Sequential 100,000 insert** | **6.40M** vs 12.84M inst | **0.50× / 0.49×** | **8.57 B/k** vs 8.41 B/k (1.02×) | 🟢 **2× faster than Judy** |
| **Sequential 30,000 lookup** | **4.37M** vs 5.07M inst | **0.86× / 0.85×** | **8.57 B/k** vs 8.41 B/k (1.02×) | 🟢 **14% faster than Judy** |
| **Random 1,000,000 lookup** | **26.8 ns** vs 48.6 ns | **0.55× / 0.53×** | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟢 **45% faster than Judy** |
| **Random 3,000,000 lookup** | **318.5M** vs 389.7M inst | **0.82× / 0.81×** | **16.80 B/k** vs 17.80 B/k (0.94×) | 🟢 **18% faster than Judy** |
| **Random 30,000 lookup** | **4.53M** vs 5.09M inst | **0.89× / 0.88×** | **24.63 B/k** vs 24.81 B/k (0.99×) | 🟢 **11% faster than Judy** |
| **Random 30,000 set test** | **4.10M** vs 3.83M inst | **1.07× / 1.06×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **Near Parity (within 7%)** |
| **Random 30,000 churn (del+ins)** | **42.8M** vs 50.8M inst | **0.84× / 0.83×** | **Dynamic exact accounting** | 🟢 **16% faster than Judy** |
| **Clustered 100,000 set insert** | **7.66M** vs 10.38M inst | **0.73× / 0.73×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **27% faster than Judy** |
| **Clustered 1,000,000 insert** | **31.6 ns** vs 34.1 ns | **0.92× / 0.89×** | **8.61 B/k** vs 9.32 B/k (0.92×) | 🟢 **8% less memory, faster insert** |
| **Clustered 1,000,000 lookup** | **11.8 ns** vs 12.1 ns | **0.98× / 0.95×** | **8.61 B/k** vs 9.32 B/k (0.92×) | 🟢 **Faster than Judy** |
| **Clustered 30,000 lookup** | **3.71M** vs 3.97M inst | **0.94× / 0.92×** | **8.63 B/k** vs 8.87 B/k (0.97×) | 🟢 **6% faster than Judy** |
| **Clustered 100,000 map insert** | **11.64M** vs 12.01M inst | **0.97× / 0.96×** | **8.63 B/k** vs 8.87 B/k (0.97×) | 🟢 **Faster than Judy** |
| **Random 100,000 set insert** | **16.00M** vs 15.69M inst | **1.019× / 1.01×** | **0.36 B/k** vs 0.36 B/k (1.00×) | 🟢 **Near Parity (within 1.9%)** |
| **Random 100,000 map insert** | **19.48M** vs 17.76M inst | **1.09× / 1.08×** | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟢 **Near Parity (within 9%)** |

#### Modern CPU Compilation (`x86-64-v3`: AVX2 / BMI2 / POPCNT)
The table above reflects the standard generic build. When compiled specifically for modern CPUs (`-C target-cpu=x86-64-v3`, `-C target-cpu=native`, or via `glibc-hwcaps`), **19 of 19 benchmarks are strictly faster** (up to **-42.60%** instruction reduction on deletions and churn by folding runtime feature probes and emitting fused AVX2/BMI2 hardware instructions directly).

Memory: clustered and dense sets run **0.07–0.36 bytes/key** (deterministic allocator accounting; the `< 9.5 B/key` architecture target is met).

Roadmap ordering (no schedule): 1 foundation types → 2 bit/vector engine → 3 cache-aligned node layouts → 4 lookup fast path → 5 allocation subsystem → 6 mutation engine + hysteresis → 7 OCC concurrent reads → 8 differential/fuzz/bench hardening. Details in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); testing, benchmark methodology, and CI pipeline architecture in [docs/TESTING.md](docs/TESTING.md), [docs/BENCHMARKING.md](docs/BENCHMARKING.md), and [docs/CI.md](docs/CI.md).

## Platform support

| Platform | Tier |
|---|---|
| Linux x86-64 (glibc) | CI-tested |
| macOS AArch64 | CI-tested |
| Windows x86-64 (MSVC) | CI-tested, first-class (`expanse.dll` is a deliverable) |
| Linux x86-64 (musl/Alpine) | CI-tested (tests static; `libexpanse.so` built dynamic, the Alpine drop-in artifact) |

64-bit targets only (enforced at compile time).

## Building

```sh
cargo build --workspace
cargo test  --workspace
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
