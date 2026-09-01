# C libjudy Compatibility Contract

> Canonical compatibility doc. Design: [ARCHITECTURE.md](ARCHITECTURE.md) · Testing: [TESTING.md](TESTING.md)

`libexpanse` (built by `crates/expanse-capi`) is a **drop-in binary replacement for libjudy**: existing consumers link-swap (or `LD_PRELOAD`) `libexpanse` in place of `libJudy` with **no source changes**. This document is the contract that defines "drop-in".

## Packaging layout

| Artifact | Name |
|---|---|
| Rust crate (core) | `expanse-trie` |
| C library | `libexpanse.so` / `expanse.dll` / `libexpanse.a` |
| Modern header | `expanse.h` — **shipped**: `expanse_set_t`, `expanse_map_t`, `expanse_bytesmap_t`, plus the concurrent `expanse_sync_set_t`/`expanse_sync_map_t` and their per-thread reader handles. Adds what classic Judy lacks: rank/select on ordered types (`count_below`/`count_range`/`by_count`), byte-exact `mem_used`, plain value returns instead of `JError_t` out-params, and optimistic concurrent readers. (`expanse_strmap_t` still to come — `JudySL*` covers ordered string maps today.) |
| Compat header | `Judy.h` (source-compatible with classic libjudy) |
| Distro packages (planned) | `libexpanse-dev`, `libexpanse1`, and `libjudy-compat` (symlinks `libJudy.so.1` → `libexpanse.so.1` and installs the `Judy.h` alias) |

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

## Non-goals

- **`Judy1MemUsed`/`JudyLMemUsed` byte equality.** Internal geometry differs (64-byte-line nodes, different allocator). Guarantee: same order of magnitude and monotonic behavior — not the same number. Documented in the shipped header.
- **Internal structure equality** — no attempt to match libjudy's node layouts, only its observable behavior.
- **`JErrno` legacy globals** beyond what the documented macro layer requires.

## Build-configuration surface matrix

`libexpanse` builds in three configurations, and they do not export the same
symbol set. Omitted entry points are **absent**, never present-and-stubbed: a
`Judy*` or `expanse_*` symbol that links but behaves differently is worse than
one that does not link, and a link error names the gap at build time.

| Configuration | Cargo invocation | Exported C symbols |
|---|---|---|
| 64-bit, `std` (default) | `cargo build -p expanse-capi` | 145 |
| 64-bit, `no_std` | `--no-default-features` | 127 |
| 32-bit (any) | `--no-default-features --target riscv32imc-unknown-none-elf` | 55 |

(Counts measured from `llvm-nm --defined-only` on the built artifacts — the
64-bit rows at commit `5e8147ae`, the 32-bit row re-measured when the
`expanse_sync32_*` surface landed for #573 (31 before it: the ordered core
plus `expanse_map_remove_range`); reproduce with the invocations above.)

**64-bit `no_std` drops only the concurrent containers** — the 18
`expanse_sync_*` entry points. `expanse_trie::sync` needs `std::sync`, so a
bare-metal 64-bit build has no one-writer/many-reader surface. Everything
else, the entire legacy `Judy*` drop-in included, is present and unchanged.

**32-bit drops the legacy `Judy*` families entirely**, plus the byte-string,
string, blob and concurrent containers, plus rank/select and value-slot
accessors. What ships is the width-parametric ordered core with full
bidirectional range navigation:

| Container | 32-bit entry points |
|---|---|
| identity | `expanse_version` |
| `expanse_set_t` | `_new`, `_free`, `_len`, `_mem_used`, `_clear`, `_insert`, `_remove`, `_contains`, `_contains_batch`, `_first`, `_last`, `_next_at_or_after`, `_next_after`, `_prev_at_or_before`, `_prev_before` |
| `expanse_map_t` | `_new`, `_free`, `_len`, `_mem_used`, `_clear`, `_insert`, `_get`, `_remove`, `_remove_range` (32-bit-only, see below), `_first`, `_last`, `_next_at_or_after`, `_next_after`, `_prev_at_or_before`, `_prev_before` |
| `expanse_sync32_map_t` / `expanse_sync32_set_t` (32-bit-only, provisional) | `_new`, `_free`, `_writer`, `_reader`, `_writer_try_insert`, `_writer_try_remove`, `_writer_try_reclaim`, `_writer_get` / `_writer_contains`, `_writer_stats`, `_reader_try_get` / `_reader_try_contains`, `_reader_try_len`; plus `expanse_sync32_mutation_headroom`, `expanse_sync32_status_str` |

The cause is engine surface, not a deliberate reduction: `ExpanseMap32` /
`ExpanseSet32` are real tries, but they carry no `count_below`/`by_count`,
no `get_value_slot`/`ins_slot`, and their `count_range` takes a `(start, end)`
pair rather than a range — so the corresponding C contracts have nothing to
translate to. `ExpanseStrMap`/`ExpanseBytesMap`/`ExpanseBlobMap` and the
`sync` module exist only at 64-bit width.

