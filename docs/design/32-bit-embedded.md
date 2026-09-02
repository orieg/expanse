# Design: 32-Bit Embedded Architecture

**Status**: Implemented (core trie engine) — 2026-08-23  
**Author**: Expanse Core Architecture Team  
**Issue**: [#109](https://github.com/orieg/expanse/issues/109)  
**Target Milestone**: Expanse v0.4.0  
**Affected Crates**: `expanse-trie` (`crates/expanse`), `expanse-capi` (`crates/expanse-capi`)  
**Canonical Documentation**: `docs/design/32-bit-embedded.md` (Design context for `docs/ARCHITECTURE.md`, `docs/COMPAT.md`, and `docs/DATABASE.md`)

---

## 0. Implementation Status

The BTree-backed placeholders for `ExpanseSet32` / `ExpanseMap32` have been
replaced by a **real 256-ary digital trie** (`crates/expanse/src/trie32.rs`),
verified by a differential oracle against `BTreeSet`/`BTreeMap`, a drain
leak-check, cross-compilation for RV32/Cortex-M, 32-bit test execution on
`i686`, and coverage-guided fuzzing (`set32_ops` / `map32_ops`).

| RFC item | Status | Notes |
|---|---|---|
| §3 4-level byte-digit descent (`L4 → L1`) | **Shipped** | `trie32` descent |
| §3.2 `Key32`, `digit32`, level constants | **Shipped** | `types32.rs` |
| §4 8-byte `Edge32` descriptor + immediates | **Shipped** | word 0 holds a 32-bit **arena handle** (see deviation below), not a raw pointer |
| §4.3 In-edge immediates | **Shipped** | set: up to 7/3/2/1 keys by width; map: single entry (`kb ≤ 3`) |
| §5.1–5.2 Linear branches `BranchL2_32` / `BranchL6_32` | **Shipped** | `node32.rs`; grow/demote ladder in `trie32` |
| §5.4 Uncompressed branch `BranchU32` (2080 B) | **Shipped** | `node32.rs` (50% vs 64-bit `BranchU`) |
| §5.5 Set bitmap leaf `LeafBitmap1_32` (64 B) | **Shipped** | dense level-1 sets |
| §6 Variable-length linear leaves + `cap_class` | **Shipped** | set `[keys]`, map `[values][keys]` |
| §5.3 Bitmap branch `BranchB32` | **Shipped** | `node32.rs` (96 B, 8 subarrays, Band 1 / Band 2 hysteresis in `trie32`) |
| §5.5 Map bitmap leaf `LeafBitmapL_32` | **Shipped** | `node32.rs` (96 B, 8 value subarrays, Band 16 hysteresis in `trie32`) |
| §7 `ValueSlot32` arena/hot-metadata mode | **Shipped** | zero-heap inline storage (`<= 3` bytes) + 12-bit slab arena with freelist recycling in `blobmap32.rs` |
| §8 `SlabPage32` custom allocator / freelist classes | **Not yet** | arena uses the global allocator via `alloc` |
| §9 `SeqVersion32` / OCC primitives | **Shipped** | `occ32.rs` provides 32-bit atomic seqlock version word and node-level bracketing; sampling is bounded (`try_sample`), never spinning, per the interrupt-handler contract |
| §9/§10 concurrent wrapper (`sync32`) | **Shipped (point ops)** | `SyncExpanseMap32`/`SyncExpanseSet32`: single writer enforced at compile time (`split(&mut)` + non-clonable handle — no mutex, no CAS, load/store+fence only, so it runs on `riscv32imc`), validated optimistic `try_get`/`try_contains`/`try_len` returning `Busy` instead of spinning, fixed-capacity arena with deferred reclamation drained at reader quiescence, `ArenaFull`/`ReclaimBacklog` refused *before* the tree is touched. Ordered scans/iteration on the concurrent surface are not yet exposed. The fixed arena is opt-in on this wrapper only — the single-threaded engines keep §2.1 expanse-proportional memory |
| §11 32-bit `Judy*` C ABI drop-in | **Not shipped** | A 32-bit `libexpanse` exports **no** `Judy*` symbols. `ExpanseMap32`/`ExpanseSet32` have no rank/select (`count_below`/`by_count`) and no value-slot accessors, so `Judy1ByCount`, `JudyLIns` and their siblings have nothing to translate to; `JudySL*`/`JudyHS*` have no 32-bit container at all. Symbols are absent rather than stubbed. Surface matrix: [COMPAT.md](../COMPAT.md#build-configuration-surface-matrix) |
| §12/§13 QEMU runners, ESP-IDF component | **Shipped** | ESP-IDF component in `components/expanse/` builds and links `libexpanse.a` for the bare-metal RISC-V target matching `IDF_TARGET`; CI cross-compilation matrix covers engine + C ABI staticlib. RISC-V parts only — the Xtensa ESP32/S2/S3 have no mainline rustc target. The CMake integration itself is not exercised in CI (no ESP-IDF lane) |
| Published density numbers | **Shipped** | `bytes_per_key_32` reports real measured B/key. These are deterministic `mem_used()/N` byte-accounting values — machine-independent and load-immune, so no quiet-host run applies. Published in `docs/visualizer_data.json` and recomputed from the engine by `tests/test_visualizer_sync.rs`, so a layout change fails CI rather than silently invalidating the figure ([#384](https://github.com/orieg/expanse/issues/384)) |

**Deviation — handle vs raw pointer.** The RFC describes `Edge32` word 0 as
a raw child pointer. A real heap pointer does not fit in a `u32` on the
64-bit hosts where this crate's tests, Miri, and differential fuzzing run,
so the engine stores nodes in a per-tree arena and keeps a 32-bit **handle**
(arena index) in word 0. This is pointer-width-independent (identical on
`i686`/RV32 and the 64-bit host), needs no `unsafe`, and — since each node
is a real heap allocation sized to the RFC's on-target byte layout — makes
`mem_used()` an exact figure. The 8-byte `Edge32` size, tag scheme, and
immediate packing are unchanged.

---

## 1. Executive Summary & Architectural Vision

Expanse is a clean-room, cache-line-optimized reimplementation of Judy arrays designed for high-density integer indexing, ordered associative mapping, and zero-allocation traversal. While Expanse v0.1.0–v0.3.0 established industry-leading performance on 64-bit server and desktop architectures (`x86-64`, `AArch64`, `RV64GC`), modern industrial automation, IoT gateways, automotive controllers, and edge networking devices increasingly deploy 32-bit RISC-V (`RV32I`, `RV32EMAC`), Espressif Xtensa/RISC-V (`ESP32`, `ESP32-S3`, `ESP32-C3`), and ARM Cortex-M (`Cortex-M0+/M3/M4/M7/M33`) microprocessors.

In these embedded environments, system memory is severely constrained—often limited to 64 KiB – 512 KiB of internal static RAM (SRAM) and 2 MiB – 16 MiB of external SPI/QSPI PSRAM. Traditional B-trees and hash tables incur unacceptable memory fragmentation and pointer overhead (e.g. 24–32 bytes of allocator metadata and pointer overhead per key-value entry).

This design establishes a unified 32-bit architecture for Expanse, scaling the digital trie from 64-bit servers down to embedded microcontrollers with **zero algorithmic compromises**:

1. **4-Level Digital Tree Hierarchy**: Keys shrink from 64-bit (`Key = u64`, Levels 8 $\rightarrow$ 1) to 32-bit (`Key = u32`, Levels 4 $\rightarrow$ 1), halving maximum descent depth from 8 hops to 4 hops and cutting lookup latency by up to $48\%$.
2. **Compact 8-Byte `Edge` Descriptor (`Edge32`)**: Replacing 16-byte edges with an 8-byte tagged union (`4B Pointer/Imm` + `3B Level-Split Aux/Pop0/Decode` + `1B Tag`), achieving an immediate **$50\%$ reduction in structural memory**.
3. **Immediate In-Edge Packing up to 7 Bytes**: Packing up to 7 1-byte keys, 3 2-byte keys, or 2 3-byte keys directly inside a single 8-byte edge without heap allocation.
4. **Polymorphic 32-Bit Value Slots (`ValueSlot32`) & Large-Value Integration (#112)**:
   - **Inline Mode ($\le 3$ bytes)**: Direct 24-bit payload packing in the slot with zero heap allocations.
   - **Arena Mode**: 12-bit hot metadata (TTL, status flags, sensor threshold) + 12-bit slab offset (4 KiB direct / 64 KiB aligned chunks) + the 8-bit tag — 12 + 12 + 8 = 32 bits exactly. (Earlier drafts of this document said 16-bit metadata, which does not fit alongside a 12-bit offset and an 8-bit tag; the shipped widths are gated in [ARCHITECTURE.md §10.5](../ARCHITECTURE.md#105-valueslot--the-8-byte-polymorphic-value-word).)
   - **Raw Word Mode (`0xFF`)**: Transparent 32-bit C ABI drop-in compatibility for classic `JudyL` on 32-bit architectures.
5. **Microarchitecture-Aware 32-Byte / 64-Byte Cache Alignment**: Optimized node geometries mapped directly to 32-byte cache lines (Cortex-M7, ESP32 cache) and tightly-coupled un-cached SRAM.
6. **`#![no_std]` & Embedded Concurrency**: Replacing 64-bit atomics with `AtomicU32` / `AtomicUsize` for base `RV32I` architectures, with pluggable support for FreeRTOS `pvPortMalloc` and ESP-IDF `esp_heap_caps_malloc`.

```
========================================================================================
64-Bit Server Architecture vs. 32-Bit Embedded Architecture
========================================================================================

           64-Bit Server (x86-64 / ARM64 / RV64)          32-Bit Embedded (RV32 / ESP32 / Cortex-M)
           ------------------------------------          ----------------------------------------
Key Type:  u64 (8 Bytes)                                 u32 (4 Bytes)
Tree Depth:8 Levels (L8 -> L1)                           4 Levels (L4 -> L1)
Edge Size: 16 Bytes                                      8 Bytes (-50% Structural RAM)
Cache Line:64 Bytes / 128 Bytes                          32 Bytes / 64 Bytes / Un-cached SRAM
Value Slot:64-bit (<=7B inline, 24-bit meta)              32-bit (<=3B inline, 12-bit meta)
Atomics:   AtomicU64 SeqVersion                          AtomicU32 SeqVersion32 (Native RV32A)
Max Heap:  Exabytes (Virtual Addressing)                 128 KiB - 16 MiB (Strict Physical SRAM)
```

---

## 2. Problem Statement & Embedded Hardware Realities

### 2.1 The Embedded Memory Wall & Allocator Fragmentation

In microcontroller applications (e.g. FreeRTOS, Zephyr, ESP-IDF, bare-metal `no_std`), memory is partitioned across distinct hardware regions:
- **Tightly Coupled Memory (TCM / DTCM / Internal SRAM)**: 64 KiB – 512 KiB, 0 wait-states (single-cycle access), highly scarce.
- **External SPI / QSPI / Octal PSRAM**: 2 MiB – 16 MiB, high latency (10–30 cycles per read burst), power-hungry.

When storing lookup tables, routing matrices, or sparse device states:
- `std::collections::BTreeMap` / `alloc::collections::BTreeMap`: Allocates node headers with minimum 16–32 entries per node, creating massive internal fragmentation when sparse ($>100\text{ bytes/key}$).
- `hashbrown::HashMap` / Swiss Tables: Requires power-of-two table resizing, 1-byte control bytes, and 8-byte bucket alignment, doubling memory usage during resizes and triggering out-of-memory (OOM) aborts.
- `libjudy` (Original C): Assumed 32-bit and 64-bit word parity but used a monolithic 2001 chunk allocator with fixed 128-byte chunk buckets that violently fragment internal microcontroller SRAM.

### 2.2 Microarchitectural Cache Line Constraints (32B vs. 64B)

Server-grade processors enforce uniform 64-byte cache lines. In embedded microarchitectures:
1. **ARM Cortex-M7 / Cortex-M55**: L1 D-Cache line size is **32 bytes** (4 words $\times$ 64 bits or 8 words $\times$ 32 bits).
2. **Espressif ESP32-S3 / ESP32-C3**: Flash/PSRAM MMU Cache line size is **32 bytes**.
3. **RISC-V Synthesizable Cores (Rocket, SiFive E-Series, Andes N25)**: Configurable **32-byte or 64-byte** I/D cache lines.
4. **Internal SRAM (DTCM / IRAM / DRAM)**: Cache-less, single-cycle flat byte-addressable array. Misaligned 32-bit word accesses incur hardware trap exceptions on older M0/RV32I cores.

Therefore, 32-bit Expanse node layouts **must align cleanly to both 32-byte and 64-byte boundaries**, ensuring a node fetch never straddles multiple cache bursts.

*Measured (Cortex-M7 half): the first on-target run on an STM32H747I-DISCO read the D-cache geometry from CCSIDR as 4-way × 128 sets × 32-byte lines and measured every fixture with the cache off and on at two core:bus ratios — see `docs/BENCHMARKING.md` "Cortex-M7 on-target" (#598). The ESP32 and Cortex-M55 halves remain unmeasured.*

### 2.3 Atomics & Instruction Set Extensions (RV32I vs. RV32A)

Standard Rust 64-bit OCC concurrency (`crates/expanse/src/occ.rs`) uses `AtomicU64` for the tree-level `SeqVersion`. 
On 32-bit architectures:
- **RV32I (Base Integer)**: No hardware atomic instructions. Concurrency requires disabling interrupts (`cpsid`/`cpsie` on ARM, `csrci mstatus, 8` on RISC-V) or mutex emulation.
- **RV32A / RV32AC (Atomic Extension)**: Provides 32-bit LR/SC (`lr.w`, `sc.w`) and AMO (`amoadd.w`, `amoswap.w`). **Zero hardware support for 64-bit atomics (`lr.d`, `sc.d`)**.
- **Cortex-M0 / M0+**: No `LDREX`/`STREX` (single-core only, critical-section based).
- **Cortex-M3 / M4 / M7 / M33**: 32-bit `LDREX`/`STREX` supported; 64-bit `LDREXD`/`STREXD` available only on select v7-M/v8-M cores.

**Per-part Espressif RISC-V facts (datasheet-sourced, not inferred).** Which side of the RV32I/RV32A split each shipped Espressif part falls on — and how many harts share its address space, which decides whether `portable-atomic`'s `unsafe-assume-single-core` is sound — is established from the Espressif datasheets and Technical Reference Manuals in **`docs/HARDWARE.md` §4.3**, with verbatim quotes, document revisions and sections. Summary: **ESP32-C2 and ESP32-C3 are RV32IMC** (ESP8684 TRM v1.3 / ESP32-C3 TRM v1.4, `misa` "Atomic Extension = 0") and **single-hart**, so they have no hardware CAS and `unsafe-assume-single-core` is the sound way to obtain one; **ESP32-H2 is RV32IMAC and single-hart**; **ESP32-C6 is RV32IMAC with a second, LP RISC-V hart that reads and writes HP SRAM** (ESP32-C6 TRM v1.2 §3.7.1); **ESP32-P4 is a dual-core RV32IMAFC HP complex plus an LP hart reaching L2 SRAM** (ESP32-P4 TRM v0.7 §2.1, §5.8.1). `unsafe-assume-single-core` is therefore **unsound on C6 and P4** and must never be enabled family-wide. No part has a native 64-bit atomic — `riscv32imac` and `riscv32imafc` both emit `target_has_atomic` 8/16/32/ptr and no 64 — which is why `SeqVersion32` below is not merely an optimization but the only available construction ([#564](https://github.com/orieg/expanse/issues/564)).

Requiring `AtomicU64` forces LLVM to link `libatomic`, which fails in `#![no_std]` embedded firmware. A 32-bit optimized seqlock (`SeqVersion32`) backed by `AtomicU32` is mandatory.

**Bit-manipulation (Zbb) & the hot `trie32` bit-count path.** Base RV32I/RV32IMAC has **no** hardware population-count or count-leading/trailing-zeros instruction; `u32::count_ones` / `leading_zeros` / `trailing_zeros` — used across the `trie32` leaf/bitmap kernels (`crates/expanse/src/bits.rs`, `trie32.rs`) — lower to a ~12-instruction SWAR sequence (popcount) or a software CLZ/CTZ loop. The **Zbb** extension (Bitmanip v1.0.0, ratified Nov 2021) adds single-instruction `cpop`, `clz`, and `ctz`, whose zero-input semantics (return XLEN) already match Rust's. On Zbb-capable RISC-V hardware, build with `-C target-feature=+zbb` so these sites emit one instruction apiece instead of the software fallback. The fallback stays intact and is the default for the shipped `riscv32imac-unknown-none-elf` config and for pre-Zbb embedded parts — `+zbb` is an additive build profile, not a requirement. See `docs/HARDWARE.md` §3.1 (spec citations) and §6 (missed-opportunity analysis); a `test-rv32-zbb` CI lane builds the core with `+zbb` to keep the profile compiling.

---

## 3. Digital Tree Hierarchy: 64-Bit vs. 32-Bit

### 3.1 Depth & Expanse Reduction

Expanse decodes keys one 8-bit byte ("digit") per level.
- **64-Bit Key**: $8\text{ levels}$ ($\text{Level } 8 \rightarrow \text{Level } 1$). Maximum descent = 8 branches.
- **32-Bit Key**: $4\text{ levels}$ ($\text{Level } 4 \rightarrow \text{Level } 1$). Maximum descent = 4 branches.

```
64-Bit Tree Hierarchy (8 Levels):
Key: [ Byte 7 (L8) | Byte 6 (L7) | Byte 5 (L6) | Byte 4 (L5) | Byte 3 (L4) | Byte 2 (L3) | Byte 1 (L2) | Byte 0 (L1) ]
      └─ Root (L8) ──> Branch (L7) ──> ... ──> Branch (L2) ──> Leaf (L1)

32-Bit Tree Hierarchy (4 Levels):
Key: [ Byte 3 (L4) | Byte 2 (L3) | Byte 1 (L2) | Byte 0 (L1) ]
      └─ Root (L4) ──> Branch (L3) ──> Branch (L2) ──> Leaf (L1)
```

### 3.2 Digit Extraction & Level Constants

```rust
// crates/expanse/src/types32.rs

/// 32-bit Machine Key
pub type Key32 = u32;

/// 32-bit Machine Value
pub type Value32 = u32;

/// Maximum decode level for 32-bit architecture
pub const MAX_LEVEL_32: u8 = 4;

/// Cache line sizing options
pub const CACHE_LINE_32: usize = 32;
pub const CACHE_LINE_64: usize = 64;

/// Extracts the decode digit for `key` at `level` (1..=4).
/// Level 4 consumes the most significant byte [31:24]; Level 1 consumes [7:0].
#[inline(always)]
#[must_use]
pub const fn digit32(key: Key32, level: u8) -> u8 {
    debug_assert!(level >= 1 && level <= MAX_LEVEL_32);
    (key >> ((level - 1) * 8)) as u8
}
```

---

## 4. Compact 8-Byte `Edge` Descriptor (`Edge32`)

### 4.1 Memory Layout & Bit Allocations

In 64-bit Expanse, `Edge` is 16 bytes. For 32-bit targets, `Edge32` is compressed to **8 bytes** with zero padding:

```
========================================================================================
8-Byte Edge32 Bit Layout
========================================================================================

Offset 0..3 (4 Bytes): Word 0
+---------------------------------------------------------------------------------------+
| Child Node Raw Pointer (*mut u8)  OR  Immediate Key Payload (4 Bytes)                 |
| [31:0] (32 bits)                                                                      |
+---------------------------------------------------------------------------------------+

Offset 4..6 (3 Bytes): Level-Split Aux Field
+---------------------------------------------------+-----------------------------------+
| Narrow Pointer Decode Bytes (High 3 - L Bytes)    | Subtree Pop0 (Low L Bytes)        |
| [23 : L*8]                                        | [L*8 - 1 : 0]                     |
+---------------------------------------------------+-----------------------------------+

Offset 7 (1 Byte): Tag Discriminant
+---------------------------------------------------------------------------------------+
| Edge Tag (Structural: 0x00..=0x0C, 0x7F | Immediate: 0x10..=0x36)                     |
| [7:0] (8 bits)                                                                        |
+---------------------------------------------------------------------------------------+
```

### 4.2 The 3-Byte Level-Split Auxiliary Field

In a 32-bit trie, a child subtree at level $L \in \{1, 2, 3\}$ has maximum key capacity:
- **Level 1 child**: Covers $256^1 = 256$ keys $\implies \text{pop0} \le 255$ fits in **1 byte** (`aux[0]`). Decode budget = $3 - 1 = \mathbf{2\text{ bytes}}$.
- **Level 2 child**: Covers $256^2 = 65,536$ keys $\implies \text{pop0} \le 65,535$ fits in **2 bytes** (`aux[0..2]`). Decode budget = $3 - 2 = \mathbf{1\text{ byte}}$.
- **Level 3 child**: Covers $256^3 = 16,777,216$ keys $\implies \text{pop0} \le 16,777,215$ fits in **3 bytes** (`aux[0..3]`). Decode budget = $\mathbf{0\text{ bytes}}$.
- **Level 4 root**: Covers $256^4 = 4,294,967,296$ keys (stored at tree root level).

The 3-byte auxiliary field perfectly accommodates both $\text{pop0}$ and narrow-pointer decode skipping across all 32-bit levels without overflowing a single bit.

### 4.3 Immediate Edge Packing in `Edge32`

An 8-byte `Edge32` offers 7 contiguous payload bytes (`Word0` [4B] + `Aux` [3B]) for immediate keys:

$$\text{IMMED\_PAYLOAD\_BYTES}_{32} = 7\text{ bytes}$$

| Undecoded Key Width ($K_B$) | Max Keys in `Edge32` ($\lfloor 7 / K_B \rfloor$) | Set Immediate Capacity | Map Immediate Capacity (Aux 3B) |
|---|---|---|---|
| **1 Byte** (Level 1) | $\lfloor 7 / 1 \rfloor = \mathbf{7\text{ keys}}$ | 7 Keys (0 heap allocs) | 3 Keys + 1 Value Array Ptr |
| **2 Bytes** (Level 2) | $\lfloor 7 / 2 \rfloor = \mathbf{3\text{ keys}}$ | 3 Keys (0 heap allocs) | 1 Key + Direct Value in W0 |
| **3 Bytes** (Level 3) | $\lfloor 7 / 3 \rfloor = \mathbf{2\text{ keys}}$ | 2 Keys (0 heap allocs) | 1 Key + Direct Value in W0 |

```rust
// crates/expanse/src/node32.rs

#[derive(Clone, Copy)]
#[repr(C)]
union Word0_32 {
    ptr: *mut u8,
    imm: [u8; 4],
}

/// The uniform 8-byte tagged edge descriptor for 32-bit embedded targets.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Edge32 {
    w0: Word0_32,
    aux: [u8; 3],
    tag: u8,
}

const _: () = {
    use core::mem::{size_of, align_of, offset_of};
    assert!(size_of::<Edge32>() == 8, "Edge32 must be exactly 8 bytes");
    assert!(align_of::<Edge32>() == 4, "Edge32 must be 4-byte aligned");
    assert!(offset_of!(Edge32, aux) == 4);
    assert!(offset_of!(Edge32, tag) == 7);
};
```

---

## 5. 32-Bit Node Geometries & Cache Models

To support both 32-byte cache line embedded microcontrollers (Cortex-M7, ESP32) and standard 64-byte systems, node structures are defined with exact compile-time sizing guarantees:

### 5.1 Linear Branch Headers (`BranchHeader32`)

```rust
/// Compact 8-byte header for 32-bit linear branches.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BranchHeader32 {
    /// OCC Version counter (even = stable, odd = mutating)
    pub version: u32,
    /// Populated child edge count
    pub num: u8,
    /// Node's current level (1..=4)
    pub level: u8,
    /// 16-bit presence bloom filter: 1 << (digit & 0x0F)
    pub presence: u16,
}
```

### 5.2 Linear Branch Flavors (`BranchL2_32` & `BranchL6_32`)

1. **`BranchL2_32` (32 Bytes — Exactly 1x 32-byte Cache Line)**:
   - Header (8B) + 4 sorted digits (4B) + 4B pad + 2 `Edge32` (16B) = **32 Bytes**.
   - Ideal for ultra-compact subtrees in internal microcontroller SRAM.
2. **`BranchL6_32` (64 Bytes — Exactly 2x 32-byte Lines / 1x 64-byte Line)**:
   - Header (8B) + 8 sorted digits (8B) + 6 `Edge32` (48B) = **64 Bytes**.
   - Direct SIMD / SWAR digit scanning across 8 bytes.

```rust
#[derive(Debug)]
#[repr(C, align(32))]
pub struct BranchL2_32 {
    pub hdr: BranchHeader32,
    pub digits: [u8; 4],
    pub _pad: [u8; 4],
    pub edges: [Edge32; 2],
}

#[derive(Debug)]
#[repr(C, align(32))]
pub struct BranchL6_32 {
    pub hdr: BranchHeader32,
    pub digits: [u8; 8],
    pub edges: [Edge32; 6],
}
```

### 5.3 Bitmap Branch (`BranchB32`) (96 Bytes — Exactly 3x 32-byte Lines)

```rust
#[derive(Debug)]
#[repr(C, align(32))]
pub struct BranchB32 {
    /// 256-bit presence bitmap (32 bytes)
    pub bitmap: Bitmap256,
    /// 8 subarray pointers to packed Edge32 arrays (8 * 4B = 32 bytes)
    pub subarrays: [*mut Edge32; 8],
    /// Cached population counts for fast rank/count (8 * 2B = 16 bytes)
    pub pop_counts: [u16; 8],
    /// OCC Version counter (4 bytes)
    pub version: u32,
    /// Node level (1 byte)
    pub level: u8,
    /// Alignment padding to 96 bytes (11 bytes)
    pub _pad: [u8; 11],
}

const _: () = {
    use core::mem::size_of;
    assert!(size_of::<BranchB32>() == 96, "BranchB32 must be exactly 96 bytes (3x 32B cache lines)");
};
```

### 5.4 Uncompressed Branch (`BranchU32`) (2080 Bytes — 50% Reduction vs. 64-Bit)

- 64-bit `BranchU`: 256 edges $\times 16\text{B} + 64\text{B} = \mathbf{4160\text{ bytes}}$.
- 32-bit `BranchU32`: 256 edges $\times 8\text{B} + 32\text{B} = \mathbf{2080\text{ bytes}}$.

Saving over **2 KiB per uncompressed branch** is decisive on microcontrollers with $\le 256\text{ KiB}$ SRAM.

```rust
#[derive(Debug)]
#[repr(C, align(32))]
pub struct BranchU32 {
    pub version: u32,
    pub _pad: [u8; 28],
    pub edges: [Edge32; 256],
}

const _: () = {
    use core::mem::size_of;
    assert!(size_of::<BranchU32>() == 2080, "BranchU32 must be exactly 2080 bytes");
};
```

### 5.5 Bitmap Leaves (`LeafBitmap1_32` & `LeafBitmapL_32`)

- **`LeafBitmap1_32` (Set Flavor, 64 Bytes = 2x 32B Lines)**:
  `Bitmap256` (32B) + `version: u32` (4B) + 28B padding = 64 Bytes.
- **`LeafBitmapL_32` (Map Flavor, 96 Bytes = 3x 32B Lines)**:
  `Bitmap256` (32B) + `values: [*mut u32; 8]` (32B) + `version: u32` (4B) + 28B padding = 96 Bytes.

---

## 6. Variable-Length Linear Leaves for 32-Bit Targets

In a 32-bit trie, linear leaf key remainders are at most **3 bytes** ($K_B \in \{1, 2, 3\}$).

### 6.1 Sizing Formulas

- **Set Leaf**: `[keys: KB * pop]`
  $$\text{Size}_{\text{Set32}}(K_B, \text{pop}) = K_B \times \text{cap\_class}(\text{pop})$$
- **Map Leaf**: `[values: u32 * pop][keys: KB * pop]`
  $$\text{Size}_{\text{Map32}}(K_B, \text{pop}) = 4 \times \text{cap\_class}(\text{pop}) + K_B \times \text{cap\_class}(\text{pop})$$

```rust
// crates/expanse/src/leaf32.rs

#[inline(always)]
#[must_use]
pub const fn size_set32(key_bytes: u8, pop: usize) -> usize {
    key_bytes as usize * crate::leaf::cap_class(pop)
}

#[inline(always)]
#[must_use]
pub const fn size_map32(key_bytes: u8, pop: usize) -> usize {
    4 * crate::leaf::cap_class(pop) + (key_bytes as usize * crate::leaf::cap_class(pop))
}

#[inline(always)]
#[must_use]
pub const fn map_keys_offset32(pop: usize) -> usize {
    4 * crate::leaf::cap_class(pop)
}
```

Since value slots are `u32` (4 bytes), values are **naturally 4-byte aligned** from offset 0 of the leaf allocation without padding!

---

## 7. 32-Bit Polymorphic Value Slots (`ValueSlot32`) & Large-Value Integration (#112)

Issue #112 established the polymorphic `ValueSlot` architecture on 64-bit systems. For 32-bit embedded platforms, we introduce **`ValueSlot32`**:

```
========================================================================================
32-Bit Value Slot Bit Layouts (ValueSlot32)
========================================================================================

1. Inline Mode (<= 3 Bytes Payload):
+------------------------------------+-------------------+-------------------+----------+
| Payload Byte 2                     | Payload Byte 1    | Payload Byte 0    | Tag      |
| [31:24]                            | [23:16]           | [15:8]            | [7:0]    |
+------------------------------------+-------------------+-------------------+----------+
  - Tag (0x00..=0x03) encodes exact payload length (0 to 3 bytes).
  - Stored directly in leaf memory: ZERO heap allocation, ZERO pointer dereference.

2. Embedded Arena Mode (Hot Metadata + 12-bit Arena Offset):
+------------------------------------+------------------------------+-------------------+
| Hot Metadata (TTL / Sensor Flags)  | Arena Chunk Offset (12 bits) | Tag (0x10)        |
| [31:20] (12 bits)                  | [19:8] (12 bits)             | [7:0]             |
+------------------------------------+------------------------------+-------------------+
  - 12-bit Hot Metadata: Directly filterable in registers (e.g. status code, TTL).
  - 12-bit Arena Offset: 4 KiB direct offset (or 64 KiB at 16B alignment) per slab.
  (Both widths are `ValueSlot32::ARENA_META_MASK` / `ARENA_OFFSET_MASK`, gated in
   ARCHITECTURE.md section 10.5. An earlier revision of this diagram claimed a
   16-bit metadata field at [31:16] overlapping a 12-bit offset at [15:8, 23:20],
   which is not a partition of a 32-bit word.)

3. Raw Uninterpreted 32-Bit Machine Word (JudyL C ABI Compatibility):
+---------------------------------------------------------------------------------------+
| Uninterpreted 32-bit Machine Word / Pointer (Tag arbitrary)                           |
| [31:0]                                                                                |
+---------------------------------------------------------------------------------------+
```

### 7.1 Rust Struct Definition & Bit Operations

```rust
// crates/expanse/src/slot32.rs

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SlotTag32 {
    Inline0 = 0x00,
    Inline1 = 0x01,
    Inline2 = 0x02,
    Inline3 = 0x03,
    ArenaShort = 0x10,
    Tombstone = 0xFE,
    RawWord = 0xFF,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ValueSlot32(pub u32);

impl ValueSlot32 {
    pub const TAG_MASK: u32 = 0xFF;
    pub const ARENA_OFFSET_MASK: u32 = 0x0FFF;

    #[inline(always)]
    #[must_use]
    pub fn new_inline(bytes: &[u8]) -> Option<Self> {
        let len = bytes.len();
        if len > 3 {
            return None;
        }
        let mut raw = len as u32;
        for (i, &b) in bytes.iter().enumerate() {
            raw |= (b as u32) << (8 * (i + 1));
        }
        Some(Self(raw))
    }

    #[inline(always)]
    #[must_use]
    pub fn new_arena_short(hot_meta: u16, arena_offset: u16) -> Option<Self> {
        if arena_offset > 0x0FFF {
            return None;
        }
        let raw = (SlotTag32::ArenaShort as u32)
            | (((arena_offset & 0x0FFF) as u32) << 8)
            | ((hot_meta as u32) << 20);
        Some(Self(raw))
    }

    #[inline(always)]
    #[must_use]
    pub fn tag(self) -> SlotTag32 {
        match (self.0 & Self::TAG_MASK) as u8 {
            0x00 => SlotTag32::Inline0,
            0x01 => SlotTag32::Inline1,
            0x02 => SlotTag32::Inline2,
            0x03 => SlotTag32::Inline3,
            0x10 => SlotTag32::ArenaShort,
            0xFE => SlotTag32::Tombstone,
            _ => SlotTag32::RawWord,
        }
    }

    #[inline(always)]
    #[must_use]
    pub fn inline_payload(self) -> ([u8; 3], usize) {
        let len = (self.0 & Self::TAG_MASK) as usize;
        let effective_len = len.min(3);
        let mut buf = [0u8; 3];
        let val = self.0 >> 8;
        for (i, byte) in buf.iter_mut().enumerate().take(effective_len) {
            *byte = ((val >> (8 * i)) & 0xFF) as u8;
        }
        (buf, effective_len)
    }

    #[inline(always)]
    #[must_use]
    pub fn hot_meta(self) -> u16 {
        (self.0 >> 20) as u16
    }

    #[inline(always)]
    #[must_use]
    pub fn arena_offset(self) -> u16 {
        ((self.0 >> 8) & Self::ARENA_OFFSET_MASK) as u16
    }
}
```

---

## 8. Embedded Microarchitecture Optimization & SRAM Alignment

### 8.1 Slab Page Sizing for Constrained SRAM

The 64-bit engine provisions 4 KiB `SlabPage` blocks. In embedded targets where total available SRAM may be only 128 KiB:
- **`SlabPage32`**: Scaled down to **512 Bytes** or **1 KiB** blocks.
- **Embedded Freelist Classes**: Reduced from 62 classes to **24 fine-grained classes** ($\le 128\text{ bytes}$), eliminating slab over-allocation.

### 8.2 Custom Allocator & `#![no_std]` Integration

```rust
// Custom Embedded Allocator Hook Example
#[cfg(feature = "embedded-alloc")]
pub trait EmbeddedAlloc: Send + Sync {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8;
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout);
}

// Support for FreeRTOS and ESP-IDF Heap Capabilities
#[cfg(feature = "esp-idf")]
pub struct EspIdfInternalAlloc;

#[cfg(feature = "esp-idf")]
unsafe impl EmbeddedAlloc for EspIdfInternalAlloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        // Force allocation into internal high-speed DRAM instead of slow external PSRAM
        esp_idf_sys::heap_caps_malloc(
            layout.size(),
            esp_idf_sys::MALLOC_CAP_INTERNAL | esp_idf_sys::MALLOC_CAP_8BIT,
        ).cast()
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        esp_idf_sys::heap_caps_free(ptr.cast());
    }
}
```

---

## 9. Atomics, Concurrency & `#![no_std]` Subsystems

### 9.1 `SeqVersion32`: Native 32-Bit OCC Reader Validation

To avoid dependency on unavailable 64-bit atomics, `SeqVersion32` is implemented entirely using `AtomicU32`:

```rust
// crates/expanse/src/occ32.rs

use core::sync::atomic::{AtomicU32, Ordering, fence};

/// 32-bit Seqlock Version Word for Embedded Microcontrollers
#[derive(Debug, Default)]
pub struct SeqVersion32(AtomicU32);

impl SeqVersion32 {
    #[inline(always)]
    pub const fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    #[inline(always)]
    pub fn begin(&self) {
        let v = self.0.load(Ordering::Relaxed);
        debug_assert!(v % 2 == 0, "nested or unpaired begin");
        self.0.store(v + 1, Ordering::Relaxed);
        fence(Ordering::Release);
    }

    #[inline(always)]
    pub fn end(&self) {
        let v = self.0.load(Ordering::Relaxed);
        debug_assert!(v % 2 == 1, "end without begin");
        self.0.store(v + 1, Ordering::Release);
    }

    #[inline(always)]
    pub fn sample(&self) -> u32 {
        loop {
            let v = self.0.load(Ordering::Acquire);
            if v % 2 == 0 {
                return v;
            }
            core::hint::spin_loop();
        }
    }

    #[inline(always)]
    pub fn validate(&self, snapshot: u32) -> bool {
        fence(Ordering::Acquire);
        self.0.load(Ordering::Relaxed) == snapshot
    }
}
```

---

## 10. Real-World Embedded Workloads & Case Studies

```
+------------------------------------------------------------------------------------+
| Real-World 32-Bit Microcontroller Embedded Workloads                               |
+------------------------------------------------------------------------------------+
| 1. Network MAC Table (Layer 2 Bridge):                                             |
|    - 48-bit MAC address lower 32-bit mapped directly to port & VLAN ID.           |
|    - Memory: < 1.2 bytes per MAC entry (vs. 32 bytes in std BTreeMap).             |
+------------------------------------------------------------------------------------+
| 2. IPv4 /24 Subnet Routing Table:                                                  |
|    - 32-bit IPv4 address mapped to next-hop gateway.                               |
|    - 4-level descent matches CIDR byte boundaries exactly; 15ns route lookups.     |
+------------------------------------------------------------------------------------+
| 3. CAN-Bus / CAN-FD Sparse Identifier Filter (Automotive ECU):                     |
|    - 11-bit standard / 29-bit extended CAN arbitration IDs.                        |
|    - Dynamic dispatch table with zero memory allocation inside real-time ISRs.     |
+------------------------------------------------------------------------------------+
| 4. OTA Firmware Block Verification Matrix:                                         |
|    - 32-bit Chunk Index -> 16-bit CRC / Block Status Tag in ValueSlot32.           |
|    - Zero allocation bit-tracking for constrained Flash bootloaders.               |
+------------------------------------------------------------------------------------+
```

---

## 11. C ABI Drop-In Compatibility for 32-Bit Targets

> **Status: not shipped, and not currently reachable.** A 32-bit `libexpanse`
> exports no `Judy*` symbols. What follows is the original RFC proposal, kept
> for the record.
>
> The blocker is engine surface, not packaging. `ExpanseMap32`/`ExpanseSet32`
> carry no `count_below`/`by_count` and no `get_value_slot`/`ins_slot`, so
> `Judy1ByCount`, `JudyLIns` and their siblings have no operation to forward
> to; `JudySL*`/`JudyHS*` have no 32-bit container at all. Shipping a symbol
> that links but behaves differently is worse than a link error that names the
> gap, so the whole family is gated out at 32-bit width. Reviving this section
> means adding those accessors to the 32-bit engine first.

On 32-bit platforms, `sizeof(Word_t) == 4`. The RFC proposed that `libexpanse` export the full classic Judy API compiled for 32-bit ABIs:

```c
// 32-bit Judy.h compatibility definitions
typedef uint32_t Word_t;
typedef Word_t*  PWord_t;
typedef void*    Pvoid_t;

// Drop-in Judy1 (ExpanseSet32) & JudyL (ExpanseMap32)
int   Judy1Test(Pvoid_t  PJ1Array, Word_t Index, JError_t *PJError);
int   Judy1Set(PPvoid_t PPJ1Array, Word_t Index, JError_t *PJError);
int   Judy1Unset(PPvoid_t PPJ1Array, Word_t Index, JError_t *PJError);

PWord_t JudyLGet(Pvoid_t  PJLArray, Word_t Index, JError_t *PJError);
PWord_t JudyLIns(PPvoid_t PPJLArray, Word_t Index, JError_t *PJError);
int     JudyLDel(PPvoid_t PPJLArray, Word_t Index, JError_t *PJError);
```

---

## 12. Phased Implementation Roadmap & Acceptance Gates

Development proceeds in strict, sequential engineering phases with measurable verification gates (no time estimates):

```
+---------------------------------------------------------------------------------------+
| PHASE 1: 32-Bit Types, Constants, and Edge32 Bit-Packing                              |
| - Implement `Key32`, `Edge32`, `ImmedType32`, and digit extraction in `types32.rs`    |
| - Compile-time layout assertions (`size_of::<Edge32>() == 8`)                        |
| - Gate: Unit tests green for 100% roundtrip bit encoding across all levels            |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
| PHASE 2: 32-Bit Node Geometries & Cache Line Alignment                                |
| - Implement `BranchL2_32`, `BranchL6_32`, `BranchB32`, `BranchU32`, `LeafBitmap1_32`  |
| - 32-byte / 64-byte alignment verification under Miri                                 |
| - Gate: `size_of`/`align_of`/`offset_of` const assertions pass 100%                   |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
| PHASE 3: 32-Bit Mutation & Leaf Search Engine (`ExpanseSet32` & `ExpanseMap32`)       |
| - Implement 4-level descent, insert/remove cascade ladders, and hysteresis on the bitmap-leaf rung (branch-rung hysteresis landed later, in #484)            |
| - Proptest differential verification against `BTreeSet<u32>` and `BTreeMap<u32, u32>` |
| - Gate: 10,000 randomized proptest cases pass without invariant violations            |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
| PHASE 4: Polymorphic ValueSlot32 & Embedded BlobMap (`ExpanseBlobMap32`)              |
| - Implement `ValueSlot32` with inline (<=3B) and 12-bit arena offset packing          |
| - Compact 512B/1KB `SlabPage32` freelist recycling                                    |
| - Gate: Zero memory leaks under Miri; churn compaction recovers >= 95% dead space     |
+-------------------------------------------+-------------------------------------------+
                                            |
                                            v
+---------------------------------------------------------------------------------------+
| PHASE 5: `#![no_std]` Hardening, RV32 Cross-Compilation & CI QEMU Matrix              |
| - Enable `#![no_std]` core build for target `riscv32imac-unknown-none-elf`           |
| - Add CI jobs for `armv7em-none-eabihf` (Cortex-M4/M7) and `riscv32imc-esp-espidf`    |
| - QEMU automated test runner executing differential test suites                       |
| - Gate: 100% green CI cross-compilation and QEMU execution across all 32-bit targets  |
+---------------------------------------------------------------------------------------+
```

---

## 13. Verification, Testing & Cross-Compilation Matrix

### 13.1 Cross-Compilation Targets for Standing CI

| Architecture Family | Target Triple | Microcontroller Reference Hardware | Test Execution Mode |
|---|---|---|---|
| **RISC-V 32-Bit Bare Metal** | `riscv32imac-unknown-none-elf` | SiFive FE310, ESP32-C6/H2, CH32V307 | QEMU `qemu-system-riscv32 -M virt` |
| **ARM Cortex-M4/M7 (Hard Float)** | `armv7em-none-eabihf` | STM32F4/F7/H7, NXP i.MX RT | QEMU `qemu-system-arm -M netduinoplus2` |
| **ARM Cortex-M0+ (Thumb-1)** | `thumbv6m-none-eabi` | RP2040, SAMD21, STM32G0 | QEMU `qemu-arm` emulation |
| **Espressif ESP32-C3 (ESP-IDF)** | `riscv32imc-esp-espidf` | ESP32-C3 DevKit | Wokwi CLI / QEMU-ESP32 |

A companion **`test-rv32-zbb`** lane cross-compiles the core against `riscv32imac-unknown-none-elf` with `RUSTFLAGS="-C target-feature=+zbb"`, keeping the Zbb bit-manipulation build profile (§2.3) green so the `cpop`/`clz`/`ctz` lowering does not silently regress.

---
