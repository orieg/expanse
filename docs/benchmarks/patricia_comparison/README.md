# Expanse vs. Patricia Trie (`patricia_tree`)

`ExpanseMap` and `ExpanseStrMap` against
[`patricia_tree`](https://github.com/sile/patricia_tree) `=0.10.2`, a byte-labelled
Patricia (compressed radix) tree whose children are sorted, singly linked sibling
lists.

**Status: no results yet.** The harnesses, the math-first envelope and the
pre-registration are committed. The first full run on the reference host is
pending, and no timing or memory figure from this suite is published until it
lands.

- Pre-registration, envelope and claims ceiling: [METHODOLOGY.md](METHODOLOGY.md)
- Envelope: `scripts/patricia_envelope.py` (exact node census, pinned against the allocator hook)
- Run: `docs/benchmarks/patricia_comparison/run.sh` on the reference host

| Pillar | Harness | Artifact |
|---|---|---|
| Point lookup, 100% hit | `patricia_lookup_hit` | `results/baseline_lookup_hit.json` (pending) |
| Point lookup, 50% hit | `patricia_lookup_miss` | `results/baseline_lookup_miss.json` (pending) |
| Cold-build insertion, both orders | `patricia_insert` | `results/baseline_insert.json` (pending) |
| Live-heap census | `patricia_memory` | `results/baseline_memory.json` (pending) |
| Shared-prefix string lookup | `patricia_string` | `results/baseline_string_lookup.json` (pending) |
