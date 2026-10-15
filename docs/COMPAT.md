# C libjudy Compatibility Contract

> Canonical compatibility doc. Design: [ARCHITECTURE.md](ARCHITECTURE.md) · Testing: [TESTING.md](TESTING.md)

`libexpanse` (built by `crates/expanse-capi`) is a **drop-in binary replacement for libjudy**: existing consumers link-swap (or `LD_PRELOAD`) `libexpanse` in place of `libJudy` with **no source changes**. This document is the contract that defines "drop-in".

## Packaging layout

| Artifact | Name |
|---|---|
| Rust crate (core) | `expanse-trie` |
| C library | `libexpanse.so` / `expanse.dll` / `libexpanse.a` |
| Modern header | `expanse.h` — **shipped**: `expanse_set_t`, `expanse_map_t`, `expanse_bytesmap_t`, `expanse_strmap_t` (ordered NUL-terminated string keys, with truncation-aware `_ex` navigation), `expanse_ordered_bytesmap_t` (ordered arbitrary byte keys with caller-allocated buffer navigation returning required length), `ExpanseBlobMap` (`expanse_blob_map_*`), plus the concurrent `expanse_sync_set_t`/`expanse_sync_map_t` and their per-thread reader handles. Adds what classic Judy lacks: rank/select on ordered types (`count_below`/`count_range`/`by_count`), byte-exact `mem_used`, plain value returns instead of `JError_t` out-params, and optimistic concurrent readers. |
| Compat header | `Judy.h` (source-compatible with classic libjudy) |
| Distro packages  | `libexpanse-dev`, `libexpanse1`, and `libjudy-compat` (symlinks `libJudy.so.1` → `libexpanse.so.1` and installs the `Judy.h` alias) |

### Supported architectures

Drop-in compatibility is officially verified for:
- **x86-64** (Linux, Windows, macOS)
- **AArch64** (Linux, macOS)
- **RISC-V 64-bit (RV64GC)** (Linux, cross-compiled)

