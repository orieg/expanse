# judy-rs — Project Instructions

Clean-room, pure-Rust Judy arrays modernized for current hardware, plus a drop-in C ABI replacement for libjudy. Cargo workspace: `crates/judy` (core, package `judy-rs`) and `crates/judy-capi` (C ABI cdylib/staticlib).

## Clean-room rules (non-negotiable)

- The original libjudy is **LGPL**. Never read, consult, or port its source code — not for "inspiration", not to settle a behavior question.
- Compatibility questions are settled by the documented API contract (man pages / published docs) or by **black-box differential testing** against a stock libjudy binary (see docs/COMPAT.md). Record doc-gap resolutions in COMPAT.md.
- `references/` holds published Judy algorithm-description PDFs: design context only, **gitignored, never committed**.

## Canonical doc homes (update these; do not create new .md files)

| Content | Home |
|---|---|
| Design / node layouts / roadmap / phase gates | `docs/ARCHITECTURE.md` |
| C compat contract, surface, acceptance gates, doc-gap resolutions | `docs/COMPAT.md` |
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
- Repo: `github.com/orieg/judy-rs` (private until first publishable milestone). Commit/push only when Nicolas asks.
