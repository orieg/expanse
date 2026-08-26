# Testing Guidelines

> Canonical testing doc. Design: [ARCHITECTURE.md](ARCHITECTURE.md) · Compat gates: [COMPAT.md](COMPAT.md) · Benchmarks: [BENCHMARKING.md](BENCHMARKING.md) · CI: [CI.md](CI.md)

Expanse is an unsafe-heavy, invariant-dense data structure whose compat story rests on behavioral equivalence with a 20-year-old C library. Testing is therefore layered: each layer catches a class of bug the others structurally cannot.

## Layers

| # | Layer | Tool | Catches |
|---|---|---|---|
| 1 | Unit tests | `cargo test`, per module | Local logic: tag encodings, digit math, SIMD lane edges, node transitions |
| 2 | Model-based op-sequence tests | Deterministic seeded harness (xorshift): random op sequences run against `BTreeMap`/`BTreeSet` as the semantic model, per key-distribution class; `proptest` with shrinking joins in Phase 8 hardening | Semantic divergence of insert/delete/get/iterate/count under arbitrary interleavings |
| 3 | Differential oracle | Same op sequences through `libexpanse` and **stock C libjudy** (dlopen'd by the harness so symbols never collide; CI job with `libjudy-dev`, active since the Phase 8 capi surface) | Contract gaps the docs under-specify; the black-box proof behind COMPAT.md G1 |
| 4 | Fuzzing | `cargo-fuzz` targets consuming op-sequence bytecode (op, key) pairs, plus `blobmap_image_corrupt` | Crashes, UB triggers, persistent snapshot corruption, pathological cascades |
| 5 | Miri | `cargo +nightly miri test` on the core crate | UB in unsafe code: aliasing, alignment, leaks, uninit reads |
| 6 | Concurrency & Loom | `loom` for OCC protocol & branch node split state machines; multi-thread stress tests | Torn reads, missed version bumps, reclamation races |
| 7 | Documentation & Visualizer Sync | `cargo test --test test_visualizer_sync` against `docs/` | Divergence between compiled Rust geometry/ladder constants and architecture visualizer representations |
| 8 | Integrations & Sanitizers | `test_expanse_memtable` with ASan, UBSan, and TSan in CI | Memory corruption, undefined behavior in arenas, and data races in concurrent leaf chaining |
| 9 | MemTable Differential Fuzzing | `test_differential_memtable` vs `std::set` / SkipList reference model | State desynchronization, incorrect MVCC sequence sorting, iterator seek errors |
| 10 | OCC Linearizability Verification | `tests/linearizability.rs` concurrent history verification harness | Non-linearizable execution traces across concurrent multi-threaded writers and readers |

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

- Runs in CI with distro `libjudy-dev`; the stock library is loaded with `dlopen`/`dlsym` at test time, so libexpanse's exported `Judy*` symbols never collide with the reference's at link time (no `judy-sys` dependency needed).
- Drives **both** stacks through the C surface, so it exercises `expanse-capi`'s marshaling too.
- Sequence generator shared with layer 2; failures re-emitted as self-contained regression cases.
- Clean-room note: linking and observing a stock binary is black-box testing; it is the *only* sanctioned way to resolve behavior the documentation leaves open (record resolved questions in COMPAT.md).

## CI mapping

Now: layer 1 on all platforms — Linux glibc, Linux musl (static-linked test run, cross-built from the glibc runner), macOS, Windows MSVC. Layers 1, 2 (deterministic model harness, active since the Phase 6 mutation engine), 3 (differential oracle, active since the Phase 8 capi surface), 5 (Miri), and 6 (loom + thread stress, active since Phase 7) run in CI. Miri is split three ways: the per-push job skips the heavy `model_*` suites **and short-circuits entirely when a PR touches no Rust sources** (it costs ~22 min against <=3 for every other job, and a docs or CI change cannot affect `unsafe`); the nightly workflow runs the full suite daily. The per-push job stays a *required* check and always reports — the path test runs inside the job, because a required check skipped at the workflow level never reports and leaves the PR pending forever. Layer 4 (fuzzing) and proptest-with-shrinking landed with the Phase 8 hardening pass — see below. Placeholders are noted in `.github/workflows/ci.yml`.

## Fuzzing and property testing (layers 4 and 2)

- **Property tests** (`crates/expanse/tests/proptest_model.rs`): generated op sequences against `BTreeSet`/`BTreeMap`, with **shrinking** — a failure reduces to a minimal counterexample instead of a 6000-op transcript. Beyond op agreement they assert the structural validator, ordered iteration, rank/select round-trips (`by_count(count_below(k)) == k` across the whole population), and drain-to-zero-bytes. Counterexamples persist in `tests/*.proptest-regressions`; **commit that file when one appears** — it is a regression test the harness replays first. Not run under Miri (proptest resolves that path via `getcwd`, which Miri's isolation forbids; the in-crate `model_*` suites carry Miri's coverage).
- **Fuzz targets** (`fuzz/fuzz_targets/`, cargo-fuzz + libFuzzer): `set_ops`, `set_algebra`, `map_ops`, `bytesmap_ops`, `strmap_ops`. Same model-agreement contract, but the coverage-guided engine discovers op shapes nobody thought to generate. `set_algebra` builds two `ExpanseSet`s from two key lists and checks every native set-algebra kernel (cardinality + materialized result + operators) against `BTreeSet`, validating each result's invariants — the overlap-prone key templates surface the lockstep descent's shared-prefix and narrow-skip alignments. Keys come from a small template set (dense / two clusters / sparse / raw) so the budget goes to sequences rather than to rediscovering that clustered keys matter; the byte-string target gets embedded NULs, shared prefixes and hash collisions for free. Every run ends with rank/select agreement, a full drain, and a `mem_used() == 0` leak check.
- **In CI**: `fuzz-smoke` discovers targets from `fuzz/Cargo.toml` (`cargo fuzz list`) and fails if a `fuzz/fuzz_targets/*.rs` file is not registered (or vice versa), so adding a target file + `[[bin]]` is sufficient to get it fuzzed; it runs 60s per target per push (build check + shallow sweep); the nightly workflow runs 20 minutes per target with the corpus cached between nights, and uploads crash artifacts on failure. First local session: ~1.9M executions across the three targets, no crashes.
- **A crash becomes a named test.** Reproduce with `cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<input>`, then write the minimal case into the normal suite — corpora and artifacts are gitignored, so a finding that lives only in `fuzz/artifacts/` is a finding that will be lost.

### Depth is a coverage dimension, not just a size

`ExpanseStrMap` is a meta-trie over 8-byte chunks, so **key length is
tree depth**. Every layer of testing originally reached only shallow
chains — the differential oracle's key generator topped out near 35
bytes, about five levels — which left the deep-chain paths (chunk
descent, emptied-node pruning, teardown) effectively uncovered in the
regime where they differ from the shallow case. That is precisely where
a recursive destructor overflows the stack, and it aborts *while
freeing*, which no caller can guard against.

Closed on three fronts: the oracle now generates keys up to 4 KiB (~512
levels, bounded because stock's own recursion depth is not inspectable
under clean-room rules); `strmap_ops` fuzzes depth explicitly with a
`Deep` key template; and unit tests run 64 KiB keys on a deliberately
small 256 KiB stack, so the guard does not depend on the default 8 MiB.

## Concurrency testing details (layer 6)

- **Loom model checks** (`RUSTFLAGS="--cfg loom" cargo test -p expanse-trie --release loom_`; CI `loom` job): exhaustively explore the `occ` module's small state machines — the seqlock version protocol (a validated read is never torn — the same construction the per-node versions use) and the EBR pin/advance pairing (the epoch never advances twice past a live pin, so nothing retired under a pin is freed). Not advisory: loom's first run found two real ordering bugs — the seqlock needed Boehm's fence construction (release fence after the odd store, acquire fence before the validating re-read; MSPC 2012), and pin/advance is a store-buffer (Dekker) pair needing SeqCst fences between each side's store and load.
- **Thread stress tests** (`sync` module, `cfg(not(miri))`): reader threads hammer lookups for a wall-clock window while a writer churns inserts/removes over cascade-heavy key distributions; every hit must return the exact value the key was mapped to (torn reads surface as garbage values), and the final state is checked against a model plus the invariant validator.
- **Scope caveat, documented in `sync`**: between a version sample and a failed validation, readers perform plain loads that race with writer stores — the classic seqlock trade (values are discarded unless validation proves no overlap). This is why the `sync` read path itself is not Miri/loom-checkable end-to-end: Miri would flag the benign-by-protocol races. Miri still covers `occ`/`alloc` and the whole single-threaded engine; loom covers the protocol; the stress tests cover the composition.

## Visualizer and documentation zero-drift verification (layer 7)

- **Sync test suite** (`crates/expanse/tests/test_visualizer_sync.rs`): Enforces that compiler constants (`ROOT_LEAF_CAP`, `BRANCH_L3_CAP`, `BRANCH_L7_CAP`, `BITMAP_TO_UNCOMPRESSED_THRESHOLD`, `MAX_LEVEL`, `BRANCH_FANOUT`, `CACHE_LINE`, `RAW_ALIGN`) and Callgrind benchmark suite definitions in `benches/instructions.rs` cannot diverge from [`docs/visualizer_data.json`](visualizer_data.json) or [`docs/architecture_visualizer.html`](architecture_visualizer.html).
- **Enforcement**: Runs as part of `cargo test --workspace` on Linux, macOS, and Windows CI. Any undocumented ladder threshold change or renamed benchmark function fails the build immediately.
