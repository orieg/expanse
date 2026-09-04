# WebAssembly (`wasm32` vs `wasm64`) Benchmark Suite: Empirical Comparative Methodology

## 1. Executive Summary & Problem Statement

WebAssembly runs under memory sandboxing across two target architectures:
- **`wasm32`**: 32-bit linear memory space (maximum $4\text{ GiB}$), where pointers and table indexes are 32-bit integers.
- **`wasm64`** (Memory64 proposal): 64-bit linear memory space, where pointers and table indexes are 64-bit integers.

In Expanse, the public aliases point to the 32-bit engine on 32-bit targets (`ExpanseMap` $\to$ `ExpanseMap32`, compact 8-byte `Edge32`), and to the 64-bit engine on 64-bit targets (16-byte `Edge`).

This suite measures the performance and memory footprint trade-offs between the two engines compiled to their respective WebAssembly targets with byte-identical test fixture source.

---

## 2. Measurement Instrument: Deterministic Fuel Accounting

1. **Why Fuel Counting**:
   - Wall-clock measurements in WebAssembly runtimes are subject to JIT compilation jitter, garbage collection pauses, and host OS CPU scheduling noise.
   - Wasmtime's deterministic fuel instrumentation decrements an exact instruction counter per executed WebAssembly instruction opcode.
   - `fuel(phase 1) − fuel(phase 0)` divided by population $N$ delivers an exact, zero-variance metric of computational work per operation.

2. **What Fuel Counts and Does Not Count**:
   - Fuel counts executed WebAssembly instructions. It is not cycle count and not native instruction retirement.
   - Address arithmetic (i32 vs i64) and bounds checking lower differently between `wasm32` and `wasm64`.
   - Fuel does not model host CPU cache hierarchy; changes trading instructions for memory locality must additionally be cross-checked on wall-clock instruments.

---

## 3. Pre-Registration & Expected Losses Matrix

Per `AGENTS.md` §8.8 commit 2 (pre-registration locked before any main data) and §8.3 (each baseline is a production-grade twin with a regime it can win):

| Metric / Regime | Expected Winner | Primary Mechanism & Structural Rationale |
|---|---|---|
| **Instruction Fuel per Operation** (point, iterate, range, remove) | **`wasm64` (64-bit engine)** | 64-bit engine operates on full 64-bit words natively; broader fanout transitions and fewer tree levels reduce total loop iterations. |
| **Memory Density (Bytes / Key)** (Sequential, Clustered, Random) | **`wasm32` (32-bit engine)** | 8-byte `Edge32` descriptors and 4-byte `ValueSlot32` achieve roughly half the pointer overhead of the 64-bit engine. |
| **Dense Sequential Set Memory Footprint** | **`wasm64`** | Sequential keys coalesce into full-expanse leaves; 64-bit full-expanse handles pack 64 keys per word vs 32 keys on 32-bit. |

---

## 4. Automated CI Regression Gating

- **Baseline Artifact**: `results/baseline_wasm_fuel.json`.
- **Thresholds**: Gated by `scripts/wasm_fuel.py --check-baseline`. A single arm $> 5\%$ above baseline fails CI; two or more arms $> 0.5\%$ fail CI.
- **Coverage**: Missing baseline arms are treated as coverage regressions and fail closed.
