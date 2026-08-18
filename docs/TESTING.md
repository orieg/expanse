# Testing Guidelines

> Canonical testing doc. Design: [ARCHITECTURE.md](ARCHITECTURE.md) · Compat gates: [COMPAT.md](COMPAT.md) · Benchmarks: [BENCHMARKING.md](BENCHMARKING.md)

Expanse is an unsafe-heavy, invariant-dense data structure whose compat story rests on behavioral equivalence with a 20-year-old C library. Testing is therefore layered: each layer catches a class of bug the others structurally cannot.

## Layers

| # | Layer | Tool | Catches |
|---|---|---|---|
| 1 | Unit tests | `cargo test`, per module | Local logic: tag encodings, digit math, SIMD lane edges, node transitions |
| 2 | Model-based op-sequence tests | Deterministic seeded harness (xorshift): random op sequences run against `BTreeMap`/`BTreeSet` as the semantic model, per key-distribution class; `proptest` with shrinking joins in Phase 8 hardening | Semantic divergence of insert/delete/get/iterate/count under arbitrary interleavings |
| 3 | Differential oracle | Same op sequences through `libexpanse` and **stock C libjudy** (FFI, Linux job with `libjudy-dev`) | Contract gaps the docs under-specify; the black-box proof behind COMPAT.md G1 |
| 4 | Fuzzing | `cargo-fuzz` targets consuming op-sequence bytecode (op, key) pairs | Crashes, UB triggers, pathological cascades no generator thinks of |
| 5 | Miri | `cargo +nightly miri test` on the core crate | UB in unsafe code: aliasing, alignment, leaks, uninit reads |
| 6 | Concurrency | `loom` for the OCC protocol’s small state machines; multi-thread stress tests for reader/writer races (Phase 7+) | Torn reads, missed version bumps, reclamation races |

Rules of engagement:

- **Every mutation-path bug fix lands with the regression case** in layer 1 or 2.
- **SIMD/fallback parity**: every accelerated primitive (Phase 2+) has a portable fallback, and a property test asserts bit-exact agreement between the two across the input space.
- Layers 3–6 gate phases as listed in ARCHITECTURE.md §6; a phase is not "done" while its gate layer is red.

## Key-distribution matrix

Every layer ≥2 draws keys from all of these classes (the published HP test methodology's classes, plus structure-specific adversaries):

| Class | Shape | Stresses |
|---|---|---|
| Sequential | `0, 1, 2, …` | Dense low-level nodes, uncompressed branches, full expanses |
| Random | Uniform 64-bit | Deep sparse trees, immediates, narrow pointers |
| Clustered | Dense runs at random bases | Leaf↔branch cascade boundaries, bitmap leaves |
| Sparse/pathological | Keys differing only in high bytes; one key per subexpanse; common long prefixes | Narrow-pointer (decode) paths, level skipping, worst-case memory |
| Boundary | `0`, `u64::MAX`, powers of two ±1, node-capacity edges (4, 7, 15, 25, 31, 192, 256 populations) | Every compression-ladder threshold and its hysteresis twin |

## Structural invariant validator (debug builds)

A debug-only tree walker validates after mutations in tests:

- every JP's `pop0`/child-population agreement, recursively;
- leaf keys sorted and unique; branch digits sorted;
- tag/level agreement (e.g. `Leaf3` only where 3 undecoded bytes remain);
- bitmap-branch cached segment counts equal recomputed popcounts;
- compression-ladder legality (no node below its down-convert floor or above its up-convert ceiling, modulo the 1-index hysteresis band).

**Negative-control rule** (imported from php-judy's debug-mirror discipline): an assertion that has never fired is not known to work. CI must include a test that deliberately corrupts an invariant and **requires** the validator to abort — implemented since Phase 6 as `set::tests::negative_control_validator_must_fire`, which corrupts a branch `pop0` and `#[should_panic]`s on the validator. A validator job that cannot fail is deleted or fixed, never trusted.

## Differential oracle details (layer 3)

- Runs on Linux CI with distro `libjudy-dev`; bindings via a small internal `judy-oracle-sys` FFI shim (or the existing `judy-sys` crate if it links cleanly).
- Drives **both** stacks through the C surface, so it exercises `expanse-capi`'s marshaling too.
- Sequence generator shared with layer 2; failures re-emitted as self-contained regression cases.
- Clean-room note: linking and observing a stock binary is black-box testing; it is the *only* sanctioned way to resolve behavior the documentation leaves open (record resolved questions in COMPAT.md).

## CI mapping

Now: layer 1 on all platforms — Linux glibc, Linux musl (static-linked test run, cross-built from the glibc runner), macOS, Windows MSVC. Layers 1, 2 (deterministic model harness, active since the Phase 6 mutation engine), and 5 (Miri, active since Phase 4 — it caught a Stacked Borrows violation on its first run) run in CI. As phases land: layer 3 with the Phase 8 capi surface, layer 4 and proptest-with-shrinking as Phase 8 hardening, layer 6 with Phase 7. Placeholders are noted in `.github/workflows/ci.yml`.
