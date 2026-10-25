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
| 64-bit, `std` (default) | `cargo build -p expanse-capi` | 278 |
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
because `modern_wide` compiles only on 64-bit targets. The fourteen
`expanse_sync_strmap_*` entry points (#1538) took the 64-bit `std` row from 195
to 209 (`nm -gU` on an arm64 macOS release dylib; 195 on their parent,
`eb351703`, by the same method); the 64-bit `no_std` and 32-bit rows are
unchanged by construction, as for the other concurrent entry points. The six
`expanse_sync_strmap_reader_*` ordered reads took the 64-bit `std` row from 209
to 215 by the same method. The thirty-four `expanse_sync_bytesmap_*` and
`expanse_sync_ordered_bytesmap_*` entry points (#1538) took it from 215 to 249
(`nm -gU` on an arm64 macOS release dylib; 215 on their parent, `2eff5bb4`, by
the same method). The twenty `expanse_sync_blob_map_*` entry points (#1538)
took it from 249 to 269 (`nm -gU` on an arm64 macOS release dylib; 249 on
their parent, `589356af`, by the same method). The nine read-modify-write
entry points of those four maps (#1538) took it from 269 to 278 (`nm -gU` on
an arm64 macOS release dylib; 269 on their parent, `41f1dcfc`, by the same
method).
Reproduce with the invocations above; the 32-bit row needs
`--features embedded-panic-handler`, as the CI job does.)

**Two exported symbols are declared in no header.** `expanse_validate(const void *array, int is_map)` and `expanse_stats(const void *array, int is_map, CExpanseStats *out)` (`crates/expanse-capi/src/lib.rs`, 64-bit only) are `#[no_mangle] extern "C"` and are counted in both 64-bit rows above, but neither `expanse.h` nor `Judy.h` declares them, so `scripts/check_abi_parity.py` does not see them and no binding wraps them. They take a `Judy1`/`JudyL` array pointer. They are not part of the supported surface until a header declares them; that is a code change, not a documentation one.

**64-bit `no_std` drops only the concurrent containers** — the 114
`expanse_sync_*` entry points (`nm -gU` count on the arm64 macOS release
dylib). `expanse_trie::sync` needs `std::sync`, so a
bare-metal 64-bit build has no one-writer/many-reader surface. Everything
else, the entire legacy `Judy*` drop-in included, is present and unchanged.

The shared string, bytes, ordered-bytes and blob maps have C handles with
point operations and read-modify-write, and the string and ordered-bytes maps
ordered reads (`expanse_sync_strmap_*`, `expanse_sync_bytesmap_*`,
`expanse_sync_ordered_bytesmap_*`, `expanse_sync_blob_map_*`, 64-bit `std`
only; [decisions and binding status](#shared-map-c-handles-decisions-and-binding-status)).

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

### `expanse_blob_map_get_into` refuses a short buffer (`expanse.h` behaviour change)

`expanse_blob_map_get_into` used to copy as many bytes as fit and return `true`, so a caller with a short buffer received a partial value that looked like success. It now copies the whole payload or nothing, matching the Rust `get_into` on the blob readers (`BlobBufferTooSmall`). The signature is unchanged (`bool`, same parameters). `true`: present and fitted, `*out_len` is the length, bytes of `buf` past it are untouched. `false` with `*out_len == 0`: absent (`*out_len` was previously left untouched). `false` with `*out_len > buf_len`: the buffer was too short, `buf` is untouched, `*out_len` is the required length and `*out_meta` the hot metadata. `buf == NULL` counts as length 0, so `NULL` with `buf_len == 0` queries the length. A caller that relied on truncation, or that ignored `false` after a short buffer, must size `buf` from `*out_len` and retry. Belongs in the next release notes. None of the language bindings calls the symbol beyond declaring it; `include/expanse.hpp`'s `blob_map::get_into` forwards it and documents the contract. `Judy.h` is unaffected.

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

The contract for `expanse_sync_set_reader_t`, `expanse_sync_map_reader_t`, `expanse_sync_strmap_reader_t`, `expanse_sync_bytesmap_reader_t`, `expanse_sync_ordered_bytesmap_reader_t` and `expanse_sync_blob_map_reader_t`, stated next to the declarations in `include/expanse.h` and in `expanse_sync(3)`:

| Operation | Contract |
|---|---|
| Use | One handle per reading thread. A handle owns one epoch slot and its pins are not reentrant, so calls on one handle from two threads at once are undefined. |
| Free from another thread | Allowed, once no call on the handle is in progress. Every call releases its epoch pin before returning, and `_reader_free` deregisters through the container's registry lock without reading thread-local state. |
| Free while another thread is inside a call on it | Undefined. |
| Free after the container | Undefined. Free every reader handle before `expanse_sync_*_free`. |

In Rust the *Use* row is enforced by the type system: `occ::Reader` and every `sync` reader handle are `Send` and not `Sync`, so safe code cannot call through one handle from two threads (`docs/ARCHITECTURE.md` §4.1, *One reader handle per thread*). A C handle is a raw pointer and carries no such bound, and neither do the FFI bindings built on it (Go `SyncMapReader`/`SyncSetReader`, Java `SyncExpanseMap.Reader`/`SyncExpanseSet.Reader`, .NET `ExpanseSyncMapReader`/`ExpanseSyncSetReader`, C++ `sync_map_reader`/`sync_set_reader`): there the row stays a caller obligation. The bindings that never hand a reader to the caller are not exposed to it — Python keeps one reader per thread in a thread-local cache, and Node registers one per call.

The cross-thread free rests on the handle types being `Send`. That bound is auto-derived from their fields, so it is asserted at compile time on the C handle types (`SyncSetReader`, `SyncMapReader`, `SyncStrMapReader`, `SyncBytesMapReader`, `SyncOrderedBytesMapReader` and `SyncBlobMapReader` in `crates/expanse-capi/src/modern_sync.rs`), on `MapReader`, `OwnedMapReader`, `DetachedMapReader`, `SetReader`, `OwnedSetReader`, `DetachedSetReader`, `StrReader`, `OwnedStrReader`, `DetachedStrReader`, `BytesReader`, `OwnedBytesReader`, `DetachedBytesReader`, `OrderedBytesReader`, `OwnedOrderedBytesReader`, `DetachedOrderedBytesReader`, `BlobReader`, `OwnedBlobReader` and `DetachedBlobReader` (`crates/expanse/src/sync.rs`) and on `occ::Reader` (`crates/expanse/src/occ.rs`): a future `!Send` field fails the build. `test_sync_reader_handles_freed_on_another_thread` (`crates/expanse-capi/tests/test_modern_capi.rs`) and, for the string, bytes, ordered-bytes and blob maps, `reader_handles_freed_on_another_thread` (`crates/expanse-capi/tests/test_sync_strmap_capi.rs`), its two counterparts in `test_sync_bytesmap_capi.rs` and its counterpart in `test_sync_blobmap_capi.rs` create handles on worker threads, read through them, and free them on the main thread before the containers. The 32-bit `expanse_sync32_*` family has its own, index-addressed reader contract (see the 32-bit concurrent story above).

### Shared string map C handle (`expanse_sync_strmap_*`)

`expanse_sync_strmap_t` is the C handle of `SyncExpanseStrMap` and
`expanse_sync_strmap_reader_t` the C handle of its borrowed `StrReader`, with
the `u64` shared map's conventions throughout
(`crates/expanse-capi/src/modern_sync.rs`, `include/expanse.h`,
`expanse_sync(3)`, #1538). Twenty-two entry points: `_new`, `_free`,
`_insert`, `_get`, `_contains`, `_remove`, `_len`, `_mem_used`, `_mem_held`,
`_shrink_to_fit`, `_reader_new`, `_reader_free`, `_reader_get`,
`_reader_contains`, the reader's ordered reads `_reader_first`,
`_reader_last`, `_reader_next_at_or_after`, `_reader_next_after`,
`_reader_prev_at_or_before`, `_reader_prev_before`, and the
[read-modify-write](#shared-map-read-modify-write-_compare_exchange-_get_or_insert) `_compare_exchange` and
`_get_or_insert`.

- **Handles.** The container handle is the boxed Rust map, callable from any
  number of threads at once except `_free`; the reader wraps the borrowed
  `StrReader` with its lifetime extended to `'static`, upheld by the header's
  rule that a reader is freed before its map. No owned or detached reader form
  and no pinned view crosses the boundary: every reader call pins the epoch
  for that call only. The [reader-handle contract](#reader-handle-ownership-64-bit-expanse_sync_)
  above applies unchanged.
- **Keys** are NUL-terminated `const char *`, read as `expanse_strmap_*` reads
  them; a C string cannot carry an interior NUL, so no key is refused for its
  content, and keys with NUL bytes go to the ordered-bytes map. A `NULL` key
  behaves like a `NULL` handle: `false` or `0`, nothing written.
- **Values and returns** are those of `expanse_sync_map_*`: `bool` with
  `uint64_t` out-pointers, each nullable and written only on `true`, except
  `_insert`, which returns `true` for a new key and otherwise writes the
  replaced value through `old_out`. `_mem_used` and `_mem_held` read with
  writers excluded; `_shrink_to_fit` excludes neither.
- **Ordered reads** are stateless calls on the reader, each one optimistic
  read of `StrReader` with the retry budget and writer-excluding fallback of
  `_reader_get`; no cursor object crosses the boundary. The key goes to a
  caller buffer under the `expanse_strmap_*_ex` contract and its
  `expanse_str_nav_status`: `OK` writes the NUL-terminated key,
  `required_len` and the value; `BUFFER_TOO_SMALL` writes only
  `required_len` (key length plus the NUL), and `NULL` with `0` is a length
  probe; `NOT_FOUND` (no such entry, a `NULL` reader or a `NULL` key) writes
  nothing. A retry after `BUFFER_TOO_SMALL` is a new read and may return a
  different key if a writer ran in between. `StrReader` returns the key as an
  owned `Vec`, so each call allocates once until the engine has a reader
  method that copies into a caller buffer.
- **Tests.** `crates/expanse-capi/tests/test_sync_strmap_capi.rs` covers every
  entry point, the `NULL` conventions, the reader lifecycle, the buffer
  contract of the ordered reads (probe, one byte short, exact fit), readers
  that never see a wrong value under a churning writer, an ordered walk under
  concurrent inserts and removes that returns every stable key in order with
  the too-small retry taken, and cross-thread reader frees; its single-threaded tests run under Miri (`ci.yml` job `miri`, shard
  3) and the whole target under ASan (job `test-asan`).
  `crates/expanse-capi/smoke/modern_api_smoke.c` calls every entry point once
  and runs a pthread writer against four reader threads, each of which also
  walks the map in order.

### Shared bytes and ordered-bytes map C handles (`expanse_sync_bytesmap_*`, `expanse_sync_ordered_bytesmap_*`)

`expanse_sync_bytesmap_t` is the C handle of `SyncExpanseBytesMap` (default
`RandomState` hasher) and `expanse_sync_ordered_bytesmap_t` that of
`SyncExpanseOrderedBytesMap`; their readers wrap the borrowed `BytesReader`
and `OrderedBytesReader` (`crates/expanse-capi/src/modern_sync.rs`,
`include/expanse.h`, `expanse_sync(3)`, #1538). Each map has fourteen entry
points with the string map's names and conventions (`_new`, `_free`,
`_insert`, `_get`, `_contains`, `_remove`, `_len`, `_mem_used`, `_mem_held`,
`_shrink_to_fit`, `_reader_new`, `_reader_free`, `_reader_get`,
`_reader_contains`) and the [read-modify-write](#shared-map-read-modify-write-_compare_exchange-_get_or_insert)
`_compare_exchange` and `_get_or_insert`, sixteen each; the ordered-bytes map
adds six ordered reads on its container handle, `_first`, `_last`,
`_next_at_or_after`, `_next_after`, `_prev_at_or_before` and `_prev_before`:
thirty-eight entry points.

- **Handles** follow the [string map's](#shared-string-map-c-handle-expanse_sync_strmap_):
  boxed container callable from any number of threads at once except `_free`,
  borrowed reader with its lifetime extended to `'static`, no owned, detached
  or pinned form across the boundary, and the
  [reader-handle contract](#reader-handle-ownership-64-bit-expanse_sync_).
- **Keys** are `(const void *key, size_t len)`, as `expanse_bytesmap_*` and
  `expanse_ordered_bytesmap_*` take them: opaque bytes, embedded NULs ordinary,
  `NULL` with `0` the empty key. A `NULL` key with a non-zero length behaves
  like a `NULL` handle: `false` or `0`, nothing written. The ordered-bytes map
  escapes NULs itself.
- **Values and returns** are those of `expanse_sync_strmap_*`.
- **Ordered reads of the ordered-bytes map block writers.**
  `SyncExpanseOrderedBytesMap` has no optimistic ordered read: its navigation
  runs under `with_locked`, so the six calls take the map handle, not a reader,
  and each quiesces writers and takes the writer mutex for its duration
  ([decision 3](#shared-map-c-handles-decisions-and-binding-status)). The key goes to a caller buffer
  under the `expanse_ordered_bytesmap_*` navigation contract and its
  `expanse_ordered_bytes_nav_status`: `OK` writes the key, `required_len` (its
  length; an empty key needs no buffer) and the value; `BUFFER_TOO_SMALL`
  writes only `required_len` and leaves the buffer untouched, and `NULL` with
  `0` is a length probe; `NOT_FOUND` (no such entry, a `NULL` map, or a `NULL`
  key with a non-zero length) writes nothing. `key` and `key_out` may be one
  buffer. A retry after `BUFFER_TOO_SMALL` is a new read.
- **How a call keeps the buffer contract.** Each call is one `with_locked`
  hold in which the single-threaded map's `*_decode_into` form runs into an
  empty buffer for the length and then, if the caller's buffer holds the key,
  into that buffer; both runs see one map, so the length reported and the key
  written agree. One decode straight into the caller's buffer would not do:
  `escape_decode_into` (`crates/expanse/src/domain.rs`) writes the prefix that
  fits before it reports `BufferTooSmall`, so a too-small buffer would be
  written for a key holding a `0x00` or `0x01` byte. The wrapper's own
  `SyncExpanseOrderedBytesMap::*_decode_into` methods take the lock once per
  call, so a probe-then-decode pair over them could see two different maps.
  The search key is copied before the hold, which is what lets `key` and
  `key_out` share a buffer. A decode error other than `BufferTooSmall` is
  unreachable for keys the map encoded and is a `debug_assert!`, not a silent
  `NOT_FOUND` (§8.1).
- **Tests.** `crates/expanse-capi/tests/test_sync_bytesmap_capi.rs` runs the
  fourteen shared entry points of each map through one table of function
  pointers (return conventions, keys with NULs and `0x01`, the `NULL`
  conventions, the reader lifecycle, readers that never see a wrong value
  under a churning writer, cross-thread reader frees), and the ordered reads:
  keys with embedded NULs round-trip through insert, get and every ordered
  read, including a walk that reuses its output buffer as the search key; the
  buffer contract (probe, one byte short with an escaped key and the buffer
  untouched, exact fit, `NULL` inputs, the empty key, an empty map); and a
  walk under concurrent inserts and removes that returns every stable key in
  order with the too-small retry taken. Its single-threaded tests run under
  Miri (`ci.yml` job `miri`, shard 2) and the whole target under ASan (job
  `test-asan`). `crates/expanse-capi/smoke/modern_api_smoke.c` calls every
  entry point once and runs a pthread writer against four reader threads on
  each map; on the ordered-bytes map each reader also walks the map in order.

### Shared blob map C handle (`expanse_sync_blob_map_*`)

`expanse_sync_blob_map_t` is the C handle of `SyncExpanseBlobMap`, and
`expanse_sync_blob_map_reader_t` wraps the borrowed `BlobReader`
(`crates/expanse-capi/src/modern_sync.rs`, `include/expanse.h`,
`expanse_sync(3)`, #1538). Twenty-three entry points: `_new(chunk_size)`,
`_new_with_capacity(chunk_size, max_capacity)`, `_free`, `_insert`, `_remove`,
`_get_into`, `_get_meta`, `_contains`, `_len`, `_mem_used`,
`_mem_used_estimate`, `_mem_held`, `_shrink_to_fit`, `_set_reclaim_at_cap`,
`_compact`, `_reader_new`, `_reader_free`, `_reader_get_into`,
`_reader_get_meta`, `_reader_contains`, and the
[read-modify-write](#shared-map-read-modify-write-_compare_exchange-_get_or_insert) `_compare_exchange`,
`_compare_exchange_meta` and `_get_or_insert`.

- **Handles** follow the [string map's](#shared-string-map-c-handle-expanse_sync_strmap_)
  and the [reader-handle contract](#reader-handle-ownership-64-bit-expanse_sync_).
  The reader's calls take a non-`const` handle, because `BlobReader`'s reads
  take `&mut self`. `BlobReader::pin` and its guard are not exposed: no epoch
  pin outlives a call.
- **Keys** are `uint64_t`, as `expanse_blob_map_*` takes them. **Payloads in**
  are `(const uint8_t *data, size_t len, uint32_t hot_meta)`, `NULL` with `0`
  the empty payload.
- **Values out are copied, never borrowed** (AGENTS.md §2.3 item 4, §2.4): no
  `ExpanseBlobView`, thread-local or rotating buffer on the shared surface.
  `_get_into` and `_reader_get_into` return `expanse_blob_status_t`: `OK` with
  the payload in `buf[0 .. *len_out)`, its metadata in `*meta_out` and the
  rest of `buf` untouched; `NOT_FOUND` with nothing written;
  `BUFFER_TOO_SMALL` with **nothing copied**, `buf` untouched, and the
  payload's length and metadata in `*len_out` and `*meta_out`;
  `INVALID_ARGUMENT` for a `NULL` handle, a `NULL` buffer with a non-zero
  length, or a length above `PTRDIFF_MAX`. `NULL` with `0` is the length probe.
  A retry with the reported length is a new read: it succeeds unless a writer
  replaced or removed the value in between, and then reports the new state.
- **How a call keeps the buffer contract.** Each `_get_into` is one call to
  `BlobReader::get_into` (the one-shot form on a throwaway reader). That read
  takes the length, the metadata and the bytes from one validated optimistic
  read, or from one hold of the writer-excluding fallback, and checks the
  length before it writes (`blob_copy_into` in `crates/expanse/src/sync.rs`),
  so the length and metadata reported with `BUFFER_TOO_SMALL` belong to the
  value that would have been copied, and no probe precedes the copy.
- **Status values.** `expanse_blob_status_t` gains `EXPANSE_BLOB_NOT_FOUND`
  (`1`), `EXPANSE_BLOB_BUFFER_TOO_SMALL` (`2`) and `EXPANSE_BLOB_MISMATCH`
  (`3`) in its outcome band and `EXPANSE_BLOB_NO_META_FIELD` (`20`) in its
  refusal band ([decision 4](#shared-map-c-handles-decisions-and-binding-status)); existing values are
  unchanged, and `expanse_blob_map_insert_ex` returns none of the four. `_insert` returns
  `expanse_blob_map_insert_ex`'s statuses, and `_compact` returns `OK`, the
  mapped refusal, or `INVALID_ARGUMENT` for a `NULL` map. `_get_meta`,
  `_contains` and `_remove` return `bool`.
- **Metadata on short payloads.** A value is stored in the slot word — a
  payload of 7 bytes or fewer, or one the codec compresses into it — only when
  it is inserted with `hot_meta == 0`, and it then reads back metadata `0`. A
  payload of any length inserted with non-zero metadata is an arena record
  and reads it back, and `scan`-style filtering selects it by it; `hot_meta`
  wider than 24 bits is `EXPANSE_BLOB_META_OVERFLOW` at every length. This is
  `ExpanseBlobMap::insert`'s rule (AGENTS.md §2.3 item 2,
  [#1548](https://github.com/orieg/expanse/issues/1548)), and it holds on the
  single-threaded `expanse_blob_map_*` surface too. The header states it, and
  `crates/expanse-capi/tests/test_sync_blobmap_capi.rs` pins it.
- **Memory.** `_mem_used` and `_mem_held` exclude writers and wait for a
  running compaction; `_mem_used_estimate` takes no lock. `_compact` and
  `_set_reclaim_at_cap` are the Rust methods; since the shared surface hands
  out no view, a compaction invalidates nothing a C caller holds.
- **Tests.** `crates/expanse-capi/tests/test_sync_blobmap_capi.rs`: return
  conventions; every value representation (raw inline at 0, 3 and 7 bytes,
  each codec, compressible and short payloads with metadata, arena values
  just past the codecs and at 1000 bytes) checked against the representation
  the engine chose, through both `_get_into` forms, and one key moved across
  every representation boundary in both directions; the buffer contract on
  every representation (absent, probe, one byte short and an empty buffer
  with the buffer untouched and the required length exact, exact fit, spare
  room left untouched, usage errors); `NULL` handles; the cap, the reclaim
  switch, compaction and each refusal; and, threaded, a writer cycling keys
  through inline, arena and compressed values and absences while compacting,
  against four readers that require every `OK` to be one whole value with its
  own metadata and every `BUFFER_TOO_SMALL` to report the long value's length
  and metadata together with the buffer untouched, plus cross-thread reader
  frees. The single-threaded tests run under Miri (`ci.yml` job `miri`, shard
  3) and the whole target under ASan (job `test-asan`).
  `crates/expanse-capi/smoke/modern_api_smoke.c` calls every entry point once
  and runs a pthread writer against four reader threads.

### Shared-map read-modify-write (`*_compare_exchange`, `*_get_or_insert`)

The string, bytes, ordered-bytes and blob maps' handles export the Rust
maps' `compare_exchange` and `get_or_insert` (#1538; the Rust operations:
[#1194](https://github.com/orieg/expanse/issues/1194)), nine entry points:
`expanse_sync_{strmap,bytesmap,ordered_bytesmap}_{compare_exchange,get_or_insert}`
and `expanse_sync_blob_map_{compare_exchange,compare_exchange_meta,get_or_insert}`
(`crates/expanse-capi/src/modern_sync.rs`, `include/expanse.h`,
`expanse_sync(3)`). Each is one call of the Rust method on the container
handle, so it is the same single linearizable step, with the same optimistic
form and exclusive fallback; the shim adds no engine path.
`SyncExpanseOrderedBytesMap::get_or_insert`, like the string and bytes
maps', is `compare_exchange(key, None, Some(value))` on the escaped key.

- **Word-valued maps** take the `u64` map's convention unchanged
  (`expanse_sync_map_compare_exchange`, `expanse_sync_map_get_or_insert`):
  presence flags on both sides, `bool` returns, the observed word reported on
  both outcomes of `_compare_exchange`, the existing word through
  `existing_out` when `_get_or_insert` returns false. A `NULL` handle, a `NULL`
  string key, or a `NULL` byte-string key with a non-zero length returns
  false and writes nothing.
- **Blob map** calls return `expanse_blob_status_t`. A value is its payload
  and its metadata, and `_compare_exchange` compares both. `OK`: stored (or
  removed), nothing written. `EXPANSE_BLOB_MISMATCH` (`3`): nothing stored;
  the current value (for `_get_or_insert`, the present one) is copied into the
  caller's buffer with its length, metadata and, for `_compare_exchange`,
  presence. `BUFFER_TOO_SMALL`: a mismatch whose payload does not fit;
  nothing copied, length and metadata written. The value reported is the one
  the Rust call returned, so length, metadata and bytes come from the step
  that compared; it is copied after that call has returned, which is what
  lets the output buffer be the buffer passed as `expected_data` or `data`.
  `_get_or_insert` writes the buffer only when the key was present, so a
  too-small buffer never accompanies an insert. The refusal band maps
  `CompareExchangeError::Unstorable` (`crates/expanse/src/blobmap.rs`)
  through `From<ArenaError>` (`crates/expanse-capi/src/blobmap.rs`):
  `MetaOverflow`, `AllocationFailed`, `OffsetOverflow` → `CAP_REFUSED` (these
  calls run no reclaim at the cap), `ArenaFull`, and `NoMetaField` →
  `EXPANSE_BLOB_NO_META_FIELD` (`20`). A refusal is reported only when the
  comparison held, as the Rust error is (#1461), so a loop that retries on
  `MISMATCH` and stops on a refusal ends. `_compare_exchange_meta` swaps the
  24-bit metadata in the slot with no payload copy; `NO_META_FIELD` is its
  refusal for an absent key or non-zero metadata on a payload stored in its
  slot word (one inserted with metadata 0). A payload of 7 bytes or fewer
  inserted with metadata keeps it
  ([#1548](https://github.com/orieg/expanse/issues/1548)), so expecting that
  metadata matches, and `_compare_exchange_meta` swaps it in place.
- **Tests.** In each map's target (`test_sync_strmap_capi.rs`,
  `test_sync_bytesmap_capi.rs` for both byte-string maps,
  `test_sync_blobmap_capi.rs`): every branch of each exchange, the `NULL`
  conventions and, for the blob map, the buffer contract (probe, one byte
  short, exact fit, nothing written on `OK` or a refusal), the refusals
  against a holding and a failing comparison, a retry loop that stops on a
  refusal, an output buffer that is also the expected payload, and short
  payloads that keep their metadata (#1548). Threaded: concurrent `_compare_exchange` increments (and, on the
  blob map, `_compare_exchange_meta` increments) that lose no update, and
  threads racing `_get_or_insert` on each key that agree on one winner and
  its value. The single-threaded tests run under Miri and the whole targets
  under ASan, as above. `crates/expanse-capi/smoke/modern_api_smoke.c` calls
  every entry point once and runs four pthreads through the retry loops on
  each map.

### Shared-map C handles: decisions and binding status

The design choices of the shared-map handles (#1538) were decided as follows.

1. **String keys:** NUL-terminated `const char *`, as `expanse_strmap_*`.
2. **Reader form:** the borrowed reader only, as the `u64` shared map.
3. **Ordered-bytes ordered reads:** on the container handle, documented as
   blocking writers for the duration of the call. An optimistic `_reader_*`
   form, if the engine gains one, is additive.
4. **Blob status:** `expanse_blob_status_t` is extended within its existing
   ranges: `NOT_FOUND`, `BUFFER_TOO_SMALL` and `MISMATCH` as outcomes,
   `NO_META_FIELD` as a refusal.
5. **Single-threaded `expanse_blob_map_get_into`:** its prefix-copying
   behaviour is corrected separately; the shared form's contract is the
   corrected one: too small, nothing copied, required length reported.
6. **Tracking:** [#1538](https://github.com/orieg/expanse/issues/1538); the
   `STATED_DEFERRALS` entries cite it.

Not exported, with the reason: **`with_locked` / `with_exclusive`** run
caller code under the writer mutex, which across C is a callback (D6, Atomic
RMW paragraph below), and a callback calling a writing entry point on the
same handle would wait on a mutex its own thread holds; **`update`** is a
caller-side retry loop over `_compare_exchange`, as for the `u64` map;
**`clear`, `stats`, `collector_*`, `layout_census`**, which the `u64` shared
handle does not export either; **a cursor object**, since no Rust cursor
exists for these maps and the `u64` precedent is stateless ordered reads.

**Binding status.** Python and Node bind the Rust types directly and wrap
these maps natively, with an `// abi-parity:` marker per symbol (Node and Python do,
below); Go,
Java and .NET reach the `u64` shared map through C and wrap these handles the
same way, with the reader-per-thread obligation a caller obligation. **Go
wraps all four** (`SyncStrMap`, `SyncBytesMap`, `SyncOrderedBytesMap`,
`SyncBlobMap`, each with its reader, in both the cgo and purego builds), and
**Java wraps all four** (`SyncExpanseStrMap`, `SyncExpanseBytesMap`,
`SyncExpanseOrderedBytesMap`, `SyncExpanseBlobMap`, `docs/bindings/java.md`
§6), and so does **.NET** (`ExpanseSyncStrMap`, `ExpanseSyncBytesMap`,
`ExpanseSyncOrderedBytesMap`, `ExpanseSyncBlobMap`, each with a reader class
that holds a reference count on its map's `SafeHandle`, so the map cannot be
freed before the reader). The .NET string-map key is a NUL-terminated UTF-8
C string, so a key with an embedded NUL character is rejected with
`ArgumentException` rather than truncated, and the .NET blob wrappers run the
`BUFFER_TOO_SMALL` retry loop internally and return a `byte[]`. **Node.js
wraps all four** (`SyncExpanseStrMap`, `SyncExpanseBytesMap`,
`SyncExpanseOrderedBytesMap`, `SyncExpanseBlobMap`, `crates/expanse-node/src/sync_maps.rs`)
over the Rust types, as `SyncExpanseMap` does: each call registers a
throwaway reader, so no reader object reaches JavaScript and the
reader-per-thread obligation does not arise. The string map rejects an
embedded NUL (`InvalidArg`, as `ExpanseStrMap` does), the bytes maps take a
`Buffer`, `Uint8Array` or string, the ordered-bytes ordered reads block
writers for the call, and the blob reads return owned `Buffer`s (a
`BUFFER_TOO_SMALL` outcome has no JavaScript form; `getWithMeta` adds
`isInline`). A blob exchange or `getOrInsert` the arena or the metadata field
refuses throws, where the C forms return a refusal status; a payload of 7
bytes or fewer keeps the metadata it was inserted with
([#1548](https://github.com/orieg/expanse/issues/1548)). None of the four
carries a `STATED_DEFERRALS` entry for Node.js; every
`expanse_sync_strmap_*`, `expanse_sync_bytesmap_*`,
`expanse_sync_ordered_bytesmap_*` and `expanse_sync_blob_map_*` symbol is
a `STATED_DEFERRALS` entry; none remains.
**Python wraps all four** (`SyncExpanseStrMap`, `SyncExpanseBytesMap`,
`SyncExpanseOrderedBytesMap`, `SyncExpanseBlobMap`, `docs/bindings/python.md`
§4.1) over the Rust types. The classes take the reader through a per-thread
cache of the Rust detached reader (the `SyncExpanseMap` pattern), so the
reader-handle obligation of the C surface is not the caller's;
`SyncExpanseOrderedBytesMap`, which has no detached reader, registers a reader
per point read. Every call releases the GIL, and the ordered reads of
`SyncExpanseOrderedBytesMap` exclude writers for their duration, as the C
container reads do. A blob exchange or `get_or_insert` returns a status string
and the observed value, as the C forms return a status.
The Go wrappers copy every key and
payload out (a `BUFFER_TOO_SMALL` read is retried inside the wrapper at the
reported length, as a new read), keep each reader's map reachable, and keep no
C pointer past the call that produced it. PHP, Ruby and the C++ `expanse.hpp`
wrapper are not checked by that script.

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

`scripts/check_abi_parity.py` checks C ABI symbol coverage in CI for the Java, .NET, Python, Node.js and Go bindings (the 64-bit surface; the `!EXPANSE_WIDE_SURFACE` block is excluded). Java, .NET and Go are checked by symbol name; Python and Node, which bind the Rust API, by a per-symbol `// abi-parity:` marker at the implementing site. The per-family function counts below are the declarations in `include/expanse.h`, and the parity row is `check_abi_parity.py --check` output: 234 symbols on the 64-bit surface, 234/234 for Go, Java, .NET, Python and Node.js (no stated deferrals remain; the `expanse_sync_strmap_*`, `expanse_sync_bytesmap_*`, `expanse_sync_ordered_bytesmap_*` and `expanse_sync_blob_map_*` ones, #1538, were the last); the 35 declarations of the `!EXPANSE_WIDE_SURFACE` block are listed separately. The PHP and Ruby bindings are not checked by it, and the Rust column is the implementation, not a binding:

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
| **`SyncExpanseStrMap` (OCC StrMap)**| `expanse_sync_strmap_*` (22 fns: point ops, RMW and a reader handle, 6 of them reader-handle ordered reads) | `SyncExpanseStrMap` | `SyncExpanseStrMap` | `ExpanseSyncStrMap` | `SyncExpanseStrMap` | `SyncExpanseStrMap` | Follow-up | Follow-up |
| **`SyncExpanseBytesMap` (OCC BytesMap)**| `expanse_sync_bytesmap_*` (16 fns: point ops, RMW and a reader handle) | `SyncExpanseBytesMap` | `SyncExpanseBytesMap` | `ExpanseSyncBytesMap` | `SyncExpanseBytesMap` | `SyncExpanseBytesMap` | Follow-up | Follow-up |
| **`SyncExpanseOrderedBytesMap` (#808)**| `expanse_sync_ordered_bytesmap_*` (22 fns: point ops, RMW and a reader handle, 6 of them writer-excluding ordered reads on the container) | `SyncExpanseOrderedBytesMap` | `SyncExpanseOrderedBytesMap` | `ExpanseSyncOrderedBytesMap` | `SyncExpanseOrderedBytesMap` | `SyncExpanseOrderedBytesMap` | Follow-up | Follow-up |
| **`SyncExpanseBlobMap` (OCC BlobMap)**| `expanse_sync_blob_map_*` (23 fns: point ops, caller-buffer reads and RMW, and a reader handle) | `SyncExpanseBlobMap` | `SyncExpanseBlobMap` | `ExpanseSyncBlobMap` | `SyncExpanseBlobMap` | `SyncExpanseBlobMap` | Follow-up | Follow-up |
| **`ExpanseBlobMap` (Large-Value)**| `expanse_blob_map_*` (17 fns) | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `BlobMap` / `ExpanseBlobMap` | `Expanse::BlobMap` |
| **Atomic RMW (`compare_exchange` / `update`) (#1194)** | `expanse_sync_map_compare_exchange` and `expanse_sync_map_get_or_insert` (`SyncExpanseMap`); the string, bytes and ordered-bytes maps' `_compare_exchange` / `_get_or_insert` and the blob map's `_compare_exchange`, `_compare_exchange_meta` / `_get_or_insert` ([#1538](#shared-map-read-modify-write-_compare_exchange-_get_or_insert)); `update` is not exported (see below). The 32-bit `expanse_sync32_map_writer_{compare_exchange,update,get_or_insert}` block is a separate surface with its own forms (a callback `update` in the narrow block). | `compare_exchange` (Map/Str/Bytes/Blob), `update` (Map/Str/Bytes) | `compareExchange`, `getOrInsert`, `update` (`SyncExpanseMap`) | `CompareExchange`, `GetOrInsert`, `Update` (`ExpanseSyncMap`, `ExpanseSyncStrMap`, `ExpanseSyncBytesMap`, `ExpanseSyncOrderedBytesMap`); `CompareExchange`, `CompareExchangeMeta`, `GetOrInsert` (`ExpanseSyncBlobMap`) | `compare_exchange`, `get_or_insert`, `update_with` (`SyncExpanseMap`; `update` names the bulk merge there) | `compareExchange`, `getOrInsert`, `update` (`SyncExpanseMap`) | Follow-up | Follow-up |
| **Compaction (`compact`) (#1200)** | Follow-up | ✅ Set/Map, Set32/Map32, SyncSet/SyncMap | Follow-up | Follow-up | Follow-up | Follow-up | Follow-up | Follow-up |
| **32-bit direct emission (`from_sorted_iter`) (#1357)** | N/A (32-bit embedded) | ✅ `from_sorted_iter` (Set32/Map32) | N/A | N/A | N/A | N/A | N/A | N/A |
| **Rank/Select (`by_count`)** | ✅ All ordered types | ✅ `count_below`/`by_count` | ✅ `rank`/`select` | ✅ `Rank`/`ByCount` | ✅ `count_below`/`by_count` | ✅ `countRange`/`byCount` | ✅ `rank`/`select` | ✅ `rank`/`select` |
| **Metadata Filtering** | ✅ Predicate callbacks | ✅ Scalar per-entry predicate closure (`scan_filtered`) | ✅ Functional predicates | ✅ Delegated predicates | ✅ Predicate callbacks | ✅ Predicate callbacks | ✅ Callback predicates | ✅ Hot metadata |
| **Optimistic Concurrency** | ✅ Epoch-based OCC | ✅ `SeqVersion` atomics | ✅ Read-coupling handles | ✅ Reader handles | ✅ GIL-free thread queries | ✅ Event-loop safe | ✅ Optimistic OCC | ✅ GVL-safe FFI |
| **C ABI Symbol Parity** | **234 declared** | implementation (not checked) | **234 / 234** | **234 / 234** | **234 / 234** | **234 / 234** | not checked by `check_abi_parity.py` | not checked by `check_abi_parity.py` |

**Atomic read-modify-write across the C boundary (#1194).** The C ABI exports `compare_exchange` and `get_or_insert`; it does not export `update`. A callback form would cost an upcall per retry, and an exception or `longjmp` thrown inside the callback cannot unwind through C (D6). Each language binding instead implements `update` as a native retry loop over `compare_exchange`, as `SyncExpanseMap::update` does in Rust: read the key, compute the new word, `compare_exchange` against the word read, and on a mismatch retry with the observed word. The computation may therefore run several times and must be free of side effects. Absence is a flag on both sides (`expected_present`, `new_present`, `observed_present_out`), so every `u64` stays a legal value. `expanse_sync_map_compare_exchange` returns true when the store happened and reports the word the compare saw on both outcomes; `expanse_sync_map_get_or_insert` follows the `expanse_sync_map_insert` convention (true when stored, otherwise false with the existing value written through the out-pointer). The loop is written out in the `include/expanse.h` comment and in `expanse_sync(3)`. The string, bytes and ordered-bytes shared maps take the same C forms, and the blob map its status-returning forms ([shared-map read-modify-write](#shared-map-read-modify-write-_compare_exchange-_get_or_insert)); Go, Java, .NET, Node.js and Python also wrap those four maps' forms; the 32-bit `expanse_sync32_map_writer_*` block is a separate surface with its own forms. Python and Node.js wrap the two symbols through the Rust methods and write the loop as `update_with(key, fn)` and `update(key, fn)` (Python's `update(other)` is the bulk merge; Python's four shared maps carry `compare_exchange`, `get_or_insert` and `update_with` as well, and the blob map's forms return a status string and the observed value, with `compare_exchange_meta` beside them); Go wraps them as `CompareExchange` and `GetOrInsert` with an absent-capable `Word` and writes the loop as `Update`; Java wraps them as `compareExchange` (an `Exchange` record) and `getOrInsert` over `OptionalLong` and writes the loop as `update`; .NET wraps them as `CompareExchange` and `GetOrInsert` over `ulong?` and writes the loop as `Update`. The eighty-three `expanse_sync_strmap_*`, `expanse_sync_bytesmap_*`, `expanse_sync_ordered_bytesmap_*` and `expanse_sync_blob_map_*` symbols were stated deferrals in `scripts/check_abi_parity.py` citing #1538 and none remains; Python wraps them (the four `SyncExpanse*` classes, with `compare_exchange`, `get_or_insert` and `update_with`, and on the blob map status-string forms and `compare_exchange_meta`); Node.js wraps them (`SyncExpanseStrMap`, `SyncExpanseBytesMap`, `SyncExpanseOrderedBytesMap`, `SyncExpanseBlobMap`, with `compareExchange` and `getOrInsert` over a `bigint | null`, and `update` as the retry loop, on the word-valued maps, and on the blob map `compareExchange` and `getOrInsert` over a `{ payload, hotMeta }` value and `compareExchangeMeta`, an exchange or insert the map refuses throwing); .NET wraps them (`ExpanseSyncStrMap`, `ExpanseSyncBytesMap`, `ExpanseSyncOrderedBytesMap`, `ExpanseSyncBlobMap`, with `CompareExchange` and `GetOrInsert` over `ulong?` and `Update` as the C# loop on the word-valued maps, and on the blob map `CompareExchange`, `CompareExchangeMeta` and `GetOrInsert` over an `ExpanseBlobValue` and a status); Java wraps them (`SyncExpanseStrMap`, `SyncExpanseBytesMap`, `SyncExpanseOrderedBytesMap`, `SyncExpanseBlobMap`, with `compareExchange` and `getOrInsert` over `OptionalLong`, and on the blob map `Optional<Blob>` and a status-carrying `Exchange`); Go wraps them (`SyncStrMap`, `SyncBytesMap`, `SyncOrderedBytesMap`, `SyncBlobMap`), with `CompareExchange`, `GetOrInsert` and, on the word-valued maps, `Update` over an absent-capable `Word`, and on the blob map a `BlobValue` / `BlobMeta` that carries payload and metadata.

