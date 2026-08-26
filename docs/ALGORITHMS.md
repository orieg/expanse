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

### 3.3 Ordered Iteration (`iter` / `range`, forward & reverse)

Ordered iteration is a **zero-allocation stack walk** (`crate::iter::RawIter`), not a
per-element `next_at_or_after` re-descent. A single fixed `[StackFrame; 8]` records
the active branch on each of the ≤8 trie levels plus a `LeafCursor` streaming the
current leaf; each key costs $O(1)$ amortized instead of $O(\text{depth})$.

**Forward (`next`, ascending).** `descend`/`descend_seek` walk to the leftmost leaf
(or the first key `≥ start`), pushing each branch frame with the *next* child to
visit going up. `next` streams the `LeafCursor` (leaf slot `idx`, bitmap word via
`trailing_zeros`, immediate slot); when a leaf is exhausted, `advance_leaf` pops to
the nearest frame with a remaining higher child and re-descends leftmost.

**Reverse (`next_back` / `iter_rev` / `range_rev`, descending).** The mirror image:
`descend_max`/`descend_seek_back` walk to the *rightmost* leaf (or the largest key
`≤ end`), pushing each branch frame with the *previous* child to visit going down
(`prev_set` on bitmap branches, `num-1` on linear branches). `next_back` streams the
`LeafCursor` from its high end — leaf slot `idx-1`, the bitmap's highest set bit via
`63 - leading_zeros`, and for map bitmap leaves a within-subexpanse `rank` walked
from `popcount-1` down — so `prev_at_or_before` semantics are **amortized across a
leaf** rather than re-descended per element. When a leaf is exhausted, `retreat_leaf`
pops to the nearest frame with a remaining lower child and re-descends rightmost.
This also unlocks the `LeafCursor::ImmedSingle` fast path in reverse.

**`DoubleEndedIterator`.** The zero-regression gate (`AGENTS.md` §5) forbids adding
any per-element work to the forward `iter`/`range` `next` (a measured hot path), so
the 64-bit `iter`/`range` iterators stay strictly forward-only and byte-identical.
Bidirectional traversal instead lives on the dedicated reverse iterators
`iter_rev`/`range_rev`, which **are** `DoubleEndedIterator`: `next` streams descending
(the primary direction) and `next_back` streams ascending. The two directions run
independent cursors (the ascending cursor is built lazily on the first `next_back`, so
a pure-descending walk never pays for it) sharing an inclusive `[lo, hi]` window —
`next` lowers `hi`, `next_back` raises `lo`, and each side stops once its next key
would leave the window, so the ends never cross or double-yield. The 32-bit
`ExpanseMap32`/`ExpanseSet32` have no measured forward-iterator baseline, so their
`iter`/`range` are directly double-ended, cursor-based over the existing
`first`/`next`/`last`/`prev` trie navigation with the same shared-window discipline.

### 3.4 Set-Algebra Kernels (`algebra.rs`, issues #339, #348)

`ExpanseSet` exposes native set algebra — `intersection` / `union` / `difference` / `symmetric_difference` (materializing a new set), their `*_len` cardinality variants, and the `BitAnd` / `BitOr` / `Sub` / `BitXor` operators — computed over the trie structure rather than by composing navigation primitives element by element (the composed path lost every Boolean cell to a word-parallel container; see `docs/benchmarks/search_inverted_index/`).

**One structural walk.** Only the **intersection cardinality** is computed structurally; the other three derive from it and the two populations (both $O(1)$):

$$|A \cap B| \text{ (structural)}, \quad |A \cup B| = |A| + |B| - |A \cap B|, \quad |A \setminus B| = |A| - |A \cap B|, \quad |A \bigtriangleup B| = |A| + |B| - 2|A \cap B|.$$

**`intersection_len` descent** (`algebra::intersection_len(ea, eb, level)`), over two non-null edges covering the same expanse:

