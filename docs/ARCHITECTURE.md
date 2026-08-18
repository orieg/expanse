# judy-rs Architecture

> Canonical design doc. Compat contract: [COMPAT.md](COMPAT.md) · Testing: [TESTING.md](TESTING.md) · Benchmarks: [BENCHMARKING.md](BENCHMARKING.md)

Clean-room reimplementation of the Judy array family (Judy1 bit set, JudyL word→word map, JudySL string→word map), redesigned for 2026 hardware. Derived from published algorithm descriptions only; no libjudy source consulted (see COMPAT.md for the clean-room rules).

## 1. The structure in one page

A Judy tree is a 256-ary digital trie: each level decodes one byte ("digit") of a 64-bit key, most-significant byte first (level 8 → level 1). Every edge is a **Judy Pointer (JP)** — a 16-byte descriptor whose type tag says what it points to. Adaptive compression keeps memory near-proportional to population:

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

Two further compressions: **narrow pointers** (a JP records skipped common bytes in its decode field, collapsing single-child chains) and **full-expanse** tags (Judy1: a completely populated subexpanse needs no node at all).

## 2. What changes vs. Judy IV (2002)

| Component | Judy IV | judy-rs | Why |
|---|---|---|---|
| Cache lines | 128-byte assumption | All nodes exactly 64 B or 128 B, 64-aligned | One node traversal = 1–2 line fills, never a straddle |
| Bit scan/rank | SWAR + lookup tables | `u64::count_ones`/`trailing_zeros` (→ `popcnt`/`tzcnt`) | Single-cycle hardware ops |
| Byte search | Unrolled scalar compares | SIMD splat-compare-movemask (SSE2/NEON via `core::arch`, portable fallback) | 16–64 bytes per compare, no branchy loop |
| Allocator | Custom word-bucket chunk allocator | System/global allocator + slab arenas for 64 B/128 B/4 KiB classes | Modern allocators already segregate; arenas kill mutation-burst overhead |
| JP density | 16 B per edge always | 16 B JPs + tagged-pointer packing where profitable | 48-bit VAs leave 16 tag bits + 3 alignment bits free |
| Concurrency | None (external mutex) | Per-node version counters, optimistic lock-coupling readers | Lock-free reads, linear read scaling |

## 3. Core layouts

### 3.1 Judy Pointer (16 B)

```
offset 0: ptr        8 B   child node pointer, or immediate key payload
offset 8: decode     5 B   narrow-pointer skipped bytes
offset 13: pop0      2 B   subtree population - 1 (span-limited; wider pops
                           live in the child header)
offset 15: tag       1 B   JP type tag
```

Tag encoding (implemented in `crates/judy/src/types.rs`):

- Structural tags `0x00..=0x0C`, `0x7F`: null, 4 branch flavors, linear leaves for 1–7 remaining bytes, bitmap leaf, full expanse.
- Immediate tags, nibble-packed `(key_bytes << 4) | (count - 1)`, valid when `key_bytes * count <= 15`. Disjoint from structural tags by construction.

### 3.2 Branches

- **LinearBranch4** (64 B = 1 line): header byte + up to 7 digit bytes, then 4 JPs. Overflow → LinearBranch8.
- **LinearBranch8** (128 B = 2 lines): header + digits, 7 JPs. Overflow → BitmapBranch.
- **BitmapBranch** (128 B = 2 lines): line 0 = 256-bit bitmap (32 B) + subarray pointers; line 1 = remaining pointers + cached per-segment pop counts (rank acceleration). Slot lookup = bitmap test + popcount rank. ≥192 populated subexpanses → UncompBranch.
- **UncompBranch** (4 KiB): flat 256 JPs, direct index.

### 3.3 Leaves

