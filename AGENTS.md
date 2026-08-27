# AGENTS.md — Multi-Agent Engineering & Quality Guide for Expanse

Welcome to **Expanse**. This document establishes mandatory engineering, architectural, and safety standards for all autonomous AI coding agents (Claude, Gemini, Antigravity, Cursor, Copilot, Cline, OpenAI, Devin, Aider, etc.) interacting with this repository.

> **Canonical source.** `AGENTS.md` is the single canonical agent guide for this repo. `CLAUDE.md` and `GEMINI.md` are **symlinks** to this file, and `.github/copilot-instructions.md` is a thin pointer back here. **Make every edit in `AGENTS.md` only** — editing a symlink target elsewhere would fork the guidance and reintroduce the drift this consolidation removed.

---

## 1. Project Mission & Identity

**Expanse** is a clean-room, pure-Rust implementation of Judy arrays modernized for current hardware, accompanied by `libexpanse` — a high-performance, drop-in C ABI replacement for `libjudy`.

Named for Judy's defining structural invariant: **partitioning digital trees by key *expanse*, rather than population**.

**Both 64-bit and 32-bit targets are supported.** The 64-bit engine is the primary surface; a parallel real 32-bit trie (`trie32`/`set32`/`map32`/`blobmap32`, shipped in #230) compiles unconditionally, and on 32-bit targets the public aliases re-point (`ExpanseMap` → `ExpanseMap32`, etc.). `lib.rs` carries a `compile_error!` that fires only on targets that are **neither** 64- nor 32-bit.

### Workspace Structure
- **`crates/expanse`** (`package: expanse-trie`): Core algorithmic engine `#![no_std]` (with `extern crate alloc`).
- **`crates/expanse-capi`** (`package: expanse-capi`): C ABI shared (`libexpanse.so` / `expanse.dll` / `libexpanse.dylib`) and static (`libexpanse.a` / `expanse.lib`) libraries providing both modern `expanse_*` and legacy `Judy*` symbols.
- **`crates/expanse-py`**: PyO3 native Python extension.
- **`crates/expanse-node`**: napi native Node.js addon.
- **`crates/expanse-rb`**: magnus native Ruby extension.
- **`crates/expanse-wasm`**: WebAssembly (wasm-bindgen) surface.
- **`crates/expanse-php`**: PHP native extension.
- **`bindings/`**: language SDK packages that wrap the native surfaces — `bindings/java` (Panama FFM), `bindings/dotnet` (P/Invoke .NET), plus `bindings/go`, `bindings/python`, `bindings/ruby`, `bindings/php` packaging.
- **`include/expanse.hpp`**: header-only C++20 wrapper over the C ABI.

### Canonical Documentation Hierarchy
Do not scatter architecture notes into arbitrary files. Update the canonical documents; do not create new `.md` files.

| Content | Home |
|---|---|
| Design / node layouts / roadmap / phase gates | `docs/ARCHITECTURE.md` |
| Bit-level encoding: `Edge`/`Edge32` fields, tag discriminants, immediate capacity, `ValueSlot`, bitmap rank, OCC version words | `docs/ARCHITECTURE.md` §10 (constants gated by `crates/expanse/tests/test_encoding_reference_sync.rs`) |
| Algorithms, search kernels, SIMD/SWAR vectorization & visualizer reference | `docs/ALGORITHMS.md` · `docs/architecture_visualizer.html` |
| Hardware capability reference: ISA primary-source citations, assumption validation, and missed-opportunity analysis | `docs/HARDWARE.md` |
| C ABI contract, drop-in parity gates, surface, packaging, doc-gap resolutions | `docs/COMPAT.md` |
| Test methodology, differential testing, invariants validator, fuzzing | `docs/TESTING.md` |
| Benchmark methodology, instruction counting, hardware counters, profiling | `docs/BENCHMARKING.md` |
| CI job catalog, the single rollup gate, path filtering, Miri tiering, regression gating | `docs/CI.md` |
| Distribution & packaging across all ecosystems | `docs/PACKAGING.md` |
| Database-engine subsystem patterns & integration blueprints | `docs/DATABASE.md` |
| Large-value / blob-arena storage design | `docs/design/large-values.md` |
| 32-bit embedded architecture design | `docs/design/32-bit-embedded.md` |
| Language binding references | `docs/bindings/python.md` · `docs/bindings/java.md` · `docs/bindings/php.md` · `docs/bindings/ruby.md` |
| Status + platform tiers | `README.md` (Status section) |

---

## 2. Core Judy Philosophy & Architectural Integrity (Non-Negotiable)

Expanse is fundamentally defined by the **Judy digital tree architecture**. Every proposal, feature, extension, optimization, and bug fix MUST strictly preserve this philosophy and must NEVER introduce architectural impedance mismatches.

### 2.1 The Core Invariants
1. **Partitioning by Key Expanse, Not Population**:
   - Digital tree (radix trie) hierarchy: keys are decomposed into digits (1 byte per level) from MSB to LSB.
   - Dynamic adaptive compression: nodes transition dynamically between Immediates (0 heap allocation), Linear leaves/branches, Bitmap nodes (POPCNT-indexed), and Uncompressed branches as population density shifts within each byte expanse.
   - Memory consumption scales with populated key expanse, never wasting cache lines on sparse empty pointers or rigid page padding.
2. **Compact Machine-Word Tagged Edges & Value Slots**:
   - Every pointer/edge is a strictly packed, cache-dense descriptor (16 bytes on 64-bit; 8 bytes on 32-bit targets).
   - `ValueSlot` in leaf arrays is strictly a single machine word (`u64` on 64-bit targets, `u32` on 32-bit targets), ensuring exactly 8 slots per 64-byte cache line and preserving JudyL C ABI `*mut Word` drop-in compatibility.
3. **Zero Heap Allocation for Small Payloads & Sparse Keys**:
   - Keys and values $\le$ immediate capacity are packed directly inside the edge or value slot without heap allocation.
4. **Deterministic, Cache-Conscious Microarchitecture**:
   - 64-byte/128-byte cache-line alignment, vector SIMD byte searches, SWAR/POPCNT rank operations, and branchless linear probing.

### 2.2 Strictly Forbidden Architectural Anti-Patterns (Impedance Mismatches)
Never propose or graft foreign data structures or complected models onto Expanse:
- ❌ **Fat Slots / Multi-Word Leaf Arrays**: Never widen `ValueSlot` or `Edge` beyond a single machine word to pack domain attributes or secondary metadata. Doing so halves leaf cache-line density (4 vs 8 slots/line) and breaks JudyL C ABI drop-in compatibility.
- ❌ **Complecting Index and Columnar Attributes**: If auxiliary metadata, zone maps, or column data are needed (e.g. for large-value blob filtering), they MUST be maintained in **decoupled columnar sidecars or chunk headers**—never squeezed into or complected with the core trie index words.
- ❌ **B-Tree / Hash Table Hybrid Mutations**: Never mutate digital tree branches into page-based B-trees, skip-lists, or hash buckets. Expanse is an ordered digital trie; $O(k)$ key descent and span-based compaction are non-negotiable.
- ❌ **Global Locking or Coarse Mutexes**: Concurrency must adhere strictly to lock-free Optimistic Concurrency Control (OCC seqlock version bracketing + Epoch-Based Reclamation), preserving single-threaded zero-overhead performance.
- ❌ **Breaking Legacy C ABI Parity**: Modern `expanse_*` capabilities extend the engine, but must never distort or compromise the zero-overhead C ABI parity for legacy `Judy1*`, `JudyL*`, `JudySL*`, and `JudyHS*` symbols.

---

## 3. Clean-Room Discipline (Strict & Non-Negotiable)

1. **Zero Exposure to LGPL Source**: The original `libjudy` is LGPL. **Never view, consult, decompile, or port original C source code** — not for inspiration, nor to resolve behavioral edge cases.
2. **Contract & Black-Box Differential Validation**: Compatibility questions are answered strictly through:
   - Official published documentation and man pages.
   - Black-box differential testing against compiled stock binaries (`stock-oracle`). Record doc-gap resolutions in `docs/COMPAT.md`.
3. **References**: The `references/` directory holds algorithm papers and Shop Manuals. These are for design context only, are gitignored, and must never be checked into git.

---

## 4. Naming Conventions & Core Invariants

- **Rust Type Names** (legacy ↔ modern type map):
  - Judy1 → `ExpanseSet`
  - JudyL → `ExpanseMap`
  - JudySL → `ExpanseStrMap`
  - JudyHS → `ExpanseBytesMap`
- **Core Identifiers**:
  - Core trie types in `expanse-trie` **never use Judy terminology**.
  - Use `Edge` (not `JudyPointer`/`JP`), `EdgeType`/`EdgeTag` (not `JpType`).
  - Judy symbols belong exclusively in `expanse-capi` and `docs/COMPAT.md`. Published-doc terminology ("Judy Pointer"/JP, Judy1/JudyL) may appear in core comments strictly as literature/compat references.
- **C ABI Prefixes**:
  - Modern API functions use the `expanse_` prefix (e.g. `expanse_map_get`); new C capabilities always use it.
  - Compat symbols retain exact `Judy1*`, `JudyL*`, `JudySL*`, `JudyHS*` signatures and **never change semantics**.

---

## 5. Rust Standards & Quality Gates

### Language & Edition
- **Rust Edition 2024**, MSRV `1.88` — enforced by the `Core / MSRV 1.88 Build` CI job, not just declared. The floor is set by let-chains in `crates/expanse/src/strmap.rs` (stable in 1.88) and by `napi-build` (requires 1.88). It was previously declared as `1.85`, which no toolchain could actually build; corrected after measuring (1.85 and 1.86 fail, 1.88 passes).
- **64-bit and 32-bit targets supported** — a `compile_error!` in `lib.rs` fires only on targets that are neither 64- nor 32-bit.

### Mandatory Local Gates (Must Pass 100% Before Committing)

**Run them as one command — mirrors CI's `lint` + `test` jobs:**
```bash
scripts/gate.sh            # fmt · clippy · workspace tests (PROPTEST_CASES=500) · repo scripts · docs hygiene
scripts/gate.sh --miri     # additionally the Tier-1 Miri filter (the per-PR CI scope)
```
The local test step excludes `expanse-php` (PHP headers, as CI does) and
`expanse-py` (its PyO3 test binary needs `libpythonX.Y` on the rpath); CI runs
both. `--with-bindings` includes them once your toolchain is set up. CI remains
the authority — a green local gate is necessary, not sufficient.

Individually, if you need to run one:
```bash
# 1. Code formatting
cargo fmt --all --check

# 2. Strict linter (zero warnings permitted)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Workspace test suite (expanse-php needs PHP headers; CI excludes it here
#    and covers it in the dedicated test-php / php-judy-* jobs)
cargo test --workspace --exclude expanse-php

# 4. Randomized model testing (heavy iterations) — CI sets this on the `test` job
PROPTEST_CASES=500 cargo test --test proptest_model

# 5. Miri: run the SAME Tier-1 filter CI runs per PR. Do NOT run the full
#    suite locally (it is slow and the nightly CI job is the authority).
cargo miri test -p expanse-trie --lib -- leaf:: node:: slot:: alloc:: bits:: types:: \
  blobmap::tests::deferred strmap::tests::deferred bytesmap::tests::deferred
```

### Unsafe Code & Undocumented Unsafe Blocks
- Expanse operates on low-level tagged pointer representations and raw memory layouts.
- **Every `unsafe` block MUST be preceded by an explicit `// SAFETY:` rationale comment** explaining pointer validity, lifetime guarantees, alignment, and bounds preservation (clippy `undocumented_unsafe_blocks` is on).
- **Stacked Borrows / Tree Borrows Hygiene**: Avoid creating temporary unique references (`&mut *ptr`) from raw pointers where ancestor/subfield borrows exist. Prefer `&raw mut` / `core::ptr::addr_of_mut!` and raw pointer manipulation to avoid invalidating pointer tags in the borrow stack.
- Every SIMD/intrinsic path has a portable fallback plus a parity test (see `docs/TESTING.md`). Public items are documented (`missing_docs` warns).

---

## 6. Performance Engineering & Fast Iteration Cycle

### Fast Remote Benchmark Validation
Before proposing or pushing performance-sensitive changes, run deterministic Callgrind profiling on a dedicated quiet benchmark host to ensure zero instruction regressions. Reference the host by an environment placeholder — never a personal hostname or home path:

```bash
rsync -az --exclude 'target' --exclude '.git' ./ "$BENCH_HOST:$BENCH_REPO/" && \
ssh "$BENCH_HOST" "export PATH=\$HOME/.cargo/bin:\$HOME/.local/bin:\$PATH; \
  export LD_LIBRARY_PATH=\$HOME/.local/lib:\$LD_LIBRARY_PATH; \
  export LIBRARY_PATH=\$HOME/.local/lib:\$LIBRARY_PATH; \
  export C_INCLUDE_PATH=\$HOME/.local/include:\$C_INCLUDE_PATH; \
  cd \"$BENCH_REPO\" && cargo test --workspace && \
  cargo build --release -p expanse-capi && \
  export EXPANSE_CDYLIB=\$PWD/target/release/libexpanse.so && \
  cargo bench --bench vs_stock -p expanse-capi"
```

### Automated Bare-Metal CI Triggers (`/bench` & `/bench extended`)
On pull requests, maintainers and authorized collaborators can trigger automated bare-metal benchmarks on the dedicated bare-metal reference host via PR comments:
- `/bench`: Runs standard dual-pass Callgrind (`vs_stock`, `instructions`) and fast comparative sweep ($N = 10,000$).
- `/bench extended` (or `/benchmark extended`): Runs full multi-population scaling sweeps ($N \in [10\text{k}, 100\text{k}, 1\text{M}]$) + microarchitectural target CPU scaling matrix (`baseline`, `x86-64-v2`, `x86-64-v3`, `native`) + Callgrind.
- `/benchmark <suite>`: Targeted suite runs (`vs_stock`, `instructions`, `comparative`, `ycsb`).
See `docs/BENCHMARKING.md` §2–3 and `docs/CI.md` for details.

### Zero-Regression Policy
- **Fewer instructions is always better.**
- **Review policy**: $0.1\%$ is Callgrind's deterministic measurement resolution; any instruction count regression $>0.1\%$ vs baseline main is considered a blocker in review and must be justified in the PR.
- **Automated CI gate** (distinct from the review threshold): `scripts/perf_report.py --fail-on-regression` fails the job at a $>5\%$ single-worst regression or $\geq 2$ arms regressed above the $0.5\%$ noise floor; the only automated override is a literal `allow-regression: <reason>` line in the PR body.
- Numbers in docs are tagged `(measured: host, commit)` or `(target)`; follow `docs/BENCHMARKING.md` (interleaved A/B arms, system-load snapshots before/between comparison runs, CI ratios ≠ publishable numbers).
- No time estimates in pull requests, comments, or documentation.

---

## 6.5 Enforcement Map — what is checked mechanically vs by review

Know which rules a machine will catch and which only a reviewer will. **CI-enforced** rules fail the `CI Gate / All Checks Passed` context; **review-enforced** rules are yours to uphold and a reviewer's to check.

| Rule | Enforced by | Where |
|---|---|---|
| fmt · clippy `-D warnings` (carries `undocumented_unsafe_blocks`, `missing_docs`) | **CI** | `lint` job |
| Workspace tests on 3 OSes, `PROPTEST_CASES=500` | **CI** | `test` job |
| MSRV floor builds | **CI** | `msrv` job |
| Tier-1 Miri (UB, provenance, borrows) · ASan · loom · fuzz smoke | **CI** | `miri`, `test-asan`, `loom`, `fuzz-smoke` |
| Node/edge layout invariants (§2.1 sizes, offsets, alignment) | **compile time** | `const _: () = { assert!(…) }` in `node.rs`, `types32.rs` |
| Instruction regression (>5 % worst, or ≥2 arms >0.5 %) + `allow-regression:` override | **CI** | `instruction-counts`, `callgrind-smoke` → `scripts/perf_report.py` |
| Memory density ceilings | **CI** | `memory-budget` |
| C ABI symbol parity · version lockstep · gate completeness · report-script self-tests | **CI** | `lint` job scripts |
| Black-box parity vs stock `libjudy` | **CI** | `differential-oracle`, `php-judy-*` |
| Doc↔code constant sync (visualizer) | **CI** | `tests/test_visualizer_sync.rs` |
| No time estimates · no PII/home paths/LAN IPs in docs and PR body | **CI** | `docs-lint` job → `scripts/check_docs_hygiene.py` |
| Provenance tags on published numbers (§8.7) | **CI (advisory warning)** + review | `docs-lint` heuristic; reviewer confirms the artifact |
| §6 review threshold (>0.1 % instructions) | **review** | `perf_report.py` renders it; a human decides |
| §8.4 BCa 95 % CI lower bound ≥ floor | **review** | no CI tooling yet — state the interval and its n in the PR |
| §2.1 / §2.2 architectural invariants beyond sizes (no B-tree/hash mutation, no coarse locks, ordered semantics) | **review** | — |
| §3 clean-room (no LGPL exposure) | **review** (`references/` is gitignored) | — |
| §8.3 symmetric baselines · §8.6 DCE sinks & realistic hit rates · §8.7 no in-place backfilling · §8.8 3-commit cadence | **review** | — |
| Conventional-commit type · branch naming | **review** | — |

---

## 7. Git & Pull Request Protocol

- **Branch Naming**: `perf/<feature>`, `feat/<feature>`, `fix/<issue>`, `refactor/<scope>`, `docs/<scope>`.
- **Commit Format**: Conventional Commits: `type(scope): message` (types: `feat`, `fix`, `perf`, `bench`, `docs`, `refactor`, `test`, `ci`, `chore`, `eval`, `poc`), atomic. Subject concise + factual; body states what + why.
- **Commit/push only when the maintainer asks.** The repo (`github.com/orieg/expanse`) is public; if on `main`, branch first.
- **Protected `main` Branch** (ruleset `main-protection`): direct pushes to `main` are rejected. All merges require:
  - A Pull Request (`gh pr create --base main`). The ruleset currently requires **0 approving reviews** — the enforced gate is the status check below, so self-merging a green PR is permitted; request a review when the change is architectural, touches `unsafe`, or moves a published number.
  - **A single required status check — `CI Gate / All Checks Passed`** (the `ci-gate` rollup job that `needs:` every other `ci.yml` job and fails if any failed or was cancelled; a `lint` step plus the gate's own self-check parse `ci.yml` and assert no job id is missing from its `needs`). Because the ruleset requires only the rollup context, renaming a *non-gate* CI job does not require editing the ruleset; only renaming `ci-gate` itself would.
  - No force-push, no deletion, no bypass actors.
  - Workflow: branch → push → `gh pr create` → watch checks → `gh pr merge`.
- **Impersonal GitHub prose.** Issue/PR/review text is published from the maintainer's own account: state what changed, where (commit SHA / file path / issue ref), and status — no external-collaboration framing, no self-attribution, no `@`-mentioning or thanking the maintainer.
- **No agent/session trailers** in commit messages (`Claude-Session:`, `Co-Authored-By:` for tools, etc.).

### Before Opening a PR (checklist — `.github/PULL_REQUEST_TEMPLATE.md` mirrors this)

1. `scripts/gate.sh` is green (fmt · clippy · tests with `PROPTEST_CASES=500` · repo scripts · docs hygiene); add `--miri` when the change touches `unsafe` or node layout.
2. Every new `unsafe` block has a `// SAFETY:` rationale; new public items are documented.
3. Hot-path change? Say so, and paste the `perf_report.py` table from `instruction-counts`; anything >0.1 % needs a justification, anything over the automated thresholds needs a literal `allow-regression: <reason>` line plus a Performance Trade-off Disclosure.
4. Every published number carries `(measured: host, commit)` resolving to a committed artifact — with n and the CI where §8.4 applies. Losing cells are published too.
5. Canonical doc updated (§1 hierarchy) — no new standalone `.md`.
6. Prose check: no time estimates, no hostnames/home paths, impersonal voice.

### Privacy & Local Infrastructure

**No PII or local-infrastructure identifiers.** Never put personally-identifying or private-infrastructure information into commit messages, PR titles/bodies, code comments, or docs: private/personal hostnames, LAN IP addresses, internal domains, home-directory paths, or OS usernames. Benchmark and test results MUST reference the host by an anonymized hardware description (CPU model, core/thread count, cache, OS) — never a personal hostname. Benchmark commands in this repo use environment placeholders (`$BENCH_HOST`, `$BENCH_REPO`) rather than concrete hostnames/paths. (The package-author email in manifest files — `Cargo.toml` / `pom.xml` / `.gemspec` / `.csproj` / etc. — is intentional public package metadata and is exempt.)

---

## 8. Benchmark & Research Integrity Discipline (Anti-Fabrication Standards)

Expanse is an empirical performance project. Autonomous agents interacting with this repo must adhere strictly to these non-negotiable research standards to prevent simulated, mocked, or fabricated performance claims.

### 8.1 Zero Silent Fallbacks & Fail-Loud Error Visibility
- **Never swallow errors with fake outputs**: If a benchmark dependency, language runtime, or native extension (`libexpanse.so`, Node addon, Python wheel) is missing or fails to compile, the harness MUST fail immediately and loudly with a non-zero exit code (`panic!`, `sys.exit(1)`, `b.Fatalf`).
- **Never substitute placeholder estimates or mocked loops**: Returning hardcoded strings, estimated numbers, or fake loops in place of an unrun benchmark is a critical methodology violation.
- **Explicit Non-Fatal Degradations**: In CI workflows where a failure is intentionally non-fatal (e.g. checking out an unbuildable historical base commit), the degradation must surface prominently (`⚠️ NO BASELINE — regression gate did not run`, matching `perf_report.py`) and must **never** render as success or "0 Regressions".

### 8.2 Dynamic Data Derivation (Zero Hardcoded Report Prose)
- **No Stamped Narrative Constants**: Benchmark reporting scripts (`bench_report.py`, `perf_report.py`, `generate_charts.py`) must never stamp hardcoded summary constants (e.g. `"4× to 10× faster point lookups"`, `"outperforms stock across all distributions"`).
- **100% Derived Outputs**: Every table cell, speedup ratio, status badge, and finding statement in generated markdown reports must be computed dynamically from the parsed JSON/Criterion artifacts of that specific run.

### 8.3 Symmetrical & Competitive Substitution-Twins
- **Production-Grade Baselines**: Competitor baselines must represent realistic production configurations (e.g. InlineSkipList-equivalent variable-height tower allocation — NOT a strawman embedding all 16 tower pointers statically at 146.7 B/entry, the audited anti-example; the fair baseline's actual footprint is established by measurement, #372).
- **Symmetric Selectivity & Workload Predicates**: Multi-structure comparisons must evaluate identical filter selectivity across all arms (e.g. matching 50% predicate pass rates across Expanse and competitors in YCSB Workload E).
- **Symmetric PRNGs**: Cross-language comparisons must use identical PRNG algorithms and seeds (e.g. matching XorShift64 algorithms and seeds across Rust, Python, Go, Node, PHP, Java, and .NET).
- **Symmetric Memory Accounting**: When measuring memory, report live resident heap via allocator instrumentation (`TrackingAlloc`/`GlobalAlloc`) across all arms, or explicitly disclose any platform-level asymmetry.

### 8.4 Metric-Scoped Statistical Gating
- **Continuous / Sampling Metrics**: Claims over wall-clock execution or continuous sampling distributions pass iff the **BCa 95% bootstrap CI lower bound $\ge$ floor** (≥1,000 resamples), NOT iff point estimate $\ge$ floor. Point estimates and CIs must use identical definitions (e.g. macro-mean with macro-CI) so the point estimate is always enclosed within the interval. Overlapping intervals must be labeled `BOUNDARY_RESULT` or `INTERMEDIATE_floor_within_ci`.
- **Deterministic Instruction Counters**: Exact Callgrind instruction counts (the primary regression instrument) are exact integers with zero variance, evaluated strictly against the deterministic threshold contract.

### 8.5 In-Repo Gitignored Scratch Path Isolation
- **No Baseline Pollution**: Quick, smoke, or developmental sweeps (`--quick`) must write strictly to gitignored scratch paths (e.g. `results/quick/` or `scratch/`) and must **never** overwrite canonical committed `results/baseline_*.json` or committed SVGs.

### 8.6 Realistic Workloads & Dead-Code Elimination (DCE) Sinks
- **Consume Every Output**: Every timed inner loop must consume its output via `std::hint::black_box`, `b.Fatalf`, or an accumulator sink to prevent compiler Dead-Code Elimination.
- **Realistic Keyspaces & Hit Rates**: Read benchmarks must specify and test realistic hit rates (e.g. 50% hit / 50% miss), never probing unbounded 64-bit random keys against sparse sets where hit rate is ~0% unless explicitly benchmarking the miss path.

### 8.7 Provenance & Pre-Registration Integrity
- **Provenance Tags Required**: Every published number in documentation must carry a provenance tag: `(measured: host, commit)` resolving to a committed JSON artifact or cited CI run.
- **No In-Place Backfilling**: Pre-registration sections, hypotheses, claims ceilings, and expected loss matrices must **never be reconciled in place** with observed outcomes. When an empirical result refutes a pre-registered hypothesis or reveals an unexpected loss, report the outcome honestly with its strict verdict label.

### 8.8 The 3-Commit Cadence for Research Spikes
Separate research and benchmark spikes into three distinct commits:
1. **Commit 1 (Math-First)**: Bound functions in committed Python/Rust with reference-pinned unit tests (zero pilot code).
2. **Commit 2 (Pre-Registration)**: Locked hypothesis, expected losses matrix, and gate taxonomy in markdown/YAML (zero main data).
3. **Commit 3 (Empirical Data)**: Benchmark scripts, raw JSON, paired CIs, and strict verdict matching pre-registration.