1. **Full-expanse shortcut.** If one side is a `FullExpanse`, every key of its expanse is present, so the intersection equals the other side's population — read in $O(1)$ from the sibling edge's `pop0` ($256^{level}$ for a full expanse). No descent.
2. **Aligned bitmap leaves.** If both edges reduce to a real-level-1 form (`LeafBitmap1`, `Leaf1`, or a 1-byte immediate — possibly reached through a narrow-pointer skip), compare their skipped middle digits: disjoint ⇒ 0; equal ⇒ `Bitmap256::count_and` — four 64-bit `AND`s and a `popcnt`, instead of iterating up to 256 elements per leaf. This is the dense/clustered win.
3. **Branch × branch.** Iterate the **present children of the smaller side** (per the caller's population ordering) and probe each digit into the other branch, peeling one narrow-pointer skip level at a time so both sides stay aligned; recurse only where **both** sides have a child. Driving from the sparser operand's actual children — rather than scanning all 256 digits — makes the cost scale with the smaller list, so whole subtrees absent on one side are never visited. This is the source of the skewed-`AND` advantage.
4. **Terminal probe.** When one side is a linear leaf or wide immediate that did not reduce to a shared bitmap (bounded to $\le 32$ / $\le 15$ keys by the leaf/immediate caps), enumerate its remainders and probe each into the sibling subtree with `get::test_set`.

The kernel operates only on the existing `Edge`/branch/leaf geometry — no new node form, no fat slot (`AGENTS.md` §2). `Bitmap256::{and, or, andnot, xor, count_and}` are the word-parallel leaf kernels shared by all four operations.

**SIMD `BranchU` pair mask — frozen negative result (#348).** A `BranchU × BranchU` step was implemented and parity-tested (an SSE2 256-bit presence mask, `trailing_zeros` over the word-parallel `AND` of both masks, streaming `_mm_prefetch` of the next pair) but **is not shipped**. Measured on the reference host it caused a **controlled +61 % dense-cardinality regression at N=10⁷** (native AND 56.5 µs → 90.9 µs; commit `4d720b0c` vs `c129836d`, with the composed and roaring controls identical to 4 significant figures), and +34 % on sparse. The reason is structural, not tuning: a `BranchU` is by definition ≥ 192 / 256 populated, so two of them share nearly every digit — a "both-present" mask has almost nothing to skip, and building two masks costs more than the scalar drive-from-the-smaller-side walk it replaced. The one regime a presence-mask skip would help (low overlap) never reaches a `BranchU` — sparse branches are `L3`/`L7`/`B`. The streaming prefetch rode the same commit and was **not** isolated; whether prefetch alone helps is a separate experiment, not claimed here either way.

**Direct-emission materialization (#348).** The materializing ops no longer merge the two ordered iterators and `insert` each surviving key. When both operands are tries, `algebra_build::materialize` emits the result tree from the same lockstep walk: aligned final-byte bitmaps combine word-parallel into one leaf; `FullExpanse` resolves structurally (`A ∩ full = clone A`, `A ∪ full = full`, `A \ full = ∅`, `full \ A` / `A △ full` = a structural per-digit **complement**, never key-by-key); branch pairs recurse per digit and assemble the parent bottom-up; a small result is re-canonicalized through the bulk builder so every emitted tree is content-equivalent to the insert path's, canonical, and never less compact. All node construction reuses the mutation engine's constructors, so the invariants validator, Miri, and the `set_algebra` fuzzer certify **one** construction path. Any operand that is a root leaf / empty falls back to an ordered-merge that feeds the same bulk builder.

**Bulk builder / `from_sorted_iter` (#348).** `algebra_build::build_subtree` constructs a canonical subtree bottom-up from a sorted, distinct key run — immediates (≤ `max_count`), linear leaves (≤ cap), bitmap leaves / full expanses, and `L3`/`L7`/`B`/`U` branches chosen by population and divergence, `pop0` and narrow-pointer decode stamped as the ladder would. An uncompressed branch reached through a shared-prefix skip is hosted under a one-child linear branch (a `BranchU` has no header level and cannot itself skip). Exposed publicly as `ExpanseSet::from_sorted_iter` for bulk loading (posting lists, image key streams, algebra results); it emits the tree in one pass instead of $O(n)$ inserts and returns a tree content-equivalent to a key-by-key build's — canonical and never less compact (it may pick the more-compact `FullExpanse` where ascending insert leaves a full bitmap leaf).

---

### 3.5 Stateful Skip-Scan Cursor (`advance_to`)

A **cursor** (`crate::cursor::{SetCursor, MapCursor}`, via `ExpanseSet::cursor` /
`cursor_from`, same for the map) is a forward-only ordered position with one extra
operation over the iterators: `advance_to(target)` returns the smallest key `≥ target`
that is `≥` the cursor's current position. It is the primitive WAND / block-max
skip-scans and sorted merge-joins want, where the stateless
`ExpanseSet::next_at_or_after` (compat `Judy1First`) re-descends from the root on
**every** call and so pays the full $O(\text{depth})$ path even when the target lies in
the current leaf or a near sibling (issue #340; `docs/benchmarks/search_inverted_index/`
Pillar 2).

**Path reuse.** The cursor wraps the same zero-allocation forward `RawIter` — the
`[StackFrame; 8]` edge stack plus the `LeafCursor` (§3.3) — and keeps it live across
calls. `RawIter::seek_forward(top, target)` repositions it by the *tightest* re-descent
that reaches `target`:

1. **Leaf-local (near skip).** If `target` lies inside the current leaf's expanse, it is
   a single in-leaf search — `leaf::locate` (SIMD/SWAR) for linear leaves, a masked
   `next_set` for bitmap leaves, a slot advance for immediates/`FullExpanse` — with no
   stack movement. This is the common WAND case and the roaring `advance_to` parity
   target.
2. **Ancestor re-descent (mid skip).** Otherwise it ascends the edge stack, popping each
   frame whose expanse (`target >> 8·level == prefix >> 8·level`) no longer covers
   `target`, and re-dispatches the **deepest** frame that still does: it picks that
   branch's first child with digit `≥ target`'s digit at that level (linear
   `partition_point`, bitmap `next_set`, uncompressed scan) and `descend_seek`s into it.
   Cost is $O(\text{levels crossed})$, never a full root descent.
3. **Root re-descent (far skip / cross-expanse).** Only when the whole current path is
   left behind — every stack frame popped — does it fall back to a `descend_seek` from
   the root edge, identical to constructing a fresh `range` iterator.

When the deepest covering frame's target digit equals the digit of the child the cursor
just descended through, the re-dispatch re-descends that *same* child. This stays correct
because `descend_seek` positions by value (`target`), landing past every consumed key
rather than by cursor offset — but it is a redundant one-level descent and a candidate
micro-optimization (descend straight into the retained child) if this path ever shows up
hot.

Each step allocates nothing and, like the iterators, carries a `// SAFETY:` note on every
raw-pointer deref. The cursor peeks one element ahead (`current`), so a `target` at or
below the current key is a no-op that never rewinds — targets are expected
non-decreasing (monotone skip-scan). A flat root-leaf container needs no stack: the seek
is a `partition_point` over the sorted key array.

**Concurrency.** No `SyncExpanseSet` reader variant is offered. The OCC readers validate
under a per-operation seqlock bracket and hold no state between calls; a cursor's whole
value is holding raw node pointers *across* calls, which a concurrent writer's mutation
would invalidate. A seqlock-bracketed cursor would have to re-descend and re-validate on
every `advance_to`, which is exactly the stateless `next_at_or_after` it exists to beat —
so the cursor is a single-threaded (`&self`) reader only.

**32-bit twins.** `ExpanseSet32`/`ExpanseMap32` expose the same
`cursor`/`cursor_from`/`advance_to` surface (`crate::cursor32`) for source parity, but the
32-bit trie has no stack iterator to reuse, so each forward advance re-descends from the
root via the stateless `first`/`next` primitives (the monotone no-op short-circuit still
applies). They gain the API, not the path-reuse speedup, until `trie32` grows a stack
iterator — at which point they can adopt this engine unchanged. On 32-bit
targets the unsuffixed `expanse::SetCursor` / `MapCursor` re-export the `*32`
cursor types, mirroring the `ExpanseSet` / `ExpanseMap` re-point, so downstream
code names the same types on either width.

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
