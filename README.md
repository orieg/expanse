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

Instructions retired through the identical C ABI on identical key streams, both libraries `dlopen`'d — deterministic callgrind counts, re-measured on every PR *(measured: GitHub ubuntu-latest CI, 2026-08-19, PR #20)*. **Below 1.00 = libexpanse does less work than the original.**

| Operation (random unless noted) | ratio | |
|---|---:|---|
| Lookup, 1.5M keys | **0.97×** | ✅ below stock |
| Lookup, 30k keys | **0.98×** | ✅ below stock |
| Set insert, clustered | 1.07× | |
| Set membership test | 1.24× | |
| Lookup, clustered | 1.27× | |
| Set insert | 1.40× | |
| Map insert | 1.47× | |
| Lookup, sequential | 1.50× | |
| Map insert, sequential | 1.68× | |
| Steady-state churn (upsert+insert+delete) | 2.27× | ← current focus |

Instructions are **cost, not time** — wall-clock confirmation on quiet hardware is tracked in [docs/BENCHMARKING.md](docs/BENCHMARKING.md), which also records every retraction and measurement-method fix behind these numbers. Memory: clustered/dense sets run **0.12–0.36 bytes/key** (deterministic allocator accounting; the `< 9.5 B/key` architecture target is met).

### What remains

- Close the insert gap (1.4–1.7×): the insert-path recursion and remaining allocator traffic are the measured targets.
- Steady-state churn at 2.27× — the newest benchmark arm and the widest remaining gap.
- Wall-clock headline numbers from a quiet host (instruction counts are the per-PR truth; time claims wait for real hardware).
- glibc-hwcaps `x86-64-v2` sub-package (dispatch-free popcnt build; runtime dispatch already serves the baseline binary).

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