All verified targets are 64-bit (LP64 or LLP64). **32-bit builds carry no `Judy*` symbols at all** — drop-in compatibility is a 64-bit-only guarantee. The 32-bit engine ships through the native Rust and `expanse_*` C surfaces instead; see the [build-configuration surface matrix](#build-configuration-surface-matrix) below and [design/32-bit-embedded.md](design/32-bit-embedded.md).

### hwcaps sub-packages (x86-64-v2 and x86-64-v3)

The shipped baseline `.so` targets baseline `x86-64-v1`, with `popcnt` and SIMD reached through
runtime CPUID dispatch at the lookup entries (one predicted branch per call;
see `get.rs`).

For glibc Linux distros (glibc ≥ 2.33 / Ubuntu 22.04+, RHEL 9+, Fedora), companion builds compiled with
`-C target-cpu=x86-64-v2` and `-C target-cpu=x86-64-v3` installed under `.../glibc-hwcaps/`
let the Linux dynamic loader pick the fused, dispatch-free binary
automatically on capable CPUs:
- **`x86-64-v2`** (installed under `.../glibc-hwcaps/x86-64-v2/`): native POPCNT, SSE4.2, SSSE3.
- **`x86-64-v3`** (installed under `.../glibc-hwcaps/x86-64-v3/`): native 256-bit AVX2, BMI1, BMI2, POPCNT, FMA, LZCNT.

musl (Alpine) has no hwcaps and no IFUNC: it keeps the runtime-dispatch binary, which
is why the baseline dispatch exists at all.

#### Packaging / Build Recipe
To build and package baseline, v2, and v3 dynamic libraries:

1. **Build the baseline binary (`x86-64-v1`)**:
   ```bash
   cargo build --release --package expanse-capi
   ```
   Install this to the standard system library path (e.g., `/usr/lib/x86_64-linux-gnu/libexpanse.so.1.0.0` with symlink `libexpanse.so.1` -> `libexpanse.so.1.0.0`).

2. **Build the `x86-64-v2` hwcaps binary**:
   ```bash
   RUSTFLAGS="-C target-cpu=x86-64-v2" cargo build --release --package expanse-capi
   ```
   Install to `/usr/lib/x86_64-linux-gnu/glibc-hwcaps/x86-64-v2/libexpanse.so.1.0.0` with symlink `libexpanse.so.1`.

3. **Build the `x86-64-v3` modern AVX2/BMI2 binary**:
   ```bash
   RUSTFLAGS="-C target-cpu=x86-64-v3" cargo build --release --package expanse-capi
   ```
   Install to `/usr/lib/x86_64-linux-gnu/glibc-hwcaps/x86-64-v3/libexpanse.so.1.0.0` with symlink `libexpanse.so.1`.

## Clean-room rules (binding)

1. The original libjudy is LGPL. **Never read, consult, or port its source.**
2. Compatibility is defined by the **documented API contract**: the published `Judy.h` man pages (`Judy1(3)`, `JudyL(3)`, `JudySL(3)`, `Judy(3)`) and published API docs.
3. Behavioral questions the docs leave open are settled by **black-box differential testing** against a stock libjudy binary (observing behavior of a linked library is not consulting source).
4. The PDFs in `references/` are published algorithm descriptions — design context only, never committed to the repo.

## Target surface

Exported with C symbol names and the platform C calling convention:

| Family | Functions |
|---|---|
| Judy1 (bit set) | `Judy1Set`, `Judy1Unset`, `Judy1Test`, `Judy1Count`, `Judy1ByCount`, `Judy1First`, `Judy1Next`, `Judy1Last`, `Judy1Prev`, `Judy1FirstEmpty`, `Judy1NextEmpty`, `Judy1LastEmpty`, `Judy1PrevEmpty`, `Judy1FreeArray`, `Judy1MemUsed` |
| JudyL (word→word) | `JudyLIns`, `JudyLDel`, `JudyLGet`, `JudyLCount`, `JudyLByCount`, `JudyLFirst`, `JudyLNext`, `JudyLLast`, `JudyLPrev`, `JudyLFirstEmpty`, `JudyLNextEmpty`, `JudyLLastEmpty`, `JudyLPrevEmpty`, `JudyLFreeArray`, `JudyLMemUsed` |
| JudySL (string→word) | `JudySLIns`, `JudySLDel`, `JudySLGet`, `JudySLFirst`, `JudySLNext`, `JudySLLast`, `JudySLPrev`, `JudySLFreeArray` |
| JudyHS (hash, bytes→word) | `JudyHSIns`, `JudyHSDel`, `JudyHSGet`, `JudyHSFreeArray` |

Shipped header `include/Judy.h` additionally provides, source-compatibly:

- Types/conventions: `Word_t`, `Pvoid_t`, `PPvoid_t`, `Pcvoid_t`, `JError_t`, `PJError_t`, `PJERR`, `JERR`, `JU_ERRNO_*`.
- Convenience macros: `J1S`, `J1U`, `J1T`, `J1C`, `J1BC`, `J1F`, `J1N`, `J1L`, `J1P`, `J1FA`, `J1MU`; `JLI`, `JLD`, `JLG`, `JLC`, `JLBC`, `JLF`, `JLN`, `JLL`, `JLP`, `JLFA`, `JLMU`; `JSLI`, `JSLD`, `JSLG`, `JSLF`, `JSLN`, `JSLL`, `JSLP`, `JSLFA`; `JHSI`, `JHSD`, `JHSG`, `JHSFA`.

## Guarantees

1. **Symbol and ABI match** — same names, same signatures, same calling convention. `LD_PRELOAD`/link-swap works on Linux/macOS; the Windows DLL exports the same symbols so consumers that today build C libjudy from source (php-judy's MSVC job) link against `expanse.dll`/`.lib` instead.
2. **Semantic match** per documented contract: pointer-to-value-slot return conventions (`JudyLIns` returns a writable `PPvoid_t`; inserted slots initialized to 0), sorted-order iteration (`First`/`Next`/`Last`/`Prev` and the `Empty` variants, inclusive-search semantics), `Count`/`ByCount` rank semantics over inclusive index ranges, null/empty-array edge behavior, `JError_t` error reporting incl. `PJERR`/`JERR` returns and `JU_ERRNO_*` codes.
3. **Ordering guarantee**: iteration is strictly sorted unsigned-key order (JudySL: byte-lexicographic) — identical to libjudy, verified differentially.

## Binary image compatibility

`ExpanseBlobMap::save_to_file` / `load_from_file` (Rust surface only; the C
ABI exposes no image save or load) write and read a memory image whose
64-byte header carries `EXPANSE_FORMAT_VERSION`
(`crates/expanse/src/blobmap.rs`). **The image is not a cross-version
format.** A build reads only images written with its own format version; an
image carrying another version is refused with
`ArenaError::UnsupportedFormatVersion { found, supported }`, never loaded
partially and never migrated. The version is
`2` since #518 (inline compressed value slots), which means images written by
v0.5.0 (`1`) do not load in later releases; nothing in the library rewrites a
v1 image, so a consumer that needs continuity across that boundary re-inserts
from its source of truth. A change to the format bumps the constant and is
called out in the release notes; a damaged header (bad magic, size or
chunk-geometry fields) is `CorruptedHeader`, a different error.

## Non-goals

- **`Judy1MemUsed`/`JudyLMemUsed` byte equality.** Internal geometry differs (64-byte-line nodes, different allocator). Guarantee: same order of magnitude and monotonic behavior — not the same number. Documented in `Judy(3)` (`man/man3/Judy.3`, *Memory Accounting*); the shipped `Judy.h` declares the two functions without comment.
- **Internal structure equality** — no attempt to match libjudy's node layouts, only its observable behavior.
- **`JErrno` legacy globals** beyond what the documented macro layer requires.

## Build-configuration surface matrix

`libexpanse` builds in three configurations, and they do not export the same
symbol set. Omitted entry points are **absent**, never present-and-stubbed: a
`Judy*` or `expanse_*` symbol that links but behaves differently is worse than
one that does not link, and a link error names the gap at build time.

| Configuration | Cargo invocation | Exported C symbols |
|---|---|---|
| 64-bit, `std` (default) | `cargo build -p expanse-capi` | 193 |
| 64-bit, `no_std` | `--no-default-features` | 164 |
| 32-bit (any) | `--no-default-features --target riscv32imc-unknown-none-elf` | 70 |

(Counts measured from `llvm-nm --defined-only` on the built artifacts — the
64-bit rows at commit `5e8147ae`, the 32-bit row re-measured each time the
narrow surface grew: 31 for the ordered core plus `expanse_map_remove_range`,
55 when the `expanse_sync32_*` surface landed for #573, 56 when
`expanse_map_for_each_range` landed for #614, 62 when the reader-handle
ordered reads landed for #900, 66 when `expanse_{map,set}_{mem_held,shrink_to_fit}` reached the 32-bit build for #1135 (62 on its parent, dabd13f8, by the same method), 67 when `expanse_map_contains` reached the 32-bit build for #1159 (66 on its parent, 81f9eda2, by the same method), and 70 when `expanse_sync32_map_writer_{compare_exchange,update,get_or_insert}` landed for #1194 (67 on its parent, f6a97022, by the same method). The #900 change also added six 64-bit symbols: the
64-bit `std` row was re-measured then with `nm -gU` on an arm64 macOS dylib,
145 before and 151 after, and 152 once `expanse_sync_map_mem_used` was added
(same `nm -gU` method; 151 on its parent); the 64-bit `no_std` row is unchanged by
construction, because the concurrent entry points compile only with `std`. The
six `expanse_{map,set,strmap}_{mem_held,shrink_to_fit}` entry points took both
64-bit rows up by six: 158 with `nm -gU` on an arm64 macOS dylib, and 133 with
`llvm-nm --defined-only --extern-only` on the `x86_64-unknown-none` staticlib
built with `--features embedded-panic-handler`, both on the branch that added
them, off `dc5cbad2`. The four `expanse_sync_{map,set}_{mem_held,shrink_to_fit}`
entry points took the 64-bit `std` row from 158 to 162 (`nm -gU` on an arm64
macOS release dylib; 158 on their parent, `94bbe22d`, by the same method); the
64-bit `no_std` row is unchanged by construction, as above. The eight modern C
API methods in #1159 took the 64-bit `std` row from 162 to 170 (`nm -gU` on an arm64
macOS dylib; 162 on parent 81f9eda2) and the 64-bit `no_std` row from 133 to 141
(`llvm-nm --defined-only --extern-only` on the `x86_64-unknown-none` staticlib
built with `--features embedded-panic-handler`). The four blob map methods in
#1315 took the 64-bit `std` row from 170 to 174 and the 64-bit `no_std` row from
141 to 145. The nineteen `expanse_ordered_bytesmap_*` entry points in #808 took
the 64-bit `std` row from 174 to 193 (`nm -gU` on an arm64 macOS dylib) and the
64-bit `no_std` row from 145 to 164; the 32-bit row is unchanged by construction,
because `modern_wide` compiles only on 64-bit targets.
Reproduce with the invocations above; the 32-bit row needs
`--features embedded-panic-handler`, as the CI job does.)

**Two exported symbols are declared in no header.** `expanse_validate(const void *array, int is_map)` and `expanse_stats(const void *array, int is_map, CExpanseStats *out)` (`crates/expanse-capi/src/lib.rs`, 64-bit only) are `#[no_mangle] extern "C"` and are counted in both 64-bit rows above, but neither `expanse.h` nor `Judy.h` declares them, so `scripts/check_abi_parity.py` does not see them and no binding wraps them. They take a `Judy1`/`JudyL` array pointer. They are not part of the supported surface until a header declares them; that is a code change, not a documentation one.

**64-bit `no_std` drops only the concurrent containers** — the 29
`expanse_sync_*` entry points. `expanse_trie::sync` needs `std::sync`, so a
bare-metal 64-bit build has no one-writer/many-reader surface. Everything
else, the entire legacy `Judy*` drop-in included, is present and unchanged.

The shared string, bytes, ordered-bytes and blob maps have no C handle in any
configuration. The surface planned for them, which would add to the 64-bit
`std` row only, is in [Planned: shared-map C handles](#planned-shared-map-c-handles-not-shipped).

**32-bit drops the legacy `Judy*` families entirely**, plus the byte-string,
string, blob and concurrent containers, plus rank/select and value-slot
accessors. What ships is the width-parametric ordered core with full
bidirectional range navigation:

| Container | 32-bit entry points |
|---|---|
| identity | `expanse_version` |
| `expanse_set_t` | `_new`, `_free`, `_len`, `_mem_used`, `_mem_held`, `_shrink_to_fit`, `_clear`, `_insert`, `_remove`, `_contains`, `_contains_batch`, `_first`, `_last`, `_next_at_or_after`, `_next_after`, `_prev_at_or_before`, `_prev_before` |
| `expanse_map_t` | `_new`, `_free`, `_len`, `_mem_used`, `_mem_held`, `_shrink_to_fit`, `_clear`, `_insert`, `_get`, `_contains`, `_remove`, `_remove_range` and `_for_each_range` (both 32-bit-only, see below), `_first`, `_last`, `_next_at_or_after`, `_next_after`, `_prev_at_or_before`, `_prev_before` |
| `expanse_sync32_map_t` / `expanse_sync32_set_t` (32-bit-only, provisional) | `_new`, `_free`, `_writer`, `_reader`, `_writer_try_insert`, `_writer_try_remove`, `_writer_try_reclaim`, `_writer_get` / `_writer_contains`, `_writer_stats`, `_reader_try_get` / `_reader_try_contains`, `_reader_try_len`; map only, the writer RMW calls `_writer_compare_exchange`, `_writer_update`, `_writer_get_or_insert` and the ordered reads `_reader_try_first`, `_reader_try_last`, `_reader_try_next_at_or_after`, `_reader_try_next_after`, `_reader_try_prev_at_or_before`, `_reader_try_prev_before`; plus `expanse_sync32_mutation_headroom`, `expanse_sync32_status_str` |

The cause is engine surface, not a deliberate reduction: `ExpanseMap32` /
`ExpanseSet32` are real tries, but they carry no `count_below`/`by_count`,
no `get_value_slot`/`ins_slot`, and their
`count_range` takes a `(start, end)` pair rather than a range — so the
corresponding C contracts have nothing to translate to. `ExpanseStrMap`/`ExpanseBytesMap` and the
`sync` module exist only at 64-bit width. `ExpanseBlobMap32` is public at every
width and is what `ExpanseBlobMap` names on a 32-bit target
(`crates/expanse/src/lib.rs`), but no C entry point wraps it: `expanse_blob_map_*`
is declared only in the wide surface.

Compaction (`compact()`) is a Rust-only memory lifecycle operation implemented
on `ExpanseSet`, `ExpanseMap`, `ExpanseSet32`, and `ExpanseMap32` (#1200, #1347),
and on concurrent containers `SyncExpanseSet` and `SyncExpanseMap` (#1354). It
rebuilds surviving elements into a fresh arena, reclaiming held memory after
bulk removals while invalidating internal node pointers. Under concurrent models,
it allocates through staged OCC allocation (`write_quiesced_staged`) under writer
exclusion and publishes under a version bracket while retiring the old tree through
the collector after bracket close. It is a no-op on trees behind `sync32`. Neither
width exports an `expanse_*_compact` C ABI symbol (pending consumer demand per #1200);
language bindings target 64-bit hosts and do not expose compaction or 32-bit types.

In #1357, `ExpanseSet32::from_sorted_iter` and `ExpanseMap32::from_sorted_iter` were
rewritten to use bottom-up direct emission (`trie32::build_set_subtree` / `build_map_subtree`),
allocating nodes directly at their final capacity class so total allocations equal
live allocations (4 instead of 33 on 100 sequential keys, per `set32.rs`), satisfying
the G-peak census identity for 32-bit compaction.

The reverse gap also exists. Two batched range entry points are declared in a
`#if !EXPANSE_WIDE_SURFACE` block and shipped **only at 32-bit width**, because
only the 32-bit engine carries the corresponding Rust methods:

- **`expanse_map_remove_range`** — batched removal of every key in `[lo, hi]`
  with a per-entry callback, the eviction primitive of
  [#578](https://github.com/orieg/expanse/issues/578).
- **`expanse_map_for_each_range`** — the read-only twin: an ordered walk of
  `[lo, hi]` whose callback returns `false` to stop it, added for
  [#614](https://github.com/orieg/expanse/issues/614). It is one descent to
  `lo` and then contiguous streaming through the leaves the range spans,
  where the stateless `expanse_map_next_after` loop it replaces pays a fresh
  `O(depth)` root descent per key. **No cursor object and no borrowed
  pointer**: keys and values reach the callback by value and nothing outlives
  the call, so the "valid until the next mutation" contract §2.3 audits
  never arises — the reason the callback form was taken over an opaque
  cursor handle. The callback must not mutate the map during the walk.

The language bindings target 64-bit hosts, so `scripts/check_abi_parity.py`
tags declarations in that block `narrow_only`, lists them, and excludes them
from binding coverage; the 32-bit-only surface is verified instead by the
i686 CI job (a Rust-side test plus a C smoke linked with `-m32` against the
i686 cdylib).

#### The 32-bit concurrent story

The 32-bit engine's concurrent wrapper — the Rust `sync32` module
(`SyncExpanseMap32`/`SyncExpanseSet32`: single writer enforced by
`split(&mut)`, validated single-attempt optimistic reads returning `Busy`,
deferred reclamation drained at reader quiescence) — reaches C as the
**`expanse_sync32_*` family**,
33 symbols declared in the `!EXPANSE_WIDE_SURFACE` block and present in every
32-bit build, std or not (`EXPANSE_HAS_SYNC32` announces it). It is a
different protocol from the 64-bit `expanse_sync_*` family and says so in the
header: **blocking optimistic lock coupling, single attempt, no mutex
fallback — not lock-free.** A read overlapping a write bracket returns
`EXPANSE_SYNC32_BUSY` at once instead of retrying, because on a single-core
part a `Busy` inside an interrupt handler means the handler preempted the
writer, which cannot progress until the handler returns.

What Rust enforced with types, the C surface makes structural rather than
checked: the writer is born with the container and reached through an
idempotent accessor; readers are addressed by index into handles the
container owns; the wrapper performs no atomic read-modify-write anywhere
(the primary target has none). The contract that remains — one writer
context, one reader handle per execution context, reader `try_*` as the only
interrupt-safe calls, `_free` only with reader interrupts masked — is stated
in the header next to the declarations, with the latency envelope (a write
bracket may contain up to `expanse_sync32_mutation_headroom()` allocator
calls; a reclamation stall is not time-bounded). Every fallible call returns
an `expanse_sync32_status_t` in three explicit bands (outcomes, refusals
that leave the tree untouched, usage errors), values through out-parameters.

**Provisional.** #573 item 3 measures this surface against a FreeRTOS mutex
around the single-threaded map on hardware, for both a task-level reader and
the interrupt-handler reader the mutex cannot serve; that pre-registered
verdict is the gate for keeping or retracting the family (the crate is
pre-1.0). Verification today is the i686 CI lane — a Rust-side test and a C
smoke linked with `-m32` that runs a paced writer thread against reader
threads asserting stable keys are never torn.

That wrapper needs **no compare-and-swap**: its whole protocol is atomic
load/store plus fences (`crates/expanse/src/sync32.rs`), which is why it
compiles for `riscv32imc-unknown-none-elf` — the ESP32-C2/C3, whose cores
implement no RISC-V A extension at all. Its single-writer/many-reader contract
is satisfied **within one hart**.

Which 32-bit parts could carry a concurrent surface, and which
compare-and-swap mechanism is sound on each, is answered per part from the
Espressif datasheets and Technical Reference Manuals in
[docs/HARDWARE.md §4.3](HARDWARE.md#43-espressif-risc-v-per-part-core-inventory--cas-soundness--validated-567).
The load-bearing results for this document: ESP32-C2, ESP32-C3 and ESP32-H2 are
single-hart; ESP32-C6 pairs its HP core with an LP RISC-V core that reads and
writes HP SRAM; ESP32-P4 is dual-core HP plus an LP core reaching L2 SRAM.
`AtomicU64` is native on none of them — which is why the 32-bit concurrent
surface is built on `AtomicU32` seqlocks
— and
`portable-atomic`'s `unsafe-assume-single-core` — the cheap CAS path — is sound
on C2/C3/H2 and **unsound on C6 and P4**, so it must never be switched on for
the Espressif family as a whole.


### Key and value width

Keys and values in the modern surface are **one machine word**, matching both
classic Judy's `Word_t` and the engine's own invariant that a value slot is a
single machine word (`docs/ARCHITECTURE.md`). `expanse.h` typedefs
`expanse_word_t` accordingly — `uint64_t` on 64-bit targets, `uint32_t` on
32-bit ones — and defines `EXPANSE_WIDE_SURFACE` so a C consumer can compile
against either library from the same header. **The 64-bit ABI is unchanged**:
`expanse_word_t` is `uint64_t` there, so every existing signature keeps its
type. Counts (`_len`, `_count_below`, `_count_range`, and `by_count`'s `n`)
stay `uint64_t` at both widths — they are populations, not keys.

## New capabilities

Modern features (optimistic concurrent reads, iterators, arena controls) are exposed through the native Rust API and through the `expanse_*` C API in `expanse.h`. Existing `Judy*` symbols never change semantics. Swapping in libexpanse must be a pure substitution.

### Blob map reclamation at the capacity cap (`expanse_blob_map_insert`)

An `expanse_blob_map_insert` that the arena's capacity cap refuses may compact the arena under the reclaim rule (`docs/design/large-values.md` §6.3.1) and retry once, instead of returning `false` straight away. The call's contract is unchanged: it returns `false` on refusal and leaves the map's contents unchanged, and any insert already invalidated every `ExpanseBlobView`. What changes in practice is that such an insert can copy every live payload and free the chunks earlier views pointed into, so a view held across an insert reads freed memory rather than stale bytes. An insert refused at the cap may first run a whole-arena compaction, which took 302.5–320.1 ms at 3,600,000 live 128 B records *(measured: reference host, `cdfbb0bc`, `docs/benchmarks/concurrency/results/gate_1300_split_h_run1.json` and `gate_1300_split_h_run2.json`)*; `docs/design/large-values.md` §6.3.1 gives the cost and how to avoid it (`compact()` off the hot path, a cap above twice the live data, or `expanse_blob_map_set_reclaim_at_cap` off).

Four additive entry points control and report it (#1300):

| Entry point | Contract |
|---|---|
| `expanse_blob_map_new_with_capacity(chunk_size, max_capacity)` | The cap on allocated chunk bytes, dead and live. `0` selects the default 1 GiB (and a `chunk_size` of `0` the default 2 MiB); otherwise clamped to `[chunk_size, 64 GiB]`, fixed for the map's life. The reclaim rule applies at any value. |
| `expanse_blob_map_set_reclaim_at_cap(map, on)` | Off, an insert the cap refuses compacts nothing, as before the rule. On by default. |
| `expanse_blob_map_insert_ex(...)` | `expanse_blob_map_insert` returning `expanse_blob_status_t`: `EXPANSE_BLOB_CAP_REFUSED` when no compaction ran (dead bytes may remain), `EXPANSE_BLOB_ARENA_FULL` when the insert compacted and the record still did not fit, plus `META_OVERFLOW`, `ALLOCATION_FAILED` and `INVALID_ARGUMENT`. |
| `expanse_blob_map_arena_stats(map, stats, sizeof *stats)` | Writes the prefix of `expanse_blob_arena_stats_t` the caller knows: live bytes, allocated chunk bytes, the cap, the chunk size and the switch. Append-only. |

None is declared in `Judy.h`, which has no blob map.

### Reader-handle ownership (64-bit `expanse_sync_*`)

The contract for `expanse_sync_set_reader_t` and `expanse_sync_map_reader_t`, stated next to the declarations in `include/expanse.h` and in `expanse_sync(3)`:

| Operation | Contract |
|---|---|
| Use | One handle per reading thread. A handle owns one epoch slot and its pins are not reentrant, so calls on one handle from two threads at once are undefined. |
| Free from another thread | Allowed, once no call on the handle is in progress. Every call releases its epoch pin before returning, and `_reader_free` deregisters through the container's registry lock without reading thread-local state. |
| Free while another thread is inside a call on it | Undefined. |
| Free after the container | Undefined. Free every reader handle before `expanse_sync_*_free`. |

In Rust the *Use* row is enforced by the type system: `occ::Reader` and every `sync` reader handle are `Send` and not `Sync`, so safe code cannot call through one handle from two threads (`docs/ARCHITECTURE.md` §4.1, *One reader handle per thread*). A C handle is a raw pointer and carries no such bound, and neither do the FFI bindings built on it (Go `SyncMapReader`/`SyncSetReader`, Java `SyncExpanseMap.Reader`/`SyncExpanseSet.Reader`, .NET `ExpanseSyncMapReader`/`ExpanseSyncSetReader`, C++ `sync_map_reader`/`sync_set_reader`): there the row stays a caller obligation. The bindings that never hand a reader to the caller are not exposed to it — Python keeps one reader per thread in a thread-local cache, and Node registers one per call.

The cross-thread free rests on the handle types being `Send`. That bound is auto-derived from their fields, so it is asserted at compile time on the C handle types (`crates/expanse-capi/src/modern_sync.rs`), on `MapReader`, `OwnedMapReader`, `DetachedMapReader`, `SetReader`, `OwnedSetReader`, `DetachedSetReader`, `StrReader`, `OwnedStrReader`, `DetachedStrReader`, `BytesReader`, `OwnedBytesReader`, `DetachedBytesReader`, `BlobReader`, `OwnedBlobReader` and `DetachedBlobReader` (`crates/expanse/src/sync.rs`) and on `occ::Reader` (`crates/expanse/src/occ.rs`): a future `!Send` field fails the build. `test_sync_reader_handles_freed_on_another_thread` (`crates/expanse-capi/tests/test_modern_capi.rs`) creates handles on worker threads, reads through them, and frees them on the main thread before the containers. The 32-bit `expanse_sync32_*` family has its own, index-addressed reader contract (see the 32-bit concurrent story above).

### Planned: shared-map C handles (not shipped)

> **This is a plan, not shipped surface.** Nothing below is declared in
> `include/expanse.h` or exported by `libexpanse`. **Done when:** each
> implementation PR replaces the part it ships with the shipped surface rows
> (the matrix and parity tables above, the header, `expanse_sync(3)`) and
> deletes that part here; the section is gone once PR 5 below lands. Tracking:
> [#1538](https://github.com/orieg/expanse/issues/1538) (the Rust operations it
> exposes: [#1194](https://github.com/orieg/expanse/issues/1194)). Section 9
> records the maintainer's decisions on the plan's design choices.

`include/expanse.h` gives C a shared handle for the `u64` map and the set
only (`expanse_sync_map_t`, `expanse_sync_set_t` and their readers,
`include/expanse.h:767-887`). The Rust shared maps `SyncExpanseStrMap`
(`crates/expanse/src/sync.rs:13795`), `SyncExpanseBytesMap` (`:15020`),
`SyncExpanseOrderedBytesMap` (`:16575`) and `SyncExpanseBlobMap` (`:11063`)
have none, so their read-modify-write cannot reach C (the Atomic RMW row of
the parity table below). This plan fits them to the existing
`expanse_sync_*` conventions.

#### What the existing shared surface does, which this plan copies

| Fact | Where |
|---|---|
| The C container handle is the boxed Rust type; every call takes it as `*const` and is safe from any thread at once; `_free` needs every reader freed first. | `crates/expanse-capi/src/modern_sync.rs:38-55`, `:182-199`; `include/expanse.h:748-765` |
| A C reader handle wraps the **borrowed** Rust reader with its lifetime extended to `'static` by `transmute`; the header's "free the reader before the container" rule upholds it. Owned (`Arc`) and detached readers are not used. | `modern_sync.rs:14-22`, `:147`, `:410` |
| The C reader wrappers are asserted `Send` at compile time, which is what the cross-thread-free contract rests on. | `modern_sync.rs:24-36`; [Reader-handle ownership](#reader-handle-ownership-64-bit-expanse_sync_) above |
| One-shot `_get`/`_contains` on the container handle register a throwaway reader per call; hot loops take a reader handle. | `modern_sync.rs:79-89`; `include/expanse.h:748-751` |
| Point operations return `bool` with values through nullable out-pointers; a NULL handle returns `false`/`0` and writes nothing. | `modern_sync.rs:201-265`, `:344-353` |
| Ordered reads are stateless calls on the reader handle, each one validated read with the retry budget and writer-excluding fallback; no cursor object crosses the boundary, although Rust has `SyncMapCursor`. | `modern_sync.rs:452-586`; `crates/expanse/src/sync_cursor.rs:288` |
| RMW: `compare_exchange` with presence flags on both sides and the observed word reported on both outcomes; `get_or_insert`; no callback `update` (D6), which each binding writes as a retry loop. | `modern_sync.rs:267-342`; `include/expanse.h:806-846`; Atomic RMW paragraph below |
| The whole family compiles only on 64-bit `std` builds. | `crates/expanse-capi/src/modern_wide.rs:992-997`; `crates/expanse/src/lib.rs:133-134` |
| Caller-buffer navigation reports `OK` / `NOT_FOUND` / `BUFFER_TOO_SMALL` with `required_len`, writing nothing on too-small; a `NULL`, `0` buffer is a valid length probe. | `include/expanse.h:580-621` (string, `expanse_str_nav_status`), `:664-733` (ordered bytes, `expanse_ordered_bytes_nav_status`) |
| `expanse_blob_status_t` is banded (outcome `0`, refusals `16..`, usage errors `32..`, other `48`), and "new values may be added within a band". | `include/expanse.h:967-982`; `crates/expanse-capi/src/blobmap.rs:92-128` |

#### 1. Handles, readers and thread rules

New opaque types, one owning handle and one reader per map kind:

| Map kind | Owning handle | Reader handle (wraps) |
|---|---|---|
| `SyncExpanseStrMap` | `expanse_sync_strmap_t` | `expanse_sync_strmap_reader_t` (`StrReader<'static>`, `sync.rs:14650`) |
| `SyncExpanseBytesMap` (default `RandomState` hasher, `sync.rs:15038-15043`) | `expanse_sync_bytesmap_t` | `expanse_sync_bytesmap_reader_t` (`BytesReader<'static>`, `sync.rs:16425`) |
| `SyncExpanseOrderedBytesMap` | `expanse_sync_ordered_bytesmap_t` | `expanse_sync_ordered_bytesmap_reader_t` (`OrderedBytesReader<'static>`, `sync.rs:16969`; point reads only) |
| `SyncExpanseBlobMap` | `expanse_sync_blob_map_t` | `expanse_sync_blob_map_reader_t` (`BlobReader<'static>`, `sync.rs:13062`) |

- **Reader form: borrowed only**, as the `u64` map does. C has no `Arc`, so
  the owned readers add nothing a C caller can use; the detached readers
  (`DetachedStrReader`, `sync.rs:14863`, and its siblings) avoid the lifetime
  extension but take the map on every call and panic on a map mismatch
  (`sync.rs:14855-14857`), which across `extern "C"` aborts (D6). Decided
  (section 9, decision 2).
- **No pin outlives a call.** Every reader call pins the epoch for that call
  only (`BlobReader::get`/`get_into`, `sync.rs:13404-13435`). `BlobReader::pin`
  and its `BlobReadGuard`/`SyncBlobView` (`sync.rs:13397`, `:13646`, `:13674`)
  are **not** exposed: a guard defers reclamation tree-wide for as long as it
  is held (`sync.rs:13376-13389`), and a C handle would hide that pin.
- **Thread rules, stated in the header beside the declarations, identical to
  the [reader-handle contract](#reader-handle-ownership-64-bit-expanse_sync_):**
  every container call is safe from any number of threads at once, except
  `_free`, which needs no call in progress and every reader freed; a reader
  is used by one thread at a time and may be freed from another thread once
  no call on it is in progress; freeing it after its container is undefined.
  Each C reader wrapper gets the compile-time `Send` assertion of
  `modern_sync.rs:32-36`.
- The blob reader's calls take `&mut self` in Rust (`sync.rs:13406`,
  `:13429`), so the blob reader parameter is a non-`const`
  `expanse_sync_blob_map_reader_t *`; the other readers take `const`, as the
  `u64` reader does.
- Creation: `_new(void)` for the three word-valued maps; the blob map takes
  `_new(chunk_size)` and `_new_with_capacity(chunk_size, max_capacity)`,
  matching `expanse_blob_map_new*` (`include/expanse.h:933-942`) and
  `SyncExpanseBlobMap::with_chunk_size*` (`sync.rs:11164`, `:11180`).

#### 2. First-version operation set

| Operation | str | bytes | ordered-bytes | blob |
|---|:-:|:-:|:-:|:-:|
| `_new`, `_free`, `_len`, `_mem_used`, `_mem_held`, `_shrink_to_fit` | yes | yes | yes | yes, plus `_new_with_capacity` |
| `_insert`, `_remove`, one-shot `_get`, `_contains` | yes | yes | yes | `_insert` returns a status; `_get_into` and `_get_meta` instead of `_get` |
| `_reader_new`, `_reader_free`, `_reader_get`, `_reader_contains` | yes | yes | yes | `_reader_get_into`, `_reader_get_meta`, `_reader_contains` |
| Ordered reads (`first`, `last`, `next_at_or_after`, `next_after`, `prev_at_or_before`, `prev_before`) | on the **reader**, optimistic (`StrReader::first`…`prev_before`, `sync.rs:14695-14746`) | none (unordered) | on the **container**, writer-excluding (`sync.rs:16561-16568`, `:16836-16929`) | none in v1 |
| `_compare_exchange`, `_get_or_insert` | yes (`sync.rs:13985`, `:14041`) | yes (`:15529`, `:15581`) | `compare_exchange` (`:16668`); `get_or_insert` in the shim as `compare_exchange(absent, value)`, since the Rust type has no `get_or_insert` (none in `sync.rs:16585-16947`) | `compare_exchange` (`:12253`), `compare_exchange_meta` (`:11990`), `get_or_insert` (`:12612`) |
| `_mem_used_estimate` | — | — | — | yes (`sync.rs:12924`): `_mem_used` waits for a compaction and makes writers wait (`sync.rs:12858-12872`) |
| `_set_reclaim_at_cap`, `_compact` | — | — | — | yes (`sync.rs:12741`, `:12703`); the shared C surface hands out no zero-copy view, so `_compact` invalidates nothing a C caller holds |

Left out of the first version, with the reason:

- **`with_locked` / `with_exclusive`** (`sync.rs:14307`, `:14334`,
  `:16182`, `:16210`, `:16729-16735`, `:13018`): both run caller code under
  the writer mutex. Across C that is a callback, which the RMW decision
  already rules out (D6, Atomic RMW paragraph), and a callback that calls a
  writing entry point on the same handle would wait on a mutex its own thread
  holds (unverified: `std::sync::Mutex` re-entry is documented as deadlock
  *or* panic).
- **`update`**: a binding-side retry loop over `compare_exchange`, as for the
  `u64` map; the header carries the loop.
- **`clear`, `stats`, `collector_*`, `layout_census`**: the `u64` shared
  handle exports none of them.
- **A cursor object** for the string and ordered-bytes maps: no Rust cursor
  exists for them (`sync_cursor.rs` defines only `SyncMapCursor`), and the
  `u64` precedent is stateless ordered reads.

#### 3. Keys and values

- **Bytes and ordered-bytes keys** are `(const void *key, size_t len)` with
  the header's existing rule: opaque bytes, embedded NULs ordinary, `NULL`
  with `0` the empty key (`include/expanse.h:28-30`). The ordered-bytes map
  escapes NULs itself (`with_encoded_key`, `sync.rs:16976-16985`).
- **String keys** are NUL-terminated `const char *`, as `expanse_strmap_*`
  takes them (`include/expanse.h:542-551`); decided (section 9, decision 1).
  The Rust map takes `&NulFreeStr`, which refuses any NUL byte
  (`crates/expanse/src/strmap.rs:746-753`); a C string cannot carry one, so
  no refusal path exists and every point call keeps the `bool` convention.
  Keys with NUL bytes go to the ordered-bytes map.
- **Values** of the three word-valued maps are `uint64_t`, in and out,
  exactly as `expanse_sync_map_*`.
- **Ordered-read keys out** go to a caller buffer with the existing
  `_ex`/ordered-bytes contract (`OK` / `NOT_FOUND` / `BUFFER_TOO_SMALL` +
  `required_len`, nothing written on too-small, `NULL`/`0` probe). Two
  consequences to state in the header: a retry after `BUFFER_TOO_SMALL` is a
  new read and may return a different key if a writer ran in between; and the
  string reader's ordered reads return an owned `Vec` (`sync.rs:14705`), so
  the shim allocates once per call until a Rust `*_into` reader method exists
  (an engine change, not part of this plan). The ordered-bytes reads use the
  zero-allocation `*_decode_into` forms. The single-threaded shim maps every
  decode error other than `BufferTooSmall` to `NOT_FOUND`
  (`modern_wide.rs:924-930`); for keys the map encoded those errors are
  unreachable, and the shared shim adds a `debug_assert!` there instead of a
  silent mapping (§8.1).
- **Blob values never leave as a borrowed pointer.** No `ExpanseBlobView` on
  the shared surface, no thread-local or rotating buffer (AGENTS.md §2.3 item
  4, §2.4). Reads copy into a caller buffer, mirroring `BlobReader::get_into`
  and `BlobBufferTooSmall` (`sync.rs:13092-13099`, `:13429-13435`):

  ```c
  /* sketch only; not declared anywhere */
  expanse_blob_status_t expanse_sync_blob_map_reader_get_into(
      expanse_sync_blob_map_reader_t *reader, uint64_t key,
      uint8_t *buf, size_t buf_len, size_t *len_out, uint32_t *meta_out);
  ```

  | Status | Meaning |
  |---|---|
  | `EXPANSE_BLOB_OK` | payload in `buf[0 .. *len_out)`, `*meta_out` its hot metadata (`0` inline); the rest of `buf` untouched |
  | `EXPANSE_BLOB_NOT_FOUND` (new, outcome band) | key absent; nothing written |
  | `EXPANSE_BLOB_BUFFER_TOO_SMALL` (new, outcome band) | key present; **nothing copied**, `buf` untouched; `*len_out` = required length, `*meta_out` = metadata, from the same validated read; a retry with that length succeeds unless a writer replaced the value |
  | `EXPANSE_BLOB_INVALID_ARGUMENT` | `NULL` reader, or `NULL` `buf` with `buf_len > 0` |

  `buf == NULL, buf_len == 0` is the length probe (`OK` only for an empty
  payload). The contract matches the corrected single-threaded
  `expanse_blob_map_get_into`: a buffer that is too small gets nothing copied
  and the required length reported. That correction is a separate pull request
  (section 9, decision 5); the shared form is written to it, not to the
  prefix-copying behaviour at `crates/expanse-capi/src/blobmap.rs:348-385`. The
  one-shot `_get_into` on the container handle has the same contract over a
  throwaway reader.
- **Blob `compare_exchange`** takes presence flags plus
  `(data, len, meta)` for `expected` and `new`, and reports the observed value
  into a caller buffer:

  | Status | Meaning |
  |---|---|
  | `EXPANSE_BLOB_OK` | stored |
  | `EXPANSE_BLOB_MISMATCH` (new, outcome band) | not stored; observed presence, length, metadata and bytes written |
  | `EXPANSE_BLOB_BUFFER_TOO_SMALL` | not stored, a mismatch whose observed bytes do not fit: presence, length and metadata written, no bytes |
  | refusal band | the current value **was** `expected` but `new` cannot be stored, `CompareExchangeError::Unstorable` (`crates/expanse/src/blobmap.rs:326-338`): `MetaOverflow` → `EXPANSE_BLOB_META_OVERFLOW`, `AllocationFailed` → `ALLOCATION_FAILED`, `OffsetOverflow` → `CAP_REFUSED` (this path runs no reclaim, `sync.rs:12231-12236`), `ArenaFull` → `ARENA_FULL`, as `From<ArenaError>` maps them (`crates/expanse-capi/src/blobmap.rs:118-127`); `NoMetaField` → a new `EXPANSE_BLOB_NO_META_FIELD` in the refusal band, since today it falls to `EXPANSE_BLOB_ERROR` |

  The mismatch and unstorable outcomes stay distinct in C as they are in Rust
  (#1461), so a retry loop that retries on `MISMATCH` and stops on a refusal
  ends. `get_or_insert` reuses the table (`OK` stored; `MISMATCH` /
  `BUFFER_TOO_SMALL` present, existing value reported; refusal band as
  above). `compare_exchange_meta` swaps the 24-bit metadata in-slot with no
  payload copy (`sync.rs:11960-11995`) and is the form the header recommends
  for retry loops.

#### 4. Status conventions

One convention per call shape, each already in the header: `bool` +
out-pointers for the word-valued point calls and RMW (as
`expanse_sync_map_*`); the two existing navigation status enums for the
ordered reads (no new enum); `expanse_blob_status_t`, extended within its
bands, for every blob call that can be refused or needs a buffer. The blob
point reads that cannot fail other than by absence (`_get_meta`,
`_contains`) stay `bool`.

#### 5. Width gating

Every planned symbol goes in the `#if EXPANSE_WIDE_SURFACE` block beside
`expanse_sync_map_*` and is compiled only with `std` (`modern_wide.rs:992-997`).
None exists on 32-bit builds: the 32-bit concurrent wrapper serves only
`ExpanseMap32`/`ExpanseSet32` (`crates/expanse/src/sync32.rs:252-253`), and
the string, bytes and shared blob maps exist only at 64-bit width (surface
matrix above). The 64-bit `no_std` and 32-bit rows of the matrix stay
unchanged by construction; the 64-bit `std` row is re-measured by each PR.
`Judy.h` is untouched (§4.1).

#### 6. Bindings

Each binding is a separate subsequent step, after the C surface it wraps has
shipped:

- **Python and Node** bind the Rust types directly (`crates/expanse-py/src/sync.rs:1`
  wraps `SyncExpanseMap`/`SyncExpanseSet` natively) and gain nothing from the C
  handles. Neither wraps the four shared maps today (no reference to them under
  `crates/expanse-py`, `crates/expanse-node`). Their parity for each new symbol
  is a native wrapper with an `// abi-parity:` marker, or a stated deferral.
- **Go, Java and .NET** reach the `u64` shared map through C
  (`bindings/go/sync_cgo.go:130`,
  `bindings/java/src/main/java/io/github/orieg/expanse/internal/ExpanseNative.java:147`,
  `bindings/dotnet/src/Expanse.NET/ExpanseSyncMap.cs`) and would wrap these
  handles the same way, with the reader-per-thread obligation as a caller
  obligation (reader-handle contract above). PHP (FFI) and Ruby (FFI) and the
  C++ `expanse.hpp` wrapper follow the same path; `check_abi_parity.py` does
  not check those three.
- Until a binding wraps a symbol, `scripts/check_abi_parity.py` needs a
  `STATED_DEFERRALS` entry for it (`scripts/check_abi_parity.py:453-470`)
  citing an **open** issue (`:540-568`). These deferrals cite #1538, not
  #1194 (section 9, decision 6).

#### 7. Gates each implementation PR carries

- `scripts/check_abi_parity.py --check` green, with the new symbols bound or
  deferred, and the parity table's counts updated.
- Header declarations with their contract comments; a section in
  `man/man3/expanse_sync.3` with an example per map kind, including the
  `compare_exchange` loop and, for the blob map, the too-small retry.
- `crates/expanse-capi/smoke/modern_api_smoke.c` (built by `ci.yml:2358`)
  exercising every new symbol once; a pthread smoke with a writer and reader
  threads asserting stable keys are never torn.
- Rust threaded tests over the `extern "C"` functions, as
  `test_sync_map_rmw_capi.rs` and `sync_surface_threaded` (`crates/expanse-capi/src/modern.rs:1206`) do:
  a cross-thread reader free (`test_sync_reader_handles_freed_on_another_thread`,
  `test_modern_capi.rs:692`), a concurrent `compare_exchange` increment loop
  that loses no update, NULL-handle behaviour, and the too-small contract
  (`buf` untouched, required length exact).
- **Miri and ASan coverage of the FFI layer, which does not exist today**: the
  Tier-1 Miri filter (AGENTS.md §5) and the `test-asan` job
  (`ci.yml:1791-1850`) run `-p expanse-trie` only. PR 1 adds a single-threaded
  `expanse-capi` test target to a Miri lane (the `'static` transmute and the
  `Box` round trip are what Miri checks) and a step running it under ASan;
  later PRs add their targets to both.
- The compile-time `Send` assertion on every new reader wrapper.
- `instruction-counts` showing no change on existing arms (the plan adds
  entry points, not engine paths; no `perf` claim is made).
- The surface-matrix 64-bit `std` row re-measured with the stated method, and
  the parts of this section the PR ships deleted.

#### 8. PR sequence

| PR | Ships | Depends on | Gate (one each) |
|---|---|---|---|
| 1 | `expanse_sync_strmap_*`: handles, reader, point ops, memory (14 symbols) | — | the string smoke and threaded tests green under ASan and Miri, with the new lanes added |
| 2 | String reader ordered reads (6) | 1 | an ordered walk built from `_reader_next_after` over a map under concurrent inserts returns every key present throughout, in order, with the too-small retry exercised |
| 3 | `expanse_sync_bytesmap_*` (14) and `expanse_sync_ordered_bytesmap_*` incl. its container ordered reads (20) | 1 | ordered-bytes keys with embedded NULs round-trip through insert, get and every ordered read |
| 4 | `expanse_sync_blob_map_*` with caller buffers and the new status values (20) | 1 | the get-into contract: absent, exact fit, one byte short (buffer untouched, required exact), probe, under a concurrent writer |
| 5 | RMW across the four: `compare_exchange` and `get_or_insert` per kind, blob `compare_exchange_meta` (9) | 1, 3, 4 | a threaded `compare_exchange` increment loop per kind loses no update, and the blob loop stops on a refusal |

Symbol counts are derived from the operation table above (14 + 6 + 34 + 20 +
9 = 83); the shipped header is the authority. PRs 3 and 4 can run in parallel
after PR 1.

#### 9. Decisions

The maintainer decided the plan's open design choices as follows.

1. **String keys:** NUL-terminated `const char *`, as `expanse_strmap_*`.
2. **Reader form:** the borrowed reader only, as the `u64` shared map.
3. **Ordered-bytes ordered reads:** shipped in the first version on the
   container handle, documented as blocking writers for the duration of the
   call. An optimistic `_reader_*` form, if the engine gains one, is additive.
4. **Blob status:** `expanse_blob_status_t` is extended within its existing
   ranges: `NOT_FOUND`, `MISMATCH` and `BUFFER_TOO_SMALL` as outcomes,
   `NO_META_FIELD` as a refusal.
5. **Single-threaded `expanse_blob_map_get_into`:** its prefix-copying
   behaviour is corrected in a separate pull request. The shared form's
   contract matches the corrected one: too small, nothing copied, required
   length reported.
6. **Tracking:** [#1538](https://github.com/orieg/expanse/issues/1538). The
   implementation PRs and their `STATED_DEFERRALS` entries cite #1538, not
   #1194.

### Slot pointer invalidation contract (`_slot`, `_ins_slot`)

Direct value slot accessors — `expanse_map_slot`, `expanse_map_ins_slot`, `expanse_strmap_slot`, `expanse_strmap_ins_slot`, `expanse_ordered_bytesmap_slot`, and `expanse_ordered_bytesmap_ins_slot` — return a direct writable pointer to the machine-word `ValueSlot` within the trie leaf array (classic JudyL convention).

The contract across all three containers:
- **Lifetime**: Pointers remain valid **only until the next structural mutation of that map** (any insert, remove, clear, or free call on that container).
- **In-place updates**: Reading and writing through the returned slot pointer (`*slot = val`) in-place does not invalidate other slot pointers and does not require re-traversing the trie.
- **Zero rotating ring buffers**: Pointers borrow directly from the leaf slot word; no thread-local scratch buffers or rotating ring buffers are used, upholding `AGENTS.md` §2.4.

## Acceptance gates ("in-place replacement" is proven, not claimed)

| Gate | Check |
|---|---|
| G1 Differential oracle | Randomized + adversarial op sequences produce identical observable results from libexpanse and stock libjudy (Linux CI job, `libjudy-dev`); see [TESTING.md](TESTING.md) |
| G2 php-judy Linux | php-judy test suite passes built against libexpanse (swap in `--with-judy` prefix via libjudy-compat) |
| G3 php-judy Windows | **Green**: the `php-judy-windows` CI job builds php-judy (pinned commit, config.w32 `--with-judy` system-lib mode: clean-room `Judy.h` + import library as `libJudy.lib`) against `expanse.dll` — no bundled C libjudy — and its suite passes (PHP 8.4 x64 NTS via php-windows-builder) |
| G4 Preload smoke | **Green**: `crates/expanse-capi/smoke/preload_smoke.c` (all four families), compiled against the stock header + library, produces identical transcripts stock vs `LD_PRELOAD`ed libexpanse — verified locally (macOS `DYLD_INSERT_LIBRARIES` flat-namespace) and as a step of the `differential-oracle` CI job |

## Doc-gap resolutions

| # | Question the docs leave open | Resolution |
|---|---|---|
| D1 | `Word_t` width on LLP64 (Windows): classic header used `unsigned long` | `Word_t` = `size_t` (pointer-width). ABI-identical to classic on LP64 Linux/macOS; 64-bit on Win64, which the targeted consumers (php-judy MSVC) require |
| D2 | `JU_ERRNO_*` numeric values (man pages name the codes, not numbers; the classic header is LGPL and unread) | libexpanse assigns its own stable numbering in `Judy.h`. Source-compatible (names match); numeric equality with classic builds is not guaranteed |
| D3 | Behavior on allocation failure | Allocation failure aborts (Rust global-allocator convention) instead of returning `JERR`/`PJERR` with `JU_ERRNO_NOMEM`. Recorded as a deviation; revisit if a consumer needs graceful OOM |
| D4 | `JudyHSFreeArray` byte total | The returned "bytes freed" is implementation-defined (libexpanse's hash-trie + bucket accounting differs from classic's internals). Both are nonzero for nonempty arrays; the oracle compares emptiness semantics, not byte totals — same stance as `MemUsed` |
| D5 | Convenience-macro grammar (`JLI`/`J1S`/…) | Statement blocks `{ … ; }`, not expressions. Black-box evidence: php-judy invokes several macros without trailing semicolons at statement position and compiles against classic system libjudy, so classic's macros tolerate that; the shipped macros match, accepting both `JLI(...)` and `JLI(...);`. Consequence: the macros cannot be used as expressions, and unbraced `if/else` around one needs braces (no observed consumer does either) |
| D6 | Control flow out of a caller-supplied callback (`expanse_map_remove_range_fn`, `expanse_map_for_each_range_fn`, `expanse_sync32_map_update_fn`, `expanse_bytesmap_iter_fn`, `expanse_predicate_fn`, `expanse_scan_cb_fn`) — the headers said what the callback must not *touch*, and nothing about how it must *return* | The callback must return normally. A C++ exception thrown through it, or a `longjmp` out of it, unwinds through Rust frames that are mid traversal or mid structural fix-up: no destructor runs and the map is left satisfying none of its invariants, with no diagnostic. Undefined behaviour, and not detectable by the library — `catch_unwind` cannot intercept a foreign exception, and catching a Rust panic mid-mutation would hand a torn map back to the caller, which is worse than aborting. A Rust panic in a callback is not an exception to the rule: `extern "C"` aborts the process rather than unwinding (verified: a panic crossing `extern "C"` reports "panic in a function that cannot unwind" and aborts, in every profile, and an outer `catch_unwind` does not intercept it), which is the defined and debuggable outcome. Callers must catch inside the callback |

Status: **all four families exported** — Judy1, JudyL, JudySL, JudyHS — with the shipped `Judy.h`, and **all four gates green**, each as a standing CI job. G1 runs the differential-oracle harness against a `dlopen`'d stock libjudy for all four families (randomized op sequences, full-sweep/rank agreement, byte-exact JudySL buffer sweeps, JudyHS byte-key sequences including zero-length keys). G2 passes the php-judy test suite built against libexpanse via the libjudy-compat prefix — 221/221 locally (macOS AArch64, PHP 8.5) and as the `php-judy-compat` Linux CI job.

---

## Cross-Language Feature & Container Parity

`scripts/check_abi_parity.py` checks C ABI symbol coverage in CI for the Java, .NET, Python, Node.js and Go bindings (the 64-bit surface; the `!EXPANSE_WIDE_SURFACE` block is excluded). Java, .NET and Go are checked by symbol name; Python and Node, which bind the Rust API, by a per-symbol `// abi-parity:` marker at the implementing site. The per-family function counts below are the declarations in `include/expanse.h`, and the parity row is `check_abi_parity.py --check` output: 151 symbols on the 64-bit surface, 151/151 for each of Java, .NET, Python, Node.js and Go (no stated deferrals); the 35 declarations of the `!EXPANSE_WIDE_SURFACE` block are listed separately. The PHP and Ruby bindings are not checked by it, and the Rust column is the implementation, not a binding:

| Container / Feature | C ABI (`expanse.h`) | Rust (`expanse-trie`) | Java 22+ (`expanse-java`) | .NET 9 (`Orieg.Expanse`) | Python (`expanse-trie`) | Node.js (`@orieg/expanse`) | PHP (`orieg/expanse`) | Ruby (`expanse`) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **`ExpanseSet` (Judy1)** | `expanse_set_*` (21 fns) | `ExpanseSet` | `ExpanseSet` | `ExpanseSet` | `ExpanseSet` | `ExpanseSet` | `Set` / `ExpanseSet` | `Expanse::Set` |
| **`ExpanseMap` (JudyL)** | `expanse_map_*` (25 fns + 2 32-bit-only; `expanse_sync32_*` 33 fns 32-bit-only, provisional) | `ExpanseMap` | `ExpanseMap` | `ExpanseMap` | `ExpanseMap` | `ExpanseMap` | `Map` / `ExpanseMap` | `Expanse::Map` |
| **`ExpanseBytesMap` (JudyHS)** | `expanse_bytesmap_*` (12 fns) | `ExpanseBytesMap` | `ExpanseBytesMap` | `ExpanseBytesMap` | `ExpanseBytesMap` | `ExpanseBytesMap` | `BytesMap` / `ExpanseBytesMap` | `Expanse::BytesMap` |
| **`ExpanseStrMap` (JudySL)** | `expanse_strmap_*` (19 fns) | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `StrMap` / `ExpanseStrMap` | `Expanse::StrMap` |
| **`ExpanseOrderedBytesMap` (#808)** | `expanse_ordered_bytesmap_*` (19 fns) | `ExpanseOrderedBytesMap` | Deferred (Stage 5(c) / #808) | Deferred (Stage 5(c) / #808) | Deferred (Stage 5(c) / #808) | Deferred (Stage 5(c) / #808) | Follow-up | Follow-up |
| **StrMap truncation-aware nav** | `expanse_strmap_*_ex` (6 fns) | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `StrMap` | `Expanse::StrMap` |
| **`SyncExpanseSet` (OCC Set)**| `expanse_sync_set_*` (11 fns; none is an ordered read: the Rust reader-handle ordered reads of #1194 have no C entry point, and the bindings none) | `SyncExpanseSet` | `SyncExpanseSet` | `SyncExpanseSet` | `SyncExpanseSet` | `SyncExpanseSet` | `SyncSet` | Via C ABI |
| **`SyncExpanseMap` (OCC Map)**| `expanse_sync_map_*` (20 fns, 6 of them reader-handle ordered reads) | `SyncExpanseMap` | `SyncExpanseMap` | `SyncExpanseMap` | `SyncExpanseMap` | `SyncExpanseMap` | `SyncMap` | Via C ABI |
| **`SyncExpanseOrderedBytesMap` (#808)**| Follow-up (`expanse_sync_ordered_bytesmap_*`, [planned](#planned-shared-map-c-handles-not-shipped)) | `SyncExpanseOrderedBytesMap` | Follow-up | Follow-up | Follow-up | Follow-up | Follow-up | Follow-up |
| **`ExpanseBlobMap` (Large-Value)**| `expanse_blob_map_*` (17 fns) | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `BlobMap` / `ExpanseBlobMap` | `Expanse::BlobMap` |
| **Atomic RMW (`compare_exchange` / `update`) (#1194)** | `expanse_sync_map_compare_exchange` and `expanse_sync_map_get_or_insert` (`SyncExpanseMap`); `update` is not exported (see below). The other shared maps are follow-ups; the 32-bit `expanse_sync32_map_writer_{compare_exchange,update,get_or_insert}` block is a separate surface with its own forms (a callback `update` in the narrow block); a blob form needs a status for an unstorable `new` distinct from its mismatch status, as the Rust `CompareExchangeError` has The string, bytes, ordered-bytes and blob shared maps have no C handle yet, so their C read-modify-write waits on a shared-handle surface. | `compare_exchange` (Map/Str/Bytes/Blob), `update` (Map/Str/Bytes) | `compareExchange`, `getOrInsert`, `update` (`SyncExpanseMap`) | `CompareExchange`, `GetOrInsert`, `Update` (`ExpanseSyncMap`) | `compare_exchange`, `get_or_insert`, `update_with` (`SyncExpanseMap`; `update` names the bulk merge there) | `compareExchange`, `getOrInsert`, `update` (`SyncExpanseMap`) | Follow-up | Follow-up |
| **Compaction (`compact`) (#1200)** | Follow-up | ✅ Set/Map, Set32/Map32, SyncSet/SyncMap | Follow-up | Follow-up | Follow-up | Follow-up | Follow-up | Follow-up |
| **32-bit direct emission (`from_sorted_iter`) (#1357)** | N/A (32-bit embedded) | ✅ `from_sorted_iter` (Set32/Map32) | N/A | N/A | N/A | N/A | N/A | N/A |
| **Rank/Select (`by_count`)** | ✅ All ordered types | ✅ `count_below`/`by_count` | ✅ `rank`/`select` | ✅ `Rank`/`ByCount` | ✅ `count_below`/`by_count` | ✅ `countRange`/`byCount` | ✅ `rank`/`select` | ✅ `rank`/`select` |
| **Metadata Filtering** | ✅ Predicate callbacks | ✅ Scalar per-entry predicate closure (`scan_filtered`) | ✅ Functional predicates | ✅ Delegated predicates | ✅ Predicate callbacks | ✅ Predicate callbacks | ✅ Callback predicates | ✅ Hot metadata |
| **Optimistic Concurrency** | ✅ Epoch-based OCC | ✅ `SeqVersion` atomics | ✅ Read-coupling handles | ✅ Reader handles | ✅ GIL-free thread queries | ✅ Event-loop safe | ✅ Optimistic OCC | ✅ GVL-safe FFI |
| **C ABI Symbol Parity** | **151 declared** | implementation (not checked) | **151 / 151 (100%)** | **151 / 151 (100%)** | **151 / 151 (100%)** | **151 / 151 (100%)** | not checked by `check_abi_parity.py` | not checked by `check_abi_parity.py` |

**Atomic read-modify-write across the C boundary (#1194).** The C ABI exports `compare_exchange` and `get_or_insert`; it does not export `update`. A callback form would cost an upcall per retry, and an exception or `longjmp` thrown inside the callback cannot unwind through C (D6). Each language binding instead implements `update` as a native retry loop over `compare_exchange`, as `SyncExpanseMap::update` does in Rust: read the key, compute the new word, `compare_exchange` against the word read, and on a mismatch retry with the observed word. The computation may therefore run several times and must be free of side effects. Absence is a flag on both sides (`expected_present`, `new_present`, `observed_present_out`), so every `u64` stays a legal value. `expanse_sync_map_compare_exchange` returns true when the store happened and reports the word the compare saw on both outcomes; `expanse_sync_map_get_or_insert` follows the `expanse_sync_map_insert` convention (true when stored, otherwise false with the existing value written through the out-pointer). The loop is written out in the `include/expanse.h` comment and in `expanse_sync(3)`. Only `SyncExpanseMap` has the C form; the string, bytes, ordered-bytes and blob shared maps and then the language bindings follow in that order; the 32-bit `expanse_sync32_map_writer_*` block is a separate surface with its own forms. Python and Node.js wrap the two symbols through the Rust methods and write the loop as `update_with(key, fn)` and `update(key, fn)` (Python's `update(other)` is the bulk merge); Go wraps them as `CompareExchange` and `GetOrInsert` with an absent-capable `Word` and writes the loop as `Update`; Java wraps them as `compareExchange` (an `Exchange` record) and `getOrInsert` over `OptionalLong` and writes the loop as `update`; .NET wraps them as `CompareExchange` and `GetOrInsert` over `ulong?` and writes the loop as `Update`. No stated deferral remains in `scripts/check_abi_parity.py`.

