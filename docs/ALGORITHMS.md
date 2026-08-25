# Expanse Architecture & Algorithm Flow Reference

This companion guide details the complete algorithmic pipeline of Expanse, describing when each algorithm triggers, its exact hardware dispatch, memory layout, and computational complexity.

An interactive visualizer is available in [`docs/architecture_visualizer.html`](architecture_visualizer.html).

---

## 1. High-Level Traversal Pipeline

Every lookup, mutation, and navigation operation proceeds through a 4-stage pipeline:

```text
[ Incoming 64-bit Key & Level ]
               │
               ▼
   [ 1. Tag & Pointer Decode ] ── (3-bit tag in Edge word 1, OCC SeqLock acquire fence)
               │
      ┌────────┴────────────────────────┬────────────────────────┐
      ▼                                 ▼                        ▼
[ Immediate Node ]             [ Linear / Bitmap Leaf ]   [ Branch Node (L3/L7/B/U) ]
(1 key in set, 1-7 in map)     (pop 2..256 keys)          (Multi-level 256-ary trie)
      │                                 │                        │
      │                                 │                        ▼
      │                                 │             [ Prefix & Decode Check ]
      │                                 │             (Narrow pointer skip match)
      │                                 │                        │
      │                                 └────────────────────────┘
      ▼
[ 2. Leaf Search / Mutation Kernel ]
  • pop <= 2: Branchless scalar comparison sum: pos = (k0 < needle) + (k1 < needle)
  • pop 3..16: 128-bit SIMD vector compare (_mm_cmplt_epi8 / NEON movemask) + POPCNT
  • pop 17..32: O(log N) binary probe with unrolled bounds
  • pop 33..256: 256-bit Bitmap rank & select (Hardware POPCNT / SWAR)
      │
      ▼
[ 3. Value Return / Shift Mutation ]
  • Lookup: Direct memory pointer dereference (0.60x vs stock Judy)
  • Insert: Monotonic append fast-path (k > last_key -> pos = pop, 0-byte shift)
  • OCC Validate: Atomic release fence & version match check
```

---

## 2. Node Form Compression Ladder & Algorithm Census

Expanse uses an adaptive least-compressed-form ladder with **1-index hysteresis** to eliminate allocation thrashing during insert/delete oscillations:

| Node Type | Capacity Threshold | Search Algorithm | Hardware Acceleration | Memory Overhead |
|---|---|---|---|---|
| **Immediate (Set)** | 1 key | Inlined in 16-byte Edge | Zero heap allocation, register-resident | **0 B** |
| **Immediate (Map)** | 1–7 keys (`7 / key_bytes`) | Inlined keys in aux bytes | Zero heap allocation | **0 B** |
| **Linear Leaf 1** | 2..25 keys (level 1) | Branchless $\le 2$ / SIMD $\le 16$ / Binary probe | `_mm_cmplt_epi8` + `POPCNT` | Size-class allocated |
| **Linear Leaf $2\dots 7$** | 2..32 keys (levels $2\dots 7$) | Branchless $\le 2$ / SIMD $\le 16$ / Binary probe | `_mm_cmplt_epi16` / `_mm_cmplt_epi32` | Size-class allocated |
| **Bitmap Leaf 1** | 26..256 keys | 256-bit Bitmap Rank & Select | Hardware `POPCNT` / BMI2 `PEXT` | **0.07–0.36 B/key** |
| **BranchL3** | 1..3 child expanses | 8-byte SWAR byte search | Zero branch scan (`find_byte_8`) | 32 B header + 3 edges |
| **BranchL7** | 4..7 child expanses | 16-byte SIMD byte search | `find_byte_16_sse2` / NEON | 64 B header + 7 edges |
| **BranchB** | 8..180 child expanses | 256-bit Bitmap digit rank | Hardware `POPCNT` subarray rank | 64 B header + packed edges |
| **BranchU** | 181..256 child expanses | Direct array indexing | $O(1)$ flat pointer load | 2048 B flat table |

---

## 3. Detailed Operation Flows

