# AVX-512 `Bitmap256` Cardinality: Empirical Comparative Methodology

## 1. Executive Summary & Problem Statement

In set algebra and inverted-index evaluation, `Bitmap256::count_and` is the inner kernel executed during bitmap leaf intersections (`algebra.rs`). On `x86_64`, a 256-bit bitmap occupies one 256-bit vector register (`ymm`), and two bitmaps occupy one 512-bit vector register (`zmm`).

The `avx512vpopcntdq` instruction set extension provides vector population count (`vpopcntq`), allowing SIMD evaluation of bitwise intersection and cardinality aggregation across multiple bitmap words in parallel.

This benchmark evaluates the performance opportunity of AVX-512 vectorized cardinality against production scalar baselines across the memory hierarchy (L1, L2, L3, DRAM).

---

## 2. Pre-Registration & Expected Losses Matrix

Per `AGENTS.md` §8.8 commit 2 (pre-registration locked before any main data) and §8.3 (each baseline is a production-grade twin with a regime it can win):

| Workload / Working Set | Expected Winner | Primary Mechanism & Structural Rationale |
|---|---|---|
| **L1 Cache Resident ($16\text{ KiB}$)** | **`v512`** | Working set fits entirely within L1 data cache; memory access latency is sub-cycle and throughput is purely compute-bound, allowing 512-bit vector width to maximize arithmetic retirements per cycle. |
| **L2 Cache Resident ($512\text{ KiB}$)** | **`v512`** / **`v256`** | Working set fits in private L2 cache; compute speedup remains significant but vector advantage narrows relative to L1. |
| **L3 Cache Resident ($16\text{ MiB}$)** | **`v512`** / **`scalar_popcnt`** | Interconnect latency and shared L3 slice traversal begin to compete with vector compute throughput. |
| **DRAM Non-Resident ($256\text{ MiB}$)** | **`scalar_popcnt`** (near parity) | DRAM memory bandwidth saturates; traversal latency is dominated by DRAM row-buffer activation and bus line transfers rather than bitwise counting instructions. |

### Baseline Selection Rule (AGENTS.md §8.3)
- Rating vector arms against `scalar_swar` would credit vectorization with an artificial win attributable strictly to hardware `popcnt` vs ~12-instruction SWAR emulation.
- The denominator baseline is strictly `scalar_popcnt` (`#[target_feature(enable = "popcnt")]`), ensuring fair measurement of SIMD vectorization alone.

---

## 3. Why Callgrind Cannot Measure AVX-512

1. **Valgrind CPUID Masking**: Valgrind masks `avx512*` CPUID bits. A runtime-dispatched kernel silently runs the scalar fallback, reporting 0 instruction delta and creating false negatives.
2. **Instruction Decoding**: Valgrind does not decode the EVEX prefix and emits `SIGILL` on `vpopcntq`.
3. **Measurement Instrument**: Measurements are performed with Criterion using BCa 95% bootstrap intervals across working set sizes on bare-metal hardware equipped with `avx512vpopcntdq`.
