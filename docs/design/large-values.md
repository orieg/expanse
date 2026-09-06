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
2. **Hot/Cold Columnar Metadata-Predicate Range Filtering**: Executing predicate filters directly over contiguous leaf value arrays without dereferencing cold payload cache lines. The `$82\%$ DRAM-traffic reduction` / `$\gt 15\times$ selective-scan` figures are **design targets gated on the wide-offset arena** — measured on the shipped 16 MiB-ceiling arena the columnar advantage is ~1.3–1.4× (the working set stays L3-resident, so payloads are never cache-cold); see §10.3.
3. **Chunked Slab/Arena Backing (`BlobArena` / `ExpanseBlobMap`)**: Append-only 2 MiB/16 MiB chunk allocation with generation counters, ABA safety, and incremental in-place compaction.
4. **Zero-Copy `mmap` / Shared-Memory IPC**: Base-relative offset encoding enabling cross-process multi-reader access with zero serialization overhead and zero memory duplication.
5. **C ABI Drop-In Compatibility**: Seamless coexistence with classic `JudyL` functions (`JudyLGet`, `JudyLIns`) returning `*mut Word`.

### Implementation status (as of commit 6c63826a)

| Pillar | Status | Notes |
|---|---|---|
| Polymorphic `ValueSlot` (inline ≤7B + `ArenaMeta` locator) | **Shipped** | `crates/expanse/src/slot.rs`, `blobmap.rs`. Inline and `ArenaMeta` tags are live. |
| Hot/cold columnar predicate filtering | **Shipped** | 24-bit hot metadata packed alongside the 32-bit arena locator into `ArenaMeta` (`[hot_meta (24) \| locator (32) \| tag (8)]`). Predicates evaluate in-slot without fetching cold DRAM payload cache lines. |
| Chunked slab/arena backing (`BlobArena` / `ExpanseBlobMap`) | **Shipped** | `BlobArena` manages 16-byte-aligned records addressable up to 64 GiB via 32-bit locator. Per-payload bound is `chunk_size − 8`. |
| Zero-copy `mmap` / shared-memory IPC (base-relative offsets) | **Not implemented** | No `RelOffset` / `shm_open` / position-independent layout exists; `ExpanseBlobMap::load_from_file` is a full `std::fs::read` + index rebuild, not an mmap. See `docs/DATABASE.md` §6 (roadmap). |

The sole arena encoding (`ArenaMeta`, tag `0x10`) provides 24-bit metadata and 32-bit locator addressing up to 64 GiB (bounded by the 1 GiB `MAX_ARENA_CAPACITY` safety cap). Sections below that describe "petabyte-scale" arenas or `mmap`/shared-memory zero-copy remain **design targets**, not shipped behavior.

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
$$\text{Scan}(K_{\text{start}} \le k \le K_{\text{end}}) \quad \text{WHERE} \quad \text{expiry} \gt T_{\text{now}} \ \land \ (\text{flags} \mathbin{\&} \text{FLAG\_DELETED}) = 0$$

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