### 3.1 Point Lookup (`get` / `locate_slot`)
1. **Root Read**: Read owning 16-byte `Edge`. If `SyncExpanseMap`, acquire seqlock read version.
2. **Tag Dispatch**:
   - `EdgeTag::Immed`: Compare low key bytes directly against embedded payload. Latency: **~4.2 ns**.
   - `EdgeTag::Leaf*`: Offset into key slice.
     - For $\text{pop} \le 2$: branchless scalar arithmetic `(k0 < needle) as usize + (k1 < needle) as usize`.
     - For $\text{pop} \in [13, 16]$ ($KB = 1$): 128-bit vector compare (`_mm_cmpeq_epi8` / `_mm_cmplt_epi8` with `0x80` unsigned-to-signed bias) + `_mm_movemask_epi8` + `POPCNT` in 4 instructions with zero branches.
     - For $KB = 2, \text{pop} = 8$: 128-bit vector compare (`_mm_cmpeq_epi16`).
     - For $KB = 4, \text{pop} = 4$: 128-bit vector compare (`_mm_cmpeq_epi32`).
     - For remaining populations: $O(\log N)$ binary probe with unrolled midpoint steps.
   - `EdgeTag::Branch*`: Decode digit at current level (`key >> (8 * (level - 1)) & 0xFF`), scan branch digits, and descend.
3. **OCC Read Validation**: If concurrent mode, verify seqlock version matches without write-lock bit.

### 3.2 Key Mutation (`insert` / `mutate_map`)
1. **Root Leaf Monotonic Fast-Path** ($\text{pop} \le 31$): If inserting into `ExpanseSet` or `ExpanseMap` root leaf and $key > \text{last\_key}$, bypass binary search and set $\text{pos} = \text{pop}$ in a single $O(1)$ scalar compare.
2. **Linear Leaf Monotonic Append Fast-Path**: If inserting into a linear leaf and $k > \text{last\_key}$, bypass binary search and set $\text{pos} = \text{pop}$.
3. **Multi-Level Sequential Run Bypass** ($\text{pop} > 31$): If inserting contiguous keys sharing the upper 56 bits ($key \gg 8 == \text{path.prefix}$), bypass all 8 branch levels and digit decodes. Directly execute 1 bit test/set in the active terminal `LeafBitmap1`/`LeafBitmapL` and increment ancestor edge $pop0$ counts in ~15 instructions with zero branch mispredicts.
4. **Class-Crossing Check**:
   - If $\text{cap\_class}(\text{pop} + 1) == \text{cap\_class}(\text{pop})$, shift keys in place (`core::ptr::copy`).
   - If class is exceeded, allocate next class from slab allocator and realloc-insert.
   - If population exceeds leaf capacity ($\text{pop} > 25$ or $\text{pop} > 32$), upgrade to `LeafBitmap1` or cascade into a `BranchL3`.

---

## 4. Microarchitecture Acceleration (`x86-64-v3` vs `v1`)

When compiled with `x86-64-v3` (AVX2, BMI2, POPCNT), Expanse replaces runtime dispatch branches with native single-instruction primitives:

* `Bitmap256::count` / `Bitmap256::subexpanse_rank`: Lowers from a 12-instruction SWAR bit sequence to a single `popcnt` instruction.
* `leaf::search_fixed` & `lower_bound_fixed`: Lowers from scalar loop branches to vector `pcmpgtb` + `pmovmskb` + `popcnt` (`_mm_cmpeq_epi8`, `_mm_cmplt_epi8`).
* **Measured Benchmark Speedup**:
  - `map_get/linear_leaf`: **-8.76% instructions (-9.79% cycles)**.
  - `map_remove/random`: **-42.60% instructions (-34.94% cycles)**.
  - `map_churn/random`: **-30.70% instructions (-24.62% cycles)**.
  - `map_get/random`: **-12.11% instructions (-13.25% cycles)**.

> Hardware ISA guarantees and per-architecture codegen for these kernels (SSE2/NEON/POPCNT/Zbb) are cited against primary sources in [`docs/HARDWARE.md`](HARDWARE.md).

---

## 5. Benchmark Arm Mapping Reference

| Traversal Path / Node Kernel | Primary Benchmark Arm in `benches/instructions.rs` |
|---|---|
| 16-element Linear Leaf SIMD Scans | `map_get/linear_leaf`, `map_insert/linear_leaf`, `set_insert/linear_leaf` |
| Monotonic Append & Root Leaf Fast Paths | `map_insert/sequential`, `set_insert/sequential` |
| 256-bit Bitmap Leaf Transitions | `map_get/dense_leaf`, `map_insert/dense_leaf`, `set_insert/dense_leaf` |
| Narrow-Pointer Skip Decoding | `map_get/clustered`, `map_insert/clustered`, `set_insert/clustered` |
| Immediate In-Pointer Key Search | `map_insert/small`, `set_contains/random`, `map_ins_slot/random` |
| Dynamic Reclassification & Hysteresis | `map_churn/random`, `map_remove/random` |

