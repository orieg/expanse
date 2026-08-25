# GitHub Copilot Instructions — Expanse

Expanse's engineering, clean-room, quality-gate, and privacy rules live in the repository-root **`AGENTS.md`**. Read and follow it in full — it is the single canonical agent guide (`CLAUDE.md` and `GEMINI.md` are symlinks to it).

TL;DR:
- **Core Judy Philosophy & Zero Impedance Mismatch**: digital trees partitioned by key expanse, compact single-word value slots (8 slots/cache line), zero heap alloc for small keys/values. Never widen slots into fat slots, complect metadata into index words, or mutate into B-trees/hash tables. Keep auxiliary attributes in decoupled sidecars.
- **Clean-room**: never view, consult, or port the LGPL `libjudy` source — settle compatibility by published docs or black-box differential testing only.
- **Before every commit**: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and Miri must all pass.
- **No PII / local-infrastructure identifiers** in commits, PRs, comments, or docs: no personal hostnames, LAN IPs, internal domains, home-directory paths, or usernames.
- **Conventional Commits** (`type(scope): description`), atomic; `main` is protected — branch, open a PR, and let the single `CI Gate / All Checks Passed` rollup gate it.
