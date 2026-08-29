# Hardware Capability Reference & Assumption Validation

This document maps every hardware capability the Expanse engine depends on — SIMD
kernels, bit-manipulation instructions, cache-line packing, address widths — to its
**authoritative primary source** (Intel SDM, AMD APM, Arm Architecture Reference
Manual, RISC-V ISA specs, vendor TRMs), records a **validation verdict** for the
assumption the code makes, and catalogs **missed opportunities** (extensions we do
not yet use but could).

Its purpose is to make every hardware assumption in the codebase *explicit and
citable*, so a reader of `docs/ARCHITECTURE.md` / `docs/ALGORITHMS.md` or the
architecture visualizer can trace "we assume POPCNT / 64-byte lines / NEON is
present" back to the exact manual section that justifies it.

## How to read this document

- **Citations are verified, not asserted.** Every section/page number below was
  extracted from the actual manual PDF (via `pypdf` / `PyMuPDF`) and quoted verbatim.
  Where a claim rests on a secondary source (e.g. Apple publishes no Arm-ARM-style
  manual), that is stated explicitly in [§8 Honest disclosure](#8-honest-disclosure).
- **Manual PDFs are cached locally, not committed.** The corpus lives under
  `docs/research/hardware/{x86,arm,riscv,embedded}/` which is **git-ignored** — the
  PDFs are large (the Arm ARM alone is ~120 MB / 17,145 pages) and freely
  re-downloadable from the URLs in [§7 References](#7-references). Citations resolve
  by *document ID + revision + section*, not by a committed blob.
- **Page numbers** are the PDF page index that was grepped; the manual's own printed
  label (e.g. "Vol. 2B 4-405") is quoted alongside where the page carried one.

## Capability × architecture matrix

| Capability (our usage) | x86-64 | AArch64 | RISC-V (RV32/64) | Embedded (Cortex-M / ESP32) |
|---|---|---|---|---|
| 128-bit SIMD leaf scan | SSE2 ✅ baseline | NEON ✅ baseline | ❌ (RVV opt., absent on targets) | ❌ (M4 no SIMD; DSP on M4/M7) |
| Population count (`count_ones`) | POPCNT ✅ (runtime-gated) | NEON `CNT`+`ADDV` ✅ (no scalar) | ⚠️ **software** (needs Zbb) | ⚠️ software (no popcount) |
| Count-zeros (`leading/trailing_zeros`) | BSF/BSR ✅ (always correct) | `CLZ` ✅ / `RBIT`+`CLZ` for CTZ | ⚠️ software (needs Zbb) | ⚠️ software |
| 64-byte cache-line packing | ✅ fixed 64 B | ⚠️ **not architectural (Apple = 128 B)** | n/a | 32 B (M7/ESP32); M0–M4 cacheless |
| Wide address space | LA57 / 57-bit ✅ | — | Sv32 (RV32) | — |
| Software prefetch (#242) | hint, no-op on OoO ✅ removed | `PRFM` hint (impl-defined) | — | — |

Legend: ✅ assumption validated · ⚠️ assumption is a risk or lowers to software.

---

## 1. x86-64 (Intel / AMD)

### 1.1 SSE2 128-bit leaf kernels — **VALIDATED** (guarantee sourced from the psABI, not the CPU manual)

*Usage:* `crates/expanse/src/bits.rs` — `find_byte_16_sse2`, `find_byte_8`, and the
`lower_bound_*` kernels use `_mm_loadu_si128`, `_mm_loadl_epi64`,
`_mm_cvtsi64_si128`, `_mm_set1_epi{8,16,32}`, `_mm_cmpeq_epi{8,16,32}`,
`_mm_cmplt_epi{8,16,32}`, `_mm_xor_si128`, `_mm_movemask_epi8`, with **no runtime
detection** (SAFETY comments assert SSE2 is part of the x86-64 baseline).

*Source:* Every intrinsic maps to an instruction whose 128-bit XMM form carries the
CPUID **SSE2** feature flag — Intel SDM Vol 2 (325383): PCMPEQB `66 0F 74 /r … SSE2`
(PDF p.985); PMOVMSKB `66 0F D7 /r … SSE2` (p.1086, "Vol. 2B 4-352"); PXOR (p.1263);
MOVQ `F3 0F 7E` = `_mm_loadl_epi64` (p.842); MOVD/MOVQ `66 [REX.W] 0F 6E` =
`_mm_cvtsi64_si128` (p.790). Detection bit: Intel SDM Vol 1 (253665) p.257 §11 —
*"If CPUID.01H:EDX.SSE2[bit 26] = 1, SSE2 extensions are present"*; AMD APM Vol 1
(24592) confirms the same EDX[26] bit. SSE2 debuted with Pentium 4.

*Verdict:* **Confirmed safe** — but the guarantee that *every* x86-64 CPU has SSE2 is
not a sentence in the CPU manuals (they present SSE2 as CPUID-detectable because they
also describe pre-SSE2 32-bit parts). The mandate comes from the **x86-64 psABI /
Rust target definition**: the base `x86_64` target enables `+fxsr,+sse,+sse2`, and the
SysV/MS calling conventions *require* SSE2 (float/double args pass in XMM0–7). Cite the
CPUID bit + Pentium-4 origin from the SDM, but attribute "always present on x86-64" to
the psABI. → *Recommended code-comment precision fix in `bits.rs`.*

### 1.2 POPCNT — **VALIDATED** (runtime gate is correct)

*Usage:* `crates/expanse/src/get.rs` — `#[target_feature(enable="popcnt")]` function
clones (`test_set_popcnt`, `get_map_popcnt`, `locate_slot_popcnt`) dispatched through
`bits::popcnt_rt::available()`, with a SWAR fallback.

*Source:* Intel SDM Vol 2 (325383) POPCNT — PDF p.1139 ("Vol. 2B 4-405"): *"`#UD` If
CPUID.01H:ECX.POPCNT[Bit 23] = 0."* POPCNT is a **separate feature bit (ECX bit 23),
distinct from SSE2 (EDX bit 26)** — it is the SSE4.2 / Nehalem-era addition.

*Verdict:* **Confirmed** — POPCNT is *not* in the x86-64 baseline, so the runtime
`is_x86_feature_detected!("popcnt")` gate + SWAR fallback is exactly correct. Scalar
`u64::count_ones` lowers to hardware POPCNT only under `+popcnt`, else SWAR.

### 1.3 TZCNT / LZCNT — **ALREADY CORRECT** (no gating needed)

*Usage:* `iter.rs`, `trie32.rs`, `mutate.rs`, `Bitmap256` use `trailing_zeros()` /
`leading_zeros()` with no target-feature gating.

*Source:* Intel SDM Vol 2 (325383): TZCNT (BMI1) p.1455 — *"On processors that do not
support TZCNT, the instruction byte encoding is executed as BSF"*; LZCNT (ABM) p.732 —
*"on processors that do not support LZCNT, the instruction byte encoding is executed as
BSR"*; BSF/BSR are baseline (since i386, p.235/237) but leave the destination
*undefined* on a zero input.

*Verdict:* **Already correct** — the byte encodings degrade to BSF/BSR on parts lacking
BMI1/ABM, and Rust's `trailing/leading_zeros` handle the zero-input case in surrounding
code (they do not rely on the hardware's defined-on-zero behavior). No gating required.

### 1.4 64-byte cache line — **VALIDATED** (correct on x86)

*Usage:* `types.rs` `CACHE_LINE = 64` (+ `const _` assert `CACHE_LINE == 4*16`, "4
edges per line"); six `#[repr(C, align(64))]` node types; `alloc.rs` aligns node
classes to `CACHE_LINE`.

*Source:* Intel Optimization Manual (248966-049): *"Cache line size of 64 bytes"*
(PDF p.151, also p.391/488/673). Runtime query if ever needed: Intel SDM Vol 2 CPUID
page p.327 — `CPUID.01H:EBX[15:8] × 8` = CLFLUSH line size; per-level via leaf 04H.

*Verdict:* **Confirmed** — 64 B is correct for all mainstream x86-64; the compile-time
constant is safe for the x86 tier. (Contrast AArch64 §2.4, where 64 B is *not* safe.)

### 1.5 Software prefetch (#242) — **finding EXPLAINS the measured no-op**

*Context:* PR #242 added SW prefetch to the hot descent loops, then removed it after it
measured as a no-op on the i9-12900F reference host.

*Source:* Intel SDM Vol 2 (325383) PREFETCHh — PDF p.1148 ("Vol. 2B 4-414"): *"The
PREFETCHh instruction is merely a hint … can be overloaded or ignored by a processor
implementation."* Intel Optimization Manual (248966-049) §6.2 — PDF p.234: *"Use the
PREFETCH instruction only when data access patterns are irregular and prefetch distance
can be pre-determined."*

*Verdict:* **Removing prefetch was correct.** A trie descent is pointer-chasing — the
next node's address is data-dependent (known only after decoding the current node), so
the prefetch distance *cannot be pre-determined*, violating the optimization guide's
precondition. On an OoO core the hint issues too late to hide the miss and the extra
µop is pure overhead. (SW prefetch *can* still help the RocksDB sequential-scan path
#197, where stride is regular and distance is known.)

*Scope of the finding:* this closes prefetch **inside one descent**, where the
precondition genuinely cannot be met. It does not reach across *independent*
lookups, whose chains have no dependency on one another — the batched descent
([ALGORITHMS.md](ALGORITHMS.md) §4c, [#430](https://github.com/orieg/expanse/issues/430))
issues its hint W-1 lane visits before it is consumed, which is a
pre-determined distance. That is a different case, not a rebuttal of this one,
and it is unmeasured: no wall-clock result for it exists on the reference host
yet, so this section still records the only prefetch measurement there is.

### 1.6 LA57 / 5-level paging / 57-bit VA — **VALIDATED** (Intel side)

*Usage:* `docs/ARCHITECTURE.md` §9 (why Expanse never steals bits 48–63 of a pointer).

*Source:* Intel SDM Vol 3A (253668): `CR4.LA57` (bit 12) — PDF p.83, *"the processor
uses 5-level paging to translate 57-bit linear addresses"*; PML5 table — p.70;
detection `CPUID.(07H,0):ECX[16] LA57` — Vol 2 p.331.

*Verdict:* **Confirmed** (57-bit / 5-level). Two attributions to correct downstream:
"57-bit = 128 PiB" is arithmetic (2⁵⁷), not a verbatim SDM phrase; and "Linux keeps
allocations < 47-bit by default" is a **Linux kernel policy** (`Documentation/arch/x86/
x86_64/5level-paging.rst`), not an Intel-manual fact.

---

## 2. AArch64 (Arm / Apple Silicon)

### 2.1 NEON Advanced SIMD — **VALIDATED** (SAFETY comment overstates the architecture)

*Usage:* `bits.rs` `find_byte_16_neon` / 8-byte variant use `vld1q_u8`, `vld1_u8`,
`vceqq_u8`, `vshrn_n_u16` (the movemask substitute), `vget_lane_u64`, gated only by
`#[cfg(target_arch="aarch64")]` with no runtime detection.

*Source:* Arm ARM (DDI 0487M.c) §A2.2 "FEAT_AdvSIMD" (p.A2-88): *"All Armv8-A systems
that support standard operating systems with rich application environments also provide
hardware support for Advanced SIMD instructions. … **FEAT_AdvSIMD is OPTIONAL from
Armv8.0.**"* Intrinsic→instruction mappings verified in the Arm NEON Intrinsics
Reference (IHI 0073G): `vld1q_u8`→`LD1 {Vt.16B}` (p.71), `vceqq_u8`→`CMEQ` (p.19),
`vshrn_n_u16`→`SHRN` (p.34), `vget_lane_u64`→`UMOV` (p.149).

*Verdict:* **Safe, but reword the SAFETY comment.** Advanced SIMD is architecturally
*optional*, not "mandatory / baseline." What actually makes the unconditional path safe
is (a) universal presence on OS-hosting AArch64 (per the quote) and (b) Rust's
`aarch64-*` targets baseline `+neon,+fp-armv8`. → *Recommended precision fix.*

### 2.2 Population count on AArch64 — **VALIDATED, architecturally noteworthy**

*Source:* Arm ARM (DDI 0487M.c): NEON per-byte `CNT` — §C7.2.39 (p.C7-3000); horizontal
sum `ADDV`/`UADDLV` — §C7.2.6 (p.C7-2934). **There is no scalar popcount in base A64** —
scalar `CNT` exists only under **FEAT_CSSC** (§C6.2.100, p.C6-1967), which is *"OPTIONAL
from Armv8.7 … mandatory from Armv8.9."*

*Verdict:* On base A64, `u64::count_ones` lowers to the NEON path (`FMOV`→`CNT`→`ADDV`),
not a single scalar instruction — a real codegen difference from x86 `POPCNT`, worth
documenting. Scalar popcount needs FEAT_CSSC (Armv8.9+, Neoverse-class).

### 2.3 `leading_zeros` / `trailing_zeros` — **VALIDATED, codegen difference**

*Source:* Arm ARM (DDI 0487M.c): `CLZ` base A64 (§C6.2.91, p.C6-1949); `RBIT`
(§C6.2.321, p.C6-2505); scalar `CTZ` only under FEAT_CSSC (§C6.2.144, p.C6-2163).

*Verdict:* `leading_zeros` → single `CLZ`; `trailing_zeros` on base A64 has **no direct
CTZ** → compiler emits `RBIT`+`CLZ` (two instructions) vs x86's single `TZCNT`. No
correctness issue; a one-line note in ALGORITHMS.

### 2.4 64-byte cache line — ⚠️ **PORTABILITY / PERF RISK ON ARM**

*Source:* Arm ARM (DDI 0487M.c) §D24.2.41 "CTR_EL0, Cache Type Register" (pp.D24-8621…
8624): `DminLine` / `IminLine` are *"IMPLEMENTATION DEFINED"* — **the cache-line size
is not architecturally fixed and must be read from `CTR_EL0`.** Real values: 64 B on
most Cortex-A / Neoverse (including GitHub Actions `ubuntu-24.04-arm` Neoverse N2 runners,
`CTR_EL0.DminLine = 4` = 64 B; Cortex-A9 was 32 B); **Apple Silicon M1–M4 = 128 B** (macOS
`sysctl hw.cachelinesize` — *secondary source, see §8*).

*Verdict:* **Correctness is preserved** — `align(64)` merely over-aligns on a 128 B
machine, and the `4 edges per cache line` invariant is about the 64-byte *node size*,
not the hardware line. But the **performance premise is wrong on Apple Silicon**: with a
128 B line, two 64 B nodes share a line (false-sharing risk under concurrent writes),
and "one node = one line" scans actually touch half a line. → *Recommendations:*
(a) document `CACHE_LINE = 64` as a **node-packing constant**, not a hardware
assumption; (b) if any false-sharing padding relies on it, gate a 128 B value for
`target_vendor="apple"` or read `CTR_EL0` / `hw.cachelinesize` at runtime; (c) cite
`CTR_EL0` §D24.2.41 in `types.rs` and ARCHITECTURE.md.

### 2.5 PRFM prefetch — prospective (relevant to #242)

*Source:* Arm ARM (DDI 0487M.c) `PRFM` §C6.2.315 (p.C6-2492): *"a hint to the memory
system … **The effect of a PRFM instruction is IMPLEMENTATION DEFINED.**"*

*Verdict:* Not currently emitted (grep finds prefetch only in bench comments). Same
caveat as x86 §1.5: on OoO Cortex-X / Neoverse / Apple cores with strong hardware
prefetchers, a `PRFM` for a pointer-chase can be a no-op or counterproductive; any
adoption must be measured per-microarch and guarded.

### 2.6 GitHub-hosted AArch64 runner capability census (Neoverse N2 / Cobalt 100) — **VALIDATED**

*Context:* Issue #397 established the `test-aarch64` native execution and Callgrind regression-gating lane on GitHub-hosted ARM64 Linux runners (`ubuntu-24.04-arm`).

*Runner hardware census* — everything in this list is read from the runner by the `test-aarch64` job, which **asserts** the first three so a fleet rotation fails the job rather than silently invalidating this section *(measured: GitHub `ubuntu-24.04-arm` runner, [run 33124978928](https://github.com/orieg/expanse/actions/runs/33124978928), ref `ci/aarch64-execution-lane-397`)*:

- **CPU implementer / Part:** `0x41` (Arm) / `0xd49` (Neoverse-N2). Note `/proc/cpuinfo` reports `CPU architecture: 8` — the legacy AArch64 field the kernel always sets to 8; that Neoverse-N2 is Armv9.0-A comes from Arm's product spec, not from this census.
- **Cache-line size:** 64 bytes, read as `coherency_line_size` from sysfs and confirmed by `getconf LEVEL1_DCACHE_LINESIZE`. **The census does not read `CTR_EL0`** — the equivalent `DminLine = 4` is inferred from the kernel-reported value, not observed, which is worth stating precisely because §2.4's point is that `CTR_EL0` is the authority. A concrete cloud-server data point that 64-byte alignment matches Neoverse lines, in contrast to Apple Silicon's 128 B (§2.4).
- **NEON / AdvSIMD:** `asimd`, `asimdhp`, `asimddp`, `asimdfhm` present. The Advanced SIMD kernels (`find_byte_16_neon`, `find_byte_8`, `CNT`+`ADDV` popcount) and their parity tests execute here — as they already do on the `test` job's `macos-latest` (arm64) runner. What this lane adds over that is glibc rather than macOS, and Callgrind, which does not run on macOS.
- **SVE / SVE2 status:** `sve`, `sve2`, `sveaes`, `svesha3`, `svesm4`, `svei8mm`, `svebf16` are **exposed in `/proc/cpuinfo`**. Vector length is not measured by the census; 128-bit is the Neoverse-N2 spec figure. SVE leaf scanning was implemented and **reverted** (#434 / this PR) after it measured as a regression on this very lane; the capability stays **testable** here (§6) — the hardware is available — but **no SVE code path exists yet**: `is_aarch64_feature_detected!` appears nowhere in `crates/`, and nothing in the lane exercises SVE. §6's standing caveat, that SVE is absent on Apple M-series, is unchanged by this.
- **FEAT_CSSC status:** no `cssc` flag in the runner's `Features` line, as expected (CSSC is Armv8.9/v9.4+; scalar popcount/CTZ continue lowering to NEON / `RBIT`+`CLZ`).

---

## 3. RISC-V (RV64 + RV32)

### 3.1 Population count & count-zeros — ⚠️ **SOFTWARE on the shipped 32-bit config**

*Usage:* 20 `count_ones` sites (`bits.rs`, `trie32.rs` — the 4×u64 leaf bitmap and
`bitmap_count_range`) + 17 `leading/trailing_zeros` sites (`trie32.rs`
`bitmap_first_ge`/`bitmap_last_le`, etc.). These run on `u64` words even on RV32 (XLEN
= 32).

*Source:* RISC-V Unpriv ISA (v20240411) §2.4.2 — the complete RV32I R-type op set is
ADD/SUB, SLT[U], AND/OR/XOR, shifts — **no bit-count op** in the base ISA. Popcount /
clz / ctz live only in the **Zbb** extension: Bitmanip v1.0.0 §2.16 `cpop` (p.30, *"The
GCC builtin `__builtin_popcount` is implemented by `cpop` on RV32"*), §2.14 `clz`
(p.28), §2.18 `ctz` (p.32). Zbb ratified Nov 2021.

*Verdict:* **CI's baseline RV32 target is `riscv32imac-unknown-none-elf` (no Zbb)** and RV64 is
`riscv64gc` (no Zbb). On baseline RV32I/RV32IMAC, **all 37 popcount/clz/ctz sites in the hot `trie32` path lower to software** (a 26-instruction SWAR sequence for `u64::count_ones` across two 32-bit halves, or software CLZ/CTZ loops).

When building with `RUSTFLAGS="-C target-feature=+zbb"` on Zbb-capable hardware *(verified: `rustc 1.88.0`, `cargo rustc -p expanse-trie --lib --target riscv32imac-unknown-none-elf --release -- --emit asm`)*:
- `u32::count_ones` lowers directly to single-instruction `cpop rd, rs`.
- `u64::count_ones` on RV32 (XLEN = 32) lowers to 2 `cpop` instructions + 1 `add` (`cpop a0, a0; cpop a1, a1; add a0, a0, a1` — 3 instructions total vs 26 instructions in software SWAR).
- `leading_zeros` lowers to single-instruction `clz rd, rs`.
- `trailing_zeros` lowers to single-instruction `ctz rd, rs`.
- Bitwise logic-with-negate (`andn`, `orn`, `xnor`) and min/max (`min`, `max`, `minu`, `maxu`) lower to single instructions.

Zbb `clz`/`ctz` return XLEN (32) on zero input, matching Rust's semantics exactly. `#![no_std]` embedded targets do not support runtime extension discovery, so compile-time dispatch via `target-feature=+zbb` is the standard delivery mechanism, guarded by CI's `test-rv32-zbb` compilation lane.

### 3.2 32-bit address space (Sv32) — **VALIDATED**

RV32 XLEN = 32 → pointer width 32; the compact 8-byte-edge trie is gated on
`target_pointer_width="32"` (`lib.rs`), which Rust derives correctly from the target.

---

## 4. Embedded (Cortex-M / ESP32)

### 4.1 `align(32)` "cache line" — **VALID for cached cores, imprecise for cacheless M4**

*Usage:* `types32.rs` `CACHE_LINE_32 = 32` (comment: *"cache line size on … Cortex-M7,
ESP32 MMU cache"*); README/RFC "BranchL2_32 = 32 B = 1 cache line on Cortex-M7/ESP32."

*Source:* Cortex-M7 = 32 B line, **cache optional** — Arm DDI 0489F (r1p2) CCSIDR p.3-11
(*"0x1 … Represents 32 bytes"*), p.5-19/5-25 (32-byte burst/boundary); p.1-4 lists "no
D-cache" as a build option. **Cortex-M4 = cacheless** — Arm DDI 0439C has zero cache
descriptions. ESP32-C3 = fixed 32 B — TRM v1.4 §3.3.3.2 p.95 (*"block size is 32
bytes"*); ESP32 (LX6) = 32 B — TRM v5.8 p.71; ESP32-S3 = 16/32/64 B configurable — TRM
v1.8 p.405. No popcount on Cortex-M (ARMv7-M ARM DDI 0403E.e: `CLZ`/`RBIT` exist,
popcount does not).

*Verdict:* The RFC's hardware facts are **accurate**. Two README-level doc-vs-reality
gaps to fix (the RFC itself is honest): (a) `types32.rs`'s one-line "cache line" comment
overclaims for the **cacheless Cortex-M4 that CI actually targets** — the real
justification (AXI burst / DMA block / misaligned-SRAM-trap avoidance) lives only in the
RFC; (b) see §4.2.

### 4.2 Embedded architecture & component packaging

- **ESP-IDF component / ESP32 packaging** (shipped in #268 / #265): The ESP-IDF component lives in `components/expanse/` with `idf_component.yml`, `CMakeLists.txt`, `Kconfig`, and SRAM capability integration (`expanse_esp_alloc_internal`). Bare-metal ESP32-C3 (`riscv32imc-unknown-none-elf`) is checked in CI alongside `riscv32imac` and `thumbv7em`.
- **CI cross-compilation matrix**: Bare-metal RV32 (`riscv32imac-unknown-none-elf`, with and without `+zbb`), ESP32-C3 (`riscv32imc-unknown-none-elf`), and Arm Cortex-M4 (`thumbv7em-none-eabihf`) are verified on every PR.

---

## 5. Validation summary

**Assumptions confirmed (with citation):** SSE2 baseline (§1.1), POPCNT runtime-gate
(§1.2), TZCNT/LZCNT always-correct (§1.3), x86 64-byte line (§1.4), prefetch-removal
(§1.5), LA57 (§1.6), NEON presence & execution parity (§2.1, §2.6), AArch64 runner
capability census & 64-byte cache line (§2.6), 32-byte embedded alignment for cached cores
(§4.1), RV32 pointer width (§3.2).

**Assumptions that are risks / lower to software:**
- ⚠️ **ARM 64-byte cache line (§2.4)** — not architectural; Apple Silicon is 128 B.
  Correctness holds, performance premise does not. *Highest-value finding.* The
  Neoverse-N2 runner measuring 64 B (§2.6) confirms the assumption on that part;
  it says nothing about Apple Silicon, which is where the premise fails and which
  remains untested — so the finding stands at full weight.
- ⚠️ **RV32 popcount/clz/ctz (§3.1)** — software on base `riscv32imac`; enabled with `+zbb` lane in CI.

**Recommended code-comment precision fixes (non-behavioral):**
- `bits.rs` — attribute the SSE2 "always present" guarantee to the psABI/Rust target,
  not the CPU manual (§1.1); note AArch64 `CNT` is NEON, scalar popcount needs FEAT_CSSC
  (§2.2).
- `types.rs` / ARCHITECTURE.md — document `CACHE_LINE = 64` as a node-packing constant;
  cite `CTR_EL0` for the ARM caveat (§2.4).
- `types32.rs` — reword the "cache line" comment for cacheless M-cores; point at the RFC
  (§4.1).
- `bits.rs` NEON SAFETY comment — "NEON guaranteed by Rust's aarch64 `+neon` baseline;
  FEAT_AdvSIMD is architecturally optional but universally present" (§2.1).

**Documentation status:** ESP-IDF component package, RV32 bare-metal cross-compilation lanes, and AArch64 native execution / Callgrind regression gating documented in `README.md`, `docs/PACKAGING.md`, `docs/CI.md`, and `components/expanse/README.md`.

---

## 6. Missed opportunities (prioritized, with citations)

| Opportunity | Where it helps | Priority | Key caveat |
|---|---|---|---|
| **RV32 `+zbb`** build profile | 37 popcount/clz/ctz sites → single `cpop`/`clz`/`ctz` | **High** (concrete, spec-cited) — **shipped**: `test-rv32-zbb` CI lane builds with `-C target-feature=+zbb`; codegen verified (see docs/design/32-bit-embedded.md §2.3) | embedded parts may predate Zbb — keep SW fallback |
| **AVX-512 VPOPCNTDQ** for `Bitmap256` rank | 4×u64 bitmap → one `_mm256_popcnt_epi64` | Moderate, benchmark-gated | AVX-512 license **downclock** (Opt. Manual p.76,81) |
| **AVX2** 32-byte leaf scan | KB≥2 / wider sorted leaves | Low–moderate | leaves rarely > 16 lanes before bitmap switch |
| **FEAT_CSSC** scalar bit-manip (AArch64) | `count_ones`/`ctz` → single scalar op | Moderate (future) | Armv8.9+ (Neoverse-class); nightly/asm only today |
| **SVE / SVE2** VL-agnostic scan (AArch64) | additive fast leaf scan on Neoverse/Graviton | **Tried and reverted — measured negative** | implemented in #434, reverted after the AArch64 lane measured `map_get/sequential` **+9.19%**, `map_get/random` **+9.86%**, `set_contains/random` **+7.73%** *(measured: GitHub AArch64 runner Neoverse-N2, jobs 98711354227 → 98737273484; x86 byte-identical across the same commits, so the change is arch-local)*. Mechanism: the adaptive ladder promotes to bitmap before linear leaves grow, so `whilelt`/`brkb`/`cntp` setup is paid per scan over ≤16 elements and never amortises. **The same argument applies to any wide-vector leaf scan**, including the AVX2 row below. Needs runtime detection (none exists yet: no `is_aarch64_feature_detected!` in `crates/`); hardware **exposed** on the GitHub AArch64 runner (§2.6) so it is now testable, absent on Apple M-series |
| **RVV** vector leaf scan (RISC-V) | `vcpop.m`/`viota.m`/`vfirst.m` on RV64 | Low (availability) | absent on embedded RV32 targets |
| **ARMv7E-M DSP SIMD** (`SADD8`, `SEL`) on M4/M7 | 8-byte digit scan | Low | Rust stable exposes no ARM DSP intrinsics |
| **FEAT_RPRFM** range prefetch (AArch64) | contiguous node/leaf arrays (#242) | Low | hint, impl-defined; measure per-µarch |

### 6.1 Anti-finding — **do NOT adopt BMI2 PEXT/PDEP for rank/select naively**

A `PEXT`-based bitmap rank looks attractive, but it **regresses badly on AMD Zen1/Zen2**,
where `PEXT`/`PDEP` are microcoded: **latency AND reciprocal-throughput ≈ 18–19 cycles**
(Agner Fog Instruction Tables 2025-09-20: Zen1 p.90, Zen2 p.104), versus latency 3 /
1-per-cycle on Zen3+ and all Intel BMI2 parts (Haswell p.266, Zen3 p.116). Because
reciprocal throughput is also ~18, they do not pipeline in a rank loop. **The existing
POPCNT-based `Bitmap256` rank/select is the correct design choice**; a PEXT path would at
minimum need a Zen1/Zen2 exclusion. (The "~300-cycle" figure sometimes quoted for
PEXT/PDEP is *not* substantiated — the measured value is 18–19 cycles.)

**Detection consequence.** That exclusion is why `bmi2_rt::detect` reads raw CPUID leaves rather than using
`is_x86_feature_detected!`. The macro answers only *is BMI2 present*; the exclusion needs *present, but this
is an AMD family where `PEXT`/`PDEP` are microcoded* — a vendor string from leaf 0 plus a family check. No
feature-flag query can express that, so raw leaves are required on any target where BMI2 dispatch runs.

Two knock-on effects, both intended. Raw CPUID is `asm!`, which Miri cannot execute, so the detect modules
are `#[cfg(all(target_arch = "x86_64", not(miri)))]` with `available() -> false` stubs under Miri — the
portable path is what Miri verifies (#468). And `is_x86_feature_detected!` is `std`-only, so raw CPUID also
keeps the x86-64 dispatch usable in a `no_std` build; that is a smaller reason than the family check, and
not the one to cite.

---

## 7. References

All PDFs cached under `docs/research/hardware/` (git-ignored); re-download from the URLs.

**x86-64:**
- Intel 64 and IA-32 Architectures SDM Vol 1 (253665), Vol 2A-2D combined (325383, Dec
  2022), Vol 3A (253668, Dec 2023) — `intel.com` / `cdrdv2-public.intel.com`.
- Intel 64 and IA-32 Architectures Optimization Reference Manual (248966-049, Dec 2023).
- AMD64 Architecture Programmer's Manual Vol 1 (24592) and Vol 3 (24594, rev 3.26, May
  2018). *(amd.com blocks automated fetch; obtained from established mirrors of the
  official PDFs.)*
- Agner Fog, "Instruction Tables" (agner.org, updated 2025-09-20) — measured
  PEXT/PDEP latencies.

**AArch64:**
- Arm Architecture Reference Manual for A-profile, DDI 0487, issue M.c (2026-06-26,
  17,145 pp) — `documentation-service.arm.com` (`documentation/ddi0487/latest`).
- Arm Neon Intrinsics Reference for ACLE, IHI 0073, issue G (2020Q3).

**RISC-V:**
- RISC-V Instruction Set Manual Vol I: Unprivileged Architecture, v20240411 (ratified).
- RISC-V Bit-Manipulation extension (Zba/Zbb/Zbc/Zbs) v1.0.0 (ratified Nov 2021).
- RISC-V "V" Vector Extension v1.0 (ratified Nov 2021).

**Embedded:**
- Arm Cortex-M7 TRM (DDI 0489F, r1p2); Cortex-M4 TRM (DDI 0439C, r0p1); ARMv7-M
  Architecture Reference Manual (DDI 0403E.e).
- Espressif ESP32 TRM (v5.8, Xtensa LX6); ESP32-S3 TRM (v1.8, LX7); ESP32-C3 TRM (v1.4,
  RISC-V) — `documentation.espressif.com`.

---

## 8. Honest disclosure

Per the project's citation discipline, the following rest on **secondary sources** or
are **arithmetic/policy**, not primary-manual facts, and are labeled as such above:

- **Apple Silicon 128-byte cache line** and **Apple M-series lacks SVE** — Apple
  publishes no Arm-ARM-style manual with citable sections; sourced from Apple developer
  docs (`hw.cachelinesize`), the LLVM/Clang M4 feature list, and community measurement
  (mimalloc PR #419). The *architectural* claim (line size is IMPLEMENTATION DEFINED,
  read `CTR_EL0`) is fully primary-sourced (§2.4).
- **"SSE2 mandatory in x86-64"** — a real guarantee, but sourced from the x86-64 psABI /
  Rust target spec, **not** a verbatim sentence in the (2005/2016-revision) Intel/AMD
  Vol 1 manuals consulted (§1.1).
- **"57-bit VA = 128 PiB"** — arithmetic (2⁵⁷); the SDM states 57-bit / 5-level paging
  (§1.6). **"Linux keeps allocations < 47-bit by default"** — Linux kernel policy
  (`5level-paging.rst`), not the Intel SDM.
- **PEXT/PDEP "~300 cycle" figure** — unsubstantiated; the verified measured value is
  18–19 cycles on Zen1/Zen2 (§6.1).
- **AMD Software Optimization Guide** per-instruction latencies — `amd.com` blocked
  automated fetch; Agner Fog's measured tables (corroborated by uops.info) were used
  instead, which are the accepted primary measurement source.
- **Cortex-M55** cache-line (referenced in the RFC) — the M7 half is verified; the M55
  TRM (DDI 0552) was not downloaded (M55 is not a repo target).

*Every other section/page citation in this document was extracted directly from the
cached primary-source PDF and quoted verbatim with its page index.*
