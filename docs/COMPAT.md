# C libjudy Compatibility Contract

> Canonical compatibility doc. Design: [ARCHITECTURE.md](ARCHITECTURE.md) · Testing: [TESTING.md](TESTING.md)

`libexpanse` (built by `crates/expanse-capi`) is a **drop-in binary replacement for libjudy**: existing consumers link-swap (or `LD_PRELOAD`) `libexpanse` in place of `libJudy` with **no source changes**. This document is the contract that defines "drop-in".

## Packaging layout

| Artifact | Name |
|---|---|
| Rust crate (core) | `expanse-trie` |
| C library | `libexpanse.so` / `expanse.dll` / `libexpanse.a` |
| Modern header | `expanse.h` (`expanse_*` API: `expanse_set_t`, `expanse_map_t`, `expanse_strmap_t`, `expanse_bytesmap_t`) |
| Compat header | `Judy.h` (source-compatible with classic libjudy) |
| Distro packages (planned) | `libexpanse-dev`, `libexpanse1`, and `libjudy-compat` (symlinks `libJudy.so.1` → `libexpanse.so.1` and installs the `Judy.h` alias) |

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
| G3 php-judy Windows | php-judy Windows CI builds against `expanse.dll` instead of compiling libjudy from source, suite passes |
| G4 Preload smoke | **Green**: `crates/expanse-capi/smoke/preload_smoke.c` (all four families), compiled against the stock header + library, produces identical transcripts stock vs `LD_PRELOAD`ed libexpanse — verified locally (macOS `DYLD_INSERT_LIBRARIES` flat-namespace) and as a step of the `differential-oracle` CI job |

## Doc-gap resolutions

| # | Question the docs leave open | Resolution |
|---|---|---|
| D1 | `Word_t` width on LLP64 (Windows): classic header used `unsigned long` | `Word_t` = `size_t` (pointer-width). ABI-identical to classic on LP64 Linux/macOS; 64-bit on Win64, which the consumers we target (php-judy MSVC) require |
| D2 | `JU_ERRNO_*` numeric values (man pages name the codes, not numbers; the classic header is LGPL and unread) | libexpanse assigns its own stable numbering in `Judy.h`. Source-compatible (names match); numeric equality with classic builds is not guaranteed |
| D3 | Behavior on allocation failure | Allocation failure aborts (Rust global-allocator convention) instead of returning `JERR`/`PJERR` with `JU_ERRNO_NOMEM`. Recorded as a deviation; revisit if a consumer needs graceful OOM |
| D4 | `JudyHSFreeArray` byte total | The returned "bytes freed" is implementation-defined (our hash-trie + bucket accounting differs from classic's internals). Both are nonzero for nonempty arrays; the oracle compares emptiness semantics, not byte totals — same stance as `MemUsed` |
| D5 | Convenience-macro grammar (`JLI`/`J1S`/…) | Statement blocks `{ … ; }`, not expressions. Black-box evidence: php-judy invokes several macros without trailing semicolons at statement position and compiles against classic system libjudy, so classic's macros tolerate that; ours match, accepting both `JLI(...)` and `JLI(...);`. Consequence: the macros cannot be used as expressions, and unbraced `if/else` around one needs braces (no observed consumer does either) |

Status: **all four families exported** — Judy1, JudyL, JudySL, and JudyHS — with the shipped `Judy.h`; the gate G1 differential-oracle harness runs in CI against a dlopen'd stock libjudy for all four (randomized op sequences, full-sweep/rank agreement, byte-exact JudySL buffer sweeps, JudyHS byte-key sequences including zero-length keys). **Gate G2 is green**: the php-judy test suite passes built against libexpanse via the libjudy-compat prefix — 221/221 locally (macOS AArch64, PHP 8.5) and as the `php-judy-compat` Linux CI job. **Gate G4 is green** (preload smoke, see gate table). Next gate: G3 (php-judy Windows against `expanse.dll`).