2. Arena Mode (Hot Metadata + 32-bit Arena Locator) — **SHIPPED (`ArenaMeta`)**:
+------------------------------------+------------------------------------------+-------------------+
| Hot Metadata (TTL / Flags / Tenant)| Arena Locator (32 bits, 16-byte units)   | Tag (0x10)        |
| [63:40] (24 bits)                  | [39:8] (32 bits)                         | [7:0]             |
+------------------------------------+------------------------------------------+-------------------+
  - 24-bit Hot Metadata: directly filterable without payload dereference (`ARENA_META_MAX = 0x00FF_FFFF`).
  - 32-bit Arena Locator: flat global address in 16-byte units addressing up to 64 GiB (`ARENA_ALIGN = 16`).
  - Sole arena encoding for `ExpanseBlobMap` (`CompactInSlot` layout, #282/#285, #287).

3. Raw Scalar / Unmanaged Word (Classic JudyL Compatibility):
+---------------------------------------------------------------------------------------------------+
| Uninterpreted 64-bit User Word / Raw Virtual Pointer (Tag bits arbitrary)                         |
| [63:0]                                                                                            |
+---------------------------------------------------------------------------------------------------+
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

    /// Backed by BlobArena: 24-bit hot metadata + 32-bit arena locator (16-byte units).
    ArenaMeta = 0x10,
    /// Off-heap / External memory reference (reserved).
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
    pub const ARENA_META_MASK: u64 = 0x00FF_FFFF;
    pub const ARENA_META_MAX: u32 = 0x00FF_FFFF;

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

    /// Creates an arena-backed value slot with 24-bit hot metadata and 32-bit locator.
    #[inline(always)]
    pub fn new_arena_meta(meta: u32, locator: u32) -> Option<Self> {
        if meta > Self::ARENA_META_MAX {
            return None;
        }
        let raw = (SlotTag::ArenaMeta as u64)
            | ((locator as u64) << 8)
            | ((meta as u64) << 40);
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
            0x10 => SlotTag::ArenaMeta,
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

    /// Extracts 24-bit hot metadata word (bits [63:40]).
    #[inline(always)]
    pub fn arena_meta_meta(self) -> u32 {
        ((self.0 >> 40) & Self::ARENA_META_MASK) as u32
    }

    /// Extracts 32-bit arena locator (bits [39:8]).
    #[inline(always)]
    pub fn arena_meta_locator(self) -> u32 {
        (self.0 >> 8) as u32
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

> **Model scope.** The table above is a *pure DRAM-traffic* model: it assumes the only per-entry cost is bytes moved, so its speedups (up to $46\times$ at $\sigma=0.1\%$) are an **upper bound that ignores index traversal**. Measured reality (§10.3): against an honest post-#355 control a correct in-slot filter that walks the trie per entry lands at **~10.7× at σ=0.001 / ~6.37× at σ=0.05** cold — below the idealized $46\times$ (the trie walk is not free) and *clearing the RFC ≥10× target only at very low σ (~σ=0.001), not at σ=0.05*. (The pre-#355 ~22× / ~10.3× figures — which appeared to clear ≥10× at σ≤0.05 — were measured against a naive baseline that itself carried the #355 redundant re-descent; §10.3. An earlier draft predicted ~4–5×; that under-counted the payload/traversal ratio in the cold regime.)

### 5.5 Configurable Metadata Layout — `CompactInSlot` (default) & `BlobLeafVector` — *proposed (#282)*

**Status:** Phase 1 (`CompactInSlot` / `ArenaMeta`) **implemented (#287) and meets the `>10×` target at σ≤0.01** (10.7× at σ=0.001; 6.37× at σ=0.05 against the honest control — §10.3, #355); the earlier ~10.3× / ~22× figures were measured against a control that carried the #355 re-descent. Phase 2 (`BlobLeafVector`) is **not needed** (go/no-go = NO-GO, §5.5.10) — retained below as a deferred, documented option only. Tracked in #282 / #285.

The `meta = 0` degeneration measured in §10.3 (any `hot_meta` predicate matches every `ArenaLong` record, since the wide locator leaves no metadata bits) and the traversal-floor ceiling above are two facets of one problem: metadata is stored **interleaved inside each 64-bit `ValueSlot`** (array-of-structs), so it is neither always present (wide locators evict it) nor densely scannable (it is strided across the value region).

There is no single layout that is best across all blob workloads, so the design exposes **two, selected per map** — both of which fix the `meta = 0` correctness bug:

- **`CompactInSlot` (default)** — a new quantized `ValueSlot` encoding that carries 24-bit metadata *and* a 32-bit locator in the existing 64-bit slot (§5.5.7). It reuses the current leaf machinery unchanged (no new leaf type), fixes the filter, and — measured against an honest post-#355 control — delivers **~10.7× at σ=0.001 / ~6.37× at σ=0.05** cold (§10.3), clearing the RFC `>10×` target at very low σ but ~6.4× at σ=0.05 (not met at σ=0.05; the pre-#355 ~10.3× / ~22× carried the #355 re-descent). This is the shipped default and covers point-lookup/write-heavy, sparse, embedded, ≤24-bit-metadata, *and* the cold-DRAM analytical scan workloads.
- **`BlobLeafVector` (opt-in, `BlobLeaf32`)** — a struct-of-arrays leaf that **transposes the value region into columns** (§5.5.1–§5.5.6), keeping full 32-bit metadata densely SIMD-scannable. **Not built (NO-GO, §5.5.10):** it was premised on `CompactInSlot` being traversal-floor bound at ~4–5×, which measurement disproved. Retained only as a deferred option for a hypothetical high-σ workload or `>`64 GiB / full-32-bit-metadata need.

§5.5.1–§5.5.6 specify the **`BlobLeafVector`** layout; §5.5.7 specifies **`CompactInSlot`**; §5.5.8–§5.5.9 cover selection, ABI, and the cost of supporting both; §5.5.10 the phasing.

> **Alignment with the codified Judy anti-patterns (`AGENTS.md` §2.2, #284) — decisive for the design.** §2.2 mandates that auxiliary/columnar metadata live in **decoupled columnar sidecars or chunk headers**, and forbids both **multi-word leaf arrays** ("Fat Slots" — the 4-vs-8 slots/line density hit) and **complecting columnar attributes into the core trie index words**. Measured against that:
> - **`CompactInSlot` (shipped, #287) is fully compliant** — a single-word `ValueSlot` carrying one lightweight scalar field, no sidecar, no fat slot.
> - **A decoupled metadata sidecar (#295) is the *prescribed* pattern** for the wide-metadata / multi-column / >64 GiB capacity axes — exactly "decoupled columnar sidecars or chunk headers."
> - **`BlobLeafVector` trips both anti-patterns.** Its per-entry `[u32 meta][u64 loc]` columns are a **multi-word leaf array**, *and* they co-locate columnar attributes **inside the leaf** (core trie index) rather than a decoupled sidecar.
>
> So although §5.5.1–§5.5.6 below present `BlobLeafVector`'s in-leaf key-ordering as an advantage over an arena-chunk sidecar, on Judy-architecture grounds that framing is **backwards**: the sidecar is the faithful option and `BlobLeafVector` is the impedance mismatch. This is an **architectural NO-GO** on top of the measured performance NO-GO (§5.5.10); the capacity work is therefore directed at the sidecar (#295), and the `BlobLeafVector` design below is retained only as a documented record of a rejected approach.

#### 5.5.1 Struct-of-arrays leaf layout

A current map leaf is `[values: u64 × pop][keys: L × pop]` (see §*Linear leaves*), with each `u64` an interleaved `ValueSlot` `[hot_meta:32 | locator:24/56 | tag:8]`. `BlobLeaf32` (instantiated only by `ExpanseBlobMap`) splits the value region into two columns:

```
scalar blob leaf:  [ (meta|loc|tag) × pop ]              [ keys: L × pop ]
                     ^ metadata strided across pop u64 words

BlobLeaf32:        [ meta: u32 × pop ] [ loc: u64 × pop ] [ keys: L × pop ]
                     ^ contiguous, cache-line aligned      ^ sorted, in-leaf
                       SIMD-scannable (8×u32/AVX2, 4×u32/NEON)
```

The name denotes the **32-bit metadata column**, not an entry count; `pop` is the usual leaf occupancy. The `loc` column is full `u64`, so **full `ArenaLong` reach is retained** with **full 32-bit metadata precision** — no bit-stealing, no capacity cap, no `meta = 0`.

#### 5.5.2 Free key recovery — the decisive advantage over an arena-chunk sidecar

Because the keys already live **in the leaf**, sorted, a SIMD match maps directly to its key with **zero reverse index** and **zero change to `BlobRecordHeader`**. An arena-chunk columnar sidecar, by contrast, produces matches in physical slab order and must recover the key from the record header — which today stores only `len + generation` (`blobmap.rs`), forcing a `+8 B`/record format change. `BlobLeaf32` avoids that entirely and **preserves lexicographic key ordering**, so it can back the *ordered* `scan_filtered`, not only an unordered OLAP scan. (This key-ordering is `BlobLeafVector`'s one genuine edge over the sidecar — but it is bought by co-locating columnar metadata in the leaf, which trips the §2.2 Fat-Slots and Complecting anti-patterns; see the §5.5 alignment callout. On Judy-architecture grounds the sidecar wins despite giving up key order.)

#### 5.5.3 Scan path — SIMD inside key-ordered iteration

Reusing the ordered leaf iterator, per blob leaf: (1) load the `meta` column (~1 cache line per 16 entries), SIMD-compare against the predicate bounds → movemask → selection bitmask; (2) for each set bit, read `loc[i]`, fetch the cold payload, invoke the callback with `key[i]` — **in key order**. Locators and payloads for non-matching lanes are never touched.

#### 5.5.4 Vectorizable-predicate constraint

The current `scan_filtered(predicate: FnMut(Key, u32) -> bool)` accepts an arbitrary closure, which cannot be vectorized. The fast path requires a **vectorizable predicate representation** — a range (`lo..=hi`), mask/equality, or set-membership — exposed as a distinct entry (e.g. `scan_meta_range(key_range, lo..=hi, callback)`). Arbitrary closures fall back to a scalar per-lane evaluation: still correct, but at the ~4–5× floor. The vectorized speedup applies only to the vectorizable form.

#### 5.5.5 Point lookup, mutation, and GC

- **Point `get()`** reconstructs a `ValueSlot` from `meta[i] + loc[i]`, touching the meta-column line plus the locator-column line (≈2 cache lines vs 1 for a scalar leaf) — a small, blob-map-only regression; the public `u64` `ValueSlot`/JudyL return contract is unchanged. Scalar `ExpanseMap`/JudyL keep 64-byte scalar leaves at maximum density and are unaffected.
- **Insert/delete** rebuild the leaf SoA exactly as scalar leaves rebuild `[values][keys]` today.
- **GC/compaction** updates `loc[i]` in place. Because metadata is *co-located with the leaf*, there is **no separate sidecar to synchronize** — GC is simpler than an arena-chunk sidecar. The metadata column inherits the standard OCC/seqlock reader discipline (invariant §10.1 #4).

#### 5.5.6 Performance model — superseded by measurement

> **This section's original premise was falsified by measurement (§10.3, RESOLVED).** It argued the payload-skip win was only ~4–5× (traversal-floor bounded), so `BlobLeaf32`'s SIMD scan was needed to reach ≥10×. **Wrong.** The ~4–5× was extrapolated from the *warm*-arena 1.4× ceiling; in the *cold* regime the in-slot-meta trie walk costs ~1.5 ns/entry against a ~34 ns cold payload fetch, so skipping non-matching payloads alone yields — against an honest post-#355 control — **~10.7× at σ=0.001 / ~6.37× at σ=0.05, Phase 1 (`ArenaMeta`) already, with no leaf change** (the pre-#355 ~22× / ~10.3× carried the #355 re-descent in the baseline; §10.3). The reasoning below is retained for the record but does not describe the shipped engine.

The original argument (now known to mis-weight the traversal cost): `BlobLeaf32` does not eliminate the trie walk; it iterates leaves in key order and performs $N/\text{pop}$ inter-leaf descents, so its ceiling was expected to be **occupancy-sensitive** — "approaches ≥10× at high leaf occupancy, degrades toward ~4–5× as occupancy drops." Since the payload-skip alone carries the cold-DRAM win (against an honest post-#355 control, ~10.7× at σ=0.001 / ~6.37× at σ=0.05 — §10.3; clearing ≥10× only at very low σ), the marginal gain of the SIMD meta scan is confined to shaving the ~1.5 ns/entry traversal. The NO-GO in §5.5.10 rests on architectural grounds (§2.2), independent of these ratios.

#### 5.5.7 `CompactInSlot` (default layout)

The simplest fix for the `meta = 0` degeneration is not a new leaf at all — it is the `ArenaMeta` `ValueSlot` encoding (tag `0x10`), which carries metadata *and* a wide-enough locator in the existing 64 bits. With an 8-bit tag, 56 bits remain for `meta + locator`; enforcing 16-byte payload alignment, the implemented split is:

```
[ hot_meta (24) | locator (32) | tag (8) ]     locator = global_offset / 16
  bits 63:40       bits 39:8      bits 7:0      -> 2^32 x 16 B = 64 GiB arena
```

`ArenaMeta` is the **sole** arena encoding — it *replaces* the former `ArenaShort` (16 MiB + 32-bit meta) and `ArenaLong` (64 PiB, metadata-less) rather than sitting between them (there is no size spectrum, hence not "Mid"). It reuses the current map-leaf machinery `[values: u64 x C][keys: L x C]` **unchanged** — the value word is still one `u64`, so there is **no new leaf type, no struct-of-arrays, and no extra per-entry footprint**. Records are already 16-byte aligned and the chunk size is rounded up to a 16-multiple, so `global_offset / 16` is exact; the locator resolves to a chunk/offset arithmetically (`global = locator·16`), preserving `with_chunk_size` configurability. Metadata is read in-slot during the standard key-ordered walk, so the filter works. **Measured cold-DRAM speedup (honest post-#355 control): ~10.7× at σ=0.001 / ~6.37× at σ=0.05** (§10.3; the pre-#355 ~22× / ~10.3× carried the #355 re-descent in the baseline) — the earlier `~4–5×` prediction here was wrong (see §5.5.6): the in-slot-meta trie walk (~1.5 ns/entry) is far cheaper than a cold payload fetch (~34 ns), so it is *not* the floor in the cold regime. Envelope: **≤ 24-bit metadata** (16.7 M states) and **≤ 64 GiB arena**. **Overflow policy (decided, #287): error at insert** — `hot_meta > 24-bit` → `ArenaError::MetaOverflow`, arena `≥ 64 GiB` → `OffsetOverflow`, validated *before* the arena allocation so a rejected insert leaves no orphan; **never silently truncated**. (Spilling to a metaless encoding was rejected — it reintroduces the `meta = 0` cliff.) The 1 GiB `MAX_ARENA_CAPACITY` safety cap keeps the arena well under the 64 GiB locator envelope, so an offset overflow cannot occur under the shipped cap.

#### 5.5.8 Layout selection & C ABI

> **Refinement (decided).** An earlier draft proposed *both* a zero-cost Rust type marker (`ExpanseBlobMap<L>`) *and* a runtime C-ABI discriminant — two dispatch mechanisms for one decision. Since the C ABI forces a runtime discriminant regardless, and blob ops are DRAM-bound (a predicted branch is free next to an ~80 ns fetch), the generic buys nothing but complexity. **The layout is a single runtime enum field on the map**, uniform across Rust and C. This is deferred to Phase 2 — Phase 1 ships one layout (`CompactInSlot`/`ArenaMeta`), so no enum or dispatch exists yet.

- **Runtime layout enum.** A `BlobLayout { Compact, Vector }` field on the map; the encode/decode seam (centralized in Phase 1) dispatches on it — one predictable branch per blob operation, serving both the Rust API and the `expanse_blob_*` C entry points.
- **Tag-directed leaf descent.** A leaf tag bit (`LEAF_BLOB_SCALAR` vs `LEAF_BLOB_VECTOR`) selects the leaf handler at leaf resolution; a given map fixes one layout at creation.

#### 5.5.9 Cost of supporting both

The cost is **asymmetric, and smaller than a naïve "two leaf implementations" reading** (which assumed a 64 B → 128 B leaf doubling; the actual map leaf is already `8·C + L·C` bytes — ~256 B at `pop=16, L=8`, several cache lines, *not* one):

- **`CompactInSlot` is nearly free** — a new `ValueSlot` encoding within the existing leaf; no new SIMD kernels, allocation logic, or mutation paths.
- **`BlobLeafVector` is the real cost** — one genuinely new leaf layout (SoA), which doubles the surface of the most safety-critical, most-optimized, most-`unsafe` code in the crate (the leaf layer, where the #225 SIMD out-of-bounds bug lived): a second set of SIMD/SWAR kernels, `cap_class`/allocation math, in-place shift logic, and Miri/fuzz/parity coverage. Its incremental footprint over the scalar blob leaf is **`+4·C` bytes** (the 32-bit meta column: `[u32 meta × C][u64 loc × C][keys × C]` = `12·C + L·C` vs `8·C + L·C`) — **+4 bytes/entry, not a doubling** — and point `get()` touches **one extra column** (meta + loc regions vs a single value region), a modest blob-map-only regression, not "1 → 2 cache lines" from a 64 B baseline.

So the "in-slot wins on point-lookup / footprint / write-churn / embedded" observations hold in **direction**, but the magnitudes are `+4 B`/entry and one extra region touched/shifted — not the 2× figures. `CompactInSlot` remains the right default; `BlobLeafVector` earns its second-leaf-layout maintenance cost only where the analytical scan speedup is demonstrated.

#### 5.5.10 Phasing & first work item

Measurement-gated, two phases (tracked in #285):

- **Phase 1 — `CompactInSlot` (default). ✅ Implemented (#287); meets the `>10×` target at σ=0.001 (10.7×), not at σ=0.05 (6.37×) against the honest control — §10.3, #355.** The `ArenaMeta` encoding (sole arena encoding, tag `0x10`, replacing `ArenaShort`/`ArenaLong`) + 24-bit-meta in-slot read + error-at-insert overflow policy. Fixes the correctness bug (metadata survives across the whole arena) **and, measured against an honest post-#355 control, delivers ~10.7× at σ=0.001 / ~6.37× at σ=0.05** (§10.3) — clearing the RFC `>10×` target at very low σ but ~6.4× at σ=0.05 (so the `>10× at σ≤0.05` target is **not** met at σ=0.05; the pre-#355 ~10.3× / ~22× carried the #355 re-descent), with no new leaf type.
- **Phase 2 — `BlobLeafVector` (opt-in). ❌ Not needed (go/no-go = NO-GO, #285).** The Phase-2 gate was: does a SIMD columnar leaf beat Phase 1 enough to justify a second leaf layout? Measurement answered no — Phase 1's payload-skip already carries the cold-DRAM win without a second leaf layout (against an honest post-#355 control, **~10.7× at σ=0.001 / ~6.37× at σ=0.05**; §10.3 — clearing ≥10× at very low σ, ~6.4× at σ=0.05), because in the cold regime the trie walk is ~1.5 ns/entry against a ~34 ns cold payload, so it was never the dominant cost. `BlobLeafVector`'s only marginal gain (SIMD-scanning the meta column to shave that ~1.5 ns/entry traversal) is bounded — it cannot manufacture cold payloads to skip — and the pre-#355 premise that Phase 1 "already clears ≥10× at σ≤0.05" is revised by the honest control (σ=0.05 is ~6.4×, not ≥10×; §10.3). **The decisive ground is anyway *architectural*, independent of these ratios:** the SoA leaf is a multi-word leaf array that co-locates columnar metadata in the trie index, tripping two `AGENTS.md` §2.2 anti-patterns (Fat Slots, Complecting Index and Columnar Attributes; see the §5.5 alignment callout). The §2.2-sanctioned home for wide/multi-column metadata is a **decoupled sidecar** — pursued in #295, which also covers the >64 GiB axis. The `BlobLeafVector` design is retained here only as a *documented record of a rejected approach*; it is not on the roadmap, and a future need for wide/multi-column/key-ordered metadata should be met by the sidecar (#295), not by fattening the leaf.

**Acceptance framing (per the §10.3 gate discussion):** gate on **correctness + a measured cold-DRAM speedup**, keeping $\ge 10\times$ as a *labelled target* rather than a hard pass/fail. Phase 1 delivers correctness; Phase 2's $\ge 10\times$ is pursued only where occupancy and workload justify it.

---

## 5.6 Scalar metadata sidecar POC (#295)

**Status:** research POC, executed under #295 — **historical record; the code is
not in tree.** The POC code (`crates/expanse/src/poc_sidecar.rs`:
`SidecarBlobMap<K>` + `InvertedIndex`, behind a non-default `poc-meta-sidecar`
feature, with the bench harness `crates/expanse/benches/poc_sidecar_295.rs`) is
frozen at the annotated tag **`poc/295-meta-sidecar`** (commit `25c6f128`, the
commit the §5.6.4 timing was measured on; the later docs-only commits on the
retained `poc/meta-sidecar-295` branch are folded into this section). It was
deliberately not merged, following the repo's precedent for NO-GO POCs (§5.5.10
BlobLeafVector; the #277 descent-prefetch experiment): the record lives here,
the code lives on an immutable tag, and nothing feature-gated rots silently in
the crate or in CI. Reproduce with
`git checkout poc/295-meta-sidecar && cargo bench -p expanse-trie --features poc-meta-sidecar --bench poc_sidecar_295`
(the tag will drift from `main`; it is a point-in-time artifact, not maintained
code). The shipped Phase-1 `ExpanseBlobMap` / `ArenaMeta` in-slot encoding
(§5.5.7) was **untouched** by the POC, which only added decoupled parallel
structures beside the same engine. Nothing graduates to the stable API — the
issue's trigger (a concrete workload needing `>24`-bit / multi-column / `>64`
GiB metadata) is not met, and §5.6.4 shows the lift is not free.

This section is a **frozen pre-registration** (§5.6.1–§5.6.2) followed by
measured results annotated in place (§5.6.3 arch-independent structural, §5.6.4
dedicated-host timing) and a resolved verdict (§5.6.5). Headline: the capacity
axes (H4) are confirmed, but the timing **refutes H1** (the sidecar is *not* at
warm parity — a σ-dependent crossover) and **confirms H3 more strongly than
predicted** (a ~1.4–3× scan-time penalty that starts past L2, not a narrow LLC
cliff), so the capacity lift is a real trade, not free.

### 5.6.1 Pre-registration — design, Step-0 math, hypotheses (frozen)

**The three capacity axes Phase 1 cannot serve** (from #295): (a) metadata
`> 24` bits (full-32-bit timestamps); (b) multi-column predicates
(`ts BETWEEN … AND tenant = … AND status = …`); (c) arena `> 64` GiB (the
shipped 32-bit `ArenaMeta` locator caps at `2^32 × 16 =` **64 GiB**,
`ARENA_META_CEILING`).

**Design (§2.2-compliant).** Move metadata **out of the 64-bit value word** into
decoupled parallel arrays keyed by a dense record handle:

- The trie value word holds only a compact dense `RecordId` (a `u32` handle) —
  *not* an arena address, *not* metadata — so `ValueSlot` never widens (no Fat
  Slot; leaf density stays 8 slots / 64 B line).
- `offsets[rid]: u64` — the arena address (a `u64` sidecar entry, so it lifts the
  64 GiB ceiling; axis *c*).
- `meta[rid]: [u32; K]` — `K` full-32-bit metadata columns (axes *a*, *b*).

During a key-ordered range scan the predicate reads `meta[rid]` (a warm dense
array) before any cold payload fetch, preserving Phase-1 key order and its
cold-payload-skip. This is exactly the "decoupled columnar sidecar" `AGENTS.md`
§2.2 prescribes. (An alternative *production* shape keeps a wide **56-bit
locator** in the slot instead of a dense handle — `pack_wide_locator`,
addressing `2^56 × 16 = 2^60` B = **1 EiB** in 16-byte units, well beyond the
issue's stated 64 PiB — still evicting metadata to the sidecar. The POC uses the
dense-handle form because it makes *both* address and metadata sidecars and
keeps the trie word narrowest.)

**Step-0 math (derivable before any run; code-pinned in
`poc_sidecar::tests::sidecar_bytes_per_record_math` on tag
`poc/295-meta-sidecar`).**

1. **Sidecar bytes per record** = `size_of::<u64>()` (offset) +
   `size_of::<[u32; K]>()` (meta) + 1 (live flag):
   - `K = 1`: 8 + 4 + 1 = **13 B/record**.
   - `K = 3`: 8 + 12 + 1 = **21 B/record**.
   (Handle recycling via a free list keeps allocated slots ≈ live count, so this
   is also the steady-state footprint per live record.)

2. **Sidecar residency vs the metadata-read regime** *(projected from the
   bytes/record above; reference host L2 = 1.25 MiB/core, L3 = 30 MiB)*:

   | records | sidecar `K=1` | sidecar `K=3` | regime |
   |---:|---:|---:|---|
   | 262 144 | 3.25 MiB | 5.25 MiB | **≪ L3 → warm** (both) |
   | 1 000 000 | 12.4 MiB | 20.0 MiB | < L3 → warm |
   | 3 000 000 | 37.2 MiB | 60.1 MiB | **> L3 → sidecar spills** |
   | 10 000 000 | 124 MiB | 200 MiB | ≫ L3 → sidecar cold |

   This is the **load-bearing pre-registered loss** (H3 below): the sidecar read
   is warm only while the *sidecar itself* is LLC-resident. Because handles are
   assigned in **insert** order, a **key-ordered** scan reads `meta[rid]` in a
   permuted order; once the sidecar exceeds the LLC that becomes a *second*
   cold-DRAM random stream — a cost Phase-1 in-slot metadata does not pay
   (its metadata rides in the trie leaf the walk already touches). The #295
   cold-DRAM harness (262 k records) is deliberately in the **warm regime**, so
   it tests the mechanism at parity; the `>`LLC-sidecar regime is a separate,
   named limitation.

   > **[Measurement correction, §5.6.4]** This pre-registered model is **wrong on
   > two counts**, kept here frozen with the correction annotated. (i) The "262 k
   > → warm parity" prediction is refuted: even at 262 k the sidecar is 1.37×
   > slower at σ=0.001 — the extra permuted read is never free (H1). (ii) The
   > boundary is **L2-residency, not LLC-residency.** The loss is already 2.70× at
   > **1 M**, where `meta[]` (11.4 MiB measured, `K=3`) is *well inside* the 30 MiB
   > L3 — because permuted access to any array larger than **L2 (1.25 MiB)** already
   > costs L3/DRAM latency. So "warm while `meta[]` `<` LLC" should read "cheap only
   > while `meta[]` `<` L2"; past L2 the sidecar pays a growing scan tax
   > (1.37×→~3×). See §5.6.4/§5.6.5.

3. **56-bit locator ceiling** (axis *c*): `2^56 × 16 B = 2^60 B = 1 EiB`
   (`WIDE_LOCATOR_CEILING`); the sidecar's `u64` `offsets` column already
   addresses the full `usize` arena, both far beyond the shipped 64 GiB
   `ArenaMeta` ceiling. Pinned in `axis_c_wide_locator_encodes_beyond_64gib`.

4. **Compaction sync-cost model.** Both engines copy every live payload into a
   fresh arena (identical payload-copy cost). They differ only in **writeback**:
   Phase-1 rewrites one trie value slot **per live record** via a random-access
   `get_value_slot(key)` descent (`compact_with_index`); the sidecar rewrites the
   **dense `offsets[rid]` array sequentially and never touches the trie** (handles
   are stable across compaction). Predicted sidecar advantage ≈ the cost of
   `N_live` random trie-slot writes. Predicted sidecar *disadvantage*: the arena
   is not renumbered, so dead handles leave holes until reused (bounded by the
   free list).

**Hypotheses & expected losses (pre-registered):**

- **H1 (parity, warm regime).** On the 262 k-record cold-DRAM harness, sidecar
  cold-scan latency **≈ Phase-1 in-slot** at every σ (both read warm metadata;
  the sidecar's one extra warm array access per entry is small against the cold
  payload fetch). *Expected loss:* a slight sidecar regression at high σ, where
  per-entry bookkeeping dominates because most payloads are touched anyway.
- **H2 (payload-skip preserved).** Both sidecar and Phase-1 clear the RFC
  cold-DRAM regime (~10× at σ=0.05, ~22× at σ=0.001) over the payload-fetch
  baseline — the payload skip, not the metadata mechanism, is the source of the
  speedup. *(pre-#355 control; honest values §10.3: ~6.37× at σ=0.05 / ~10.7× at σ=0.001 against a baseline that no longer carries the #355 re-descent.)*
- **H3 (residency cliff — expected loss).** Beyond ~2–3 M records the sidecar
  exceeds the LLC and, with insert-ordered handles, adds a second cold stream, so
  the sidecar is expected to **lose** to Phase-1 in-slot at very large N. Not
  exercised by the 262 k harness; documented as a scope limit.
- **H4 (capacity).** The sidecar serves all three axes with **key-ordered**
  output equal to a `BTreeMap` reference (32-bit ts; 3-attribute predicate; `u64`
  offsets / 56-bit locator `> 64` GiB) — verified by differential tests.
- **H5 (compaction).** Sidecar compaction is **faster** than Phase-1 by ≈ the
  `N_live` random trie-slot writes it avoids.
- **H6 (write path).** Sidecar insert ≈ Phase-1 insert (parity); the extra dense
  `Vec` pushes are amortized-cheap.
- **H7 (inverted index).** For low-cardinality discrete attributes, native
  `ExpanseSet` intersection answers multi-attribute equality; the
  high-cardinality/continuous (ts) case degenerates to ~one posting list per
  distinct value, mitigated ~500× by bucketing at the cost of a residual filter.

### 5.6.2 Implementation & tests

`SidecarBlobMap<const K: usize>` and `InvertedIndex` in `poc_sidecar.rs` (tag
`poc/295-meta-sidecar`), reusing the shipped `BlobArena` and `ExpanseMap`
unchanged. Correctness is covered by 11
differential tests (all green): point lookup / metadata round-trip; **axis a**
(full-32-bit ts range scan, key-ordered, `== BTreeMap`); **axis b** (3-attribute
predicate, `== BTreeMap`); **axis c** (56-bit locator encodes `>` 64 GiB where
the shipped 32-bit locator overflows); overwrite handle reuse; remove→recycle→
compaction data preservation; an 8 000-op randomized differential vs `BTreeMap`
across a compaction; inverted-index intersection vs a brute-force reference; ts
range exact-vs-bucketed agreement; and the bucketing list-count collapse. All
mandatory gates passed with the feature on at the tagged commit (`fmt`,
`clippy -D warnings`, workspace tests, `PROPTEST_CASES=500`; CI green on
`25c6f128`); default builds were unaffected (feature-gated, non-default).

### 5.6.3 Measured — arch-independent (structural) *(measured: this engine at tag `poc/295-meta-sidecar`, reproducible via `poc_sidecar::characterize::characterize_poc_295`; counts/bytes are deterministic and host-independent, so no timing host is needed — same basis as the §10.3 match-rate table)*

Harvested by `poc_sidecar::characterize::characterize_poc_295` (N = 262 144,
1 KiB payloads, matching the cold-DRAM harness).

**Sidecar footprint (the space cost of the capacity lift):**

| | `K=1` | `K=3` |
|---|---:|---:|
| sidecar bytes / record | 13 B | 21 B |
| sidecar arrays @262 k | 3.25 MiB | 5.25 MiB |
| total `mem_used` (incl. 260 MiB arena + trie) | 325.4 MiB | 327.4 MiB |

The sidecar adds **13–21 B/record** over Phase-1 (whose metadata rides free in
the slot) — ≈ 1–2 % of the payload arena at 1 KiB payloads. Both sidecars are
LLC-resident at 262 k (warm regime; H1).

**Compaction work (H5, structural):** at N = 262 144 with 50 % deleted, the
sidecar relocates **131 072** live records → **131 072 dense `offsets[]`
writes** and **131 072 random-access trie value-slot writes AVOIDED** vs
Phase-1. (Timing in §5.6.4.)

**Inverted index (H7, structural):** at N = 262 144:

| column | distinct values | posting lists | postings | set memory |
|---|---:|---:|---:|---:|
| tenant | 64 | 64 | 262 144 | 1.03 MiB |
| status | 4 | 4 | 262 144 | 0.31 MiB |
| ts (exact) | ~245 856 | **245 856** | 262 144 | 3.76 MiB |
| ts (bucketed, `2^12`) | — | **489** | 262 144 | 3.80 MiB |

The exact-ts column degenerates to ≈ one posting list per key (245 856 lists for
262 144 near-unique timestamps) — the pre-registered high-cardinality weak point.
Bucketing at `2^12` collapses it to 489 lists (**503× fewer**) for a
range-query fan-out win, at similar memory and the cost of a residual exact
filter on the two boundary buckets. Representative
`count(tenant=3 AND status=1) = 1082` via `intersection_len` (no materialization).

### 5.6.4 Measured — timing (dedicated host) *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit `25c6f128`; window 16:38:58–16:47:22Z, load before 0.00 / after 1.07, no concurrent bench; `poc_sidecar_295` bench, criterion medians, `sample_size = 10`)*

> **Load-hygiene note.** The `run.sh` "before" snapshot read loadavg 3.92, taken
> *immediately after* the release build; that is a 1-min average still decaying
> from the compile (5-min 1.32, 15-min 0.47 — a build spike, not a concurrent
> workload), so the **first cell** (`sidecar_cold_dram/phase1/σ=0.001`) was
> measured during that decay. Three independent checks say the run is
> nonetheless clean: (a) the box has 24 threads and the bench is single-threaded,
> so even at loadavg ~4 it is not core-starved; (b) every criterion median spread
> is `< 0.5 %` (e.g. 409.29 / 409.82 / 410.66 µs), which contention would widen
> into outliers; (c) that first cell's 409.8 µs **matches the independent #287
> §10.3 measurement (~404 µs)** of the same Phase-1 arm on the same harness —
> a decisive cross-check that it was not inflated by the build decay.

**Warm arm — `sidecar_cold_dram` (262 k, 1 KiB payloads, `K=1`; sidecar
`meta[]` = 3.25 MiB).** "naive" = touch every payload (the payload-fetch
baseline), measured on the sidecar map.

| σ | Phase-1 in-slot | sidecar | naive (payload-fetch) | Phase-1 vs naive | sidecar vs naive | **sidecar / Phase-1** |
|---|---:|---:|---:|---:|---:|---:|
| 0.001 | 409.8 µs | 561.7 µs | 5.232 ms | 12.8× | 9.3× | **1.37× slower** |
| 0.05 | 871.7 µs | 908.4 µs | 5.221 ms | 6.0× | 5.7× | **1.04× (≈ parity)** |
| 0.20 | 1.644 ms | 1.390 ms | 5.297 ms | 3.2× | 3.8× | **0.85× (faster)** |
| 1.0 | 9.568 ms | 5.310 ms | 5.401 ms | 0.56× | 1.02× | **0.55× (faster)** |

**H1 (warm parity) is REFUTED — replaced by a σ-dependent crossover.** The
pre-registered "sidecar ≈ Phase-1 in the warm regime" is wrong: the sidecar read
is **never free**. Per scanned record it is **one extra dependent random access**
— `meta[rid]` (and, on a match, `offsets[rid]`) — and because record handles are
assigned in **insert** order, a key-ordered walk hits `meta[]` in *permuted*
order, so that access is an L2 miss → ~40 ns L3 hit (or a DRAM fetch once
`meta[]` is large). Phase-1's in-slot metadata, by contrast, rides in the value
word the range walk **already loads**, so it is not a separate access at all.

At low σ (scan-bound) that extra access dominates and the sidecar is *slower*
(1.37× at σ=0.001; 1.04× at σ=0.05). At high σ (fetch-bound) the sidecar is
*faster* (0.85× at σ=0.20, 0.55× at σ=1.0) — for a *different* reason (the
Phase-1 re-lookup inefficiency, next paragraph), not because its metadata read
got cheaper. The crossover sits near σ ≈ 0.05–0.2.

**Anomaly investigated — Phase-1 is 1.8× slower than the naive payload-fetch arm
at σ=1.0 (9.57 ms vs 5.40 ms; workload: `workload_large_values`), and slower than the sidecar at σ≥0.20. Root cause:
a redundant trie re-descent in the shipped `scan_filtered`, NOT a harness
artifact.** The loop `for (key, raw_slot) in self.index.range(range)` already
holds `raw_slot` (the locator), but on **every match** it calls `self.get(key)`,
which calls `self.index.get_slot_ptr(key)` — a *fresh* `O(k)` trie descent to
re-find the slot it already has — before `resolve_meta`. So at σ=1.0 Phase-1 pays
`N` redundant descents on top of the `N` payload fetches, while the sidecar-based
naive resolves payloads directly via `offsets[rid]` (no descent). Two facts
confirm this is real Phase-1 behaviour, not a measurement artifact: (a) the code
path above is the shipped `ExpanseBlobMap::scan_filtered` (`blobmap.rs`), which
the harness calls unmodified; (b) **#287 §10.3's own naive arm — which *is*
Phase-1's `get()`-per-entry full-deref — measured ~8.9 ms, ≈ this run's Phase-1
`scan_filtered` at σ=1.0 (9.57 ms)**; both pay the same redundant descent. The
fix is one line (resolve the payload from the `raw_slot` already in hand instead
of calling `get(key)`), and it would speed the shipped engine at high match rates
independently of the sidecar — filed as its own Phase-1 optimization (see §5.6.5).
**Confirmed by #355** *(measured: reference host, commit `4f2f3a18`)*: with the re-descent removed, `predicate_scan_cold_dram_large`'s `naive_row_deref` arm (the `scan_filtered` full-deref that measured ~8.9 ms here) drops to ~4.05 ms — the honest touch-all floor — and the columnar arm at σ=1.0 falls from 8.86 ms to 4.07 ms, confirming the ~8.9 ms was the redundant descent, not the payload fetch. The H1 σ ≥ 0.20 crossover where the sidecar appeared to "win" was this artifact; see the re-measured §10.3 row.

**H2 (≥10× vs payload-fetch at σ≤0.05) — and why this arm reads 12.8× / 6.0×
where #287 published ~22× / ~10.3×.** This is **not** a lower re-measurement of
#287. The harness is **identical** to #287 §10.3's `bench_predicate_scan_cold_dram_large`
— same `N = 262 144`, same 1 KiB payloads, same ~260 MiB arena (64 MiB chunks,
≈ 8.7× the 30 MiB LLC), same shuffled insert order — and this run's **Phase-1 arm
reproduces #287's columnar arm exactly** (409.8 µs / 871.7 µs here vs ~404 µs /
~872 µs there). The *only* thing that differs is the **payload-fetch baseline**:
this bench's naive resolves payloads through the sidecar's handle (`offsets[rid]`,
re-lookup-free) and measures **5.2 ms**, whereas #287's naive used Phase-1's
`get()`-per-entry full-deref and measured **~8.9 ms** — the ~3.7 ms gap is exactly
the redundant-re-descent overhead (the anomaly above) applied to all 262 k
entries. So the speedup ratio is baseline-sensitive: against #287's Phase-1-based
baseline the Phase-1 arm still lands ~10× / ~22×; against this run's tougher
(re-lookup-free) baseline it is 12.8× / 6.0× (sidecar 9.3× / 5.7×). #287's
numbers are arithmetically unchanged *for that baseline*; the smaller ratios here
reflect a *harder* denominator, not a slower Phase-1. **#355 resolves which
baseline is honest:** #287's Phase-1-based denominator is exactly the
`get()`-per-entry full-deref that carried the redundant re-descent #355 removes,
so the re-lookup-free ratio here (~6.0× at σ=0.05 / ~12.8× at σ=0.001) is the
faithful one — and the post-#355 re-measurement agrees (~6.37× / ~10.7×; §10.3).
The `>10× at σ≤0.05` target is therefore **not** met at σ=0.05 once the baseline
no longer carries the descent; it is cleared only at very low σ (~σ=0.001).

**`>`LLC arm — `sidecar_cold_dram_xllc` (H3 residency cliff; declared scaled
proxy).** The cliff is driven by the per-entry `meta[]` array (`4·K·N` bytes),
not payload size; a literal 1 KiB payload at these N would exceed the shipped
1 GiB `MAX_ARENA_CAPACITY` cap (Phase-1 code, out of scope), so payload size is
dropped to keep every arena `<` 1 GiB while `K` sizes `meta[]` across the 30 MiB
L3 (encoding reach for the true `>64` GiB regime is covered by §5.6.1 axis *c*;
this arm measures the *residency* effect). Sizes are the harness's logged
`alloc`-rounded totals; the guard `xllc_6m_128b_fits_under_arena_cap` confirms
they fit the cap.

| N | K | payload | `meta[]` | ×L3 | σ | Phase-1 | sidecar | naive | **sidecar / Phase-1** | Phase-1 vs naive |
|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|
| 1 M | 3 | 256 B | 11.4 MiB | 0.38× | 0.001 | 1.815 ms | 4.898 ms | 31.96 ms | **2.70× slower** | 17.6× |
| 1 M | 3 | 256 B | 11.4 MiB | 0.38× | 0.05 | 4.608 ms | 8.956 ms | 32.09 ms | **1.94× slower** | 7.0× |
| 3 M | 3 | 256 B | 34.3 MiB | 1.14× | 0.001 | 11.12 ms | 33.27 ms | 123.4 ms | **2.99× slower** | 11.1× |
| 3 M | 3 | 256 B | 34.3 MiB | 1.14× | 0.05 | 24.10 ms | 46.64 ms | 124.0 ms | **1.94× slower** | 5.1× |
| 6 M | 4 | 128 B | 91.6 MiB | 3.05× | 0.001 | 25.66 ms | 74.91 ms | 258.9 ms | **2.92× slower** | 10.1× |
| 6 M | 4 | 128 B | 91.6 MiB | 3.05× | 0.05 | 49.94 ms | 102.6 ms | 259.9 ms | **2.05× slower** | 5.2× |

**H3 is CONFIRMED — and stronger / earlier than predicted.** The sidecar's
scan penalty is **not a sharp LLC cliff** but a *monotone degradation that
saturates*: at σ=0.001 the sidecar/Phase-1 ratio grows 1.37× (262 k, 3.25 MiB) →
2.70× (1 M, 11.4 MiB) → 2.99× (3 M, 34.3 MiB) → 2.92× (6 M, 91.6 MiB). Crucially
it is already 2.70× at **1 M, where `meta[]` (11.4 MiB) is well *inside* the
30 MiB L3** — because permuted access to an array larger than **L2** (1.25 MiB)
already costs L3/DRAM latency, while Phase-1's in-leaf metadata stays free. The
penalty saturates near ~3× once `meta[]` is DRAM-bound (by 6 M, Phase-1's trie is
cold too). So the true limitation is broader than "the sidecar must fit the LLC":
**on scan-bound (low-σ) workloads past ~L2, the sidecar loses ~2–3× to Phase-1
in-slot metadata.** At σ=0.05 the gap narrows (1.94–2.05×) as the on-match savings
begin to offset, but the sidecar still loses across every `>`LLC cell.

**Compaction — `sidecar_compaction` (H5): CONFIRMED, ~1.4× faster.**

| N (50 %-delete) | Phase-1 | sidecar | sidecar speedup |
|---:|---:|---:|---:|
| 20 000 | 412.9 µs | 290.2 µs | **1.42×** |
| 100 000 | 2.492 ms | 1.809 ms | **1.38×** |

Sidecar compaction rewrites the dense `offsets[]` and skips the per-record
random-access trie value-slot rewrite Phase-1 performs — a measured ~1.4× win.

**Write path — `sidecar_write_path` (H6): CONFIRMED, ≈ parity.** 50 k inserts:
Phase-1 1.518 ms vs sidecar 1.582 ms (**sidecar 1.04× slower**; workload: `workload_large_values` — the extra dense
`Vec` pushes; negligible).

**Inverted index — `inverted_index` (H7): CONFIRMED.** 262 k corpus:

| query | median |
|---|---:|
| `intersection` materialize (tenant=3 ∩ status=1, ~1082 keys) | 40.5 µs |
| `intersection_len` (count only) | 16.0 µs |
| ts range exact (100 k-wide, ~many lists) | 949.8 µs |
| ts range bucketed (`2^12`) | 596.5 µs |

Native set intersection answers a 2-attribute query in tens of µs;
`intersection_len` is **2.5× faster** than materializing (no result set built);
bucketed ts-range is **1.6× faster** than exact (fewer posting lists to union,
even with the residual boundary filter).

### 5.6.5 Verdict & recommendation *(RESOLVED — structural + timing both measured)*

- **Correctness / capacity (H4): CONFIRMED.** The scalar sidecar serves all three
  axes — full-32-bit metadata, multi-column predicates, `> 64` GiB addressing —
  with **key-ordered** output equal to a `BTreeMap` reference, keeping `ValueSlot`
  a single machine word (§2.2-compliant). Phase-1 simply *cannot* express any of
  the three; the sidecar is the faithful home for them.
- **Scan cost (H1 REFUTED, H3 CONFIRMED — the load-bearing result): the capacity
  lift is NOT free.** The sidecar adds a permuted metadata stream that Phase-1
  gets free from the trie leaf, so on **scan-bound (low-σ)** workloads it is
  **1.4× slower at 262 k and ~2.7–3.0× slower at 1 M–6 M** — the penalty starts
  as soon as `meta[]` exceeds **L2** (not the LLC) and saturates near ~3×. It is
  *not* a narrow `>`LLC cliff; it is a broad scan-time tax that grows with N.
- **But the sidecar WINS on fetch-bound and compaction workloads.** At high σ its
  handle resolves payloads directly and **beats Phase-1** (0.55× at σ=1.0),
  because Phase-1's `scan_filtered` re-descends the trie per match; and compaction
  is **~1.4× faster** (H5). Write path is parity (H6, 1.04× slower).
- **Inverted index (H7): a complement, not a replacement.** Native set
  intersection at tens of µs for low-cardinality discrete attributes
  (tenant/status → 64/4 posting lists); `intersection_len` 2.5× faster than
  materialize; unsuitable for continuous/high-cardinality fields without bucketing
  (503× list-count blow-up, mitigated to 1.6× faster range queries when bucketed).
- **Bonus finding (Phase-1, independent of the sidecar):** the shipped
  `ExpanseBlobMap::scan_filtered` re-looks-up each matching key via `get(key)`
  though it already holds the slot from the range walk — a redundant O(k) trie
  descent per match. It is pathological at high σ (σ=1.0: 9.57 ms, *slower than
  the touch-every-payload baseline* at 5.40 ms). Resolving the payload directly
  from the range-walk slot would remove it. Filed as its own Phase-1 optimization
  ([#355](https://github.com/orieg/expanse/issues/355)), orthogonal to the
  capacity question; **the H1 crossover above (sidecar "wins" at σ ≥ 0.20) should
  be re-read once #355 lands** — that apparent advantage is an artifact of this
  re-descent, not a genuine sidecar edge.
  - **#355 landed** *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit `4f2f3a18`)*: `scan_filtered` now resolves each match from the slot the range walk already holds. High-σ pathology gone — the shipped scan at σ=1.0 drops from **8.86 ms → 4.07 ms** (cold-DRAM `predicate_scan_cold_dram_large`) and from **467.7 µs → 133.9 µs** (warm `predicate_scan_selectivity_sweep`, now 3.16× faster than the naive `get()` loop rather than 0.91× slower); no low-σ regression. This also re-baselines §10.3: the naive control there carried the same descent, so the honest cold-DRAM columnar-vs-naive speedup is ~10.7× @ σ=0.001 / ~6.37× @ σ=0.05 (was 22× / 10.3×) — see the re-measured §10.3 row. The H1 σ ≥ 0.20 "sidecar wins" crossover was indeed an artifact of this re-descent.

**Recommendation.**

1. **Which §2.2-compliant shape covers which axis:** the **scalar metadata
   sidecar** is the correct home for wide (`>24`-bit) / multi-column / `>64` GiB
   metadata **with key order preserved** — but it trades a measured **~1.4–3×
   scan-time penalty** (low-σ, growing with N past L2) for that capacity, plus
   wins at high σ and on compaction. The **inverted index** complements it for
   low-cardinality discrete attributes; a **columnar-SIMD sidecar** stays reserved
   for a proven compute-bound *warm* analytical scan (still not demonstrated).
2. **Graduation:** nothing graduates now — consistent with #295's parked status,
   and the timing sharpens *why*: the sidecar is not a free parity swap, so it
   should be adopted only when a workload **needs** the capacity (Phase-1 can't
   serve it) **or** is fetch-bound / compaction-heavy enough to profit. For
   scan-bound low-σ analytics inside Phase-1's 24-bit / 64 GiB envelope, Phase-1
   in-slot metadata is faster — keep it. The POC code stays frozen on tag
   `poc/295-meta-sidecar` (out of tree, out of CI, out of the public API),
   re-runnable when a workload trips the trigger — at which point it is revived
   into a proper home (its own crate), not re-enabled from the tag; that workload weighs the scan penalty against its N
   and σ, and picks a residency mitigation (key-correlated handles — which trades
   H5 away — vs accept the penalty).
3. **Actionable now (independent of graduation):** the Phase-1 `scan_filtered`
   redundant-re-descent optimization is filed as
   [#355](https://github.com/orieg/expanse/issues/355) — it speeds the shipped
   engine at high σ regardless of the sidecar, and its fix invalidates the H1
   σ ≥ 0.20 crossover measured here (which is an artifact of the re-descent).

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

### 6.4 Concurrent Access: `SyncExpanseBlobMap` (issue #219 Phase 1, shipped)

The Phase 7 OCC protocol (`occ` + `sync`) extends to the blob map. The single
writer serializes on the wrapper mutex and brackets every operation with the
tree-level `SeqVersion`; the index trie's `NodeAlloc` and the `BlobArena` are
both handed to one epoch `Collector` at construction (`BlobArena::defer_to`).

- **Reader chunk resolution — RCU chunk table.** Optimistic readers must never
  touch `BlobArena`'s `Vec<ArenaChunk>` (the global allocator frees its buffer
  on growth with no grace period). Instead the arena publishes an immutable
  snapshot table — one allocation: `{len, chunk_size}` header + per-chunk
  `{ptr, capacity, generation}` entries — through an atomic pointer,
  republished on every chunk-set change (`alloc_blob` growth, `push_chunk`,
  `clear`, compaction). Publication always precedes retirement, so a block is
  unreachable before it enters the grace period; superseded tables retire
  through the collector like any node.
- **Read path.** The validated hand-over-hand index walk yields the 64-bit
  `ValueSlot`. Inline (≤ 7 B) payloads decode **by value** from the validated
  word — zero slab reads, exactly the §5 hot-metadata promise. `ArenaMeta`
  slots resolve `locator → (chunk, offset)` against the pinned table with
  capacity bounds + record-generation checks (unwritten chunk bytes are
  zeroed; generation 0 is never live), then re-validate the tree version
  before the borrow is handed out. Bounded retries fall back to an owned copy
  under the writer lock.
- **Zero-copy guards.** `BlobReader::pin()` returns an epoch-pinned
  `BlobReadGuard` (the issue's `SyncBlobReaderGuard`); its `SyncBlobView`
  arena borrows stay byte-stable for the guard's lifetime because arena
  records are never rewritten in place and retired chunks stay mapped while
  pinned — including across a full compaction
  (`sync::tests::sync_blob_guard_view_survives_compaction`,
  `blobmap::tests::deferred_arena_retires_chunks_and_tables` (Miri-clean)).
  Holding `BlobReadGuard` defers epoch advances tree-wide (both arena chunks
  and trie index nodes) until dropped. Callers keep guards in short-lived lexical
  blocks; long-lived views use `BlobReader::get` (owned copy) or `view.as_bytes().to_vec()`.
  Retained garbage under concurrency is observable via `occ_stats` (`retained_bytes`,
  `retained_hwm`) and `Collector::retained_bytes()` (#525).
- **Compaction under concurrency.** `compact_with_index` installs the
  compacted chunk set piecewise, republishes the table, then retires the old
  chunks — never `*self = new_arena` (which would free them immediately).
- Hot-metadata-only lookups (`get_meta`) answer from the validated slot word
  without touching payload cache lines; multi-field reads (`scan_filtered`,
  `mem_used`, persistence) go through `with_locked`.

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
|   (format version is 2 since #518; a mismatch is UnsupportedFormatVersion, see COMPAT.md)  |
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
- `bench_inline_vs_heap_small_blobs`: Measure throughput (ops/sec) and allocations for 1–7 byte keys. Target: $0$ heap allocations, $\gt 3\times$ insert throughput vs `BTreeMap<u64, Vec<u8>>`.
- `bench_predicate_scan_selectivity_sweep`: Measure scan latency across $\sigma \in \{0.001, 0.01, 0.05, 0.20, 1.0\}$. Target: $\gt 10\times$ speedup at $\sigma \le 0.05$.
- `bench_predicate_scan_cold_dram_sweep`: the falsifiable variant of the above with a payload-*touching* baseline (both arms share the `scan_filtered` traversal, so the only difference is payload cache-line loads). Measures the speedup the columnar pushdown actually yields; see §10.3 for why the arena ceiling keeps it warm.
- `bench_arena_compaction_churn`: Measure pause times and memory reclamation under heavy overwrite workloads.

### 10.3 Measured status *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit 695b98d; `benches/large_values.rs`, criterion medians)*

- **`inline_vs_heap_small_blobs`** — `ExpanseBlobMap` (inline value slots) vs `BTreeMap<u64, Vec<u8>>` (heap), 7-byte payload (workload: `workload_large_values`): **insert 199.5 µs vs 587.1 µs (2.9× faster)**, **get 56.8 µs vs 290.0 µs (5.1× faster)** (0-byte payload: 2.3× / 5.1×). Insert is just under the RFC's `>3×` target; get comfortably clears it. Zero-heap-allocation for ≤7-byte payloads is a structural property (inline slots), separately asserted by unit tests.
- **`predicate_scan_selectivity_sweep`** (original, mismatched baseline) — target `>10× at σ≤0.05` is **not demonstrated by this bench**; the `columnar_filtered_scan` (`scan_filtered`) arm is **~2× slower** than the `naive_unfiltered_deref` arm at every selectivity (e.g. σ=0.001: 741 µs vs 367 µs; workload: `workload_large_values`). The baseline is the problem: it uses a raw `get()` loop (different, cheaper traversal than `scan_filtered`) and reads only the payload *length* on a match, so it never touches payload bytes — both arms avoid cold-payload loads and the pushdown has nothing to win, while the traversal-cost mismatch makes the columnar arm look slower.

- **`predicate_scan_cold_dram_sweep`** (falsifiable variant, corrected baseline) — *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, commit `43b46f38`; criterion medians)*. Both arms share the identical `scan_filtered` traversal (so the trie-walk cost cancels) and the naive arm touches **every** payload byte while the columnar arm touches only matches. With that fix the pushdown **is** faster, not slower — **but it does not reach `>10×`, and cannot on the current engine:**

  | σ | columnar `scan_filtered` | naive full-deref | speedup |
  |---|---:|---:|---:|
  | 0.001 | 228.0 µs | 323.5 µs | **1.42×** |
  | 0.01 | 230.7 µs | 325.5 µs | 1.41× |
  | 0.05 | 244.7 µs | 323.8 µs | **1.32×** |
  | 0.20 | 293.1 µs | 323.2 µs | 1.10× |
  | 1.0 | 322.9 µs | 323.3 µs | 1.00× |

  **Why it is capped at ~1.4×, and why the premise is unreachable here.** The RFC's `>10× / 82%-DRAM-traffic` argument requires the skipped payloads to be **cache-cold** — a real DRAM fetch (~80 ns), so avoiding it dwarfs the scan bookkeeping. That needs the arena working set to exceed the host LLC. But the 64-bit `ExpanseBlobMap` arena is **hard-capped at 16 MiB** by the 24-bit `ArenaShort` value-slot offset (`blobmap.rs` "Capacity limits"; the wider `ArenaLong`/`External` encodings that would lift it were **unimplemented** at this commit — `ArenaLong` has since shipped in #244; see the follow-up note after the verdict). 16 MiB < a typical server LLC (the reference host's L3 is **30 MiB**), so the whole arena is forced L3-resident: a skipped payload saves an ~L3 hit (~7 ns here, measured), not a DRAM fetch. The columnar arm is then bounded by the traversal floor — full-scan cost 323 µs ÷ traversal-only floor 228 µs ≈ **1.42× maximum**, reached as σ→0. This is a derivable ceiling, not a tuning gap.

  **Verdict (RFC §10.3 revised, target → measured):** the `>10× at σ≤0.05` speedup is **not achievable on the current `ExpanseBlobMap`** because its 16 MiB arena ceiling keeps the working set inside the LLC, so the cold-DRAM regime the claim depends on never occurs. With a correct payload-touching baseline the measured columnar advantage on a warm (L3-resident) arena is **~1.3–1.4× at σ ≤ 0.05**, traversal-floor bounded. Reaching the `>10×` / `82% traffic reduction` figures is **gated on implementing the wide-offset (`ArenaLong`/`External`) arena** so the payload store can exceed the LLC, *and* on an ordered-scan fast path to lower the traversal floor. Until then the `>10×` / `>15×` / `82%` numbers in this RFC's §Overview remain **targets, not measured results**, and are explicitly gated on that arena work.

  **Follow-up — the size blocker is lifted (#244, merged #276), but a *second* blocker surfaced.** The wide-offset `ArenaLong` arena shipped in #244 (merged as #276), so the 16 MiB ceiling no longer forces the payload store L3-resident: an arena can now be sized *beyond* the LLC (encoding ceiling `65536 × chunk_size`, bounded by the 1 GiB `MAX_ARENA_CAPACITY` safety cap — 1 GiB ≫ the reference host's 30 MiB L3). The cold-DRAM regime the `>10× at σ≤0.05` hypothesis depends on is therefore now reachable for the first time, and the §10.3 re-benchmark was run against it (`bench_predicate_scan_cold_dram_large`, a 260 MiB / **8.7× LLC** arena, keys inserted in shuffled order so an ordered key-scan hits arena offsets randomly and defeats the hardware prefetcher).

  **Measured finding — the columnar pushdown degenerates over a >LLC arena, because `ArenaLong` carries no hot metadata.** `ArenaLong` slots report `hot_meta = 0` (all 56 non-tag bits address the chunk/offset; see `blobmap.rs` "Inline / `ArenaLong` metadata"). A >LLC arena is therefore ~94% `ArenaLong` (only the first 16 MiB stays `ArenaShort` with metadata), so a `meta <= threshold` predicate matches every spilled entry regardless of σ. The re-benchmark's realized columnar match-rate confirms this exactly *(measured: this engine at `cbacf46d`; match count is arch-independent, so no timing host needed)*:

  | σ | columnar match-rate | a working hot-meta filter would match |
  |---|---:|---:|
  | 0.001 | **93.9%** (246 027 / 262 144) | 0.1% |
  | 0.05 | 94.1% | 5.0% |
  | 0.20 | 95.1% | 20.0% |
  | 1.0 | 100.0% | 100.0% |

  The 93.9% at σ=0.001 matches the `ArenaLong` fraction `(260−16)/260 = 93.85%` almost exactly (≈246 011 metadata-less `ArenaLong` entries all match `0 ≤ threshold`, plus ~16 genuine `ArenaShort` matches). The pushdown thus touches ~94% of payloads at *every* σ — it cannot beat the naive full-deref arm (bounded ≈ `1/0.94 ≈ 1.06×`), so no timing run is warranted on the current engine.

  **Verdict (RFC §10.3, second revision):** #276 removed the *size* ceiling but exposed a *metadata* ceiling. The metadata-bearing slots (`ArenaShort`) are exactly the 16 MiB warm ones; the size-unbounded cold slots (`ArenaLong`) have none — the two requirements for `>10×` (arena > LLC **and** a selective hot-meta filter) are in direct tension in the current encoding. Reaching `>10×` is now gated on a **metadata-carrying wide encoding** (a columnar hot-meta sidecar indexed parallel to the arena, decoupled from the 64-bit value-slot word) — tracked separately and in progress — **and** still on the ordered-scan fast path to lower the traversal floor. The `>10×` / `>15×` / `82%` figures in this RFC's §Overview remain **targets, not measured results**. The re-benchmark harness (`bench_predicate_scan_cold_dram_large`) is committed and ready; the definitive cold-DRAM *timing* sweep is deferred until the metadata-carrying encoding lands, then run on a quiet dedicated host (interleaved A/B arms, load snapshots per `docs/BENCHMARKING.md`).

  **Verdict (RFC §10.3, RESOLVED — `>10×` target MET by `ArenaMeta` alone) — pre-#355 row; re-measured below after #355** *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu; commit `c6f234cc`; `bench_predicate_scan_cold_dram_large`, 260 MiB / 8.7× LLC arena; 3 interleaved reps, loads 0.13–0.81, naive control stable ~8.9 ms)*. Phase 1 (`ArenaMeta`, #287) made metadata available across the whole `>`LLC arena — the match-rate now tracks σ exactly (0.1% at σ=0.001, was 93.9%), so the columnar arm skips cold payloads for non-matches. Measured columnar-vs-naive cold-DRAM speedup:

  | σ | columnar | naive | speedup |
  |---|---:|---:|---:|
  | 0.001 | ~404 µs | ~8.93 ms | **~22×** (21.7–23.2× across reps) |
  | 0.05 | ~869 µs | ~8.97 ms | **~10.3×** (worst-case within-rep ≥ 10.1×) |
  | 0.20 | ~1.64 ms | ~8.91 ms | ~5.4× |
  | 1.0 | ~9.33 ms | ~8.92 ms | ~1.0× |

  *(Pre-#355 row — tagged historical record; verdict corrected by the re-measured row below.)* **Against this run's Phase-1-based baseline — which carried the #355 redundant re-descent — the wide-offset metadata-carrying `ArenaMeta` arena measured 10.3× at σ=0.05 and 22× at σ=0.001**, which read as clearing `>10×` at σ≤0.05. The honest post-#355 re-measurement below corrects that: the `>10×` target is **met at σ≤0.01 only** (10.7× at σ=0.001; σ=0.05 is 6.37× against a baseline that no longer carries the re-descent). These pre-#355 numbers are retained as the tagged record. Both prior verdicts (and this document's earlier `~4–5×` / "traversal-floor bounded" predictions) were **wrong**: they extrapolated the *warm*-arena 1.4× ceiling into the cold regime, assuming the key-ordered trie walk was the floor. It is not — in the cold regime the in-slot-meta trie walk costs ~1.5 ns/entry while a cold payload fetch is ~34 ns, so skipping 99.9% of payloads yields ~22× **with no SIMD and no columnar sidecar/leaf**. The "second requirement" (an ordered-scan fast path / `BlobLeafVector`) is **not needed** to reach the target — see §5.5.10. The `82%`-DRAM-traffic figure was not measured directly (this bench times the scan; it does not count DRAM bytes) but is consistent with the timing — the columnar arm avoids the cold payload fetches that dominate the naive arm's ~8.9 ms at low σ. Only the higher-σ regime (σ ≥ 0.2, where most rows match and most payloads must be touched anyway) stays below 10×, as expected. Single bench, 3 reps; a full multi-seed traffic-counter study is out of scope for the go/no-go.

  **Verdict (RFC §10.3, RESOLVED — target met at σ=0.001 only (10.7×), NOT met at σ=0.05 (6.37×) against the honest control; the earlier ~10.3× / ~22× ratios were measured against a control that carried the #355 re-descent)** *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit `4f2f3a18`; `bench_predicate_scan_cold_dram_large`, 260 MiB / 8.7× LLC arena; interleaved A/B, 3 rounds, load ~1.0 throughout; round-to-round spread sub-percent)*. Both arms of this bench call `scan_filtered`, so the naive control (`naive_row_deref`) itself paid the #355 redundant `get()` re-descent per entry — its ~8.9 ms above is **not** an honest touch-all floor. With #355 landed (payload resolved from the slot the range walk already holds, no per-match re-descent) the naive touch-all floor drops to **~4.05 ms**, and the columnar arm drops correspondingly:

  | σ | columnar (post-#355) | naive touch-all (post-#355) | honest speedup | pre-#355 speedup |
  |---|---:|---:|---:|---:|
  | 0.001 | 378.2 µs | 4.051 ms | **10.7×** | 22× |
  | 0.05  | 634.7 µs | 4.045 ms | **6.37×** | 10.3× |
  | 0.20  | 964.2 µs | 4.040 ms | 4.19× | 5.4× |
  | 1.0   | 4.068 ms | 4.199 ms | 1.03× | 1.0× |

  **Status against the pre-registered `>10× at σ≤0.05` target: NOT met at σ=0.05.** The honest columnar-vs-naive speedup is lower than the pre-#355 row reported, because that row's denominator double-counted the descent the fix removes from *both* arms. The target is cleared at **σ=0.001 (10.7×)** but at **σ=0.05 the honest advantage is 6.37×**, not 10.3× — the pushdown genuinely skips ~95% of cold payloads there, but against an honest touch-all baseline that is ~6.4×; the extra headroom in the old number was shared re-descent overhead inflating the denominator. The exact ≥10× crossover lies **between σ=0.001 and σ=0.05 and was not pinned by this sweep** (the cold-DRAM bench samples σ ∈ {0.001, 0.05, 0.20, 1.0}), so "met at σ≤0.01" is inferred, not measured. Absolute columnar scan time also improved once matches no longer re-descend (σ=1.0: **8.86 ms → 4.07 ms**; at σ=1.0 the columnar arm now equals the naive touch-all floor within noise, 4.068 vs 4.199 ms; workload: `workload_large_values`). No get/insert path was touched (CI Callgrind zero-regression clean); the `naive_unfiltered_deref` `get()`-loop arm of `predicate_scan_selectivity_sweep` is unchanged A vs B (1.00×), confirming the change is localized to `scan_filtered`. See §5.6.5 / #355.

  > ⚠️ **Harness methodology update (#454):** The historical `bench_inline_vs_heap_small_blobs` and `bench_predicate_scan_selectivity_sweep` benchmarks were updated in #454 to dereference payload bytes (`view[0]` / `vec[0]`) and isolate container teardown (`b.iter_batched`), ensuring realistic DRAM cache-line fills and clean timed regions. The primary cold-DRAM gate above (`bench_predicate_scan_cold_dram_large`) already dereferenced payload bytes per #355.
- **`arena_compaction_churn`** — compaction over 20k entries: ~475 µs after a 50%-delete churn, ~200 µs after 80%-delete (informational; no target).

---

## 11. References & Prior Art

1. **Baskins, D.** (2002). *Judy IV Shop Manual*. HP Laboratories.
2. **Silverstein, A.** (2002). *Judy Arrays: Fast and Space-Efficient Trie Variants*.
3. **Boehm, H.-J.** (2012). *Can Seqlocks Get Along With Programming Language Memory Models?* ACM SIGPLAN.
4. **Boncz, P. A., Manegold, S., & Kersten, M. L.** (1999). *Database Architecture Optimized for the Operating System: The Design and Implementation of MonetDB*.
5. **Lemire, D., & Boytsov, L.** (2015). *Decoding billions of integers per second through vectorization*. Software: Practice and Experience.
