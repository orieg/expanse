# Expanse Architecture

> Canonical design doc. Compat contract: [COMPAT.md](COMPAT.md) · Testing: [TESTING.md](TESTING.md) · Benchmarks: [BENCHMARKING.md](BENCHMARKING.md)

Expanse is a clean-room reimplementation of the Judy array family (Judy1 bit set, JudyL word→word map, JudySL string→word map), redesigned for 2026 hardware and named for Judy's defining idea: partitioning keys by *expanse* rather than by population. Derived from published algorithm descriptions only; no libjudy source consulted (see COMPAT.md for the clean-room rules).

## 1. The structure in one page

The trie is 256-ary: each level decodes one byte ("digit") of a 64-bit key, most-significant byte first (level 8 → level 1). Every edge is an **`Edge`** — a 16-byte tagged descriptor saying what it points to. (Terminology: the original literature calls this a "Judy Pointer"/JP; Judy names are reserved for the `expanse-capi` compat layer, and core code/docs use `Edge`.) Adaptive compression keeps memory near-proportional to population:

```
 Root ── (small pop) ──> root-level leaf
      └─ (pop ≥ threshold) ──> population/metadata node ──> top branch

 Branch flavors (by subexpanse density):
   LinearBranch   — few children: sorted digit array + JP array
   BitmapBranch   — moderate: 256-bit membership bitmap + packed JP arrays
   UncompBranch   — dense: flat array of 256 JPs

 Leaf flavors (by remaining key bytes and population):
   Immediate      — keys packed inside the 16-byte JP itself
   LinearLeaf     — packed undecoded key remainders (1..7 bytes each)
   BitmapLeaf     — level 1, high pop: 256-bit mask over the last byte
```

Two further compressions: **narrow pointers** (a JP records skipped common bytes in its decode field, collapsing single-child chains) and **full-expanse** tags (set flavor: a completely populated subexpanse needs no node at all).

## 2. What changes vs. Judy IV (2002)

| Component | Judy IV | Expanse | Why |
|---|---|---|---|
| Cache lines | 128-byte assumption | All nodes exactly 64 B or 128 B, 64-aligned | One node traversal = 1–2 line fills, never a straddle |
| Bit scan/rank | SWAR + lookup tables | `u64::count_ones`/`trailing_zeros` (→ `popcnt`/`tzcnt`) | Single-cycle hardware ops |
| Byte search | Unrolled scalar compares | SIMD splat-compare-movemask (SSE2/NEON via `core::arch`, portable fallback) | 16–64 bytes per compare, no branchy loop |
| Allocator | Custom word-bucket chunk allocator | System/global allocator + slab arenas for 64 B/128 B/4 KiB classes | Modern allocators already segregate; arenas kill mutation-burst overhead |
| Edge density | 16 B per edge always | 16 B edges + tagged-pointer packing where profitable | 48-bit VAs leave 16 tag bits + 3 alignment bits free |
| Concurrency | None (external mutex) | Per-node version counters, optimistic lock-coupling readers | Lock-free reads, linear read scaling |

## 3. Core layouts

### 3.1 Edge (16 B)

```
offset 0: word 0     8 B   child node pointer, or immediate key payload
offset 8: aux        7 B   level-split: low L bytes pop0, high bytes decode
offset 15: tag       1 B   edge type tag
```

The 7-byte aux field is **level-split** (as in the published Judy IV
design): for a child at level `L`, its low `L` bytes hold `pop0` (a
level-`L` subtree holds at most `256^L` keys, so `L` bytes always suffice)
and the remaining high bytes hold the narrow-pointer decode bytes — the two
never overlap, and no branch header needs a wide population field.
Implemented in `crates/expanse/src/node.rs`.

Tag encoding (implemented in `crates/expanse/src/types.rs`):

- Structural tags `0x00..=0x0C`, `0x7F`: null, 4 branch flavors, linear leaves for 1–7 remaining bytes, bitmap leaf, full expanse.
- Immediate tags, nibble-packed `(key_bytes << 4) | (count - 1)`, valid when `key_bytes * count <= 15`. Disjoint from structural tags by construction.

### 3.2 Branches

Linear branches share a 16-byte header: OCC version counter (u32, plain
until Phase 7), child count, and an 8-byte digit array searched as one
64-bit word (`bits::find_byte_8`). Geometry note: the naive "8 B header +
4 edges" one-line branch is arithmetically impossible (8 + 4×16 = 72 > 64);
capacity 3 with the 16-byte header is exact — and buys the OCC version slot.

