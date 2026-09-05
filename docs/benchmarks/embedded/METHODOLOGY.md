# Embedded Storage Engines & MemTable Shapes: Empirical Comparative Methodology

## 1. Executive Summary & Problem Statement

Embedded edge computing environments (microcontrollers, automotive ECUs, IoT telemetry nodes) operate under strict constraints:
- Narrow machine words (32-bit addresses and pointers).
- Limited SRAM budgets ($32\text{ KiB}$ to $512\text{ KiB}$).
- Strict latency floors for interrupt service routines (ISRs) and real-time query paths.
- Deterministic memory overhead per key without unpredictable dynamic reallocation spikes.

This benchmark evaluates Expanse's 32-bit embedded engine (`trie32`, `ExpanseMap32`, `sync32`) across two distinct evaluation lanes:
1. **Host-Side MemTable Benchmarks (`embedded_memtable.rs`)**: Evaluated via Criterion with BCa 95% bootstrap intervals on quiet reference hardware against `std::collections::BTreeMap` and `hashbrown::HashMap`.
2. **On-Target Hardware Harvesting (`esp32.json`)**: On-device cycle counting on microcontroller hardware (ESP32 Xtensa dual-core @ 160 MHz) against twin C container baselines (`twin_containers.h`).

---

## 2. Pre-Registration & Expected Losses Matrix

Per `AGENTS.md` §8.8 commit 2 (pre-registration locked before any main data) and §8.3 (each baseline is a production-grade twin with a regime it can win):

| Workload / Regime | Expected Winner | Primary Mechanism & Structural Rationale |
|---|---|---|
| **Unordered Random Ingest** | **`hashbrown::HashMap`** | Flat open-addressing hash table writes directly to pre-allocated buckets with 0 tree descent or node allocation overhead. Expanse pays digital tree expanse traversal. |
| **Unordered Random Point Lookup** | **`hashbrown::HashMap`** | $O(1)$ SIMD group-probe hash lookup executes in ~1-2 cache accesses; digital trie requires $O(k)$ byte-by-byte descent. |
| **Steady-State TTL Eviction ($25\text{ of }2,000$ expired)** | **`ExpanseMap32` (`remove_range`)** | Expanse uses an ordered composite time-key index; `remove_range` prunes expired nodes by entire expanse subtrees without visiting unexpired live keys ($O(\text{expired})$). Hash tables must perform full scans ($O(N)$). |
| **Bulk TTL Eviction ($600\text{ of }2,000$ expired)** | **`hashbrown::HashMap`** | When 30% of the population expires in a burst, flat table memory sweep is cache-line linear; dual-index de-referencing and node freeing incurs higher random pointer overhead. |
| **Ordered Range Queries & CAN-ID Route Scans** | **`ExpanseMap32`** | Contiguous 32-byte-aligned leaf array traversals provide zero-allocation iterator scans; hash tables require sorting or auxiliary indexes. |
| **Memory Density on Sequential/Clustered Keys** | **`ExpanseMap32`** | Leaf bitmap packing achieves sub-byte to single-digit bytes per key, eliminating pointer indirection and hash bucket power-of-two over-provisioning. |

---

## 3. Two Separate Metrics Domains (AGENTS.md §8.12)

1. **Host Wall-Clock vs On-Device Cycles**:
   - Host benchmarks measure wall-clock nanoseconds on server microarchitectures.
   - On-device microcontroller benchmarks measure CPU clock cycles via hardware cycle counters (`DWT_CYCCNT` / `CCOUNT`).
   - The two metric domains must never be juxtaposed or averaged together on a single canvas or table.

2. **Per-Key Divisors Are Derived, Never Literal**:
   - The range-aggregation arms report cycles per key aggregated, and that divisor comes from
     `agg.count` — the keys the fold actually visited — not from the range's nominal width
     (AGENTS.md §8.2).
   - The distinction is not cosmetic here. The aggregation range `[base+100, base+600]` holds
     **400** keys at `pop=500` and **501** at `pop=2000`, because at the smaller population the
     range runs past the end of the populated keyspace. A literal `500` divisor therefore
     understated `pop=500` cycles/key by **25%** and overstated `pop=2000` by 0.2%.
   - All four arms fold the same key set, so the divisor was identical across them: **ratios
     within one population were unaffected** and remain valid, per the §8.10 ratio-vs-absolute
     split. What the literal divisor invalidated was absolute cycles/key at `pop=500` and every
     **cross-population** reading — a fixed-width range walk appeared to cost 20.6% more per key
     at `pop=2000` than at `pop=500` when, corrected, it is flat to slightly cheaper.
   - Both populations were re-harvested with the derived divisor at commit `49b3b6c1`, so no
     cell in `results/esp32.json` carries the literal-500 divisor any more. The `pop=500`
     aggregation cells now report ~400 keys and the `pop=2000` cells ~501, which is what the
     range actually holds.

