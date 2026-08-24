# Expanse — Project Instructions

Clean-room, pure-Rust Judy arrays modernized for current hardware, plus `libexpanse` — a drop-in C ABI replacement for libjudy. Named for Judy's defining idea: partitioning keys by *expanse*, not population. Cargo workspace: `crates/expanse` (core, package `expanse-trie`) and `crates/expanse-capi` (builds `libexpanse` cdylib/staticlib).

## Naming

- Brand: **Expanse**. Rationale + supporting quotes (Baskins 10-minute description; Silverstein Judy IV Shop Manual): README "Why Expanse?" section — that is the canonical home for naming justification. Rust crate: `expanse-trie` (bare `expanse` is squatted on crates.io by an abandoned Flexbox crate — decided 2026-08-18). C library: `libexpanse` (`libexpanse.so` / `expanse.dll` / `libexpanse.a`). Headers: `expanse.h` (modern `expanse_*` API) + `Judy.h` (legacy compat). Distro packaging plan (libexpanse-dev / libexpanse1 / libjudy-compat) lives in docs/COMPAT.md.
- Legacy↔modern type map: Judy1→`ExpanseSet`, JudyL→`ExpanseMap`, JudySL→`ExpanseStrMap`, JudyHS→`ExpanseBytesMap`.
- New C capabilities use the `expanse_` prefix; `Judy*` symbols never change semantics.
- **Core-crate identifiers never use Judy terms** (`Edge`, not `JudyPointer`; `EdgeType`/`EdgeTag`, not `JpType`). Judy names live only in `expanse-capi` and COMPAT.md. Published-doc terminology ("Judy Pointer"/JP, Judy1/JudyL) may appear in core comments strictly as literature/compat references.

## Clean-room rules (non-negotiable)

- The original libjudy is **LGPL**. Never read, consult, or port its source code — not for "inspiration", not to settle a behavior question.
- Compatibility questions are settled by the documented API contract (man pages / published docs) or by **black-box differential testing** against a stock libjudy binary (see docs/COMPAT.md). Record doc-gap resolutions in COMPAT.md.
- `references/` holds published Judy algorithm-description PDFs: design context only, **gitignored, never committed**.

## Canonical doc homes (update these; do not create new .md files)

| Content | Home |
|---|---|
| Design / node layouts / roadmap / phase gates | `docs/ARCHITECTURE.md` |
| Algorithms, traversal kernels & visualizer reference | `docs/ALGORITHMS.md` · `docs/architecture_visualizer.html` |
| C compat contract, surface, packaging, acceptance gates, doc-gap resolutions | `docs/COMPAT.md` |
| Testing methodology, invariant validator, oracle rules | `docs/TESTING.md` |
| Benchmark methodology, comparison targets, results policy | `docs/BENCHMARKING.md` |
| CI pipeline, job catalog, rollup gate, regression gating | `docs/CI.md` |
| Distribution & packaging across all ecosystems | `docs/PACKAGING.md` |
| Database-engine subsystem patterns & integration blueprints | `docs/DATABASE.md` |
| Large-value / blob-arena design RFC | `docs/RFC_LARGE_VALUES.md` |
| 32-bit embedded architecture RFC | `docs/RFC_32BIT_EMBEDDED.md` |
| Python / Java binding references | `docs/BINDINGS_PYTHON.md` · `docs/BINDINGS_JAVA.md` |
| Status + platform tiers | `README.md` (Status section) |

## Rust conventions

- Edition 2024, `rust-version` 1.85. Both 64-bit and 32-bit targets are supported: `lib.rs` carries a `compile_error!` that fires only on targets that are neither 64- nor 32-bit. The 64-bit engine modules are `#[cfg(target_pointer_width = "64")]`; a parallel real 32-bit trie (`trie32`/`set32`/`map32`/`blobmap32`, shipped in #230) compiles unconditionally, and on 32-bit targets the public aliases re-point (`ExpanseMap` → `ExpanseMap32`, etc.).
- CI runs `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` — keep both clean locally before committing.
- Every `unsafe` block carries a `// SAFETY:` comment (clippy `undocumented_unsafe_blocks` is on).
- Every SIMD/intrinsic path has a portable fallback plus a parity test (see docs/TESTING.md).
- Public items are documented (`missing_docs` warns).

## Performance claims

- Numbers in docs are tagged `(measured: machine, commit)` or `(target)`. Follow docs/BENCHMARKING.md: interleaved A/B arms, system-load snapshots before/between comparison runs, CI ratios ≠ publishable numbers.
- No time estimates anywhere (global rule).

## Git

- `type(scope): description` commits (feat/fix/docs/refactor/chore/eval/poc), atomic.
- Repo: `github.com/orieg/expanse` (**public** since 2026-08-19 — the first publishable milestone: all four COMPAT.md gates green, modern API shipped; renamed from judy-rs, old URL redirects). Commit/push only when Nicolas asks.
- **`main` is protected** (ruleset `main-protection`, active since the repo went public 2026-08-19): PR required, **a single required status check** — `CI Gate / All Checks Passed` (the `ci-gate` rollup job that `needs:` every other ci.yml job — 28 jobs total, 27 non-gate — and fails if any failed or was cancelled; a `lint` step plus the gate's own self-check parse `ci.yml` and assert no job id is missing from its `needs`) — no force-push, no deletion, **no bypass actors** — an admin bypass would have made it advisory for the only person who commits here. Workflow: branch → push → `gh pr create` → watch checks → `gh pr merge`. Verified: a direct push to `main` is rejected. Because the ruleset requires only the rollup context, renaming a *non-gate* CI job no longer requires editing the ruleset (the self-check guards completeness instead); only renaming `ci-gate` itself would. The `php-judy-compat` check clones php-judy at a pinned SHA (see ci.yml); bump it deliberately.
- **CI concurrency**: one in-flight run per PR (`concurrency` group keyed on the PR number); a new commit cancels the previous run, so only the branch tip is fully verified. Pushes to `main` are keyed **per commit (`github.sha`)**, so main runs queue behind each other and never evict one another. `cancel-in-progress: false` alone does **not** achieve that: GitHub keeps only one *pending* run per group and cancels any previously pending one regardless of the flag. That was not a theoretical risk — three merges landing in quick succession cancelled the middle commit's run with zero jobs executed, so a commit reached the protected branch with no verification record. A commit on `main` with no run is invisible in a way a red check is not: nothing reports, so nothing looks wrong.
- **Repo is public** — Actions minutes are free again. While private, the macOS (10x) and Windows (2x) multipliers exhausted the 2,000-minute free tier in a single day of heavy CI use; if it ever goes private again, move macOS/Windows and the heavy jobs (miri, fuzz, php-judy-windows, instruction-counts) to nightly-only first.
