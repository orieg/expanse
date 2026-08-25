# Design: Large-Value / Blob-Arena Storage

**Status**: Partially Implemented — core shipped in #159 / #161  
**Author**: Expanse Core Team  
**Issue**: [#112](https://github.com/orieg/expanse/issues/112)  
**Target Milestone**: Expanse v0.3.0  
**Affected Crates**: `expanse-trie` (`crates/expanse`), `expanse-capi` (`crates/expanse-capi`)  
**Canonical Documentation**: `docs/design/large-values.md` (Design context for `docs/ARCHITECTURE.md` and `docs/COMPAT.md`)

---

## 1. Executive Summary

Expanse is a clean-room, cache-line-optimized reimplementation of Judy arrays designed for modern 64-bit microarchitectures. While Judy arrays excel at dense integer mapping with $O(1)$ to $O(\log_{256} N)$ traversal and minimal memory footprint, their traditional mapping domain is strictly scalar: 64-bit keys (`Key = u64`) to 64-bit words (`Word = u64`).

When storing arbitrary-sized payloads (strings, JSON documents, protocol buffers, vector embeddings, database records), naive pointer-based representations cause:
1. **Allocator Metadata Overhead**: 8–16 bytes of malloc/jemalloc chunk headers per payload (up to 100–200% overhead for small payloads).
2. **Virtual Address Space Fragmentation**: Pointers scattered across non-contiguous heap pages degrade TLB reach.
3. **Severe Cache Thrashing During Range Scans**: Evaluating simple filtering predicates (TTL expiration, soft-deletion tombstones, tenant partitioning) forces the CPU to chase pointers and load cold DRAM cache lines for every candidate entry.

This design introduces a unified, zero-copy, cache-conscious large-value architecture for Expanse across four complementary pillars:
1. **Polymorphic 64-bit Value Slots (`ValueSlot`)**: Packing $\le 7$-byte values directly inline with zero heap allocation, and packing 32-bit hot metadata (TTL, flags, tenant ID) alongside a 24-bit arena locator into a single 64-bit word.
2. **Hot/Cold Columnar Metadata-Predicate Range Filtering**: Executing predicate filters directly over contiguous leaf value arrays without dereferencing cold payload cache lines. The `$82\%$ DRAM-traffic reduction` / `$>15\times$ selective-scan` figures are **design targets gated on the wide-offset arena** — measured on the shipped 16 MiB-ceiling arena the columnar advantage is ~1.3–1.4× (the working set stays L3-resident, so payloads are never cache-cold); see §10.3.
3. **Chunked Slab/Arena Backing (`BlobArena` / `ExpanseBlobMap`)**: Append-only 2 MiB/16 MiB chunk allocation with generation counters, ABA safety, and incremental in-place compaction.
4. **Zero-Copy `mmap` / Shared-Memory IPC**: Base-relative offset encoding enabling cross-process multi-reader access with zero serialization overhead and zero memory duplication.
5. **C ABI Drop-In Compatibility**: Seamless coexistence with classic `JudyL` functions (`JudyLGet`, `JudyLIns`) returning `*mut Word`.

### Implementation status (as of commit 6c63826a)

| Pillar | Status | Notes |
|---|---|---|
| Polymorphic `ValueSlot` (inline ≤7B + `ArenaShort` locator) | **Shipped** | `crates/expanse/src/slot.rs`, `blobmap.rs`. Inline and `ArenaShort` tags are live. |
| Hot/cold columnar predicate filtering | **Shipped** | 32-bit hot metadata packed alongside the arena locator. |
| Chunked slab/arena backing (`BlobArena` / `ExpanseBlobMap`) | **Shipped, 16 MiB ceiling** | The live arena is capped at **16 MiB** by the 24-bit `ARENA_OFFSET_MASK` (`0x00FF_FFFF`); `alloc_blob` returns `ArenaError::OffsetOverflow` past that. Per-payload bound is `chunk_size − 8`, not 4 GiB. |
| `ArenaLong` (16-bit chunk id + 40-bit offset, >16 MiB) | **Reserved / not implemented** | Encoding + accessors (`new_arena_long`, `arena_long_loc`) exist with a round-trip test, but the blob map never produces or reads `ArenaLong` slots. `External = 0x12` is likewise reserved. |
| Zero-copy `mmap` / shared-memory IPC (base-relative offsets) | **Not implemented** | No `RelOffset` / `shm_open` / position-independent layout exists; `ExpanseBlobMap::load_from_file` is a full `std::fs::read` + index rebuild, not an mmap. See `docs/DATABASE.md` §6 (roadmap). |

Sections below that describe `ArenaLong` multi-chunk mode, ">16 MiB / petabyte-scale" arenas, "256 MiB at 16 B alignment" locators, or `mmap`/shared-memory zero-copy are **design targets**, not shipped behavior.

---

## 2. Problem Statement & Microarchitectural Motivation

### 2.1 The Small/Large Payload Dilemma in Word Maps

In standard JudyL / `ExpanseMap`, value storage is 64-bit scalar:
```rust
// Standard ExpanseMap
pub struct ExpanseMap { /* ... */ }
// Value slot is a raw u64 / Word
```

When an application associates variable-length values with 64-bit keys:
- **Small Payloads (1–7 bytes)**: Storing a 4-byte IPv4 address, a 6-byte MAC address, or a short string requires heap allocation (`Box<[u8]>` or `malloc`), incurring 16 bytes of allocator tracking, a 64-bit pointer slot in the leaf, and an extra pointer dereference.
- **Medium/Large Payloads (8 bytes – 16 MiB)**: Payloads allocated via standard allocators are scattered across the heap. During sequential range iteration, CPU hardware prefetchers cannot anticipate the scattered pointer targets.

```
Standard Pointer Model (Scattered Dereferencing):
Leaf Cache Line (64 B):  [ Ptr A | Ptr B | Ptr C | Ptr D | Ptr E | Ptr F | Ptr G | Ptr H ]
                               |       |       |
                               v       v       v
                          (Heap A)  (Heap B) (Heap C)  <-- 3 DRAM Cache Misses (200-240 cycles each)
```

### 2.2 The Columnar Filter Bottleneck

In production databases, search engines, and real-time caches, range queries frequently apply metadata predicates:
$$\text{Scan}(K_{\text{start}} \le k \le K_{\text{end}}) \quad \text{WHERE} \quad \text{expiry} > T_{\text{now}} \ \land \ (\text{flags} \ \& \ \text{FLAG\_DELETED}) == 0$$

Under the conventional architecture, even if $95\%$ of keys are expired or deleted, the CPU **must load the payload cache line** from main memory for every single candidate key solely to inspect the header metadata.

| Metric | Pointer-per-Blob Model | Hot/Cold Polymorphic Slot Model |
|---|---|---|
| **DRAM Access per Evaluated Key** | 1 Cache Line (64 B) cold fetch | 0 Cold Fetches (hits contiguous leaf line) |
| **DRAM Bus Traffic ($10^6$ keys, $\sigma=5\%$)** | $64.0\text{ MB}$ | $11.2\text{ MB}$ ($82.5\%$ reduction) |
| **SIMD Vectorization of Predicates** | Impossible (pointer chasing) | 8–16 predicates evaluated per vector instruction |
| **Hardware Prefetcher Efficiency** | Broken (random heap pointers) | Maximum (streaming L1/L2 spatial locality) |

---

## 3. Standardized Polymorphic 64-bit Value Slots

To resolve this bottleneck without widening the trie's 16-byte `Edge` or altering leaf structures, the 64-bit value slot itself is formatted as a polymorphic tagged union: `ValueSlot`.

```
========================================================================================
64-Bit Value Slot Bit Layouts
========================================================================================

1. Inline Mode (<= 7 bytes payload):
+-------------------------------------------------------------------+-------------------+
| Payload Byte 6 | Byte 5 | Byte 4 | Byte 3 | Byte 2 | Byte 1 | Byte 0 | Tag (0x00..=0x07) |
| [63:56]        | [55:48]| [47:40]| [39:32]| [31:24]| [23:16]| [15:8] | [7:0]             |
+-------------------------------------------------------------------+-------------------+
  - Tag encodes payload length (0 to 7 bytes).
  - Zero heap allocations, zero pointer dereferences.

2. Arena Mode (Hot Metadata + 24-bit Arena Offset):
+------------------------------------+------------------------------+-------------------+
| Hot Metadata (TTL / Flags / Tenant)| Arena Locator (24 bits)      | Tag (0x10)        |
| [63:32] (32 bits)                  | [31:8] (24 bits)             | [7:0]             |
+------------------------------------+------------------------------+-------------------+
  - 32-bit Hot Metadata: directly filterable without payload dereference.
  - 24-bit Arena Locator: 16 MiB direct byte offset. (The shipped locator is a raw byte offset; the "256 MiB at 16 B alignment" scheme is *not* implemented.)

3. Arena Long Mode (> 16 MiB address space / Multi-Chunk) — **RESERVED, NOT IMPLEMENTED**:
+-------------------------------------------------------------------+-------------------+
| Arena Chunk ID (16 bits) | Chunk Byte Offset (40 bits)            | Tag (0x11)        |
| [63:48]                  | [47:8]                                 | [7:0]             |
+-------------------------------------------------------------------+-------------------+
  - *(target)* Would support petabyte-scale memory-mapped files and persistent stores. The `ArenaLong` encoding and its accessors exist in `slot.rs`, but no blob-map code path produces or consumes them; the live ceiling is 16 MiB.

4. Raw Scalar / Unmanaged Word (Classic JudyL Compatibility):
+---------------------------------------------------------------------------------------+
| Uninterpreted 64-bit User Word / Raw Virtual Pointer (Tag bits arbitrary)             |
| [63:0]                                                                                |
+---------------------------------------------------------------------------------------+
```

### 3.1 Tag Discriminants

The least significant byte (`bits [7:0]`) serves as the discriminant tag:

```rust
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SlotTag {
    /// Inline payload of 0 bytes (empty value).
    Inline0 = 0x00,
    /// Inline payload of 1 byte (in bits [15:8]).
    Inline1 = 0x01,
    /// Inline payload of 2 bytes (in bits [23:8]).
    Inline2 = 0x02,
    /// Inline payload of 3 bytes (in bits [31:8]).
    Inline3 = 0x03,
    /// Inline payload of 4 bytes (in bits [39:8]).
    Inline4 = 0x04,
    /// Inline payload of 5 bytes (in bits [47:8]).
    Inline5 = 0x05,
    /// Inline payload of 6 bytes (in bits [55:8]).
    Inline6 = 0x06,
    /// Inline payload of 7 bytes (in bits [63:8]).
    Inline7 = 0x07,

    /// Backed by BlobArena: 32-bit hot metadata + 24-bit arena locator.
    ArenaShort = 0x10,
    /// Backed by Large/Multi-Chunk Arena: 16-bit chunk ID + 40-bit offset.
    ArenaLong = 0x11,
    /// Off-heap / External memory reference.
    External = 0x12,

    /// Soft-deleted tombstone marker.
    Tombstone = 0xFE,
    /// Raw uninterpreted 64-bit word (unmanaged).
    RawWord = 0xFF,
}
```

### 3.2 Rust Struct Definition and Bit-Twiddling

```rust
/// A 64-bit polymorphic value slot packed directly into an Expanse leaf node.
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct ValueSlot(pub u64);

impl ValueSlot {
    pub const TAG_MASK: u64 = 0xFF;
    pub const ARENA_OFFSET_MASK: u64 = 0x00FF_FFFF;
    pub const META_MASK: u64 = 0xFFFF_FFFF_0000_0000;

    /// Creates an inline value slot from a byte slice (len <= 7).
    #[inline(always)]
    pub fn new_inline(bytes: &[u8]) -> Option<Self> {
        let len = bytes.len();
        if len > 7 {
            return None;
        }
        let mut raw = len as u64; // Tag is Inline0..Inline7
        for (i, &b) in bytes.iter().enumerate() {
            raw |= (b as u64) << (8 * (i + 1));
        }
        Some(Self(raw))
    }

    /// Creates an arena-backed value slot with 32-bit hot metadata and 24-bit offset.
    #[inline(always)]
    pub fn new_arena_short(meta: u32, arena_offset: u32) -> Option<Self> {
        if arena_offset > 0x00FF_FFFF {
            return None;
        }
        let raw = (SlotTag::ArenaShort as u64)
            | ((arena_offset as u64) << 8)
            | ((meta as u64) << 32);
        Some(Self(raw))
    }

    /// Returns the slot tag.
    #[inline(always)]
    pub fn tag(self) -> SlotTag {
        match (self.0 & Self::TAG_MASK) as u8 {
            0x00 => SlotTag::Inline0,
            0x01 => SlotTag::Inline1,
            0x02 => SlotTag::Inline2,
            0x03 => SlotTag::Inline3,
            0x04 => SlotTag::Inline4,
            0x05 => SlotTag::Inline5,
            0x06 => SlotTag::Inline6,
            0x07 => SlotTag::Inline7,
            0x10 => SlotTag::ArenaShort,
            0x11 => SlotTag::ArenaLong,
            0x12 => SlotTag::External,
            0xFE => SlotTag::Tombstone,
            _ => SlotTag::RawWord,
        }
    }

    /// Extracts inline payload into a fixed 7-byte buffer.
    #[inline(always)]
    pub fn inline_payload(self) -> ([u8; 7], usize) {
        let len = (self.0 & Self::TAG_MASK) as usize;
        let mut buf = [0u8; 7];
        let val = self.0 >> 8;
        for i in 0..len.min(7) {
            buf[i] = ((val >> (8 * i)) & 0xFF) as u8;
        }
        (buf, len)
    }

    /// Extracts 32-bit hot metadata word.
    #[inline(always)]
    pub fn hot_meta(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Extracts 24-bit arena locator.
    #[inline(always)]
    pub fn arena_offset(self) -> u32 {
        ((self.0 >> 8) & Self::ARENA_OFFSET_MASK) as u32
    }

    /// Raw uninterpreted integer conversion.
    #[inline(always)]
    pub fn to_raw(self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}
```

---

## 4. C ABI Compatibility Strategy

A core invariant of Expanse is clean-room, 100% C ABI parity with classic `libjudy` (`JudyL*` family), alongside a modern typed C API (`expanse_*`).

```
                              +---------------------------------------+
                              |         C Application Layer           |
                              +-------------------+-------------------+
                                                  |
                     +----------------------------+----------------------------+
                     |                                                         |
                     v                                                         v
        +--------------------------+                             +--------------------------+
        | Legacy JudyL Interface   |                             | Modern Blob Interface    |
        | (Judy.h)                 |                             | (expanse.h)              |
        | - JudyLIns() -> *mut Word|                             | - expanse_blob_insert()  |
        | - JudyLGet() -> *mut Word|                             | - expanse_blob_get()     |
        | (Uninterpreted raw u64)  |                             | - expanse_blob_scan()    |
        +------------+-------------+                             +------------+-------------+
                     |                                                         |
                     |                 +-----------------------+               |
                     +---------------->| Core Expanse Trie     |<--------------+
                                       | (crates/expanse)      |
                                       +-----------+-----------+
                                                   |
                                                   v
                                       +-----------------------+
                                       | Slab/Blob Arena       |
                                       | (BlobArena)           |
                                       +-----------------------+
```

### 4.1 Classic `JudyL` Preservation

In classic libjudy, `JudyLIns` and `JudyLGet` return a raw pointer to a machine word (`PPWord_t` / `*mut Word`). Callers directly read and write:
```c
PWord_t PValue;
JLI(PValue, PJLArray, Index);
*PValue = 123456; // Direct memory store into leaf slot
```

**ABI Guarantee**:
- Classic `JudyL` functions in `libexpanse` will continue returning a direct, writable `*mut Word` pointing to the exact 64-bit slot in the leaf node.
- Raw values written by legacy C callers are stored verbatim.
- No discriminant tagging is enforced on legacy `JudyL` calls; `ValueSlot::from_raw(*slot)` defaults to `SlotTag::RawWord` if the tag does not match known tag ranges.

### 4.2 Modern `expanse_blob_*` C API

For C consumers requiring automatic inline packing, arena management, and predicate filtering:

```c
// include/expanse.h - Modern Large-Value C API

typedef struct ExpanseBlobMap ExpanseBlobMap;

typedef struct {
    const uint8_t* ptr;
    size_t len;
    uint32_t hot_meta;
    bool is_inline;
} ExpanseBlobView;

typedef bool (*expanse_predicate_fn)(uint64_t key, uint32_t hot_meta, void* user_ctx);
typedef bool (*expanse_scan_cb_fn)(uint64_t key, ExpanseBlobView view, void* user_ctx);

// Lifecycle
ExpanseBlobMap* expanse_blob_map_new(size_t chunk_size);
void            expanse_blob_map_free(ExpanseBlobMap* map);

// Mutation
bool expanse_blob_map_insert(
    ExpanseBlobMap* map,
    uint64_t key,
    const uint8_t* data,
    size_t len,
    uint32_t hot_meta
);

bool expanse_blob_map_remove(ExpanseBlobMap* map, uint64_t key);

// Point Retrieval
bool expanse_blob_map_get(
    const ExpanseBlobMap* map,
    uint64_t key,
    ExpanseBlobView* out_view
);

// High-Performance Filtered Range Scan
size_t expanse_blob_map_scan_filtered(
    const ExpanseBlobMap* map,
    uint64_t start_key,
    uint64_t end_key,
    expanse_predicate_fn predicate,
    expanse_scan_cb_fn callback,
    void* user_ctx
);
```

---

## 5. Hot/Cold Columnar Metadata-Predicate Range Filtering

### 5.1 Microarchitectural Spatial Locality in Expanse Leaves

In `ExpanseMap`, leaf nodes group value words contiguously:
- **`LinearLeaf` (Map Flavor)**: Allocated as `[values: [u64; pop]][keys: [u8; L * pop]]`.
- **`LeafBitmapL` (Level 1 Map Leaf)**: 8 sub-array pointers addressed by popcount rank, with values stored in dense 8-word blocks.

Because 8 value slots (each 8 bytes) fit inside a single **64-byte L1 cache line**, scanning 8 consecutive keys loads all 8 value slots in **a single DRAM burst**:

```
Single 64-Byte Cache Line in LinearLeaf:
+---------------------------------------------------------------------------------------+
| Slot 0 (8B) | Slot 1 (8B) | Slot 2 (8B) | Slot 3 (8B) | Slot 4 (8B) | Slot 5 (8B) | ... |
+---------------------------------------------------------------------------------------+
  ^             ^             ^             ^
  |             |             |             |
  Meta 0        Meta 1        Meta 2        Meta 3  (Evaluated simultaneously in registers)
```

### 5.2 The Predicate Filter Pipeline

During a range scan `scan_filtered(from..=to, predicate, callback)`:

```
                          Range Scan Step
                                |
                                v
               +----------------------------------+
               |  Fetch Next Contiguous Leaf Line | (1 Cache Line = 8 Slots)
               +----------------+-----------------+
                                |
                                v
               +----------------------------------+
               | Vectorized Predicate Evaluation  |
               | (Compare TTL / Flags in Regs)    |
               +----------------+-----------------+
                                |
                 +--------------+--------------+
                 |                             |
          [Predicate FALSE]             [Predicate TRUE]
                 |                             |
                 v                             v
       +--------------------+       +--------------------+
       | Skip Payload Fetch |       | Dereference Payload|
       | (0 DRAM Misses!)   |       | from Arena Chunk   |
       +--------------------+       +---------+----------+
                                               |
                                               v
                                    +--------------------+
                                    | Invoke User CB     |
                                    +--------------------+
```

### 5.3 SIMD/SWAR Vectorization Kernels

When evaluating range bounds on 32-bit timestamps:
$$\text{Predicate}(V) = (V_{\text{meta}} \ge T_{\text{min}}) \land (V_{\text{meta}} \le T_{\text{max}})$$

Using AVX2 / NEON vector intrinsics, 4–8 slots are unpacked and filtered in parallel:

```rust
// Conceptual SIMD Predicate Kernel (AVX2 / SSE4.2 / NEON)
#[inline(always)]
pub unsafe fn filter_leaf_slots_avx2(
    slots: &[u64; 4],
    min_meta: u32,
    max_meta: u32,
) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        use core::arch::x86_64::*;
        // Load 4 x 64-bit slots into 256-bit register
        // SAFETY: slots is 32-byte aligned in cache line
        let vslots = unsafe { _mm256_loadu_si256(slots.as_ptr().cast()) };
        // Shift right 32 bits to extract hot metadata
        let vmeta = unsafe { _mm256_srli_epi64(vslots, 32) };
        let vmin = unsafe { _mm256_set1_epi64x(min_meta as i64) };
        let vmax = unsafe { _mm256_set1_epi64x(max_meta as i64) };

        // Compare: vmin <= vmeta <= vmax
        let cmp_ge = unsafe { _mm256_cmpgt_epi64(vmeta, vmin) };
        let cmp_le = unsafe { _mm256_cmpgt_epi64(vmax, vmeta) };
        let match_mask = unsafe { _mm256_and_si256(cmp_ge, cmp_le) };

        // Extract bitmask of passing slots
        unsafe { _mm256_movemask_pd(_mm256_castsi256_pd(match_mask)) as u32 }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        // Portable SWAR fallback
        let mut mask = 0u32;
        for (i, &slot) in slots.iter().enumerate() {
            let meta = (slot >> 32) as u32;
            if meta >= min_meta && meta <= max_meta {
                mask |= 1 << i;
            }
        }
        mask
    }
}
```

### 5.4 Mathematical Modeling of DRAM Bandwidth and Latency Savings

Let:
- $N$ = Number of keys in scan range.
- $\sigma \in [0, 1]$ = Selectivity of predicate ($\sigma = \frac{\text{matching keys}}{N}$).
- $S_{\text{payload}}$ = Average payload size in bytes (e.g. 128 bytes = 2 cache lines).
- $B_{\text{line}}$ = Cache line size (64 bytes).
- $T_{\text{L1}}$ = L1 hit latency ($\approx 1.0\text{ ns}$ / 4 cycles).
- $T_{\text{DRAM}}$ = Cold DRAM cache miss latency ($\approx 60.0\text{ ns}$ / 240 cycles).

#### DRAM Traffic Model
Under the naive pointer-per-payload approach:
$$\text{DRAM}_{\text{naive}}(N, \sigma) = N \cdot \left( \frac{8}{B_{\text{line}}} \cdot B_{\text{line}} \right) + N \cdot \lceil S_{\text{payload}} / B_{\text{line}} \rceil \cdot B_{\text{line}}$$
Since each pointer points to an independent heap allocation, every key forces at least one cold 64-byte line fill:
$$\text{DRAM}_{\text{naive}} = N \cdot 8\text{ B (leaf)} + N \cdot 64\text{ B (payload)} = 72N\text{ bytes}$$

Under the Expanse Columnar Filter model:
$$\text{DRAM}_{\text{expanse}}(N, \sigma) = N \cdot 8\text{ B (leaf streaming)} + \sigma \cdot N \cdot 64\text{ B (matching payloads)}$$
$$\text{DRAM}_{\text{expanse}}(N, \sigma) = N \cdot (8 + 64\sigma)\text{ bytes}$$

#### Bandwidth Reduction Ratio ($\mathcal{R}_{\text{BW}}$)
$$\mathcal{R}_{\text{BW}}(\sigma) = \frac{\text{DRAM}_{\text{expanse}}}{\text{DRAM}_{\text{naive}}} = \frac{8 + 64\sigma}{72} = \frac{1 + 8\sigma}{9}$$

| Selectivity ($\sigma$) | Naive DRAM Traffic ($10^6$ keys) | Expanse DRAM Traffic ($10^6$ keys) | Traffic Reduction | Speedup Factor ($\frac{T_{\text{naive}}}{T_{\text{expanse}}}$) |
|---|---|---|---|---|
| **$0.1\%$** ($1,000$ matches) | $72.0\text{ MB}$ | $8.06\text{ MB}$ | **$88.8\%$** | **$46.2\times$** |
| **$1.0\%$** ($10,000$ matches) | $72.0\text{ MB}$ | $8.64\text{ MB}$ | **$88.0\%$** | **$31.8\times$** |
| **$5.0\%$** ($50,000$ matches) | $72.0\text{ MB}$ | $11.20\text{ MB}$ | **$84.4\%$** | **$13.4\times$** |
| **$10.0\%$** ($100,000$ matches) | $72.0\text{ MB}$ | $14.40\text{ MB}$ | **$80.0\%$** | **$7.8\times$** |
| **$50.0\%$** ($500,000$ matches) | $72.0\text{ MB}$ | $40.00\text{ MB}$ | **$44.4\%$** | **$1.8\times$** |

---

## 6. Slab / Arena Allocator (`BlobArena` & `ExpanseBlobMap`)

### 6.1 Chunked Arena Architecture

`BlobArena` allocates large, contiguous memory slabs as 16-byte-aligned system allocations (`std::alloc::alloc_zeroed` with an explicit `Layout` — *not* `mmap`; see `ArenaChunk::new` in `blobmap.rs`):

```
+------------------------------------------------------------------------------------+
| Chunk 0 (2 MiB Slab)                                                               |
| +-------------------+-------------------------+-------------------+--------------+ |
| | Header (32 B)     | Blob 0 (Len + Data)     | Blob 1 (Len+Data) | ... Bump Ptr | |
| +-------------------+-------------------------+-------------------+--------------+ |
+------------------------------------------------------------------------------------+
| Chunk 1 (2 MiB Slab)                                                               |
| +-------------------+-------------------------+-------------------+--------------+ |
| | Header (32 B)     | Blob N                  | Free Space ...    |              | |
| +-------------------+-------------------------+-------------------+--------------+ |
+------------------------------------------------------------------------------------+
```

### 6.2 Blob Record Layout & Generation Counters

Every arena payload is prefixed with an 8-byte packed header:

```rust
#[repr(C, packed)]
pub struct BlobRecordHeader {
    /// Payload length in bytes. The field is a `u32`, but the real limit is the
    /// per-payload bound `chunk_size − 8` under the live 16 MiB arena ceiling —
    /// not 4 GiB.
    pub len: u32,
    /// Generation counter for ABA protection and compaction validation.
    pub generation: u32,
}
```

- **Bump Allocation**: Sub-nanosecond allocation cost (single atomic fetch-add or thread-local bump pointer).
- **Zero Allocator Metadata**: No malloc tracking headers; overhead is strictly the 8-byte `BlobRecordHeader`.
- **16-Byte Alignment**: Enables vector load/store instructions on payload buffers.

### 6.3 Garbage Collection & Compaction Algorithm

Because blobs are append-allocated, updates and deletions generate dead space in older chunks.

```
Compaction Trigger Condition:
                  Total Allocated Bytes - Active Live Bytes
Fragmentation =  -------------------------------------------  > 35%
                            Total Allocated Bytes
```

#### In-Place Compaction Walk:
1. Allocate a fresh, contiguous consolidation chunk.
2. Traverse the `ExpanseMap` index sequentially via `iter_slots_mut()`.
3. For each `ValueSlot` with tag `SlotTag::ArenaShort` or `SlotTag::ArenaLong`:
   - Read active payload from old chunk.
   - Copy payload into new chunk at `new_offset`.
   - Increment record generation: `header.generation += 1`.
   - Update `ValueSlot` in the trie leaf in-place: `slot.set_arena_offset(new_offset)`.
4. Release/unmap old fragmented chunks back to the OS via `munmap` / `madvise(MADV_DONTNEED)`.

```rust
pub struct BlobArena {
    chunks: Vec<ArenaChunk>,
    active_chunk: usize,
    chunk_size: usize,
    total_allocated: usize,
    live_bytes: usize,
}

pub struct ArenaChunk {
    ptr: NonNull<u8>,
    capacity: usize,
    cursor: usize,
    generation: u32,
}
```

---

## 7. Zero-Copy `mmap` & Shared-Memory IPC

### 7.1 Relocatable Base-Relative Offset Architecture

A fatal limitation of absolute 64-bit pointers is that a memory region mapped via `mmap` across multiple processes may be placed at different virtual addresses due to Address Space Layout Randomization (ASLR).

Expanse solves this through **Base-Relative Addressing**:
- All pointers between trie nodes, leaves, and arena chunks are stored as **relative offsets** ($u32$ or $u48$) from the base address of the mapped region ($P_{\text{base}}$).
- Physical address resolution:
  $$P_{\text{target}} = P_{\text{base}} + \text{offset}$$

```
========================================================================================
Shared-Memory / File Image Binary Format
========================================================================================

+--------------------------------------------------------------------------------------+
| File Header (64 Bytes)                                                               |
| - Magic: "EXPANSE\0" (8B)       - Format Version: u32 (4B)   - Flags: u32 (4B)       |
| - Root Edge Offset: u64 (8B)    - Total File Size: u64 (8B)  - Entry Count: u64 (8B) |
| - Chunk Table Offset: u64 (8B)  - Checksum / Blake3: [u8; 16] (16B)                  |
+--------------------------------------------------------------------------------------+
| Trie Index Segment                                                                   |
| - Level-8 / Intermediate Branch Nodes                                                |
| - Linear / Bitmap Leaves with packed ValueSlots                                      |
+--------------------------------------------------------------------------------------+
| Blob Arena Chunk Table & Payload Slabs                                               |
| - Chunk 0: [BlobHeader | Payload] [BlobHeader | Payload] ...                         |
| - Chunk 1: [BlobHeader | Payload] ...                                                |
+--------------------------------------------------------------------------------------+
```

### 7.2 Multi-Process Shared-Memory Reader Flow

```
   [ Primary Writer Process ]                         [ Read-Only Worker Process ]
                |                                                  |
                v                                                  v
     shm_open("expanse_shm")                            shm_open("expanse_shm")
     ftruncate(...)                                     mmap(PROT_READ)
     mmap(PROT_READ | PROT_WRITE)                                  |
                |                                                  v
                v                                        Zero-Copy ExpanseBlobMap
       Expanse Mutex / Writes                            Read Point & Range Queries
       OCC Version Updates ----------------------------> Lock-Free Optimistic Walk
```

- **Zero Serialization**: Workers read structured data directly from shared memory without JSON, Protobuf, or Cap'n Proto decoding.
- **Zero RAM Duplication**: 100 worker processes share a single 32 GB cache in RAM.

---

## 8. Complete Reference Types & API Signatures

```rust
// crates/expanse/src/blobmap.rs

use crate::map::ExpanseMap;
use crate::types::Key;
use core::ptr::NonNull;

/// A typed view of a retrieved value payload.
pub enum BlobView<'a> {
    /// Inlined value (<= 7 bytes) borrowing from internal stack buffer.
    Inline(&'a [u8]),
    /// Arena-allocated value borrowing directly from arena slab.
    Arena(&'a [u8]),
}

impl<'a> BlobView<'a> {
    #[inline(always)]
    pub fn as_bytes(&self) -> &'a [u8] {
        match self {
            BlobView::Inline(slice) => slice,
            BlobView::Arena(slice) => slice,
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// High-level map from 64-bit keys to arbitrary-length byte blobs.
pub struct ExpanseBlobMap {
    index: ExpanseMap,
    arena: BlobArena,
}

impl ExpanseBlobMap {
    /// Creates a new blob map with default 2 MiB arena slabs.
    pub fn new() -> Self {
        Self::with_chunk_size(2 * 1024 * 1024)
    }

    /// Creates a new blob map with custom chunk size.
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            index: ExpanseMap::new(),
            arena: BlobArena::new(chunk_size),
        }
    }

    /// Inserts a key-blob pair with 32-bit hot metadata.
    pub fn insert(&mut self, key: Key, data: &[u8], hot_meta: u32) -> Result<(), ArenaError> {
        if data.len() <= 7 {
            let slot = ValueSlot::new_inline(data)
                .expect("len <= 7 validated");
            self.index.insert(key, slot.to_raw());
            Ok(())
        } else {
            let offset = self.arena.alloc_blob(data)?;
            let slot = ValueSlot::new_arena_short(hot_meta, offset)
                .ok_or(ArenaError::OffsetOverflow)?;
            self.index.insert(key, slot.to_raw());
            Ok(())
        }
    }

    /// Point lookup returning a zero-copy BlobView.
    pub fn get<'a>(&'a self, key: Key) -> Option<(BlobView<'a>, u32)> {
        let raw_slot = self.index.get(key)?;
        let slot = ValueSlot::from_raw(raw_slot);
        match slot.tag() {
            SlotTag::Inline0 | SlotTag::Inline1 | SlotTag::Inline2 |
            SlotTag::Inline3 | SlotTag::Inline4 | SlotTag::Inline5 |
            SlotTag::Inline6 | SlotTag::Inline7 => {
                let (buf, len) = slot.inline_payload();
                // Inlined view
                Some((BlobView::Inline(&buf[..len]), 0))
            }
            SlotTag::ArenaShort => {
                let offset = slot.arena_offset();
                let meta = slot.hot_meta();
                let slice = self.arena.get_blob_slice(offset)?;
                Some((BlobView::Arena(slice), meta))
            }
            _ => None,
        }
    }

    /// Executes a range scan with a predicate evaluated against hot metadata
    /// before dereferencing cold payload cache lines.
    pub fn scan_filtered<P, F>(
        &self,
        range: core::ops::RangeInclusive<Key>,
        mut predicate: P,
        mut callback: F,
    ) where
        P: FnMut(Key, u32) -> bool,
        F: FnMut(Key, BlobView<'_>, u32) -> bool,
    {
        for (key, raw_slot) in self.index.range(range) {
            let slot = ValueSlot::from_raw(raw_slot);
            let meta = slot.hot_meta();
            // Evaluate predicate directly from leaf value slot
            if predicate(key, meta) {
                if let Some((view, _)) = self.get(key) {
                    if !callback(key, view, meta) {
                        break;
                    }
                }
            }
        }
    }

    /// Runs in-place garbage collection and compaction.
    pub fn compact(&mut self) -> Result<CompactionStats, ArenaError> {
        self.arena.compact_with_index(&mut self.index)
    }
}
```

---

## 9. Phased Implementation Roadmap & Acceptance Gates

Per Expanse development rules, development proceeds in strict sequential phases with measurable acceptance gates (no time estimates):

```
+---------------------------------------------------------------------------------------+
| PHASE A: Core Polymorphic Value Slots & Bit Packing                                   |
| - Implement `ValueSlot`, `SlotTag`, `HotMeta` in `crates/expanse/src/slot.rs`         |
| - Unit tests for 100% roundtrip bit fidelity across all tag variants                  |
| - Gate: `cargo test` and `cargo miri test` 100% green                                 |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
| PHASE B: Hot/Cold Columnar Predicate Filtered Iteration                               |
| - Implement `scan_prefix_filtered` and `range_filtered` in `nav.rs`                   |
| - Implement SIMD predicate vectorization kernels (AVX2 / NEON / SWAR fallback)        |
| - Gate: Measured DRAM bandwidth reduction >= 75% on selective synthetic sweep         |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
| PHASE C: Chunked Slab/Arena Allocator (`BlobArena` & `ExpanseBlobMap`)                |
| - Implement `BlobArena` chunk manager and `ExpanseBlobMap` typed container            |
| - Implement generation counters and in-place GC compaction algorithm                  |
| - Gate: Zero memory leaks under Miri; churn compaction recovers >= 95% dead space     |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
| PHASE D: Zero-Copy `mmap` & Shared-Memory IPC Engine                                  |
| - Implement relocatable relative offset encoding & file header binary format          |
| - Multi-process integration tests (reader/writer worker model via `shm_open`)         |
| - Gate: Multi-process stress test passes with zero IPC serialization overhead         |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
| PHASE E: C ABI Extensions & Drop-In Verification                                      |
| - Export `expanse_blob_*` symbols in `crates/expanse-capi` & `include/expanse.h`      |
| - Differential oracle validation vs stock JudyL                                       |
| - Gate: All 13 CI status checks green; zero instruction regression on Callgrind       |
+---------------------------------------------------------------------------------------+
```

---

## 10. Verification, Testing & Benchmarking Matrix

### 10.1 Invariant Validation Rules
1. **Bit Packing Soundness**: For every byte sequence $B$ with $|B| \le 7$, `ValueSlot::new_inline(B).unwrap().inline_payload() == B`.
2. **Tag Non-Collision**: Inline tags `0x00..=0x07` are strictly disjoint from `ArenaShort (0x10)` and `RawWord (0xFF)`.
3. **C ABI Non-Interference**: Uninterpreted writes via `*JudyLIns(...) = val` preserve raw 64-bit patterns unmodified.
4. **OCC Reader Safety**: Readers traversing `BlobArena` during compaction never read uninitialized memory; generational checks detect stale offsets.

### 10.2 Benchmark Suite Additions (`benches/large_values.rs`)
- `bench_inline_vs_heap_small_blobs`: Measure throughput (ops/sec) and allocations for 1–7 byte keys. Target: $0$ heap allocations, $>3\times$ insert throughput vs `BTreeMap<u64, Vec<u8>>`.
- `bench_predicate_scan_selectivity_sweep`: Measure scan latency across $\sigma \in \{0.001, 0.01, 0.05, 0.20, 1.0\}$. Target: $>10\times$ speedup at $\sigma \le 0.05$.
- `bench_predicate_scan_cold_dram_sweep`: the falsifiable variant of the above with a payload-*touching* baseline (both arms share the `scan_filtered` traversal, so the only difference is payload cache-line loads). Measures the speedup the columnar pushdown actually yields; see §10.3 for why the arena ceiling keeps it warm.
- `bench_arena_compaction_churn`: Measure pause times and memory reclamation under heavy overwrite workloads.

### 10.3 Measured status *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit 695b98d; `benches/large_values.rs`, criterion medians)*

- **`inline_vs_heap_small_blobs`** — `ExpanseBlobMap` (inline value slots) vs `BTreeMap<u64, Vec<u8>>` (heap), 7-byte payload: **insert 199.5 µs vs 587.1 µs (2.9× faster)**, **get 56.8 µs vs 290.0 µs (5.1× faster)** (0-byte payload: 2.3× / 5.1×). Insert is just under the RFC's `>3×` target; get comfortably clears it. Zero-heap-allocation for ≤7-byte payloads is a structural property (inline slots), separately asserted by unit tests.
- **`predicate_scan_selectivity_sweep`** (original, mismatched baseline) — target `>10× at σ≤0.05` is **not demonstrated by this bench**; the `columnar_filtered_scan` (`scan_filtered`) arm is **~2× slower** than the `naive_unfiltered_deref` arm at every selectivity (e.g. σ=0.001: 741 µs vs 367 µs). The baseline is the problem: it uses a raw `get()` loop (different, cheaper traversal than `scan_filtered`) and reads only the payload *length* on a match, so it never touches payload bytes — both arms avoid cold-payload loads and the pushdown has nothing to win, while the traversal-cost mismatch makes the columnar arm look slower.

- **`predicate_scan_cold_dram_sweep`** (falsifiable variant, corrected baseline) — *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, commit `43b46f38`; criterion medians)*. Both arms share the identical `scan_filtered` traversal (so the trie-walk cost cancels) and the naive arm touches **every** payload byte while the columnar arm touches only matches. With that fix the pushdown **is** faster, not slower — **but it does not reach `>10×`, and cannot on the current engine:**

  | σ | columnar `scan_filtered` | naive full-deref | speedup |
  |---|---:|---:|---:|
  | 0.001 | 228.0 µs | 323.5 µs | **1.42×** |
  | 0.01 | 230.7 µs | 325.5 µs | 1.41× |
  | 0.05 | 244.7 µs | 323.8 µs | **1.32×** |
  | 0.20 | 293.1 µs | 323.2 µs | 1.10× |
  | 1.0 | 322.9 µs | 323.3 µs | 1.00× |

  **Why it is capped at ~1.4×, and why the premise is unreachable here.** The RFC's `>10× / 82%-DRAM-traffic` argument requires the skipped payloads to be **cache-cold** — a real DRAM fetch (~80 ns), so avoiding it dwarfs the scan bookkeeping. That needs the arena working set to exceed the host LLC. But the 64-bit `ExpanseBlobMap` arena is **hard-capped at 16 MiB** by the 24-bit `ArenaShort` value-slot offset (`blobmap.rs` "Capacity limits"; the wider `ArenaLong`/`External` encodings that would lift it are **unimplemented**). 16 MiB < a typical server LLC (the reference host's L3 is **30 MiB**), so the whole arena is forced L3-resident: a skipped payload saves an ~L3 hit (~7 ns here, measured), not a DRAM fetch. The columnar arm is then bounded by the traversal floor — full-scan cost 323 µs ÷ traversal-only floor 228 µs ≈ **1.42× maximum**, reached as σ→0. This is a derivable ceiling, not a tuning gap.

  **Verdict (RFC §10.3 revised, target → measured):** the `>10× at σ≤0.05` speedup is **not achievable on the current `ExpanseBlobMap`** because its 16 MiB arena ceiling keeps the working set inside the LLC, so the cold-DRAM regime the claim depends on never occurs. With a correct payload-touching baseline the measured columnar advantage on a warm (L3-resident) arena is **~1.3–1.4× at σ ≤ 0.05**, traversal-floor bounded. Reaching the `>10×` / `82% traffic reduction` figures is **gated on implementing the wide-offset (`ArenaLong`/`External`) arena** so the payload store can exceed the LLC, *and* on an ordered-scan fast path to lower the traversal floor. Until then the `>10×` / `>15×` / `82%` numbers in this RFC's §Overview remain **targets, not measured results**, and are explicitly gated on that arena work.
- **`arena_compaction_churn`** — compaction over 20k entries: ~475 µs after a 50%-delete churn, ~200 µs after 80%-delete (informational; no target).

---

## 11. References & Prior Art

1. **Baskins, D.** (2002). *Judy IV Shop Manual*. HP Laboratories.
2. **Silverstein, A.** (2002). *Judy Arrays: Fast and Space-Efficient Trie Variants*.
3. **Boehm, H.-J.** (2012). *Can Seqlocks Get Along With Programming Language Memory Models?* ACM SIGPLAN.
4. **Boncz, P. A., Manegold, S., & Kersten, M. L.** (1999). *Database Architecture Optimized for the Operating System: The Design and Implementation of MonetDB*.
5. **Lemire, D., & Boytsov, L.** (2015). *Decoding billions of integers per second through vectorization*. Software: Practice and Experience.
