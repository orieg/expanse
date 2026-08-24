# C libjudy Compatibility Contract

> Canonical compatibility doc. Design: [ARCHITECTURE.md](ARCHITECTURE.md) · Testing: [TESTING.md](TESTING.md)

`libexpanse` (built by `crates/expanse-capi`) is a **drop-in binary replacement for libjudy**: existing consumers link-swap (or `LD_PRELOAD`) `libexpanse` in place of `libJudy` with **no source changes**. This document is the contract that defines "drop-in".

## Packaging layout

| Artifact | Name |
|---|---|
| Rust crate (core) | `expanse-trie` |
| C library | `libexpanse.so` / `expanse.dll` / `libexpanse.a` |
| Modern header | `expanse.h` — **shipped**: `expanse_set_t`, `expanse_map_t`, `expanse_bytesmap_t`, plus the concurrent `expanse_sync_set_t`/`expanse_sync_map_t` and their per-thread reader handles. Adds what classic Judy lacks: rank/select on ordered types (`count_below`/`count_range`/`by_count`), byte-exact `mem_used`, plain value returns instead of `JError_t` out-params, and lock-free concurrent readers. (`expanse_strmap_t` still to come — `JudySL*` covers ordered string maps today.) |
| Compat header | `Judy.h` (source-compatible with classic libjudy) |
| Distro packages (planned) | `libexpanse-dev`, `libexpanse1`, and `libjudy-compat` (symlinks `libJudy.so.1` → `libexpanse.so.1` and installs the `Judy.h` alias) |

### Supported architectures

Drop-in compatibility is officially verified for:
- **x86-64** (Linux, Windows, macOS)
- **AArch64** (Linux, macOS)
- **RISC-V 64-bit (RV64GC)** (Linux, cross-compiled)

All targets are 64-bit (LP64 or LLP64). 32-bit platforms are not currently supported.

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

Shipped header `crates/expanse-capi/include/Judy.h` additionally provides, source-compatibly:

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

## New capabilities

