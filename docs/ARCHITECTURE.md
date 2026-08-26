# Expanse Architecture

> Canonical design doc. Compat contract: [COMPAT.md](COMPAT.md) · Testing: [TESTING.md](TESTING.md) · Benchmarks: [BENCHMARKING.md](BENCHMARKING.md) · Database Engines: [DATABASE.md](DATABASE.md) · 32-Bit & Embedded: [design/32-bit-embedded.md](design/32-bit-embedded.md) · Large Values: [design/large-values.md](design/large-values.md)

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
| Bit scan/rank | SWAR + lookup tables | `u64::count_ones`/`trailing_zeros` | Single-cycle on AArch64 (`cnt`/`rbit`). **On x86-64 `popcnt` is NOT in the base target**: without `-C target-cpu=x86-64-v2` (or `+popcnt`) these lower to a ~12-15 instruction SWAR sequence. No target-cpu is set today, so every benchmark and shipped artifact takes the software path — an open item, not a delivered advantage |
| Byte search | Unrolled scalar compares | SIMD splat-compare-movemask (SSE2/NEON via `core::arch`, portable fallback) | 16–64 bytes per compare, no branchy loop |
| Allocator | Custom word-bucket chunk allocator | Intrusive 4 KiB `SlabPage` freelist arena with 62 size classes and $O(1)$ static `RAW_CLASS_TABLE` lookup, cache-line aligned, byte-exact accounting | Sub-10ns allocation recycling with zero heap fragmentation, outperforming stock Judy on churn by >22% |
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

Tag encoding (implemented in `crates/expanse/src/types.rs`):

- Structural tags `0x00..=0x0C`, `0x7F`: null, 4 branch flavors, linear leaves for 1–7 remaining bytes, bitmap leaf, full expanse.
- Immediate tags, nibble-packed `(key_bytes << 4) | (count - 1)`, valid when `key_bytes * count <= 15`. Disjoint from structural tags by construction.

### 3.2 Branches

