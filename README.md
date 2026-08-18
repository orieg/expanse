# judy-rs

A **clean-room, pure-Rust implementation of Judy arrays**, modernized for current hardware, with a **drop-in C ABI replacement for libjudy**.

Judy arrays (invented by Doug Baskins at Hewlett-Packard, ~2002) are sparse, dynamic associative structures built as 256-ary digital tries partitioned by *expanse* (decoding keys byte by byte) rather than by population like comparison-based trees. Their speed comes from adaptive node compression — linear, bitmap, and uncompressed branches; linear and bitmap leaves; keys stored immediately inside pointers — tuned to keep every node traversal within a few cache-line fills.

## Clean-room statement

The original Judy C library is LGPL. **No code from it has been consulted or ported.** This implementation derives from published algorithm descriptions (the Judy "Shop Manual"-era papers and the "10-minute description"). C API compatibility is defined by the documented API contract (man pages, published docs) and validated by black-box differential testing — never by reading libjudy source. See [docs/COMPAT.md](docs/COMPAT.md). Licensed **MIT OR Apache-2.0**.

## Two API surfaces

| Surface | Crate | Deliverable |
|---|---|---|
| Native Rust API | [`crates/judy`](crates/judy) (package `judy-rs`) | Rust library: `Judy1` (bit set), `JudyL` (word→word map), `JudySL` (string→word map), plus modern capabilities (iterators, lock-free concurrent reads) |
| Classic C ABI | [`crates/judy-capi`](crates/judy-capi) | `cdylib`/`staticlib` exporting the `Judy.h` surface (`Judy1*`, `JudyL*`, `JudySL*`) so existing consumers — e.g. [php-judy](https://github.com/orieg/php-judy) — can swap `libJudy` for this library without source changes |

## Modernization thesis

| Component | Original Judy IV (2002) | judy-rs (2026) |
|---|---|---|
| Cache-line geometry | Assumed 128-byte lines | Nodes sized to 64-byte lines (1 or 2 lines per node) |
| Bit scan / rank | SWAR bit hacks, unrolled loops | Hardware `popcnt`/`tzcnt`/`lzcnt` |
| Linear search | Scalar unrolled byte compares | SIMD byte scan (SSE/AVX2, NEON) |
| Allocation | Custom 2001 chunk/buddy allocator | Modern allocator + fixed-size slab arenas |
| Pointer layout | Full 16-byte JP per edge | Tagged pointers exploiting 48-bit virtual addressing |
| Concurrency | Single-threaded, external locks | Optimistic concurrency control for lock-free reads |

Full design: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Status

**Phase 1 — core types.** Type-tag encoding, geometry constants, and digit extraction are implemented and tested. The capi crate is a compiling stub (packaging and CI artifacts exercised; no exported symbols yet).

Roadmap (ordering, no schedule): 1 foundation types → 2 bit/vector engine → 3 cache-aligned node layouts → 4 lookup fast path → 5 allocation subsystem → 6 mutation engine + hysteresis → 7 OCC concurrent reads → 8 differential/fuzz/bench hardening. Details in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); testing and benchmark methodology in [docs/TESTING.md](docs/TESTING.md) and [docs/BENCHMARKING.md](docs/BENCHMARKING.md).

## Platform support

| Platform | Tier |
|---|---|
| Linux x86-64 (glibc) | CI-tested |
| macOS AArch64 | CI-tested |
| Windows x86-64 (MSVC) | CI-tested, first-class (capi DLL is a deliverable) |
| Linux musl (Alpine) | Planned (differential/oracle phase) |

64-bit targets only (enforced at compile time).

## Building

```sh
cargo build --workspace
cargo test  --workspace
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