Modern features (lock-free concurrent reads, iterators, arena controls) are exposed through the native Rust API and through the `expanse_*` C API in `expanse.h`. Existing `Judy*` symbols never change semantics. Swapping in libexpanse must be a pure substitution.

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
| D1 | `Word_t` width on LLP64 (Windows): classic header used `unsigned long` | `Word_t` = `size_t` (pointer-width). ABI-identical to classic on LP64 Linux/macOS; 64-bit on Win64, which the consumers we target (php-judy MSVC) require |
| D2 | `JU_ERRNO_*` numeric values (man pages name the codes, not numbers; the classic header is LGPL and unread) | libexpanse assigns its own stable numbering in `Judy.h`. Source-compatible (names match); numeric equality with classic builds is not guaranteed |
| D3 | Behavior on allocation failure | Allocation failure aborts (Rust global-allocator convention) instead of returning `JERR`/`PJERR` with `JU_ERRNO_NOMEM`. Recorded as a deviation; revisit if a consumer needs graceful OOM |
| D4 | `JudyHSFreeArray` byte total | The returned "bytes freed" is implementation-defined (our hash-trie + bucket accounting differs from classic's internals). Both are nonzero for nonempty arrays; the oracle compares emptiness semantics, not byte totals — same stance as `MemUsed` |
| D5 | Convenience-macro grammar (`JLI`/`J1S`/…) | Statement blocks `{ … ; }`, not expressions. Black-box evidence: php-judy invokes several macros without trailing semicolons at statement position and compiles against classic system libjudy, so classic's macros tolerate that; ours match, accepting both `JLI(...)` and `JLI(...);`. Consequence: the macros cannot be used as expressions, and unbraced `if/else` around one needs braces (no observed consumer does either) |

Status: **all four families exported** — Judy1, JudyL, JudySL, and JudyHS — with the shipped `Judy.h`; the gate G1 differential-oracle harness runs in CI against a dlopen'd stock libjudy for all four (randomized op sequences, full-sweep/rank agreement, byte-exact JudySL buffer sweeps, JudyHS byte-key sequences including zero-length keys). **Gate G2 is green**: the php-judy test suite passes built against libexpanse via the libjudy-compat prefix — 221/221 locally (macOS AArch64, PHP 8.5) and as the `php-judy-compat` Linux CI job. **All four gates are green** — G1 (differential oracle), G2 (php-judy Linux), G3 (php-judy Windows against `expanse.dll`), G4 (`LD_PRELOAD` smoke) — each as a standing CI job.

---

## Cross-Language Feature & Container Parity

Expanse provides 100% C ABI symbol coverage across all high-level language bindings, continuously validated by `scripts/check_abi_parity.py` in CI:

| Container / Feature | C ABI (`expanse.h`) | Rust (`expanse-trie`) | Java 22+ (`expanse-java`) | .NET 9 (`Orieg.Expanse`) | Python (`expanse-trie`) | Node.js (`@orieg/expanse`) | PHP (`orieg/expanse`) | Ruby (`expanse`) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **`ExpanseSet` (Judy1)** | `expanse_set_*` (17 fns) | `ExpanseSet` | `ExpanseSet` | `ExpanseSet` | `ExpanseSet` | `ExpanseSet` | `Set` / `ExpanseSet` | `Expanse::Set` |
| **`ExpanseMap` (JudyL)** | `expanse_map_*` (19 fns) | `ExpanseMap` | `ExpanseMap` | `ExpanseMap` | `ExpanseMap` | `ExpanseMap` | `Map` / `ExpanseMap` | `Expanse::Map` |
| **`ExpanseBytesMap` (JudyHS)** | `expanse_bytesmap_*` (10 fns) | `ExpanseBytesMap` | `ExpanseBytesMap` | `ExpanseBytesMap` | `ExpanseBytesMap` | `ExpanseBytesMap` | `BytesMap` / `ExpanseBytesMap` | `Expanse::BytesMap` |
| **`ExpanseStrMap` (JudySL)** | `expanse_strmap_*` (16 fns) | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `StrMap` / `ExpanseStrMap` | `Expanse::StrMap` |
| **StrMap truncation-aware nav** | `expanse_strmap_*_ex` (6 fns) | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `ExpanseStrMap` | `StrMap` | `Expanse::StrMap` |
| **`SyncExpanseSet` (OCC Set)**| `expanse_sync_set_*` (9 fns) | `SyncExpanseSet` | `SyncExpanseSet` | `SyncExpanseSet` | `SyncExpanseSet` | `SyncExpanseSet` | `SyncSet` | Via C ABI |
| **`SyncExpanseMap` (OCC Map)**| `expanse_sync_map_*` (9 fns) | `SyncExpanseMap` | `SyncExpanseMap` | `SyncExpanseMap` | `SyncExpanseMap` | `SyncExpanseMap` | `SyncMap` | Via C ABI |
| **`ExpanseBlobMap` (Large-Value)**| `expanse_blob_map_*` (11 fns) | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `ExpanseBlobMap` | `BlobMap` / `ExpanseBlobMap` | `Expanse::BlobMap` |
| **Rank/Select (`by_count`)** | ✅ All ordered types | ✅ `count_below`/`by_count` | ✅ `rank`/`select` | ✅ `Rank`/`ByCount` | ✅ `count_below`/`by_count` | ✅ `countRange`/`byCount` | ✅ `rank`/`select` | ✅ `rank`/`select` |
| **Metadata Filtering** | ✅ Predicate callbacks | ✅ SWAR vector kernels | ✅ Functional predicates | ✅ Delegated predicates | ✅ Predicate callbacks | ✅ Predicate callbacks | ✅ Callback predicates | ✅ Hot metadata |
| **Lock-Free Concurrency** | ✅ Epoch-based OCC | ✅ `SeqVersion` atomics | ✅ Read-coupling handles | ✅ Reader handles | ✅ GIL-free thread queries | ✅ Event-loop safe | ✅ Lock-free OCC | ✅ GVL-safe FFI |
| **C ABI Symbol Parity** | **98 / 98 (100%)** | **98 / 98 (100%)** | **98 / 98 (100%)** | **98 / 98 (100%)** | **98 / 98 (100%)** | **98 / 98 (100%)** | **98 / 98 (100%)** | **98 / 98 (100%)** |