---

## 6. Interactive Architecture Visualizer & Developer Protocol

The interactive tool in [`docs/architecture_visualizer.html`](architecture_visualizer.html) provides an interactive graph representation of the entire trie lifecycle and execution flow.

### 6.1 Visualizer Architecture & Views
1. **⚡ Architecture & Dynamic Execution DAG** (`#dag`):
   - **Interactive Parameter Switcher**: Allows live manipulation of operation (`Lookup`, `Insert`, `Sync Read`, `Churn`, `Nav`), trie flavor (`ExpanseSet` vs `ExpanseMap`), key distribution (Sequential, Random, Clustered, Linear leaf, Dense leaf, Small), population scale (1 to 1,000,000 keys across 15 milestones), and ISA target (`x86-64-v3` AVX2/BMI2 vs `x86-64-v1` SWAR vs `AArch64 NEON`).
   - **Structural Component DAG**: SVG-rendered hierarchy showing pointer tags (`JAP`), root transition (`JPM`), 256-ary routing branches (`Level 8`, `BranchL3`, `BranchL7`, `BranchB`, `BranchU`), and terminal leaves (`Immediate`, `LinearLeaf`, `LeafBitmap`, `FullExpanse`). Clicking any node triggers the **Node Inspector Modal** detailing struct layout, memory alignment, and transition triggers.
   - **Active Execution Pipeline & Algorithm Trace**: Dynamically steps through the exact algorithmic flow (e.g. *SeqLock acquire fence* $\rightarrow$ *Sequential Run Bypass* $\rightarrow$ *SIMD vector scan* $\rightarrow$ *POPCNT rank* $\rightarrow$ *Release fence*).
   - **Hardware Impact HUD**: Real-time instruction cost, memory overhead, cache line touches (64 B / 128 B), and hardware acceleration speedup percentages.
2. **📊 Benchmark Intelligence & Memory Census** (`#bench`):
   - **Deterministic Callgrind Explorer**: Full filterable dataset of all 22 benchmark arms (50,000 operations per test, retired instructions, L1 cache hit counts, RAM traffic, and `x86-64-v3` deltas).
   - **Deterministic Memory Budget Matrix**: Byte-per-key density across 1K, 100K, and 1M key bands.
   - **Node Capacity & Lifecycle Specs**: Precise fanout and promotion ceilings.

### 6.2 Zero-Drift Synchronization Protocol
To prevent divergence between Rust code and the visualizer:
* **Single Source of Truth**: The Rust codebase (`crates/expanse/src/types.rs`, `set.rs`, `leaf.rs`, `node.rs`, `benches/instructions.rs`, `examples/bytes_per_key.rs`) is the single source of truth for all constants, bitmasks, capacities, and benchmark numbers.
* **Machine-Readable Dataset**: All ladder constants and benchmark results are recorded in [`docs/visualizer_data.json`](visualizer_data.json).
* **Dual-Mode Loading**:
  - When served over `http://` / `https://`, `docs/architecture_visualizer.html` dynamically fetches `visualizer_data.json` at runtime.
  - When opened offline via `file://`, it uses the embedded CI-verified fallback dataset.
* **Automated CI Enforcement**:
  - The integration test [`crates/expanse/tests/test_visualizer_sync.rs`](../crates/expanse/tests/test_visualizer_sync.rs) runs on every push/PR across Linux, macOS, and Windows.
  - It asserts that `ROOT_LEAF_CAP`, `BRANCH_L3_CAP`, `BRANCH_L7_CAP`, `BITMAP_TO_UNCOMPRESSED_THRESHOLD`, `MAX_LEVEL`, and all 22 Callgrind benchmark function names in `instructions.rs` match bit-for-bit between the Rust compiler, `docs/visualizer_data.json`, and `docs/architecture_visualizer.html`.

### 6.3 Instructions for Modifying or Extending the Visualizer
If you add a new node type, adjust promotion thresholds, or add benchmark arms:
1. **Update Rust Code**: Define the constant or benchmark in `crates/expanse/src/` or `crates/expanse/benches/instructions.rs`.
2. **Update JSON Dataset**: Add or update the values in [`docs/visualizer_data.json`](visualizer_data.json).
3. **Update Visualizer HTML**: Update the constants in `docs/architecture_visualizer.html` (in `LADDER_SPEC`, `BENCHMARK_DATA`, or `POP_MILESTONES`).
4. **Run Sync Test**: Verify that `cargo test --test test_visualizer_sync` passes before committing.
