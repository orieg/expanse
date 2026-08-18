# C libjudy Compatibility Contract

> Canonical compatibility doc. Design: [ARCHITECTURE.md](ARCHITECTURE.md) · Testing: [TESTING.md](TESTING.md)

`judy-capi` is a **drop-in binary replacement for libjudy**: existing consumers link-swap (or `LD_PRELOAD`) `judy_capi` in place of `libJudy` with **no source changes**. This document is the contract that defines "drop-in".

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

Shipped header `crates/judy-capi/include/Judy.h` additionally provides, source-compatibly:

- Types/conventions: `Word_t`, `Pvoid_t`, `PPvoid_t`, `Pcvoid_t`, `JError_t`, `PJError_t`, `PJERR`, `JERR`, `JU_ERRNO_*`.
- Convenience macros: `J1S`, `J1U`, `J1T`, `J1C`, `J1BC`, `J1F`, `J1N`, `J1L`, `J1P`, `J1FA`, `J1MU`; `JLI`, `JLD`, `JLG`, `JLC`, `JLBC`, `JLF`, `JLN`, `JLL`, `JLP`, `JLFA`, `JLMU`; `JSLI`, `JSLD`, `JSLG`, `JSLF`, `JSLN`, `JSLL`, `JSLP`, `JSLFA`; `JHSI`, `JHSD`, `JHSG`, `JHSFA`.

## Guarantees

1. **Symbol and ABI match** — same names, same signatures, same calling convention. `LD_PRELOAD`/link-swap works on Linux/macOS; the Windows DLL exports the same symbols so consumers that today build C libjudy from source (php-judy's MSVC job) link against `judy_capi.dll`/`.lib` instead.
2. **Semantic match** per documented contract: pointer-to-value-slot return conventions (`JudyLIns` returns a writable `PPvoid_t`; inserted slots initialized to 0), sorted-order iteration (`First`/`Next`/`Last`/`Prev` and the `Empty` variants, inclusive-search semantics), `Count`/`ByCount` rank semantics over inclusive index ranges, null/empty-array edge behavior, `JError_t` error reporting incl. `PJERR`/`JERR` returns and `JU_ERRNO_*` codes.
3. **Ordering guarantee**: iteration is strictly sorted unsigned-key order (JudySL: byte-lexicographic) — identical to libjudy, verified differentially.

## Non-goals

- **`Judy1MemUsed`/`JudyLMemUsed` byte equality.** Internal geometry differs (64-byte-line nodes, different allocator). Guarantee: same order of magnitude and monotonic behavior — not the same number. Documented in the shipped header.
- **Internal structure equality** — no attempt to match libjudy's node layouts, only its observable behavior.
- **`JErrno` legacy globals** beyond what the documented macro layer requires.

## New capabilities

Modern features (lock-free concurrent reads, iterators, arena controls) are exposed **only** through the native Rust API and, if ever needed in C, through *new* `judyrs_`-prefixed symbols/headers. Existing `Judy*` symbols never change semantics. Swapping in judy-capi must be a pure substitution.

## Acceptance gates ("in-place replacement" is proven, not claimed)

| Gate | Check |
|---|---|
| G1 Differential oracle | Randomized + adversarial op sequences produce identical observable results from judy-capi and stock libjudy (Linux CI job, `libjudy-dev`); see [TESTING.md](TESTING.md) |
| G2 php-judy Linux | php-judy test suite passes built against judy-capi (swap in `--with-judy` prefix) |
| G3 php-judy Windows | php-judy Windows CI builds against `judy_capi.dll` instead of compiling libjudy from source, suite passes |
| G4 Preload smoke | An unmodified prebuilt libjudy consumer runs correctly under `LD_PRELOAD` of `libjudy_capi.so` with a `libJudy.so.1` compatibility symlink/soname shim |

Status: contract defined; surface not yet exported (core phases 4/6 pending). The stub crate exists so packaging and CI artifacts are exercised from day one.