- **BranchL3** (64 B = 1 line): 16 B header + 3 edges. Overflow → BranchL7.
- **BranchL7** (128 B = 2 lines): 16 B header + 7 edges. Overflow → BranchB.
- **BranchB** (128 B = 2 lines): line 0 = 256-bit bitmap (32 B) + first 4 of 8 subarray pointers; line 1 = remaining pointers + cached per-subexpanse pop counts (`[u16; 8]`, rank acceleration) + OCC version. Slot lookup = bitmap test + popcount rank. ≥192 populated subexpanses → BranchU.
- **BranchU** (4 KiB): flat 256 edges, direct index.

### 3.3 Leaves

- **Immediate**: up to 15 key-remainder bytes inside the edge (e.g. 15×1-byte, 7×2-byte, 2×7-byte). Map-flavor immediates keep keys in the 7 aux bytes: a single key's value lives in word 0, several keys point to a value array.
- **LinearLeaf1..7**: header-less variable-length allocations — population lives in the parent edge's `pop0`, so a leaf is nothing but payload (as in the original). Set flavor: `[keys: L×pop]`; map flavor: `[values: u64×pop][keys: L×pop]` in one 64-aligned allocation (values first = free 8-alignment). Scan search baseline; the Phase 8 bench pass decides whether SIMD/binary search earns its complexity.
- **LeafBitmap1** (level 1, 64 B): 32-byte bitmask + OCC version (set flavor). **LeafBitmapL** (128 B): bitmask + 8 value-subarray pointers addressed by popcount rank + OCC version (map flavor).

### 3.4 Tagged pointers (read-optimized paths)

x86-64/AArch64 user VAs fit in 48 (or 57) bits; 8-byte alignment frees the low 3 bits. A compact 8-byte edge variant packs `[type:16][address:45][level:3]` for read-dominated structures and for caching branch metadata without extra line fills. Must stay behind an abstraction that also supports full 16-byte edges (LAM/TBI and 57-bit VA systems change the free-bit budget — feature-detected, never assumed).

## 4. Algorithms

- **Lookup** (Phases 4–5, done — `get::test_set`/`get::get_map`): iterative tag-dispatched descent; branch step = SWAR digit find in the header word (linear), bitmap test + subexpanse popcount rank (bitmap), direct index (uncompressed); terminal step = linear-leaf scan, bitmap-leaf test/rank, or immediate key scan, with narrow-pointer decode validation on leaf children. Zero allocation, zero locks. Level skips: leaves skip via decode bytes, branches via header-stored levels (see §6 step 3); immediates never skip (their key size *is* their level), full-expanse edges cover their whole current expanse, and `BranchU`/level-8 slots never skip.
- **Insert** (Phase 6, done — `mutate::insert` / `mutate_map::map_insert`): descend to the failing point, then grow along the least-compressed-form ladder: Immediate → LinearLeaf → (level 1) BitmapLeaf → FullExpanse / (level ≥2) cascade into BranchL3 → L7 → BranchB → BranchU. Map flavor: immediates keep keys in the 7 aux bytes (value in word 0 for one key, value-array pointer for several, capacity `7 / key_bytes`); map leaves are `[values][keys]`; level-1 overflow goes to `LeafBitmapL` (no map full expanse — values must exist). Narrow-pointer creation: cascades place their branch (or bitmap leaf) at the keys' divergence level; divergence inside a skipped prefix splits at the highest diverging decode level (§6 step 3).
- **Delete** (Phase 6, done — `mutate::remove` / `mutate_map::map_remove`): inverse ladder with **1-index hysteresis** — every down-conversion runs one index below its up-convert twin (leaf→immed at `max_count − 1`, bitmap→leaf at 24, L7→L3 at 2, B→L7 at 6, U→B at 191), preventing thrash on alternating insert/delete at a boundary. Deleting from a full expanse first materializes one decompression step.
- **Count/rank** (compat: `Judy1Count`/`JudyLCount`/`ByCount`): O(depth) using edge `pop0` fields plus bitmap-branch cached segment counts.
- **Concurrent reads** (Phase 7): each node header embeds a version counter; writers bump to odd (mutating) then even (stable) with release stores; readers sample with acquire loads before/after and retry on change. Structural node replacement uses epoch-deferred reclamation so readers never dereference freed nodes.

## 5. Crate structure

