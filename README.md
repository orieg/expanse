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

Instructions retired and wall-clock latency through the identical C ABI on identical key streams, both libraries `dlopen`'d — measured in CI via deterministic Callgrind and paired wall-clock rounds *(measured: GitHub ubuntu-latest CI, 2026-08-20, main)*. **Below 1.00 = libexpanse does less work / runs faster than the original.**

| Benchmark Workload | Wall-Clock Latency (Expanse vs Stock) | Ratio | Memory Overhead (Expanse vs Stock) | Status |
|---|---|---:|---|---|
| **Random 1,000,000 lookup** | **36.5 ns** vs 60.5 ns | **0.60×** | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟢 **40% faster than Judy** |
| **Random 100,000 lookup** | **18.8 ns** vs 30.6 ns | **0.62×** | **24.62 B/k** vs 24.81 B/k (0.99×) | 🟢 **38% faster than Judy** |
| **Clustered 100,000 lookup** | **16.3 ns** vs 16.3 ns | **1.00×** | **8.62 B/k** vs 8.87 B/k (0.97×) | 🟢 **Parity with Judy** |
| **Random 1,000,000 insert** | **120.7 ns** vs 120.0 ns | **1.01×** | **16.70 B/k** vs 17.67 B/k (0.95×) | 🟢 **Parity with Judy** |
| **Clustered 1,000,000 memory** | — | — | **8.61 B/k** vs 9.32 B/k (0.92×) | 🟢 **8% less memory than Judy** |
| **Clustered 100,000 insert** | 45.6 ns vs 38.6 ns | 1.18× | 8.62 B/k vs 8.87 B/k (0.97×) | 🟡 Closing the gap |
| **Sequential 1,000,000 lookup** | 27.1 ns vs 23.7 ns | 1.14× | 8.56 B/k vs 8.32 B/k (1.03×) | 🟡 Closing the gap |
| **Sequential 100,000 insert** | 60.3 ns vs 34.9 ns | 1.73× | 8.57 B/k vs 8.41 B/k (1.02×) | 🔄 Issue #32 target |

#### Modern Architecture (`x86-64-v3`: AVX2 / BMI2 / POPCNT)
When compiled for modern CPUs (`-C target-cpu=x86-64-v3` or via `glibc-hwcaps`), **19 of 19 benchmarks are strictly faster** (up to **-42.60%** instruction reduction on deletions and churn).

Memory: clustered and dense sets run **0.07–0.36 bytes/key** (deterministic allocator accounting; the `< 9.5 B/key` architecture target is met).

### What remains

- **Sequential run bypass across tree levels (Issue #32)**: eliding branch descent for contiguous key streaks to close the remaining sequential insert gap.
- **AVX2 / SSE2 SIMD vector scans for linear leaves**: evaluating 16 keys in parallel in 4 CPU instructions.
- **Branch locality & 64-byte alignment**: minimizing cache misses during wide branch traversals.

Roadmap ordering (no schedule): 1 foundation types → 2 bit/vector engine → 3 cache-aligned node layouts → 4 lookup fast path → 5 allocation subsystem → 6 mutation engine + hysteresis → 7 OCC concurrent reads → 8 differential/fuzz/bench hardening. Details in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); testing and benchmark methodology in [docs/TESTING.md](docs/TESTING.md) and [docs/BENCHMARKING.md](docs/BENCHMARKING.md).

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
