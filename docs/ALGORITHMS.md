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
     - For $\text{pop} \le 2$: branchless scalar arithmetic.
     - For $\text{pop} \le 16$: SIMD vector compare + popcount.
     - For $\text{pop} > 16$: binary search probe.
   - `EdgeTag::Branch*`: Decode digit at current level (`key >> (8 * (level - 1)) & 0xFF`), scan branch digits, and descend.
3. **OCC Read Validation**: If concurrent mode, verify seqlock version matches without write-lock bit.

### 3.2 Key Mutation (`insert` / `mutate_map`)
1. **Monotonic Append Fast-Path**: If inserting into a linear leaf and $k > \text{last\_key}$, bypass binary search and set $\text{pos} = \text{pop}$.
2. **Class-Crossing Check**:
   - If $\text{cap\_class}(\text{pop} + 1) == \text{cap\_class}(\text{pop})$, shift keys in place (`core::ptr::copy`).
   - If class is exceeded, allocate next class from slab allocator and realloc-insert.
   - If population exceeds leaf capacity ($\text{pop} > 25$ or $\text{pop} > 32$), upgrade to `LeafBitmap1` or cascade into a `BranchL3`.

---

## 4. Microarchitecture Acceleration (`x86-64-v3` vs `v1`)

When compiled with `x86-64-v3` (AVX2, BMI2, POPCNT), Expanse replaces runtime dispatch branches with native single-instruction primitives:

* `Bitmap256::count`: Lowers from a 12-instruction SWAR bit sequence to a single `popcnt` instruction.
* `leaf::lower_bound`: Lowers from scalar loop branches to vector `pcmpgtb` + `pmovmskb` + `popcnt`.
* **Result**: Up to **-42.60%** instruction count reduction across churn and mutation benchmarks.
