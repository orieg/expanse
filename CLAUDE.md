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
| C compat contract, surface, packaging, acceptance gates, doc-gap resolutions | `docs/COMPAT.md` |
| Testing methodology, invariant validator, oracle rules | `docs/TESTING.md` |
| Benchmark methodology, comparison targets, results policy | `docs/BENCHMARKING.md` |
| Status + platform tiers | `README.md` (Status section) |

## Rust conventions

- Edition 2024, `rust-version` 1.85, 64-bit targets only (compile-time enforced).
- CI runs `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` — keep both clean locally before committing.
- Every `unsafe` block carries a `// SAFETY:` comment (clippy `undocumented_unsafe_blocks` is on).
- Every SIMD/intrinsic path has a portable fallback plus a parity test (see docs/TESTING.md).
- Public items are documented (`missing_docs` warns).

## Performance claims

- Numbers in docs are tagged `(measured: machine, commit)` or `(target)`. Follow docs/BENCHMARKING.md: interleaved A/B arms, system-load snapshots before/between comparison runs, CI ratios ≠ publishable numbers.
- No time estimates anywhere (global rule).

## Git

- `type(scope): description` commits (feat/fix/docs/refactor/chore/eval/poc), atomic.
- Repo: `github.com/orieg/expanse` (private until first publishable milestone; renamed from judy-rs — old URL redirects). Commit/push only when Nicolas asks.
- **Main protection: blocked on plan tier** — rulesets/branch protection need GitHub Pro on private repos (403 as of 2026-08-18). Enable the `main-protection` ruleset (PR + all ci.yml job names as required checks, no force-push, no bypass actors) as soon as the repo goes public or the plan upgrades. Until then: direct pushes allowed, always watch the CI run and fix-forward immediately. The `php-judy-compat` check clones php-judy at a pinned SHA (see ci.yml); bump it deliberately.