The reverse gap also exists. `expanse_map_remove_range` — batched removal of
every key in `[lo, hi]` with a per-entry callback, the eviction primitive of
[#578](https://github.com/orieg/expanse/issues/578) — is declared in a
`#if !EXPANSE_WIDE_SURFACE` block and shipped **only at 32-bit width**, because
only the 32-bit engine has `remove_range`. The language bindings target
64-bit hosts, so `scripts/check_abi_parity.py` tags declarations in that
block `narrow_only`, lists them, and excludes them from binding coverage;
the 32-bit-only surface is verified instead by the i686 CI job (a Rust-side
test plus a C smoke linked with `-m32` against the i686 cdylib).

#### The 32-bit concurrent story

The 32-bit engine's concurrent wrapper — the Rust `sync32` module
(`SyncExpanseMap32`/`SyncExpanseSet32`: single writer enforced by
`split(&mut)`, validated single-attempt optimistic reads returning `Busy`,
deferred reclamation drained at reader quiescence) — reaches C as the
**`expanse_sync32_*` family** ([#573](https://github.com/orieg/expanse/issues/573)),
24 symbols declared in the `!EXPANSE_WIDE_SURFACE` block and present in every
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
([#564](https://github.com/orieg/expanse/issues/564)) — and
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

Status: **all four families exported** — Judy1, JudyL, JudySL, JudyHS — with the shipped `Judy.h`, and **all four gates green**, each as a standing CI job. G1 runs the differential-oracle harness against a `dlopen`'d stock libjudy for all four families (randomized op sequences, full-sweep/rank agreement, byte-exact JudySL buffer sweeps, JudyHS byte-key sequences including zero-length keys). G2 passes the php-judy test suite built against libexpanse via the libjudy-compat prefix — 221/221 locally (macOS AArch64, PHP 8.5) and as the `php-judy-compat` Linux CI job.

---

## Cross-Language Feature & Container Parity

Expanse provides 100% C ABI symbol coverage across all high-level language bindings, continuously validated by `scripts/check_abi_parity.py` in CI:

| Container / Feature | C ABI (`expanse.h`) | Rust (`expanse-trie`) | Java 22+ (`expanse-java`) | .NET 9 (`Orieg.Expanse`) | Python (`expanse-trie`) | Node.js (`@orieg/expanse`) | PHP (`orieg/expanse`) | Ruby (`expanse`) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **`ExpanseSet` (Judy1)** | `expanse_set_*` (17 fns) | `ExpanseSet` | `ExpanseSet` | `ExpanseSet` | `ExpanseSet` | `ExpanseSet` | `Set` / `ExpanseSet` | `Expanse::Set` |
| **`ExpanseMap` (JudyL)** | `expanse_map_*` (19 fns + 1 32-bit-only; `expanse_sync32_*` 24 fns 32-bit-only, provisional) | `ExpanseMap` | `ExpanseMap` | `ExpanseMap` | `ExpanseMap` | `ExpanseMap` | `Map` / `ExpanseMap` | `Expanse::Map` |
| **`ExpanseBytesMap` (JudyHS)** | `expanse_bytesmap_*` (10 fns) | `ExpanseBytesMap` | `ExpanseBytesMap` | `ExpanseBytesMap` | `ExpanseBytesMap` | `ExpanseBytesMap` | `BytesMap` / `ExpanseBytesMap` | `Expanse::BytesMap` |
| **`ExpanseStrMap` (JudySL)** | `expanse_strmap_*` (16 fns) | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `StrMap` / `ExpanseStrMap` | `Expanse::StrMap` |
| **StrMap truncation-aware nav** | `expanse_strmap_*_ex` (6 fns) | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `StrMap` | `Expanse::StrMap` |
| **`SyncExpanseSet` (OCC Set)**| `expanse_sync_set_*` (9 fns) | `SyncExpanseSet` | `SyncExpanseSet` | `SyncExpanseSet` | `SyncExpanseSet` | `SyncExpanseSet` | `SyncSet` | Via C ABI |
| **`SyncExpanseMap` (OCC Map)**| `expanse_sync_map_*` (9 fns) | `SyncExpanseMap` | `SyncExpanseMap` | `SyncExpanseMap` | `SyncExpanseMap` | `SyncExpanseMap` | `SyncMap` | Via C ABI |
| **`ExpanseBlobMap` (Large-Value)**| `expanse_blob_map_*` (11 fns) | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `BlobMap` / `ExpanseBlobMap` | `Expanse::BlobMap` |
| **Rank/Select (`by_count`)** | ✅ All ordered types | ✅ `count_below`/`by_count` | ✅ `rank`/`select` | ✅ `Rank`/`ByCount` | ✅ `count_below`/`by_count` | ✅ `countRange`/`byCount` | ✅ `rank`/`select` | ✅ `rank`/`select` |
| **Metadata Filtering** | ✅ Predicate callbacks | ✅ SWAR vector kernels | ✅ Functional predicates | ✅ Delegated predicates | ✅ Predicate callbacks | ✅ Predicate callbacks | ✅ Callback predicates | ✅ Hot metadata |
| **Optimistic Concurrency** | ✅ Epoch-based OCC | ✅ `SeqVersion` atomics | ✅ Read-coupling handles | ✅ Reader handles | ✅ GIL-free thread queries | ✅ Event-loop safe | ✅ Optimistic OCC | ✅ GVL-safe FFI |
| **C ABI Symbol Parity** | **101 / 101 (100%)** | **101 / 101 (100%)** | **101 / 101 (100%)** | **101 / 101 (100%)** | **101 / 101 (100%)** | **101 / 101 (100%)** | **101 / 101 (100%)** | **101 / 101 (100%)** |

