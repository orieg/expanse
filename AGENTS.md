# AGENTS.md — Multi-Agent Engineering & Quality Guide for Expanse

Welcome to **Expanse**. This document establishes mandatory engineering, architectural, and safety standards for all autonomous AI coding agents (Claude, Antigravity, Cursor, Copilot, Cline, OpenAI, Devin, Aider, etc.) interacting with this repository.

---

## 1. Project Mission & Identity

**Expanse** is a clean-room, pure-Rust implementation of Judy arrays modernized for modern 64-bit microarchitectures, accompanied by `libexpanse` — a high-performance, drop-in C ABI replacement for `libjudy`.

Named for Judy's defining structural invariant: **partitioning digital trees by key *expanse*, rather than population**.

### Workspace Structure
- **`crates/expanse`** (`package: expanse-trie`): Core algorithmic engine `#![no_std]` (with `extern crate alloc`).
- **`crates/expanse-capi`** (`package: expanse-capi`): C ABI shared (`libexpanse.so` / `expanse.dll` / `libexpanse.dylib`) and static (`libexpanse.a` / `expanse.lib`) libraries providing both modern `expanse_*` and legacy `Judy*` symbols.

### Canonical Documentation Hierarchy
Do not scatter architecture notes into arbitrary files. Use and maintain the canonical documentation:
- **`docs/ARCHITECTURE.md`**: Trie node layouts, memory packing, pointer tagging, concurrency design.
- **`docs/ALGORITHMS.md`**: Algorithmic specifications, search kernels, SIMD/SWAR vectorization.
- **`docs/COMPAT.md`**: C ABI contracts, drop-in parity gates, error handling, packaging specifications.
- **`docs/TESTING.md`**: Test methodology, differential testing, invariants validator, fuzzing.
- **`docs/BENCHMARKING.md`**: Benchmarking methodology, instruction counting, hardware counters, profiling.

---

## 2. Clean-Room Discipline (Strict & Non-Negotiable)

1. **Zero Exposure to LGPL Source**: The original `libjudy` is LGPL. **Never view, consult, decompile, or port original C source code** — not for inspiration, nor to resolve behavioral edge cases.
2. **Contract & Black-Box Differential Validation**: Compatibility questions are answered strictly through:
   - Official published documentation and man pages.
   - Black-box differential testing against compiled stock binaries (`stock-oracle`).
3. **References**: The `references/` directory holds algorithm papers and Shop Manuals. These are for design context only, are gitignored, and must never be checked into git.

---

## 3. Naming Conventions & Core Invariants

- **Rust Type Names**:
  - Judy1 $\rightarrow$ `ExpanseSet`
  - JudyL $\rightarrow$ `ExpanseMap`
  - JudySL $\rightarrow$ `ExpanseStrMap`
  - JudyHS $\rightarrow$ `ExpanseBytesMap`
- **Core Identifiers**:
  - Core trie types in `expanse-trie` **never use Judy terminology**.
  - Use `Edge` (not `JudyPointer`/`JP`), `EdgeType`/`EdgeTag` (not `JpType`).
  - Judy symbols belong exclusively in `expanse-capi` and `COMPAT.md`.
- **C ABI Prefixes**:
  - Modern API functions use `expanse_` prefix (e.g. `expanse_map_get`).
  - Compat symbols retain exact `Judy1*`, `JudyL*`, `JudySL*`, `JudyHS*` signatures.

---

## 4. Rust Standards & Quality Gates

### Language & Edition
- **Rust Edition 2024**, MSRV `1.85`.
- **64-bit architectures only** (`target_pointer_width = "64"` compile-time assertion).

### Mandatory Local Gates (Must Pass 100% Before Committing)
```bash
# 1. Code formatting
cargo fmt --check

# 2. Strict linter (zero warnings permitted)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Complete workspace test suite
cargo test --workspace

# 4. Randomized model testing (heavy iterations)
PROPTEST_CASES=500 cargo test --test proptest_model

# 5. Miri undefined behavior verification (skipping long-running model sweeps)
cargo miri test -p expanse-trie -- --skip model_
```

### Unsafe Code & Undocumented Unsafe Blocks
- Expanse operates on low-level tagged pointer representations and raw memory layouts.
- **Every `unsafe` block MUST be preceded by an explicit `// SAFETY:` rationale comment** explaining pointer validity, lifetime guarantees, alignment, and bounds preservation.
- **Stacked Borrows / Tree Borrows Hygiene**: Avoid creating temporary unique references (`&mut *ptr`) from raw pointers where ancestor/subfield borrows exist. Prefer `&raw mut` / `core::ptr::addr_of_mut!` and raw pointer manipulation to avoid invalidating pointer tags in the borrow stack.

---

## 5. Performance Engineering & Fast Iteration Cycle

### Fast Remote Benchmark Validation (`honeycomb`)
Before proposing or pushing performance-sensitive changes, run deterministic Callgrind profiling on `honeycomb` to ensure zero instruction regressions:

```bash
rsync -az --exclude 'target' --exclude '.git' ./ honeycomb:/home/nicolas/expanse/ && \
ssh honeycomb "export PATH=\$HOME/.cargo/bin:\$HOME/.local/bin:\$PATH; \
  export LD_LIBRARY_PATH=\$HOME/.local/lib:\$LD_LIBRARY_PATH; \
  export LIBRARY_PATH=\$HOME/.local/lib:\$LIBRARY_PATH; \
  export C_INCLUDE_PATH=\$HOME/.local/include:\$C_INCLUDE_PATH; \
  cd /home/nicolas/expanse && cargo test --workspace && \
  cargo build --release -p expanse-capi && \
  export EXPANSE_CDYLIB=\$PWD/target/release/libexpanse.so && \
  cargo bench --bench vs_stock -p expanse-capi"
```

### Zero-Regression Policy
- **Fewer instructions is always better.**
- Any instruction count regression $>0.1\%$ vs baseline main in deterministic Callgrind is considered a blocker.
- No time estimates in pull requests, comments, or documentation.

---

## 6. Git & Pull Request Protocol

- **Branch Naming**: `perf/<feature>`, `feat/<feature>`, `fix/<issue>`, `refactor/<scope>`.
- **Commit Format**: Conventional Commits: `type(scope): message` (e.g., `perf(mutate): eliminate recursion in single-threaded mutation`).
- **Protected `main` Branch**: Direct pushes to `main` are rejected. All merges require:
  - An approved Pull Request (`gh pr create`).
  - 13-of-13 green status checks on GitHub Actions CI.