Linear branches share a 16-byte header:
```
offset 0: version     4 B (u32)    Phase 7 OCC version counter
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
- **LinearLeaf1..7**: header-less variable-length allocations — population lives in the parent edge's `pop0`, so a leaf is nothing but payload (as in the original). Set flavor: `[keys: L×pop]`; map flavor: `[values: u64×pop][keys: L×pop]` in one 64-aligned allocation (values first = free 8-alignment). Scan search baseline; the Phase 8 bench pass decides whether SIMD/binary search earns its complexity.
- **LeafBitmap1** (level 1, 64 B): 32-byte bitmask + OCC version (set flavor). **LeafBitmapL** (128 B): bitmask + 8 value-subarray pointers addressed by popcount rank + OCC version (map flavor).

### 3.4 Tagged pointers (read-optimized paths)

x86-64/AArch64/RISC-V 64-bit user VAs fit in 48 (or 57) bits; 8-byte alignment frees the low 3 bits. A compact 8-byte edge variant packs `[type:16][address:45][level:3]` for read-dominated structures and for caching branch metadata without extra line fills. Must stay behind an abstraction that also supports full 16-byte edges (LAM/TBI/Sv57 and 57-bit VA systems change the free-bit budget — feature-detected, never assumed).

## 4. Algorithms

- **Lookup** (Phases 4–5, done — `get::test_set`/`get::get_map`): iterative tag-dispatched descent; branch step = SWAR digit find in the header word (linear), bitmap test + subexpanse popcount rank (bitmap), direct index (uncompressed); terminal step = linear-leaf scan, bitmap-leaf test/rank, or immediate key scan, with narrow-pointer decode validation on leaf children. Zero allocation, zero locks. Level skips: leaves skip via decode bytes, branches via header-stored levels (see §6 step 3); immediates never skip (their key size *is* their level), full-expanse edges cover their whole current expanse, and `BranchU`/level-8 slots never skip.
- **Insert** (Phase 6, done — `mutate::insert` / `mutate_map::map_insert`): descend to the failing point, then grow along the least-compressed-form ladder: Immediate → LinearLeaf → (level 1) BitmapLeaf → FullExpanse / (level ≥2) cascade into BranchL3 → L7 → BranchB → BranchU. Multi-level descent uses monomorphic match arms with direct scalar comparisons and branchless `linear_insert_slot_l3` for BranchL3, 16-bit presence filtering for BranchL7, zero-loop `const fn new_immed_single_map`, compile-time tag-specialized `locate_fixed::<KB>` and `map_insert_at_fixed::<KB>` for linear leaves, and depth-guarded bypass paths (`InsertPath::clear`). Map flavor: immediates keep keys in the 7 aux bytes (value in word 0 for one key, value-array pointer for several, capacity `7 / key_bytes`); map leaves are `[values][keys]`; level-1 overflow goes to `LeafBitmapL` (no map full expanse — values must exist). Narrow-pointer creation: cascades place their branch (or bitmap leaf) at the keys' divergence level; divergence inside a skipped prefix splits at the highest diverging decode level (§6 step 3).
- **Delete** (Phase 6, done — `mutate::remove` / `mutate_map::map_remove`): inverse ladder with **1-index hysteresis** — every down-conversion runs one index below its up-convert twin (leaf→immed at `max_count − 1`, bitmap→leaf at 24, L7→L3 at 2, B→L7 at 6, U→B at 191), preventing thrash on alternating insert/delete at a boundary. Deleting from a full expanse first materializes one decompression step.
- **Count/rank** (compat: `Judy1Count`/`JudyLCount`/`ByCount`): O(depth) using edge `pop0` fields plus bitmap-branch cached segment counts.
- **Concurrent reads** (Phase 7, done — `occ` + `sync`): **per-node seqlock versions** in every branch header (Boehm fence construction; writers bracket a node's in-place mutation region including the recursion beneath it, only when the tree is concurrently shared), validated hand-over-hand by readers — each node sampled even before its fields are read, re-validated before anything read from it is dereferenced, terminal payloads covered by their parent's version; the tree-level version covers the root snapshot; epoch-deferred reclamation keeps every pinned pointer live; bounded retries fall back to the writer mutex. `SyncExpanseBlobMap` (issue #219 Phase 1) extends the protocol to variable-length payloads: the validated index walk yields the 64-bit `ValueSlot` (inline ≤ 7 B payloads decode by value from it), arena payloads resolve through an RCU-published immutable chunk table (readers never touch the arena's chunk vector), and dead chunks/superseded tables retire through the same epoch collector — an epoch-pinned read guard hands out zero-copy payload borrows that stay byte-stable across concurrent compaction.

## 5. Crate structure

- `crates/expanse` (package `expanse-trie`): core. Planned modules: `types` (done), `bits` (done: SIMD/SWAR byte find, `Bitmap256` rank/select/navigation), `node` (done: JP + branch/bitmap-leaf layouts, compile-time layout asserts; variable-length linear-leaf layout lands with the Phase 5 allocator), `get` (done: set/map lookup walk incl. linear leaves), `alloc` (done: cache-line-aligned allocation + byte accounting behind one handle), `leaf` (done: linear-leaf layout + search), `mutate` (done, set flavor: insert/remove ladder + hysteresis, subtree free, invariant validator), `set` (done: `ExpanseSet` with root-leaf → level-8 trie organization), `mutate_map` + `map` (done: map-flavor engine sharing the branch machinery; `ExpanseMap`), `nav` (done: flavor-generic ordered navigation — next/prev/first/last, O(depth) rank via pop0, 0-based select; public iterators and count_range/by_count on both types). `strmap` (done: `ExpanseStrMap`, a meta-trie of word-map nodes over big-endian 8-byte chunks — numeric order = byte-lexicographic order; backs the exported `JudySL*`). `occ` + `sync` (done: Phase 7 seqlock/EBR primitives and the `SyncExpanseSet`/`SyncExpanseMap`/`SyncExpanseBlobMap` concurrent wrappers), `bytesmap` (done: `ExpanseBytesMap`, the unordered byte-string map — a 64-bit-hash-keyed `ExpanseMap` over byte-exact collision buckets; backs the exported `JudyHS*`).
- `crates/expanse-capi` (`libexpanse`): `extern "C"` surface per [COMPAT.md](COMPAT.md) — legacy `Judy.h` compat plus the modern `expanse.h` API. Thin translation layer only — no logic beyond ABI marshaling and `JError_t` mapping.

## 6. Phase roadmap

| Phase | Deliverable | Gate to next |
|---|---|---|
| 1. Foundation types | Tags, constants, digit math (done) | Tests green |
| 2. Bit/vector engine | popcount/ctz/SIMD byte-find + portable fallbacks (done) | Unit tests incl. edge lanes; parity between SIMD and fallback |
| 3. Node layouts | 64 B/128 B structs, layout `const` asserts (done; linear-leaf layout deferred to Phase 5 alloc) | `size_of`/`align_of`/`offset_of` asserts green |
| 4. Lookup engine | `get`/`test` over hand-built trees (done) | Differential vs `BTreeMap` model on fixed corpora |
| 5. Allocation | Cache-line-aligned alloc + accounting; linear-leaf layout + lookup integration (done) | Miri-clean; leak checks |
| 6. Mutation engine | insert/delete cascades + hysteresis (done: `ExpanseSet` + `ExpanseMap`) | Property tests + invariant validator (TESTING.md) green |
| 7. OCC reads | Tree-level seqlock + EBR (done: `occ` + `sync` — `SyncExpanseSet`/`SyncExpanseMap`, single writer + lock-free validated readers; per-node versions reserved as the contention refinement) | Loom/stress suites green (done — CI `loom` job + thread stress) |
| 8. Hardening | capi surface, differential oracle vs C libjudy, fuzzing, benches | COMPAT.md acceptance gates; php-judy suite green against libexpanse |

### Sequencing after external architect review (2026-08-18)

An external review confirmed Phases 1–6b and identified five gaps (narrow-pointer synthesis, asymmetric root-leaf lifecycle, missing ordered navigation/rank APIs, `NodeAlloc` `Cell` counters, capi stub). Agreed order, with rationale:

1. **Ordered navigation + rank/count** — done: `nav` module + public APIs (`first`/`last`/`next_at_or_after`/`next_after`/`prev_at_or_before`/`prev_before`, `iter()`, `count_below`/`count_range`, 0-based `by_count`) on both `ExpanseSet` and `ExpanseMap`, differentially tested against the `BTree` models; map navigation returns values (the compat layer's `First`/`Next` hand out value pointers).
2. **Phase 8: capi exports + differential oracle + benches** — the COMPAT.md acceptance gates are the project's falsifiable criteria; the oracle also retro-tests everything above, and the bench harness produces the bytes/key evidence the next step needs.
3. **Narrow-pointer synthesis in mutation** — **done, both halves.** *Leaf-targeted* (measured: clustered bytes/key 1.34 → 0.35; vs-stock clustered get 5.31× slower → parity): cascades whose keys diverge only in the last byte build one skip-carrying bitmap leaf instead of a branch chain; shrink conversions absorb decode bytes back into slot-level immediates. *Branch-targeted* (via **header-stored levels**: `BranchHeader.level` / `BranchB.level`; `BranchU` has no header and never skips — a full skipping `BranchB` is first wrapped one level above its form): a leaf cascade places its branch at the keys' true divergence level with the shared prefix as decode bytes, and an insert diverging inside any skipped prefix splits at the **highest** diverging decode level (`split_skip`) instead of materializing one chain level per step. Level-8 slots never skip (the root edge cannot hold both pop0 and decode bytes). Measured: two 512-key clusters cost 192 structural bytes vs 960 under per-level chains (`branch_skip_clusters` tests); wide-cluster (4096-run) set flavor 0.19 → 0.12 B/key at 1M keys.
4. **Root-leaf shrink hysteresis** — trivially implementable once ordered iteration exists (collect ≤ 31 survivors), pointless before.
5. **Phase 7 OCC** — **done, including the per-node refinement.** `occ` (seqlock `SeqVersion` with Boehm's fence construction; epoch `Collector` with pin/advance SeqCst-fence pairing — both loom-model-checked, and loom found real ordering bugs in the first drafts of each) + `sync` (`SyncExpanseSet`/`SyncExpanseMap`: writers serialize on a mutex inside version brackets, readers run a validated walk — re-check before every dereference, bounded retries, mutex fallback — with frees routed through the collector; `NodeAlloc` accounting moved `Cell`→relaxed atomics). One tree-level version was the correctness landing; the measured collapse under writer churn (BENCHMARKING.md: single-reader reads fell to 3.4 K/s against a saturating writer) then justified the **per-node refinement**: `BranchU` gained a version header (4 KiB + one line), writers bracket every node's in-place mutation region — child-slot rewrites and the recursion beneath them — with that node's version (skipped entirely on single-threaded trees via `NodeAlloc::occ_enabled`), and readers validate hand-over-hand, node by node, falling back to the tree version only for the root snapshot. Measured: single-reader churn throughput ~700× (3.4 K/s → 2.4 M/s).

Performance targets (measured per BENCHMARKING.md before any claim): point lookup < 15 ns on random 64-bit keys (target); < 9.5 bytes/key on dense/clustered distributions (target). Comparative benchmarking against `RoaringBitmap`, `hashbrown::HashMap`, and `std::collections::BTreeMap`, as well as multithreaded concurrency scaling models (1..16 threads), are implemented in `benches/comparative.rs` and `benches/concurrency.rs`.

---

## 7. Database Engine Subsystems & Integration

Expanse is architecturally suited as a high-density, low-latency primitive across core database subsystems:

- **Inverted Indexes & Posting Lists (`ExpanseSet`)**: Tracks 64-bit document IDs at **0.07–0.36 bytes/docID** on clustered/dense sets (outperforming Roaring Bitmaps) with bitwise set algebra executed directly over compressed trie edges and $O(\text{depth})$ skip-scans (`next_at_or_after`).
- **MVCC Visibility Maps & Active Transaction Tracking (`SyncExpanseSet`)**: Provides lock-free reader validation over active transaction IDs (`xid`) with zero reader-writer locks, single-digit nanosecond point lookups, and epoch-based safe reclamation under continuous OLTP commit/vacuum churn.
- **Columnar String & Symbol Dictionaries (`ExpanseStrMap`)**: Maps high-cardinality strings to 32/64-bit symbol IDs using 8-byte big-endian cross-chunk path folding, preserving lexicographical sort order while eliminating redundant prefix storage (70%+ memory reduction on URLs and paths).
- **Secondary Indexes & MemTables (`ExpanseMap`)**: Serves as a rebalance-free LSM MemTable and secondary index engine with contiguous 64-byte SIMD leaf scans. Full ordered `iter()` is **faster than `BTreeMap::iter()` for dense key distributions** at 1M keys — sequential 0.7×, clustered 0.8×, random 0.5× (2× faster) the time of `BTreeMap::iter()`, after #245's stack-based zero-allocation iterator; **sparse-key iteration remains ~4.7× slower**, a structural residual tracked in [#270](https://github.com/orieg/expanse/issues/270) *(measured: reference host — Intel i9-12900F, 24 threads, commit 46529f19, `benches/compare.rs`)*.
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
Value Slot:64-bit (<=7B Inline, 32B Meta)                32-bit (<=3B Inline, 16B Meta)
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
- **`LeafBitmap1_32`**: 32B 256-bit bitmask + 4B pop0/header = **36 Bytes** with 32B alignment.

### 8.2 Polymorphic 32-Bit Value Slots (`ValueSlot32`)

- **Inline Mode ($\le 3\text{ B}$)**: Direct in-slot storage with zero heap allocation.
- **Arena Mode**: 16-bit hot metadata (TTL, flags) + 12-bit slab offset (up to 4096 entries per chunk).
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