3. **The Dispatch Control Sizes the One Remaining Arm Asymmetry**:
   - `expanse_memtable_aggregate_range` crosses the C ABI into Rust and is called back once
     per key through `expanse_map_for_each_range`; every twin folds inline. Lock symmetry
     (§8.16) is satisfied — all arms take the same recursive mutex once per call — but this
     per-key dispatch is a genuine §8.3 asymmetry that the Expanse-vs-twin ratio carries.
   - It was disclosed and never measured, which made the disclosure unfalsifiable. The
     `sorted_array_dispatch_inlined` / `sorted_array_dispatch_indirect` pair measures it:
     `twin_sorted_aggregate_indirect` is `twin_sorted_aggregate` with `agg_add(out, val)`
     replaced by `visit(key, val, out)`. Both run back to back on the container the
     `sorted_array` arm has already built, after one untimed walk so neither carries the
     cache fill, so **the gap between the two is the per-key dispatch and nothing else**.
   - **The pair allocates nothing, and that is a correctness requirement rather than tidiness.**
     The first cut built its own twin; on-device that shifted the heap enough that
     `twin_hash_create` later failed at n=2000 and four BLE benchmarks silently lost their
     hash comparator. A control that removes another arm is not a control. No host build can
     see this — only a device run reports it.
   - Because the pair runs warm and the published arms run cold, its absolute values are not
     comparable to `sorted_array`; only the difference within the pair is. Any ratio adjusted
     with that difference is **derived, not measured**, and is labelled so wherever it appears.
   - The visitor is read through a `volatile` so the target cannot be devirtualised under
     LTO. Expanse's visitor is reached from Rust across the C ABI and never can be, so a
     control that inlined its own callback would measure ~0 and report the asymmetry as
     free — a silent wrong answer rather than a loud failure (§8.1).
   - **It is a floor, not an estimate.** Dispatch is measured in a contiguous array walk,
     not inside Expanse's leaf walk where register pressure differs. It bounds what the
     callback costs Expanse from below; it does not say what it costs there.
   - **Measured** *(ESP32-D0WD-V3 rev v3.1, 160 MHz, commit `49b3b6c1`, medians of 10)*: the
     dispatch costs **+26.4 cycles per key** at `pop=2000` (45.06 inlined -> 71.46 indirect) and
     **+29.3** at `pop=500` (45.73 -> 75.00). Against a `sorted_array` arm at 55.2 cycles/key,
     that is on the order of half the twin's own walk.
   - What that does **not** license is a corrected ratio quoted as a measurement. Adding a
     warm-measured delta to cold-measured arms is arithmetic, so any adjusted figure is
     **derived**; and the dispatch is measured in a contiguous array walk rather than inside
     Expanse's leaf walk, so it bounds the cost from below rather than estimating it. The
     published Expanse-vs-twin ratios stand as measured, with the asymmetry now sized instead
     of merely disclosed.

4. **The Task Watchdog Is Off, Deliberately**:
   - The harvest runs every Unity case and benchmark arm on the main task, compute-bound and
     without yielding, so `IDLE0` never feeds the task watchdog and it fires on its 5 s period
     for the length of the sweep — ~16 times in an 82 s run, each printing a backtrace over the
     UART **while arms are being timed**.
   - Measured cost, both on hardware: one firing landed mid-line in a benchmark's JSON row and
     the harvester dropped it, so an arm reported 9 repetitions rather than 10 — deterministically,
     in the same cell, across two captures; and a watchdog ISR plus six lines of 115200-baud UART
     inside a timed window is the contamination `CONTAMINATION_RATIO` exists to catch.
   - `CONFIG_ESP_TASK_WDT_INIT=n` in `integrations/esp32/sdkconfig.defaults`. The interrupt
     watchdog stays on, and a wedged harness is visible as a missing `EXPANSE esp32 harvest done`
     sentinel, so nothing that mattered is being switched off.
   - **Pending device confirmation**: the committed `results/esp32.json` predates this and still
     carries the 9-sample arm. The next harvest is expected to read 10/10 on every arm
     ([#695](https://github.com/orieg/expanse/issues/695)); until one runs, that expectation is
     unverified.

5. **Run-to-Run Drift & Flash Cache Sensitivity**:
   - Microcontroller execution is sensitive to binary link layout and instruction-cache / flash-cache alignment.
   - Results are reported using median cycles across clean repetitions alongside BCa 95% intervals, with contamination sampling to identify ISR or flash-miss outliers.