- **Immediate**: up to 15 key-remainder bytes inside the JP (e.g. 15×1-byte, 7×2-byte, 2×7-byte). JudyL immediates constrain count further (values need a home — exact layout fixed in Phase 3).
- **LinearLeaf1..7**: packed sorted key remainders; JudyL adds a parallel value array. Sized so leaf search stays within 1–2 cache lines; searched with SIMD/binary search.
- **BitmapLeaf** (level 1): 32-byte bitmask (Judy1); JudyL adds sub-allocated value chunks addressed by popcount rank.

### 3.4 Tagged pointers (read-optimized paths)

x86-64/AArch64 user VAs fit in 48 (or 57) bits; 8-byte alignment frees the low 3 bits. A compact 8-byte JP variant packs `[type:16][address:45][level:3]` for read-dominated structures and for caching branch metadata without extra line fills. Must stay behind an abstraction that also supports full 16-byte JPs (LAM/TBI and 57-bit VA systems change the free-bit budget — feature-detected, never assumed).

## 4. Algorithms

- **Lookup** (Phase 4): iterative descent, `switch` on tag per level; branch step = SIMD digit find (linear), bitmap test + popcount rank (bitmap), direct index (uncompressed); leaf step = SIMD/bitmap search. Target: zero allocation, zero locks, ≤ `O(levels)` line fills.
- **Insert** (Phase 6): descend to the failing point, then grow along the least-compressed-form ladder: Immediate → LinearLeaf → (level 1) BitmapLeaf / (higher) cascade into LinearBranch4 → L8 → Bitmap → Uncompressed. Narrow-pointer creation when a leaf splits on a long common prefix.
- **Delete** (Phase 6): inverse ladder with **1-index hysteresis** — down-convert one step later than the up-convert threshold, preventing thrash on alternating insert/delete at a boundary.
- **Count/rank** (`Judy1Count`, `JudyLCount`, `ByCount`): O(depth) using JP `pop0` fields plus bitmap-branch cached segment counts.
- **Concurrent reads** (Phase 7): each node header embeds a version counter; writers bump to odd (mutating) then even (stable) with release stores; readers sample with acquire loads before/after and retry on change. Structural node replacement uses epoch-deferred reclamation so readers never dereference freed nodes.

## 5. Crate structure

- `crates/judy` (package `judy-rs`): core. Planned modules: `types` (done), `bits` (Phase 2 scan/rank primitives), `node` (Phase 3 layouts), `get` (Phase 4), `alloc` (Phase 5), `ins`/`del` (Phase 6), `occ` (Phase 7), `judy1`/`judyl`/`judysl` public API.
- `crates/judy-capi`: `extern "C"` surface per [COMPAT.md](COMPAT.md), plus the shipped `include/Judy.h`. Thin translation layer only — no logic beyond ABI marshaling and `JError_t` mapping.

## 6. Phase roadmap

| Phase | Deliverable | Gate to next |
|---|---|---|
| 1. Foundation types | Tags, constants, digit math (done) | Tests green |
| 2. Bit/vector engine | popcount/ctz/SIMD byte-find + portable fallbacks | Unit tests incl. edge lanes; parity between SIMD and fallback |
| 3. Node layouts | 64 B/128 B structs, layout `const` asserts | `size_of`/`align_of` asserts green |
| 4. Lookup engine | `get`/`test` over hand-built trees | Differential vs `BTreeMap` model on fixed corpora |
| 5. Allocation | Aligned slab arenas | Miri-clean; leak checks |
| 6. Mutation engine | insert/delete cascades + hysteresis | Property tests + invariant validator (TESTING.md) green |
| 7. OCC reads | Versioned nodes, epoch reclamation | Loom/stress suites green |
| 8. Hardening | capi surface, differential oracle vs C libjudy, fuzzing, benches | COMPAT.md acceptance gates; php-judy suite green against judy-capi |

Performance targets (measured per BENCHMARKING.md before any claim): point lookup < 15 ns on random 64-bit keys (target); < 9.5 bytes/key on dense/clustered distributions (target).
