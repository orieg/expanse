# Expanse vs. Patricia and Radix Tries

`ExpanseMap` and `ExpanseStrMap` against three compressed radix tries that
differ in how a node finds its child:
- [`patricia_tree`](https://github.com/sile/patricia_tree) `=0.10.2`: a linked
  list of siblings.
- [`fast_radix_trie`](https://github.com/bluecatengineering/fast_radix_trie)
  `=1.2.0`: an inline array of children.
- [`qp-trie`](https://github.com/sdleffler/qp-trie-rs) `=0.8.2`: a nybble
  branch with a popcount-indexed child array.

**Status: no results yet.** The harnesses, the math-first envelope and the
pre-registration are committed. The first reference-host run is pending, and
no timing or memory figure from this suite is published until it lands.

`fast_radix_trie` 1.2.0 stores a node's child count as a `u8`, so it cannot
hold the integer key sets here. Its cells are recorded as `INVALID`
([METHODOLOGY.md](METHODOLOGY.md) §1).

- Pre-registration, envelope and claims ceiling: [METHODOLOGY.md](METHODOLOGY.md)
- Envelope: `scripts/patricia_envelope.py`, an exact `patricia_tree` node
  census pinned against the allocator hook, plus path and branch counts for the
  other twins.
- Run: `/benchmark patricia_comparison` on a pull request, which uploads the
  artifacts, or `run.sh` on the host.

| Pillar | Harness | Artifact |
|---|---|---|
| `u64` point lookup, 100% hit | `patricia_lookup_hit` | `results/baseline_lookup_hit.json` (pending) |
| `u64` point lookup, 50% hit, in-range misses | `patricia_lookup_miss` | `results/baseline_lookup_miss.json` (pending) |
| Cold-build insertion, both orders | `patricia_insert` | `results/baseline_insert.json` (pending) |
| Live-heap census, requested and usable bytes | `patricia_memory` | `results/baseline_memory.json` (pending) |
| String lookup across four shared-prefix lengths | `patricia_string` | `results/baseline_string_lookup.json` (pending) |
| Full traversal and prefix scan | `patricia_scan` | `results/baseline_scan.json` (pending) |