- `crates/expanse` (package `expanse-trie`): core. Planned modules: `types` (done), `bits` (done: SIMD/SWAR byte find, `Bitmap256` rank/select/navigation), `node` (done: JP + branch/bitmap-leaf layouts, compile-time layout asserts; variable-length linear-leaf layout lands with the Phase 5 allocator), `get` (done: set/map lookup walk incl. linear leaves), `alloc` (done: cache-line-aligned allocation + byte accounting behind one handle; slab caches only if Phase 8 benches justify), `leaf` (done: linear-leaf layout + search), `mutate` (done, set flavor: insert/remove ladder + hysteresis, subtree free, invariant validator), `set` (done: `ExpanseSet` with root-leaf → level-8 trie organization), `mutate_map` + `map` (done: map-flavor engine sharing the branch machinery; `ExpanseMap`), `nav` (done: flavor-generic ordered navigation — next/prev/first/last, O(depth) rank via pop0, 0-based select; public iterators and count_range/by_count on both types). `strmap` (done: `ExpanseStrMap`, a meta-trie of word-map nodes over big-endian 8-byte chunks — numeric order = byte-lexicographic order; backs the exported `JudySL*`). Ahead: `occ` (Phase 7), `ExpanseBytesMap`/`JudyHS`.
- `crates/expanse-capi` (`libexpanse`): `extern "C"` surface per [COMPAT.md](COMPAT.md) — legacy `Judy.h` compat plus the modern `expanse.h` API. Thin translation layer only — no logic beyond ABI marshaling and `JError_t` mapping.

## 6. Phase roadmap

| Phase | Deliverable | Gate to next |
|---|---|---|
| 1. Foundation types | Tags, constants, digit math (done) | Tests green |
| 2. Bit/vector engine | popcount/ctz/SIMD byte-find + portable fallbacks (done) | Unit tests incl. edge lanes; parity between SIMD and fallback |
| 3. Node layouts | 64 B/128 B structs, layout `const` asserts (done; linear-leaf layout deferred to Phase 5 alloc) | `size_of`/`align_of`/`offset_of` asserts green |
| 4. Lookup engine | `get`/`test` over hand-built trees (done) | Differential vs `BTreeMap` model on fixed corpora |
| 5. Allocation | Cache-line-aligned alloc + accounting; linear-leaf layout + lookup integration (done; slab caches deferred to Phase 8 measurement) | Miri-clean; leak checks |
| 6. Mutation engine | insert/delete cascades + hysteresis (done: `ExpanseSet` + `ExpanseMap`) | Property tests + invariant validator (TESTING.md) green |
| 7. OCC reads | Versioned nodes, epoch reclamation | Loom/stress suites green |
| 8. Hardening | capi surface, differential oracle vs C libjudy, fuzzing, benches | COMPAT.md acceptance gates; php-judy suite green against libexpanse |

### Sequencing after external architect review (2026-08-18)

An external review confirmed Phases 1–6b and identified five gaps (narrow-pointer synthesis, asymmetric root-leaf lifecycle, missing ordered navigation/rank APIs, `NodeAlloc` `Cell` counters, capi stub). Agreed order, with rationale:

1. **Ordered navigation + rank/count** — done: `nav` module + public APIs (`first`/`last`/`next_at_or_after`/`next_after`/`prev_at_or_before`/`prev_before`, `iter()`, `count_below`/`count_range`, 0-based `by_count`) on both `ExpanseSet` and `ExpanseMap`, differentially tested against the `BTree` models; map navigation returns values (the compat layer's `First`/`Next` hand out value pointers).
2. **Phase 8: capi exports + differential oracle + benches** — the COMPAT.md acceptance gates are the project's falsifiable criteria; the oracle also retro-tests everything above, and the bench harness produces the bytes/key evidence the next step needs.
3. **Narrow-pointer synthesis in mutation** — **done, both halves.** *Leaf-targeted* (measured: clustered bytes/key 1.34 → 0.35; vs-stock clustered get 5.31× slower → parity): cascades whose keys diverge only in the last byte build one skip-carrying bitmap leaf instead of a branch chain; shrink conversions absorb decode bytes back into slot-level immediates. *Branch-targeted* (via **header-stored levels**: `BranchHeader.level` / `BranchB.level`; `BranchU` has no header and never skips — a full skipping `BranchB` is first wrapped one level above its form): a leaf cascade places its branch at the keys' true divergence level with the shared prefix as decode bytes, and an insert diverging inside any skipped prefix splits at the **highest** diverging decode level (`split_skip`) instead of materializing one chain level per step. Level-8 slots never skip (the root edge cannot hold both pop0 and decode bytes). Measured: two 512-key clusters cost 192 structural bytes vs 960 under per-level chains (`branch_skip_clusters` tests); wide-cluster (4096-run) set flavor 0.19 → 0.12 B/key at 1M keys.
4. **Root-leaf shrink hysteresis** — trivially implementable once ordered iteration exists (collect ≤ 31 survivors), pointless before.
5. **Phase 7 OCC** (atomic version counters, seqlock readers, EBR; includes the `NodeAlloc` `Cell`→atomic accounting move) — last, so the concurrency-hardening pass runs over the final single-threaded structure instead of twice.

Performance targets (measured per BENCHMARKING.md before any claim): point lookup < 15 ns on random 64-bit keys (target); < 9.5 bytes/key on dense/clustered distributions (target).
