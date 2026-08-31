# Expanse Architecture

> Canonical design doc. Bit-level encoding reference: [§10](#10-bit-level-encoding-reference) · Compat contract: [COMPAT.md](COMPAT.md) · Testing: [TESTING.md](TESTING.md) · Benchmarks: [BENCHMARKING.md](BENCHMARKING.md) · Database Engines: [DATABASE.md](DATABASE.md) · 32-Bit & Embedded: [design/32-bit-embedded.md](design/32-bit-embedded.md) · Large Values: [design/large-values.md](design/large-values.md)

Expanse is a clean-room reimplementation of the Judy array family (Judy1 bit set, JudyL word→word map, JudySL string→word map), redesigned for 2026 hardware and named for Judy's defining idea: partitioning keys by *expanse* rather than by population. Derived from published algorithm descriptions only; no libjudy source consulted (see COMPAT.md for the clean-room rules).

## 1. The structure in one page

The trie is 256-ary: each level decodes one byte ("digit") of a 64-bit key, most-significant byte first (level 8 → level 1). Every edge is an **`Edge`**, a 16-byte tagged descriptor saying what it points to. The original literature calls this a "Judy Pointer"/JP; Judy names are reserved for the `expanse-capi` compat layer, and core code and docs use `Edge`. Adaptive compression keeps memory near-proportional to population:

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
| Bit scan/rank | SWAR + lookup tables | `u64::count_ones`/`trailing_zeros` | Single-cycle on AArch64 (`cnt`/`rbit`). On x86-64 runtime CPUID dispatch selects `popcnt` and BMI2 `pdep` on hot read paths, with a portable SWAR fallback on generic baseline builds; native instructions in `glibc-hwcaps` packages (`x86-64-v2`/`v3`) |
| Byte search | Unrolled scalar compares | SIMD splat-compare-movemask (SSE2/NEON via `core::arch`, portable fallback) | 16–64 bytes per compare, no branchy loop |
| Allocator | Custom word-bucket chunk allocator | Intrusive 4 KiB `SlabPage` freelist arena with 62 size classes and $O(1)$ static `RAW_CLASS_TABLE` lookup, cache-line aligned, byte-exact accounting | Recycling is a freelist head swap with no size search or coalescing pass, so it neither fragments the heap nor scales with population; retiring 25% fewer instructions than stock Judy on `judyl_churn/random` (38,679,495 vs 51,572,661; instructions retired, not wall clock — [`docs/visualizer_data.json`](visualizer_data.json)) |
| Edge representation | 16 B hybrid pointer stealing | Dual-word 16 B Edge: Word 0 holds raw unmasked 64-bit pointer / immediate; Word 1 holds aux + tag | Full 57-bit (PML5/LA57) & 52-bit (ARM64-LVA) virtual address safety with zero upper-bit pointer stealing |
| Concurrency | None (external mutex) | Per-node version counters, optimistic lock-coupling readers | Lock-free reads, linear read scaling |

> The 64-byte cache-line assumption behind the node layouts above is validated against primary sources in [`docs/HARDWARE.md` §1.4](HARDWARE.md#14-64-byte-cache-line--validated-correct-on-x86) (x86) and [§2.4](HARDWARE.md#24-64-byte-cache-line---portability--perf-risk-on-arm) (⚠️ ARM portability note: Apple Silicon uses 128-byte lines).

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

Byte-exact field positions, per-tag word-0 contents, and why the pointer is stored unmasked are in [§10.1](#101-edge--the-16-byte-tagged-descriptor-64-bit-targets).

Tag encoding (implemented in `crates/expanse/src/types.rs`):

- Structural tags `0x00..=0x0C`, `0x7F`: null, 4 branch flavors, linear leaves for 1–7 remaining bytes, bitmap leaf, full expanse.
- Immediate tags, nibble-packed `(key_bytes << 4) | (count - 1)`, valid when `key_bytes * count <= 15`. Disjoint from structural tags by construction.

### 3.2 Branches

Linear branches share a 16-byte header:
```
offset 0: version     4 B (u32)    OCC seqlock version counter
offset 4: num         1 B (u8)     active child count
offset 5: level       1 B (u8)     node level (1..8)
offset 6: presence    2 B (u16)    16-bit bloom presence filter (bit 1 << (digit & 0x0F))
offset 8: digits      8 B ([u8;8]) sorted digit array searched as 64-bit word
```
Geometry note: the naive "8 B header + 4 edges" one-line branch is arithmetically impossible (8 + 4×16 = 72 > 64); capacity 3 with the 16-byte header is exact — and buys the OCC version and 16-bit presence filter slots.

- **BranchL3** (64 B = 1 line): 16 B header + 3 edges. Direct branchless compare on descent. Overflow → BranchL7.
- **BranchL7** (128 B = 2 lines): 16 B header + 7 edges. Uses 16-bit presence filter to reject absent digits before SIMD scan. Overflow → BranchB.
- **BranchB** (128 B = 2 lines): line 0 = 256-bit bitmap (32 B) + first 4 of 8 subarray pointers; line 1 = remaining pointers + cached per-subexpanse pop counts (`[u16; 8]`, rank acceleration) + OCC version. Slot lookup = bitmap test + popcount rank. ≥192 populated subexpanses → BranchU.
- **BranchU** (4 KiB + 1 line): a header line (OCC version; a `BranchU` never skips, so no level) + flat 256 edges, direct index.

### 3.3 Leaves

- **Immediate**: up to 15 key-remainder bytes inside the edge (e.g. 15×1-byte, 7×2-byte, 2×7-byte). Map-flavor immediates keep keys in the 7 aux bytes: a single key's value lives in word 0, several keys point to a value array.
- **LinearLeaf1..7**: header-less variable-length allocations — population lives in the parent edge's `pop0`, so a leaf is nothing but payload (as in the original). Set flavor: `[keys: L×pop]`; map flavor: `[values: u64×pop][keys: L×pop]` in one 64-aligned allocation (values first = free 8-alignment). Search is a linear scan; whether SIMD or binary search earns its complexity is a benchmark question, not a settled one.
- **LeafBitmap1** (level 1, 64 B): 32-byte bitmask + OCC version (set flavor). **LeafBitmapL** (128 B): bitmask + 8 value-subarray pointers addressed by popcount rank + OCC version (map flavor).

The per-width immediate capacity tables, the bitmap rank/select addressing, and the `ValueSlot` encodings are in [§10.4](#104-immediate-capacity)–[§10.6](#106-bitmap-structures).

### 3.4 Tagged pointers (read-optimized paths) — *design note, not shipped*

> **Nothing in this subsection is implemented.** The shipped 64-bit `Edge` is the full 16-byte descriptor of §3.1, whose word 0 holds the raw untruncated pointer with zero bit-stealing ([§10.1](#why-word-0-is-stored-unmasked-gated)). Do not cite this subsection as a description of the current representation.

x86-64/AArch64/RISC-V 64-bit user VAs fit in 48 (or 57) bits; 8-byte alignment frees the low 3 bits. A compact 8-byte edge variant would pack `[type:16][address:45][level:3]` for read-dominated structures and for caching branch metadata without extra line fills. It would have to stay behind an abstraction that also supports full 16-byte edges (LAM/TBI/Sv57 and 57-bit VA systems change the free-bit budget — feature-detected, never assumed).

## 4. Algorithms

- **Lookup** (`get::test_set` / `get::get_map`): iterative tag-dispatched descent. Zero allocation, zero locks. The branch step is a SWAR digit find in the header word (linear), a bitmap test plus subexpanse popcount rank (bitmap), or a direct index (uncompressed). The terminal step is a linear-leaf scan, a bitmap-leaf test/rank, or an immediate key scan, with narrow-pointer decode validation on leaf children. Leaves skip via decode bytes, branches via header-stored levels (see §6 step 3). Immediates never skip — their key size *is* their level. Full-expanse edges cover their whole current expanse, and `BranchU`/level-8 slots never skip.
- **Insert** (`mutate::insert` / `mutate_map::map_insert`): descend to the failing point, then grow along the least-compressed-form ladder: Immediate → LinearLeaf → (level 1) BitmapLeaf → FullExpanse / (level ≥2) cascade into BranchL3 → L7 → BranchB → BranchU. Multi-level descent uses monomorphic match arms with direct scalar comparisons: branchless `linear_insert_slot_l3` for BranchL3, 16-bit presence filtering for BranchL7, zero-loop `const fn new_immed_single_map`, compile-time tag-specialized `locate_fixed::<KB>` and `map_insert_at_fixed::<KB>` for linear leaves, and depth-guarded bypass paths (`InsertPath::clear`). Map flavor: immediates keep keys in the 7 aux bytes (value in word 0 for one key, value-array pointer for several, capacity `7 / key_bytes`); map leaves are `[values][keys]`; level-1 overflow goes to `LeafBitmapL` (no map full expanse — values must exist). Narrow-pointer creation: cascades place their branch (or bitmap leaf) at the keys' divergence level; divergence inside a skipped prefix splits at the highest diverging decode level (§6 step 3).
- **Delete** (`mutate::remove` / `mutate_map::map_remove`): inverse ladder with **1-index hysteresis** — every down-conversion runs one index below its up-convert twin (leaf→immed at `max_count − 1`, bitmap→leaf at 24, L7→L3 at 2, B→L7 at 6, U→B at 191), preventing thrash on alternating insert/delete at a boundary. Deleting from a full expanse first materializes one decompression step.
- **Count/rank** (compat: `Judy1Count`/`JudyLCount`/`ByCount`): O(depth) using edge `pop0` fields plus bitmap-branch cached segment counts.

### 4.1 Concurrent reads (`occ` + `sync`)

Readers are lock-free and validated; writers serialize on a mutex.

**Base protocol.** Every branch header carries a **per-node seqlock version** (Boehm fence construction). A writer brackets a node's in-place mutation region — including the recursion beneath it — with that node's version, and only when the tree is concurrently shared. Readers validate hand-over-hand: each node is sampled before its fields are read, and re-validated before anything read from it is dereferenced. Terminal payloads are covered by their parent's version; the tree-level version covers the root snapshot. Epoch-deferred reclamation keeps every pinned pointer live, and bounded retries fall back to the writer mutex.

**`SyncExpanseBlobMap`** (issue #219) extends the protocol to variable-length payloads. The validated index walk yields the 64-bit `ValueSlot`, from which inline payloads of ≤ 7 B decode by value. Arena payloads resolve through an RCU-published immutable chunk table, so readers never touch the arena's chunk vector. Dead chunks and superseded tables retire through the same epoch collector. An epoch-pinned read guard (`BlobReadGuard`) hands out zero-copy payload borrows that stay byte-stable across concurrent compaction; holding the guard defers reclamation tree-wide until dropped, with pending uncollected memory observable via `occ_stats` (`retained_bytes`, `retained_hwm`) and `Collector::retained_bytes()` (#525).

**`SyncExpanseStrMap`** (issue #219) extends it to string keys. A lookup cascades across the meta-trie's sub-maps: one validated hand-over-hand walk per 8-byte chunk, each entered against the one shared tree version. The path prefix is therefore snapshot-consistent, and the terminal value carries the same per-node-cover linearizability as a `SyncExpanseMap` read. Suffix leaves are write-once after publication — a split publishes a replacement child and retires the old suffix, while a value update mutates one word in place under the bracket. Unlinked nodes and suffixes retire through the same collector.

**`SyncExpanseBytesMap`** (issue #362) completes the family for unordered byte keys: one validated walk over the hash trie, then a byte-exact comparison against a collision bucket that is write-once after publication. Structural changes publish a replacement bucket and retire the old shell, entries and key buffers through the same collector. Only value words mutate in place, covered by the reader's final tree-version validation.

**Deferred mode must be entered before an allocator ever slab-carves** (`NodeAlloc::defer_to` asserts this). The sync wrappers therefore share a populated structure by rebuilding it through pre-deferred allocators.

## 5. Crate structure

`crates/expanse` (package `expanse-trie`) is the core engine (`std` by default, full `#![no_std]` supported via `default = ["std"]`). Its principal modules:

| Module | Contents |
|---|---|
| `types` | Key/value word types, node geometry constants, the edge tag encoding, digit extraction |
| `bits` | SIMD/SWAR byte find, `Bitmap256` rank/select/navigation |
| `node` | Edge + branch/bitmap-leaf layouts, compile-time layout asserts |
| `alloc` | Cache-line-aligned allocation + byte accounting behind one handle |
| `leaf` | Linear-leaf layout (variable-length, allocator-backed) + search |
| `get` | Set/map lookup walk including linear leaves |
| `mutate` | Set flavor: insert/remove ladder + hysteresis, subtree free, invariant validator |
| `set` | `ExpanseSet`, root-leaf → level-8 trie organization |
| `mutate_map` + `map` | Map-flavor engine sharing the branch machinery; `ExpanseMap` |
| `nav` | Flavor-generic ordered navigation — next/prev/first/last, O(depth) rank via pop0, 0-based select; public iterators and count_range/by_count on both types |
| `strmap` | `ExpanseStrMap`, a meta-trie of word-map nodes over big-endian 8-byte chunks (numeric order = byte-lexicographic order); backs the exported `JudySL*` |
| `bytesmap` | `ExpanseBytesMap`, the unordered byte-string map — a 64-bit-hash-keyed `ExpanseMap` over byte-exact collision buckets; backs the exported `JudyHS*`. In `std` builds, `DefaultBuildHasher` uses process-randomized `RandomState` (DoS-resistant); in `no_std` builds, it defaults to deterministic FNV-1a (supply your own `S: BuildHasher` via `with_hasher` if keys are untrusted). |
| `slot` | Polymorphic 64-bit `ValueSlot`: inline payloads up to 7 B, or 24-bit hot metadata plus a 32-bit arena locator in one word; columnar predicate filter kernels |
| `blobmap` | `ExpanseBlobMap` — variable-length payloads: ≤ 7 B inline in the slot, larger ones bump-allocated in 16-byte-aligned `BlobArena` slabs ([design/large-values.md](design/large-values.md)) |
| `occ` + `sync` | Seqlock/EBR primitives and the `SyncExpanseSet`/`SyncExpanseMap`/`SyncExpanseBlobMap`/`SyncExpanseStrMap`/`SyncExpanseBytesMap` wrappers (§4.1) |
| `trie32` + `set32`/`map32`/`blobmap32` | The parallel 32-bit engine (§8); compiled unconditionally |

`crates/expanse-capi` (`libexpanse`) is the `extern "C"` surface per [COMPAT.md](COMPAT.md) — legacy `Judy.h` compat plus the modern `expanse.h` API. Thin translation layer only: no logic beyond ABI marshaling and `JError_t` mapping.

## 6. Phase roadmap

| Phase | Deliverable | Gate to next |
|---|---|---|
| 1. Foundation types | Tags, constants, digit math (done) | Tests green |
| 2. Bit/vector engine | popcount/ctz/SIMD byte-find + portable fallbacks (done) | Unit tests incl. edge lanes; parity between SIMD and fallback |
| 3. Node layouts | 64 B/128 B structs, layout `const` asserts (done; linear-leaf layout deferred to Phase 5 alloc) | `size_of`/`align_of`/`offset_of` asserts green |
| 4. Lookup engine | `get`/`test` over hand-built trees (done) | Differential vs `BTreeMap` model on fixed corpora |
| 5. Allocation | Cache-line-aligned alloc + accounting; linear-leaf layout + lookup integration (done) | Miri-clean; leak checks |
| 6. Mutation engine | insert/delete cascades + hysteresis (done: `ExpanseSet` + `ExpanseMap`) | Property tests + invariant validator (TESTING.md) green |
| 7. OCC reads | Seqlock + EBR (done: `occ` + `sync` — single writer, lock-free validated readers, per-node versions) | Loom/stress suites green (done — CI `loom` job + thread stress) |
| 8. Hardening | capi surface, differential oracle vs C libjudy, fuzzing, benches | COMPAT.md acceptance gates; php-judy suite green against libexpanse |

### Sequencing after external architect review (2026-08-18)

An external review confirmed Phases 1–6b and identified five gaps: narrow-pointer synthesis, asymmetric root-leaf lifecycle, missing ordered navigation/rank APIs, `NodeAlloc` `Cell` counters, and the capi stub. Agreed order, with rationale:

1. **Ordered navigation + rank/count** — done. The `nav` module plus public APIs (`first`/`last`/`next_at_or_after`/`next_after`/`prev_at_or_before`/`prev_before`, `iter()`, `count_below`/`count_range`, 0-based `by_count`) on both `ExpanseSet` and `ExpanseMap`, differentially tested against the `BTree` models. Map navigation returns values; the compat layer's `First`/`Next` hand out value pointers.
2. **Phase 8: capi exports + differential oracle + benches** — the COMPAT.md acceptance gates are the project's falsifiable criteria. The oracle also retro-tests everything above, and the bench harness produces the bytes/key evidence the next step needs.
3. **Narrow-pointer synthesis in mutation** — done, both halves.
   - *Leaf-targeted*: cascades whose keys diverge only in the last byte build one skip-carrying bitmap leaf instead of a branch chain, and shrink conversions absorb decode bytes back into slot-level immediates. Measured: clustered bytes/key 1.34 → 0.35; vs-stock clustered get 5.31× slower → parity.
   - *Branch-targeted*, via **header-stored levels** (`BranchHeader.level` / `BranchB.level`): a leaf cascade places its branch at the keys' true divergence level with the shared prefix as decode bytes, and an insert diverging inside any skipped prefix splits at the **highest** diverging decode level (`split_skip`) instead of materializing one chain level per step. `BranchU` has no header and never skips, so a full skipping `BranchB` is first wrapped one level above its form. Level-8 slots never skip — the root edge cannot hold both pop0 and decode bytes. Measured: two 512-key clusters cost 192 structural bytes vs 960 under per-level chains (`branch_skip_clusters` tests); wide-cluster (4096-run) set flavor 0.19 → 0.12 B/key at 1M keys.
4. **Root-leaf shrink hysteresis** — trivially implementable once ordered iteration exists (collect ≤ 31 survivors), pointless before.
5. **Phase 7 OCC** — done, including the per-node refinement.
   - `occ` supplies the seqlock `SeqVersion` (Boehm's fence construction) and the epoch `Collector` (pin/advance SeqCst-fence pairing). Both are loom-model-checked, and loom found real ordering bugs in the first drafts of each.
   - `sync` supplies the wrappers: writers serialize on a mutex inside version brackets; readers run a validated walk with a re-check before every dereference, bounded retries and a mutex fallback; frees route through the collector. `NodeAlloc` accounting moved from `Cell` to relaxed atomics.
   - One tree-level version was the correctness landing. The measured collapse under writer churn — single-reader reads fell to 3.4 K/s against a saturating writer (BENCHMARKING.md) — then justified the **per-node refinement**: `BranchU` gained a version header (4 KiB + one line), writers bracket every node's in-place mutation region (child-slot rewrites and the recursion beneath them) with that node's version, and readers validate hand-over-hand, node by node, falling back to the tree version only for the root snapshot. Bracketing is skipped entirely on single-threaded trees via `NodeAlloc::occ_enabled`. Measured: single-reader churn throughput ~700× (3.4 K/s → 2.4 M/s).

Performance targets, measured per BENCHMARKING.md before any claim: point lookup < 15 ns on random 64-bit keys (target); < 9.5 bytes/key on dense/clustered distributions (target). `benches/comparative.rs` and `benches/concurrency.rs` implement comparison against `RoaringBitmap`, `hashbrown::HashMap` and `std::collections::BTreeMap`, plus multithreaded scaling models (1..16 threads).

---

## 7. Database Engine Subsystems & Integration

Expanse is architecturally suited as a high-density, low-latency primitive across core database subsystems:

- **Inverted Indexes & Posting Lists (`ExpanseSet`)**: Tracks 64-bit document IDs at **0.07–0.36 bytes/docID** on clustered/dense sets (outperforming Roaring Bitmaps) with bitwise set algebra executed directly over compressed trie edges and $O(\text{depth})$ skip-scans (`next_at_or_after`).
- **MVCC Visibility Maps & Active Transaction Tracking (`SyncExpanseSet`)**: Provides lock-free reader validation over active transaction IDs (`xid`) with zero reader-writer locks, single-digit nanosecond point lookups, and epoch-based safe reclamation under continuous OLTP commit/vacuum churn.
- **Columnar String & Symbol Dictionaries (`ExpanseStrMap`)**: Maps high-cardinality strings to 32/64-bit symbol IDs using 8-byte big-endian chunk decomposition and tail collapse, preserving lexicographical sort order while sharing common prefix nodes.
- **Secondary Indexes & MemTables (`ExpanseMap`)**: Serves as a rebalance-free LSM MemTable and secondary index engine with contiguous 64-byte SIMD leaf scans. Full ordered `iter()` is **faster than `BTreeMap::iter()` for dense key distributions** at 1M keys — sequential 0.7×, clustered 0.8×, random 0.5× (2× faster) the time of `BTreeMap::iter()`, after #245's stack-based zero-allocation iterator. **Sparse-key iteration remains ~4.7× slower**, a structural residual tracked in [#270](https://github.com/orieg/expanse/issues/270). *(measured: reference host — Intel i9-12900F, 24 threads, commit 46529f19, `benches/compare.rs`)*
- **Zero-Copy Shared-Memory Analytics**: Off-heap / mmap base-relative layouts enable cross-worker zero-serialization analytics for parallel multi-process query execution.

For detailed architecture, integration mechanics, algorithms, and code blueprints, see [DATABASE.md](DATABASE.md).
 
---
 
## 8. 32-Bit Architecture & Embedded Microprocessor Support (RV32 / ESP32 / Cortex-M)

For 32-bit microcontrollers (RISC-V `RV32I`/`RV32EMAC`, Espressif `ESP32`/`ESP32-S3`/`ESP32-C3`, ARM `Cortex-M0+`/`M3`/`M4`/`M7`/`M33`), Expanse ships a real 32-bit trie engine (`ExpanseSet32`, `ExpanseMap32`, `ExpanseBlobMap32`) designed for severely constrained SRAM (64 KiB – 512 KiB). This shipped in v0.3.0 (#230): `trie32`/`set32`/`map32`/`blobmap32` compile unconditionally, and on 32-bit targets the public aliases re-point (`ExpanseMap` → `ExpanseMap32`, etc.).

- **4-Level Digital Tree Hierarchy**: 32-bit keys (`Key = u32`, Levels 4 $\rightarrow$ 1) halve maximum trie descent depth from 8 to 4 hops.
- **Compact 8-Byte `Edge32`**: 4-byte pointer/immediate + 3-byte level-split `pop0`/decode field + 1-byte tag discriminant, delivering an immediate **50% reduction in structural memory** (byte offsets in §8.1).
- **Immediate In-Edge Storage**: Up to 7 1-byte keys, 3 2-byte keys, or 2 3-byte keys packed directly inside a single 8-byte edge without heap allocation.
- **Polymorphic 32-Bit Value Slots (`ValueSlot32`)**: inline, arena, and raw-word modes for zero-allocation payloads and classic `JudyL` 32-bit C ABI drop-in compatibility (enumerated in §8.2).
- **32-Byte Cache Alignment & Native Atomics**: Node geometries tailored for 32-byte cache lines (Cortex-M7, ESP32) and un-cached internal SRAM, with native 32-bit OCC reader validation (`SeqVersion32` via `AtomicU32`) on `RV32A` without 64-bit atomic dependencies.

### 8.1 Node Geometries

```
========================================================================================
64-Bit Server Architecture vs. 32-Bit Embedded Architecture
========================================================================================

           64-Bit Server (x86-64 / ARM64 / RV64)          32-Bit Embedded (RV32 / ESP32 / Cortex-M)
           ------------------------------------          ----------------------------------------
Key Type:  u64 (8 Bytes)                                 u32 (4 Bytes)
Tree Depth:8 Levels (L8 -> L1)                           4 Levels (L4 -> L1, 48% lower latency)
Edge Size: 16 Bytes                                      8 Bytes (-50% Structural RAM)
Cache Line:64 Bytes / 128 Bytes                          32 Bytes (Cortex-M7, ESP32) / Flat SRAM
Value Slot:64-bit (<=7B inline, 24-bit meta)              32-bit (<=3B inline, 12-bit meta)
Atomics:   AtomicU64 SeqVersion                          AtomicU32 SeqVersion32 (Native RV32A)
Max Heap:  Exabytes (Virtual Addressing)                 64 KiB - 16 MiB (Physical SRAM)
```

**Compact 8-Byte `Edge32` layout:**
```
offset 0: word 0     4 B   child node pointer, or immediate key/value payload
offset 4: aux        3 B   level-split: pop0 count + narrow pointer decode bytes
offset 7: tag        1 B   edge type discriminant tag
```

**32-byte microcontroller cache-line alignment.** Embedded cores like ARM Cortex-M7 and ESP32 MMU caches feature **32-byte cache lines**:
- **`BranchL2_32`**: 8B Header + 2B Digits + 6B Pad + 16B Child Edges = **32 Bytes** (exactly 1 cache line).
- **`BranchL6_32`**: 8B Header + 6B Digits + 2B Pad + 48B Child Edges = **64 Bytes** (exactly 2 cache lines).
- **`LeafBitmap1_32`**: 32B 256-bit bitmask + 4B pop0/level/pad = 36 declared bytes, which `#[repr(C, align(32))]` rounds to **64 bytes** — the figure the engine's own accounting and its bitmap-leaf conversion threshold use ([§10.6](#106-bitmap-structures)).

### 8.2 Polymorphic 32-Bit Value Slots (`ValueSlot32`)

- **Inline Mode ($\le 3\text{ B}$)**: Direct in-slot storage with zero heap allocation.
- **Arena Mode**: 12-bit hot metadata (TTL, flags) + 12-bit slab offset (up to 4096 entries per chunk) + the 8-bit tag — 12 + 12 + 8 = 32 bits exactly ([§10.5](#105-valueslot--the-8-byte-polymorphic-value-word)).
- **Raw Word Mode (`0xFF`)**: Drop-in 32-bit `JudyL` C ABI compatibility (`uint32_t`).

For complete struct definitions, bit layouts, cache models, and implementation phase gates, see [design/32-bit-embedded.md](design/32-bit-embedded.md).

---

## 9. 57-Bit / 64-Bit Virtual Addressing & 5-Level Paging (PML5 / LA57 / ARMv8.2-LVA)

On modern 64-bit server architectures, **48-bit virtual addressing is an obsolete assumption**:
- **x86-64 5-Level Paging (PML5 / LA57)**: Current Intel (Ice Lake, Sapphire Rapids, Emerald Rapids, Granite Rapids) and AMD (Zen 4 Genoa, Zen 5 Turin) processors widen the virtual address from 48 bits to **57 bits (128 PiB total address space)**. That space is split into a lower (user) half and an upper (kernel) half, so the Linux **userspace lower half grows from 47 bits (128 TiB) to 56 bits (64 PiB)**.
- **ARM64 Large Virtual Addressing (ARMv8.2-A+ / LVA)**: Modern ARM architectures (AWS Graviton 3/4, Apple Silicon, Neoverse V2) extend virtual addressing to **52 bits (userspace lower half up to 4 PiB)**.
- **High-Memory Nodes & ASLR**: On LA57 the Linux kernel's **default `mmap` window stays below bit 47** — this preserves compatibility with pointer-tagging schemes (LAM/TBI) and allocators that stash bits in the top of a pointer. Allocations move **above** bit 47 (`0x0000_7FFF_FFFF_FFFF`) only when a caller passes a high address hint (`mmap` with `MAP_FIXED`/hint) or exceeds the default window — as jemalloc, the Go runtime, and high-entropy ASLR do — at which point pointers legitimately occupy the full 57-bit range.

### The Classic Flaw (Upper-Bit Pointer Stealing)
Older 2002-era C implementations (including classic `libjudy` and V8-style NaN-tagging) packed metadata by assuming bits 48–63 of pointers were unused zeros. On PML5/LVA hardware, allocating heap memory above 48 bits causes pointer corruption and non-canonical address `#GP` (General Protection Fault) segfaults when dereferenced.

### Expanse's First-Principles 64-Bit Design

1. **Dual-Word 16-Byte `Edge` Representation (Zero Upper-Bit Stealing)**:
   - **Word 0 (8 Bytes)** holds the raw, untruncated 64-bit virtual pointer or integer immediate payload. All bits 0–63 are preserved intact without bit-stealing.
   - **Word 1 (8 Bytes)** holds structural metadata: 7 bytes of level-split population/decode bytes and a dedicated 1-byte discriminant tag (`EdgeTag`).
   - Pointers are always canonical and directly dereferenceable with zero masking overhead.
2. **Low-Bit Alignment Guarantees (Bottom 4 Bits, Never Top 16)**:
   - Where compact discriminant tagging is required internally, Expanse relies exclusively on **allocator-enforced low-bit alignment**:
     - All internal trie nodes (Branches, Linear Leaves, Bitmap Leaves) are 16-byte or 64-byte aligned (`#[repr(align(16))]` / `#[repr(align(64))]`).
     - By mathematical definition, bits 0..3 of valid node pointers are guaranteed to be `0b0000`.
3. **Base-Relative Offsets in Large-Value Arenas (`BlobArena`)**:
   - In `ExpanseBlobMap`, chunks and slabs are indexed via 32-bit chunk indices and relative offsets from a base arena pointer, allowing unlimited virtual memory expansion across terabytes on PML5 systems without tag collisions.

Expanse functions transparently on 48-bit legacy systems, 52-bit ARM64, and 57-bit x86-64 PML5/LA57 hardware without address truncation, bit masking, or `#GP` faults.

> **Primary-source citation.** The 57-bit VA / LA57 assumption is validated against the Intel SDM in [`docs/HARDWARE.md` §1.6](HARDWARE.md#16-la57--5-level-paging--57-bit-va--validated-intel-side); the ARM64 52-bit LVA and low-bit alignment guarantees are covered in [`docs/HARDWARE.md` §2](HARDWARE.md#2-aarch64-arm--apple-silicon).

---

## 10. Bit-level encoding reference

§2–3 give node *geometry* and [ALGORITHMS.md](ALGORITHMS.md) gives descent *flow*. This section is the layer between them: the exact bit and byte encoding of every tagged word the engine reads or writes. It exists because that layer previously lived only in scattered doc comments, and external writing repeatedly restated it wrongly (see §10.9).

**Every number below is derived from the compiled source, not from prose.** The pinned-constant table in §10.8, the tag tables in §10.3, the capacity tables in §10.4 and the behavioural claims flagged *(gated)* are asserted against the compiled crate by `crates/expanse/tests/test_encoding_reference_sync.rs`. A layout change fails that test instead of silently invalidating this text. Do not hand-edit a gated value; change the source and re-run the test, which prints the expected value.

Citations are `path:line` at the commit this section was written against. The gate checks that each cited file and line still exists and still mentions the symbol; a `path:line` that has merely drifted will fail the test with the symbol it was looking for.

### 10.1 `Edge` — the 16-byte tagged descriptor (64-bit targets)

```
byte  0 ..  7   word 0   child-node pointer, or immediate payload
byte  8 .. 14   aux      7 B, level-split: low L bytes pop0, high bytes decode
byte 15         tag      1 B type tag
```

Declared at `crates/expanse/src/node.rs:62`; `size_of` = 16, `align_of` = 8, `aux` at offset 8, `tag` at offset 15, all const-asserted at `crates/expanse/src/node.rs:556`–`559`.

**Word 0 is a `union`** (`crates/expanse/src/node.rs:47`) of `*mut u8` and `[u8; 8]`. What it carries depends on the tag class:

| Tag class | Word 0 holds |
|---|---|
| `Null` | zero (`Edge::NULL`, `crates/expanse/src/node.rs:82`) |
| Branch / linear-leaf / bitmap-leaf tags | the raw child-node pointer (`Edge::new_node`, `crates/expanse/src/node.rs:91`) |
| `FullExpanse` | unused — the tag alone states that the whole subexpanse is present |
| Set immediate | the first 8 of up to 15 packed key-remainder bytes (`Edge::imm_payload`, `crates/expanse/src/node.rs:194`) |
| Map immediate, 1 key | the value word (`Edge::new_immed_single_map`, `crates/expanse/src/node.rs:115`) |
| Map immediate, ≥ 2 keys | a pointer to a heap value array of `8 × cap_class(n)` bytes (`crates/expanse/src/mutate_map.rs:86`, sized by `map_immed_val_size`, `crates/expanse/src/mutate_map.rs:33`) |

**Word 1 is the aux/tag word.** `Edge::aux_word` (`crates/expanse/src/node.rs:247`) reads `aux[0..7]` plus the tag byte as one little-endian `u64`: `aux[0]` is the low byte, the tag is the high byte. The little-endian requirement is const-asserted at `crates/expanse/src/node.rs:563`.

The 7 aux bytes are **level-split** for a pointer-carrying edge whose child sits at level `L` (1..=7):

- low `L` bytes: `pop0`, the subtree population minus one. A level-`L` subtree holds at most `256^L` keys, so `L` bytes always suffice. Masked by `POP0_MASKS` (`crates/expanse/src/node.rs:69`), read by `Edge::pop0` (`crates/expanse/src/node.rs:226`) and written by `Edge::set_pop0` (`crates/expanse/src/node.rs:266`) as a masked read-modify-write of the same word, so the decode bytes and the tag survive.
- high `7 - L` bytes: the narrow-pointer *decode* bytes naming the digits this edge skips. `Edge::decode_bytes` (`crates/expanse/src/node.rs:281`) returns `&self.aux[L..]`.

The two regions never overlap. That is why no branch header carries a wide population field, and why level-8 slots can never skip: at `L = 8` there are no aux bytes left over for decode digits.

For **immediate** edges the aux bytes are key or value storage instead, and there is no `pop0` (the tag's key count *is* the population). See §10.4.

#### Why word 0 is stored unmasked *(gated)*

Word 0 holds the full, untruncated 64-bit pointer. No bit of it is stolen for metadata — the tag has its own byte, and the population and decode fields have their own word. `Edge::node_ptr` (`crates/expanse/src/node.rs:163`) reads the union member and returns it with no masking, shifting or sign-extension.

This is the deliberate opposite of the classic 48-bit-virtual-address assumption, and it is what keeps the representation correct on 57-bit x86-64 (LA57 / PML5) and 52-bit ARM64 (LVA) hardware, where a heap pointer can legitimately use bits above 47. §9 gives the hardware background and the primary-source citations.

Two things are commonly confused with this and are **not** shipped:

- The compact 8-byte edge variant sketched in §3.4 (`[type:16][address:45][level:3]`) is a **design note, not implemented**. No type in `crates/expanse/src/` packs an address into a bitfield on a 64-bit target.
- `Edge32` (§10.2) *is* 8 bytes, but that is the 32-bit-target descriptor, and even there word 0 is a whole 32-bit word — an arena handle in the shipped engine — not a packed address field.

The gate asserts the round trip directly: an `Edge::new_node` built over a pointer value with every bit above 47 set returns bit-identical from `node_ptr()`, and the tag byte is unaffected.

### 10.2 `Edge32` — the 8-byte descriptor (32-bit targets)

```
byte 0 .. 3   word 0   child handle / pointer, or immediate payload
byte 4 .. 6   aux      3 B, decode digits / population / more payload
byte 7        tag      1 B tag discriminant
```

Declared at `crates/expanse/src/types32.rs:128`; `size_of` = 8, `align_of` = 4, const-asserted at `crates/expanse/src/types32.rs:137`–`138`. `trie32`/`set32`/`map32`/`blobmap32` compile unconditionally on every target; on a 32-bit target the public aliases re-point (`ExpanseMap` → `ExpanseMap32`, and so on, `crates/expanse/src/lib.rs:107`–`116`).

Three divergences from the 64-bit `Edge` matter:

1. **Word 0 is a handle, not a pointer, in the shipped engine.** `Edge32::new_node` (`crates/expanse/src/types32.rs:171`) will store a truncated `*mut u8`, but the real 32-bit trie keeps nodes in a per-tree arena and stores a 32-bit arena index in word 0 instead, so the engine behaves identically on a 64-bit host and on RV32 — and so `Arena::bytes_in_use` is an exact memory figure. The rationale is stated at `crates/expanse/src/trie32.rs:13`–`24`.
2. **The aux field is 3 bytes, not 7**, and `Edge32::aux_u24` (`crates/expanse/src/types32.rs:193`) reads it as one little-endian 24-bit value. There are only 4 decode levels (`MAX_LEVEL_32 = 4`, `crates/expanse/src/types32.rs:18`), so the level-split budget is correspondingly smaller.
3. **The tag space is different, and there are two of them.** `Tag32` (`crates/expanse/src/types32.rs:59`) is the design-document enumeration; the shipped engine writes its own raw tag bytes (`crates/expanse/src/trie32.rs:107`–`120`) and decodes them with `kind_of` (`crates/expanse/src/trie32.rs:153`). `Tag32` is referenced nowhere outside its own module. Both are tabulated in §10.3.3 so neither is mistaken for the other.

### 10.3 Tag discriminants

#### 10.3.1 `EdgeType` — structural tags (64-bit)

Declared at `crates/expanse/src/types.rs:90`, decoded by `EdgeType::from_u8` (`crates/expanse/src/types.rs:126`). The variant names below are the compiled `Debug` names; the gate decodes every listed byte and compares.

<!-- ENCODING-TABLE edge_type -->

| Variant | Tag byte | Refers to |
|---|---|---|
| `Null` | 0x00 | empty subexpanse; no keys below this edge |
| `BranchL3` | 0x01 | one-line linear branch, ≤ 3 child edges |
| `BranchL7` | 0x02 | two-line linear branch, ≤ 7 child edges |
| `BranchB` | 0x03 | bitmap branch: 256-bit membership + 8 packed edge subarrays |
| `BranchU` | 0x04 | uncompressed branch: flat 256-edge page |
| `Leaf1` | 0x05 | linear leaf, 1-byte key remainders |
| `Leaf2` | 0x06 | linear leaf, 2-byte key remainders |
| `Leaf3` | 0x07 | linear leaf, 3-byte key remainders |
| `Leaf4` | 0x08 | linear leaf, 4-byte key remainders |
| `Leaf5` | 0x09 | linear leaf, 5-byte key remainders |
| `Leaf6` | 0x0A | linear leaf, 6-byte key remainders |
| `Leaf7` | 0x0B | linear leaf, 7-byte key remainders |
| `LeafB1` | 0x0C | level-1 bitmap leaf: 256-bit mask over the final key byte |
| `FullExpanse` | 0x7F | set flavor only: every key in this subexpanse is present, no node allocated |

<!-- /ENCODING-TABLE -->

`is_branch` (`crates/expanse/src/types.rs:156`) is true for `0x01..=0x04`; `is_leaf` (`crates/expanse/src/types.rs:166`) is true for `0x05..=0x0C`. `FullExpanse` is neither, and `leaf_key_bytes` (`crates/expanse/src/types.rs:184`) returns `Some(n)` only for `Leaf1..Leaf7`.

#### 10.3.2 Immediate tags (64-bit)

Immediate tags occupy a nibble-packed space disjoint from the structural bytes: `(key_bytes << 4) | (key_count - 1)`, built and validated by `ImmedType::new` (`crates/expanse/src/types.rs:214`) and decoded by `ImmedType::from_u8` (`crates/expanse/src/types.rs:232`). A byte is a valid immediate tag exactly when

```
1 <= key_bytes <= 7   and   1 <= key_count   and   key_bytes * key_count <= IMMED_PAYLOAD_BYTES
```

with `IMMED_PAYLOAD_BYTES = 15` (`crates/expanse/src/types.rs:82`). The raw values therefore span `0x10..=0x71`, never colliding with `0x00..=0x0C` or `0x7F`. `EdgeTag` (`crates/expanse/src/types.rs:267`) unifies both spaces and is total: every one of the 256 bytes decodes as structural, immediate, or invalid, never two of those.

Valid tag-byte counts *(gated)*: 14 structural + 37 immediate = 51 of 256.

The structural and immediate tags fall inside the 7-bit envelope `0x00..=0x7F` (with unused gaps at `0x0D..=0x0F` and `0x72..=0x7E`). Two attempts to expand tag dispatch (#433 hot-first prefilter at +4% to +9%, and #441 seven `BranchL3` level-specialised tags at +26% to +44%) showed that adding dispatch arms or branching costs significantly more in retired instructions across all operations than any unquantified ALU or load latency it was intended to save.

#### 10.3.3 32-bit tag spaces

`Tag32` (`crates/expanse/src/types32.rs:95`), the design-document enumeration:

<!-- ENCODING-TABLE tag32 -->

| Variant | Tag byte |
|---|---|
| `Null` | 0x00 |
| `LeafBitmap1` | 0x01 |
| `LeafLinear1` | 0x02 |
| `LeafLinear2` | 0x03 |
| `LeafLinear3` | 0x04 |
| `BranchL2` | 0x05 |
| `BranchL6` | 0x06 |
| `BranchB` | 0x07 |
| `BranchU` | 0x08 |
| `LeafBitmapL` | 0x09 |
| `ImmedSet` | 0x10 |
| `ImmedMap` | 0x11 |
| `ValueSlotInline` | 0x20 |
| `ValueSlotArena` | 0x21 |
| `ValueSlotRaw` | 0x22 |
| `Custom` | 0xFF |

<!-- /ENCODING-TABLE -->

`Tag32::from_u8` maps every unlisted byte to `Custom`, so `Custom` is a catch-all rather than a reserved value.

The tags the shipped 32-bit engine actually writes are module-private constants in `crates/expanse/src/trie32.rs:114`–`129`. They are gated by a source scan rather than by symbol reference, because the engine deliberately keeps them private:

<!-- ENCODING-TABLE trie32_tags -->

| Constant | Value | Meaning |
|---|---|---|
| `T_NULL` | 0 | empty edge |
| `T_L2` | 1 | `BranchL2_32` |
| `T_L6` | 2 | `BranchL6_32` |
| `T_U` | 3 | `BranchU32` |
| `T_BITMAP` | 4 | `LeafBitmap1_32` |
| `T_SET_LEAF_BASE` | 4 | set linear leaf tag is `T_SET_LEAF_BASE + key_bytes`, so 5..=8 |
| `T_MAP_LEAF_BASE` | 8 | map linear leaf tag is `T_MAP_LEAF_BASE + key_bytes`, so 9..=12 |
| `T_B` | 13 | `BranchB32` |
| `T_MAP_BITMAP` | 14 | `LeafBitmapL_32` |
| `T_SET_IMMED_BASE` | 0x40 | set immediate tag is `0x40 \| ((key_bytes - 1) << 3) \| (key_count - 1)`, so 0x40..=0x5F |
| `T_MAP_IMMED_BASE` | 0x60 | map immediate tag is `0x60 \| (key_bytes - 1)`, single entry only, so 0x60..=0x62 |

<!-- /ENCODING-TABLE -->

`kind_of` (`crates/expanse/src/trie32.rs:153`) decodes these back into a `Kind`; any byte outside the listed ranges decodes as `Kind::Null`.

### 10.4 Immediate capacity

The budget rule is a byte count, not a key count. This is the fact most often restated wrongly: an immediate holds **15 bytes** of set-flavor key payload, which is 15 keys only when the remainder is 1 byte wide, and 2 keys when it is 7 bytes wide.

- **Set flavor, 64-bit.** Keys pack across word 0 and the aux bytes — bytes 0..14 of the edge, 15 usable bytes, byte 15 being the tag (`Edge::imm_payload`, `crates/expanse/src/node.rs:194`; writer `write_immed`, `crates/expanse/src/mutate.rs:301`). Capacity is `IMMED_PAYLOAD_BYTES / key_bytes`, i.e. `ImmedType::max_count` (`crates/expanse/src/types.rs:260`).
- **Map flavor, 64-bit.** Keys live in the 7 aux bytes only, because word 0 is spent on the value (one key) or on the value-array pointer (two or more) — `write_map_immed`, `crates/expanse/src/mutate_map.rs:75`. Capacity is `7 / key_bytes`, `mutate::map_immed_max` (`crates/expanse/src/mutate.rs:325`). The gate derives the 7 from the compiled length of `Edge::aux_bytes()` rather than from this sentence.
- **Set flavor, 32-bit.** Keys pack across word 0 and the 3 aux bytes — 7 usable bytes of an 8-byte edge. Capacity is `7 / key_bytes`, `trie32::set_immed_cap` (`crates/expanse/src/trie32.rs:85`), for `key_bytes` in 1..=4.
- **Map flavor, 32-bit.** A map immediate is single-entry by construction: the tag encodes only `key_bytes` (1..=3) and word 0 is the value (`crates/expanse/src/trie32.rs:119`–`120`, `crates/expanse/src/types32.rs:243`).

<!-- ENCODING-TABLE immediate_capacity -->

| Key bytes | 64-bit set max keys | 64-bit map max keys | 32-bit set max keys |
|---|---|---|---|
| 1 | 15 | 7 | 7 |
| 2 | 7 | 3 | 3 |
| 3 | 5 | 2 | 2 |
| 4 | 3 | 1 | 1 |
| 5 | 3 | 1 | n/a |
| 6 | 2 | 1 | n/a |
| 7 | 2 | 1 | n/a |

<!-- /ENCODING-TABLE -->

The `n/a` rows are widths a 32-bit key cannot produce: a 4-level trie leaves at most 4 undecoded bytes.

Pinning tests for these numbers live in `crates/expanse/src/types.rs:367` (`immed_capacity_bounds`), `crates/expanse/src/types.rs:343` (`tag_spaces_are_disjoint_and_total`), and `crates/expanse/src/trie32.rs:1732` (`immediate_payload_round_trips_all_widths`), in addition to the doc gate.

### 10.5 `ValueSlot` — the 8-byte polymorphic value word

`ValueSlot` (`crates/expanse/src/slot.rs:174`) is `#[repr(transparent)]` over a `u64`, so a map leaf's value area is exactly 8 slots per 64-byte line and the `JudyL` C ABI `*mut Word` contract is preserved. The low byte is always the tag.

`SlotTag` (`crates/expanse/src/slot.rs:33`), decoded by `SlotTag::from_u8` (`crates/expanse/src/slot.rs:79`):

<!-- ENCODING-TABLE slot_tag -->

| Variant | Tag byte | Inline length | Rest of the word |
|---|---|---|---|
| `Inline0` | 0x00 | 0 | unused |
| `Inline1` | 0x01 | 1 | payload byte in bits 15:8 |
| `Inline2` | 0x02 | 2 | payload bytes in bits 23:8 |
| `Inline3` | 0x03 | 3 | payload bytes in bits 31:8 |
| `Inline4` | 0x04 | 4 | payload bytes in bits 39:8 |
| `Inline5` | 0x05 | 5 | payload bytes in bits 47:8 |
| `Inline6` | 0x06 | 6 | payload bytes in bits 55:8 |
| `Inline7` | 0x07 | 7 | payload bytes in bits 63:8 |
| `ArenaMeta` | 0x10 | — | hot metadata in bits 63:40, arena locator in bits 39:8 |
| `External` | 0x12 | — | reserved; no code path produces or consumes it |
| `CompressedZeroTrim8` | 0x20 | — | 8-byte LE integer with upper zero byte in bits 63:8 |
| `CompressedAlnum8` | 0x22 | — | 8 6-bit alphanumeric chars packed in bits 55:8 |
| `CompressedAlnum9` | 0x23 | — | 9 6-bit alphanumeric chars packed in bits 61:8 |
| `CompressedNibble8` | 0x28 | — | 8 4-bit decimal digits packed in bits 39:8 |
| `CompressedNibble9` | 0x29 | — | 9 4-bit decimal digits packed in bits 43:8 |
| `CompressedNibble10` | 0x2A | — | 10 4-bit decimal digits packed in bits 47:8 |
| `CompressedNibble11` | 0x2B | — | 11 4-bit decimal digits packed in bits 51:8 |
| `CompressedNibble12` | 0x2C | — | 12 4-bit decimal digits packed in bits 55:8 |
| `CompressedNibble13` | 0x2D | — | 13 4-bit decimal digits packed in bits 59:8 |
| `CompressedNibble14` | 0x2E | — | 14 4-bit decimal digits packed in bits 63:8 |
| `Tombstone` | 0xFE | — | soft-deleted marker |
| `RawWord` | 0xFF | — | uninterpreted 64-bit word |

<!-- /ENCODING-TABLE -->

`from_u8` maps every unlisted byte to `RawWord`, so `RawWord` is the catch-all; `is_inline` (`crates/expanse/src/slot.rs:155`) is simply `tag <= 0x07`, and `inline_len` (`crates/expanse/src/slot.rs:162`) returns the tag itself as the length.

**Inline encoding** *(gated)*. `ValueSlot::new_inline` (`crates/expanse/src/slot.rs:194`) writes `raw = len | Σ bytes[i] << (8 * (i + 1))`: the length is the tag byte, and the payload occupies bits 63:8 little-endian. The payload is the whole word above the tag, which is precisely why an inline slot carries **no metadata field** — `ExpanseBlobMap` ignores the `hot_meta` argument for payloads of ≤ 7 bytes and reports their metadata as `0` (`crates/expanse/src/blobmap.rs:27`–`33`, `crates/expanse/src/blobmap.rs:1120`). No cold fetch is needed for them in any case: the payload is already in the slot.

This is also where `ExpanseBlobMap` puts small payloads — in the leaf's value slot, **not** inside an edge.

**`ArenaMeta` encoding** *(gated)*. `ValueSlot::new_arena_meta` (`crates/expanse/src/slot.rs:215`) writes

```
raw = (hot_meta << 40) | (locator << 8) | 0x10

  bits 63:40   hot_meta   24 bits, capped by ARENA_META_MAX = 0x00FF_FFFF
  bits 39: 8   locator    32 bits
  bits  7: 0   tag        0x10
```

`hot_meta` above the 24-bit field is rejected (`None`), never truncated. `arena_meta_meta` (`crates/expanse/src/slot.rs:247`) and `arena_meta_locator` (`crates/expanse/src/slot.rs:255`) read the two fields back; `with_arena_meta_meta` (`crates/expanse/src/slot.rs:263`) rewrites the metadata in place without disturbing the locator. This is the sole arena encoding — there is no metadata-less spill form, so a predicate over metadata is always evaluable in-slot.

**Locator arithmetic** *(gated)*. The locator is not a chunk/offset pair; it is a flat global address in 16-byte units:

```
locator       = global_offset / ARENA_ALIGN          (ARENA_ALIGN = 16)
global_offset = locator * ARENA_ALIGN
```

`slot_from_global` at `crates/expanse/src/blobmap.rs:501` performs the first, `resolve_meta` at `crates/expanse/src/blobmap.rs:851` and `resolve_meta_in_table` at `crates/expanse/src/blobmap.rs:576` the second. The chunk/offset split is resolved by the arena geometry afterwards, so a chunk boundary must stay a multiple of 16 — a loaded image with a misaligned boundary is rejected (`crates/expanse/src/blobmap.rs:1424`). The envelope is `ARENA_META_CEILING = 2^32 × 16` = 64 GiB (`crates/expanse/src/blobmap.rs:473`), well above the shipped `MAX_ARENA_CAPACITY` growth cap of 1 GiB (`crates/expanse/src/blobmap.rs:491`), so a locator overflow cannot occur under the shipped cap.

**`ValueSlot32`** (`crates/expanse/src/slot32.rs:47`) is the 32-bit counterpart, `#[repr(transparent)]` over a `u32`, same low-byte-is-tag convention:

<!-- ENCODING-TABLE slot_tag32 -->

| Variant | Tag byte | Rest of the word |
|---|---|---|
| `Inline0` | 0x00 | unused |
| `Inline1` | 0x01 | payload byte in bits 15:8 |
| `Inline2` | 0x02 | payload bytes in bits 23:8 |
| `Inline3` | 0x03 | payload bytes in bits 31:8 |
| `Arena` | 0x10 | hot metadata in bits 31:20, slab offset in bits 19:8 |
| `RawWord` | 0xFF | uninterpreted 32-bit word (C ABI drop-in) |

<!-- /ENCODING-TABLE -->

Both arena fields are **12 bits wide**: `ARENA_OFFSET_MASK = 0x000F_FF00` at shift 8 and `ARENA_META_MASK = 0xFFF0_0000` at shift 20 (`crates/expanse/src/slot32.rs:59`–`66`), and `ValueSlot32::new_arena` (`crates/expanse/src/slot32.rs:119`) rejects either argument above `0x0FFF`. 12 bits of metadata and 4096 addressable slab entries — not the 16 bits some older prose claimed.

### 10.6 Bitmap structures

`Bitmap256` (`crates/expanse/src/bits.rs:491`) is four `u64` words, 32 bytes, covering one decode byte's 256 values. Bit `idx` is word `idx >> 6`, bit `idx & 63` (`test`, `crates/expanse/src/bits.rs:568`).

Both bitmap branches and bitmap map-leaves partition those 256 values into **eight 32-digit subexpanses**, each with its own packed array, so the rank that finds a slot is a rank *within a subexpanse*, not a global rank:

- `subexpanse_rank` (`crates/expanse/src/bits.rs:713`) reinterprets the four `u64` words as eight `u32` subwords, loads subword `idx >> 5`, and popcounts the bits below `idx & 31`. That is one 32-bit load and one popcount — no loop over preceding words.
- `test_and_subexpanse_rank` (`crates/expanse/src/bits.rs:727`) fuses the membership test with that rank, and `test_and_subexpanse_rank_with_sub` (`crates/expanse/src/bits.rs:746`) also returns the subexpanse index.
- `subexpanse_count` (`crates/expanse/src/bits.rs:765`) is the length of one subexpanse's packed array.
- `rank` (`crates/expanse/src/bits.rs:695`) is the *global* count of members below `idx`, used for ordered navigation rather than slot addressing; `select` (`crates/expanse/src/bits.rs:776`) inverts it and is the `ByCount` primitive.

**`BranchB`** (`crates/expanse/src/node.rs:425`) is 128 bytes: the bitmap at offset 0, `subarrays: [*mut Edge; 8]` at offset 32, `pop_counts: [u16; 8]` at offset 96, `version` at 112. Line 0 therefore holds the bitmap plus the first four subarray pointers, so a lookup landing in digits `0x00..0x7F` touches one line before the child edge. Reaching a child is: `test_and_subexpanse_rank(digit)` → `subarrays[digit >> 5]` → `.add(rank)`.

**`LeafBitmapL`** (`crates/expanse/src/node.rs:523`) is the map-flavor bitmap leaf, also 128 bytes: bitmap at 0, `values: [*mut u64; 8]` at offset 32, `version` at 96. Reaching a value is the same three steps against the value subarrays — bitmap test, subexpanse rank, index into `values[digit >> 5]`.

**`LeafBitmap1`** (`crates/expanse/src/node.rs:491`) is the set-flavor level-1 leaf, 64 bytes: the bitmap *is* the membership answer, so there is no subarray and no rank on the lookup path.

The 32-bit bitmap leaf `LeafBitmap1_32` (`crates/expanse/src/node32.rs:170`) stores its 256-bit mask as `[u64; 4]` plus a `u16` population and a level byte. Its declared fields total 36 bytes but `#[repr(C, align(32))]` rounds the type to 64 bytes, and 64 is the figure the engine's own accounting and conversion threshold use (`crates/expanse/src/trie32.rs:93`, `crates/expanse/src/trie32.rs:247`).

### 10.7 Per-node OCC version words

Each version word is a plain `u32` seqlock counter: even means stable, odd means a mutation is in progress. Writers bracket a node's in-place mutation region — the child-slot rewrites *and* the recursion beneath them — with `version_begin_if` / `version_end_if` (`crates/expanse/src/occ.rs:119`, `crates/expanse/src/occ.rs:131`), and only when the tree is concurrently shared. Readers sample and re-validate hand-over-hand with `node_sample` / `node_validate` (`crates/expanse/src/occ.rs:169`, `crates/expanse/src/occ.rs:182`). The tree-level `SeqVersion` (`crates/expanse/src/occ.rs:40`) is a separate `AtomicU64` covering the root snapshot.

| Node type | Field | Byte offset | In the read protocol? |
|---|---|---|---|
| `BranchL3` / `BranchL7` | `hdr.version` | 0 | yes — `crates/expanse/src/sync.rs:218` |
| `BranchB` | `version` | 112 | yes |
| `BranchU` | `version` | 0 | yes |
| `LeafBitmap1` | `version` | 32 | no — field present, never bracketed |
| `LeafBitmapL` | `version` | 96 | no — field present, never bracketed |

The offsets are gated in §10.8. The last column is not: it reflects that `crates/expanse/src/occ.rs:97`–`99` names exactly three carriers (`BranchHeader.version`, `BranchB.version`, `BranchU.version`), and no `version_begin_if` call site in `mutate.rs` or `mutate_map.rs` passes a bitmap leaf's field. Bitmap-leaf payloads are covered by the parent branch's version instead, per §4.1's "terminal payloads are covered by their parent's version". The two leaf fields are reserved capacity, not live protocol state.

### 10.8 Pinned constants

Values are decimal unless prefixed `0x`. The gate asserts each against the compiled crate and checks that the cited file and line still exist and still mention the symbol.

<!-- ENCODING-CONSTANTS -->

| Symbol | Value | Source |
|---|---|---|
| `size_of::<Edge>()` | 16 | `crates/expanse/src/node.rs:556` |
| `align_of::<Edge>()` | 8 | `crates/expanse/src/node.rs:557` |
| `offset_of!(Edge, aux)` | 8 | `crates/expanse/src/node.rs:558` |
| `offset_of!(Edge, tag)` | 15 | `crates/expanse/src/node.rs:559` |
| `MAX_LEVEL` | 8 | `crates/expanse/src/types.rs:61` |
| `BRANCH_FANOUT` | 256 | `crates/expanse/src/types.rs:64` |
| `BRANCH_L3_CAP` | 3 | `crates/expanse/src/types.rs:71` |
| `BRANCH_L7_CAP` | 7 | `crates/expanse/src/types.rs:74` |
| `BRANCHB_TO_L7_DOWN` | 6 | `crates/expanse/src/types.rs:77` |
| `BITMAP_TO_UNCOMPRESSED_THRESHOLD` | 192 | `crates/expanse/src/types.rs:81` |
| `BRANCHU_TO_B_DOWN` | 191 | `crates/expanse/src/types.rs:84` |
| `IMMED_PAYLOAD_BYTES` | 15 | `crates/expanse/src/types.rs:88` |
| `LEAF1_CAP` | 25 | `crates/expanse/src/types.rs:92` |
| `LEAFB1_DOWN` | 21 | `crates/expanse/src/types.rs:95` |
| `LEAF_CAP` | 32 | `crates/expanse/src/types.rs:99` |
| `ROOT_LEAF_CAP` | 31 | `crates/expanse/src/types.rs:102` |
| `CACHE_LINE` | 64 | `crates/expanse/src/types.rs:43` |
| `RAW_ALIGN` | 16 | `crates/expanse/src/types.rs:58` |
| `size_of::<BranchHeader>()` | 16 | `crates/expanse/src/node.rs:565` |
| `offset_of!(BranchHeader, version)` | 0 | `crates/expanse/src/node.rs:317` |
| `offset_of!(BranchHeader, digits)` | 8 | `crates/expanse/src/node.rs:566` |
| `size_of::<BranchL3>()` | 64 | `crates/expanse/src/node.rs:568` |
| `offset_of!(BranchL3, edges)` | 16 | `crates/expanse/src/node.rs:570` |
| `size_of::<BranchL7>()` | 128 | `crates/expanse/src/node.rs:572` |
| `offset_of!(BranchL7, edges)` | 16 | `crates/expanse/src/node.rs:574` |
| `size_of::<BranchB>()` | 128 | `crates/expanse/src/node.rs:577` |
| `offset_of!(BranchB, subarrays)` | 32 | `crates/expanse/src/node.rs:580` |
| `offset_of!(BranchB, pop_counts)` | 96 | `crates/expanse/src/node.rs:582` |
| `offset_of!(BranchB, version)` | 112 | `crates/expanse/src/node.rs:583` |
| `size_of::<BranchU>()` | 4160 | `crates/expanse/src/node.rs:585` |
| `offset_of!(BranchU, version)` | 0 | `crates/expanse/src/node.rs:462` |
| `size_of::<LeafBitmap1>()` | 64 | `crates/expanse/src/node.rs:588` |
| `offset_of!(LeafBitmap1, version)` | 32 | `crates/expanse/src/node.rs:495` |
| `size_of::<LeafBitmapL>()` | 128 | `crates/expanse/src/node.rs:589` |
| `offset_of!(LeafBitmapL, values)` | 32 | `crates/expanse/src/node.rs:590` |
| `offset_of!(LeafBitmapL, version)` | 96 | `crates/expanse/src/node.rs:529` |
| `size_of::<Bitmap256>()` | 32 | `crates/expanse/src/node.rs:576` |
| `size_of::<ValueSlot>()` | 8 | `crates/expanse/src/slot.rs:174` |
| `ValueSlot::TAG_MASK` | 0xFF | `crates/expanse/src/slot.rs:183` |
| `ValueSlot::ARENA_META_MASK` | 0xFFFFFF | `crates/expanse/src/slot.rs:185` |
| `ValueSlot::ARENA_META_MAX` | 16777215 | `crates/expanse/src/slot.rs:187` |
| `ARENA_ALIGN` | 16 | `crates/expanse/src/blobmap.rs:468` |
| `ARENA_META_CEILING` | 68719476736 | `crates/expanse/src/blobmap.rs:473` |
| `MAX_ARENA_CHUNKS` | 65536 | `crates/expanse/src/blobmap.rs:480` |
| `MAX_ARENA_CAPACITY` | 1073741824 | `crates/expanse/src/blobmap.rs:491` |
| `DEFAULT_CHUNK_SIZE` | 2097152 | `crates/expanse/src/blobmap.rs:463` |
| `size_of::<Edge32>()` | 8 | `crates/expanse/src/types32.rs:176` |
| `align_of::<Edge32>()` | 4 | `crates/expanse/src/types32.rs:177` |
| `MAX_LEVEL_32` | 4 | `crates/expanse/src/types32.rs:18` |
| `CACHE_LINE_32` | 32 | `crates/expanse/src/types32.rs:27` |
| `size_of::<BranchHeader32>()` | 8 | `crates/expanse/src/node32.rs:23` |
| `size_of::<BranchL2_32>()` | 32 | `crates/expanse/src/node32.rs:53` |
| `size_of::<BranchL6_32>()` | 64 | `crates/expanse/src/node32.rs:100` |
| `size_of::<BranchB32>()` | 96 | `crates/expanse/src/node32.rs:149` |
| `size_of::<BranchU32>()` | 2080 | `crates/expanse/src/node32.rs:187` |
| `size_of::<LeafBitmap1_32>()` | 64 | `crates/expanse/src/node32.rs:219` |
| `size_of::<LeafBitmapL_32>()` | 96 | `crates/expanse/src/node32.rs:311` |
| `size_of::<ValueSlot32>()` | 4 | `crates/expanse/src/slot32.rs:47` |
| `ValueSlot32::TAG_MASK` | 0xFF | `crates/expanse/src/slot32.rs:56` |
| `ValueSlot32::ARENA_OFFSET_MASK` | 0xFFF00 | `crates/expanse/src/slot32.rs:59` |
| `ValueSlot32::ARENA_OFFSET_SHIFT` | 8 | `crates/expanse/src/slot32.rs:61` |
| `ValueSlot32::ARENA_META_MASK` | 0xFFF00000 | `crates/expanse/src/slot32.rs:64` |
| `ValueSlot32::ARENA_META_SHIFT` | 20 | `crates/expanse/src/slot32.rs:66` |

<!-- /ENCODING-CONSTANTS -->

### 10.9 Corrections this section supersedes

Three claims that circulated in draft external writing, and what the source says:

| Claim | What the source says |
|---|---|
| "`Edge` supports 48-bit virtual addressing" | The opposite. Word 0 is the raw untruncated 64-bit pointer with zero upper-bit stealing (§10.1), which is what keeps it correct under 52-bit ARM64 LVA and 57-bit LA57. The compact 48-bit-style packing in §3.4 is a design note, not shipped. |
| "Immediates hold between 1 and 15 keys" | The budget is 15 **bytes**. 15 keys only at 1-byte remainders, 2 at 7-byte, and the map flavor is tighter still at `7 / key_bytes` because word 0 carries the value or the value-array pointer (§10.4). |
| "`ExpanseBlobMap` inlines payloads inside the edge pointers" | Inline payloads live in the leaf's 64-bit **value slot**, bits 63:8, with the low byte carrying the length tag (§10.5). |
