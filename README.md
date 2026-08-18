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
| Bit scan / rank | SWAR bit hacks, unrolled loops | Hardware `popcnt`/`tzcnt`/`lzcnt` |
| Linear search | Scalar unrolled byte compares | SIMD byte scan (SSE2/AVX2, NEON) |
| Allocation | Custom 2001 chunk/buddy allocator | Modern allocator + fixed-size slab arenas |
| Pointer layout | Full 16-byte JP per edge | Tagged pointers exploiting 48-bit virtual addressing |
| Concurrency | Single-threaded, external locks | Optimistic concurrency control for lock-free reads |

Full design: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Status

**Phase 6 — mutation engine (both flavors).** Done: type-tag encoding, geometry constants, digit extraction (Phase 1); SIMD byte-vector search (SSE2/NEON with portable parity-tested fallbacks) and the 256-bit bitmap with popcount rank/select and ordered navigation (Phase 2); cache-line-native node layouts — the 16-byte tagged `Edge` (the literature's "Judy Pointer") with level-split pop0/decode aux field, 64/128-byte linear and bitmap branches, bitmap leaves, all layout-proven by compile-time asserts (Phase 3; variable-length linear leaves land with the Phase 5 allocator); the read-only lookup engine — set-flavor `test` and map-flavor `get` over branches, bitmap leaves, immediates, full-expanse and narrow pointers, differentially tested against `BTreeSet`/`BTreeMap` models on hand-built trees and Miri-clean (Phase 4); cache-line-aligned allocation with byte-exact accounting (the future `MemUsed`/bytes-per-key source) plus the header-less linear-leaf layout and its lookup integration, leak-checked under Miri (Phase 5); the set-flavor mutation engine — `ExpanseSet` with insert/remove over the full compression ladder (immediate → linear leaf → bitmap leaf → full expanse; branch cascades L3 → L7 → bitmap → uncompressed) with 1-index delete hysteresis, a root-leaf small-population form, a structural invariant validator with a CI negative control, and model-based differential tests against `BTreeSet` across the TESTING.md key distributions (Phase 6); the map-flavor engine and `ExpanseMap` — insert-returns-old/get/remove-returns-old semantics, map immediates (value in word 0 or value-array pointer), `[values][keys]` map leaves, `LeafBitmapL` value subarrays, differentially tested against `BTreeMap` (Phase 6b); ordered navigation — first/last/next/prev, ascending iterators, O(depth) rank (`count_below`/`count_range`) and select (`by_count`) on both types, differentially tested against the `BTree` models (the engine behind the compat `First/Next/Last/Prev/Count/ByCount` surface). Phase 8 in progress: `libexpanse` now **exports the full Judy1 and JudyL families** (30 entry points incl. the `*Empty` searches) with the shipped clean-room `Judy.h`, and the COMPAT.md gate G1 **differential oracle runs in CI** — randomized op sequences through libexpanse and a dlopen'd stock libjudy must agree exactly (they do). The bench harness is in (criterion grids + deterministic bytes/key + a dlopen'd head-to-head vs stock libjudy), and **leaf-targeted narrow-pointer synthesis** landed with measured wins: clustered keys dropped from 1.34 to 0.35 B/key and clustered lookup went from 5.3× slower than stock libjudy to parity (BENCHMARKING.md). Remaining: JudySL/JudyHS, php-judy swap gates, insert-path optimization, branch-targeted skips.

Roadmap (ordering, no schedule): 1 foundation types → 2 bit/vector engine → 3 cache-aligned node layouts → 4 lookup fast path → 5 allocation subsystem → 6 mutation engine + hysteresis → 7 OCC concurrent reads → 8 differential/fuzz/bench hardening. Details in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); testing and benchmark methodology in [docs/TESTING.md](docs/TESTING.md) and [docs/BENCHMARKING.md](docs/BENCHMARKING.md).

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
