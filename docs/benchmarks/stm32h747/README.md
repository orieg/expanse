# STM32H747I-DISCO on-target suite: results and how to read them

*(measured: STM32H747I-DISCO, silicon rev V; Cortex-M7 at 64 MHz HSI, 160 MHz PLL1/VOS3 and 400 MHz PLL1/VOS1, D-cache 4-way × 128 sets × 32-byte lines, off and on; Cortex-M4 at 200 MHz HCLK, no cache; clocks host-verified over 320M cycles at 64.1 / 160.2 / 400.5 / 200.3 MHz; libexpanse staticlib commit `22908c15`; DWT cycles per operation, min of 5 passes, one board; source `docs/benchmarks/stm32h747/results.json`, transcript alongside; every chart value is derived from that file by `scripts/generate_stm32_svg.py`. The full tables and the verdict against the #598 pre-registration are in [`docs/BENCHMARKING.md`](../../BENCHMARKING.md), "Cortex-M7 on-target"; the pre-registration record is [`METHODOLOGY.md`](METHODOLOGY.md); the firmware and the flash/capture flow are [`integrations/stm32h747/`](../../../integrations/stm32h747/README.md); `run.sh` here reproduces the run with the board attached.)*

> **Paired on the same board.** `28dd572e` (`main` immediately before #625) and
> `22908c15` were flashed back to back with identical harness firmware, so the
> engine is the only difference. `ingest` is the arm the #625 insert-path change
> reaches.
>
> **How to read the verdict.** The three alternatives in a row — sorted array,
> open-addressing hash, `tsearch` — are byte-identical C in both arms and run
> the same board, so any movement *they* show between the two runs is noise:
> layout, thermal, timing. The **noise floor** column is the largest such
> movement in that row. An Expanse movement smaller than it is not claimed; one
> larger than it is attributed to the engine change. The floor is often tiny
> here (the twins are very stable on this part), which is why a 0.3% move can
> sit outside it — that is a statement about the noise, not about the size of
> the effect.
>
> | core | MHz | D-cache | `28dd572e` | `22908c15` | Δ | noise floor | verdict |
> |---|---:|---|---:|---:|---:|---:|---|
> | `m7` | 64 | off | 1,397 | **1,097** | **-21.5%** | 0.1% | outside noise — attributed |
> | `m7` | 64 | on | 965 | **718** | **-25.5%** | 0.5% | outside noise — attributed |
> | `m7` | 160 | off | 1,762 | **1,438** | **-18.4%** | 0.0% | outside noise — attributed |
> | `m7` | 160 | on | 1,006 | **745** | **-26.0%** | 0.7% | outside noise — attributed |
> | `m7` | 400 | off | 1,762 | **1,438** | **-18.4%** | 0.0% | outside noise — attributed |
> | `m7` | 400 | on | 1,008 | **745** | **-26.1%** | 0.6% | outside noise — attributed |
> | `m4` | 200 | off | 4,145 | **3,037** | **-26.7%** | 1.0% | outside noise — attributed |
>
> The previously published artifact (`05575498`) predates #622 as well. Paired
> against the same control on the same board, that step — cap-classing the
> 32-bit bitmap subarrays — had never been measured on this part; it had been on
> the ESP32 only (#624):
>
> | core | MHz | D-cache | `05575498` | `28dd572e` | Δ | noise floor | verdict |
> |---|---:|---|---:|---:|---:|---:|---|
> | `m7` | 64 | off | 1,762 | **1,397** | **-20.7%** | 2.3% | outside noise — attributed |
> | `m7` | 64 | on | 1,176 | **965** | **-17.9%** | 3.5% | outside noise — attributed |
> | `m7` | 160 | off | 2,249 | **1,762** | **-21.7%** | 0.0% | outside noise — attributed |
> | `m7` | 160 | on | 1,199 | **1,006** | **-16.1%** | 6.9% | outside noise — attributed |
> | `m7` | 400 | off | 2,249 | **1,762** | **-21.6%** | 0.0% | outside noise — attributed |
> | `m7` | 400 | on | 1,199 | **1,008** | **-15.9%** | 6.8% | outside noise — attributed |
> | `m4` | 200 | off | 5,074 | **4,145** | **-18.3%** | 6.8% | outside noise — attributed |

### 1. Expanse on the M7: does it run, and does the cache-line node layout pay off?

![Expanse on the STM32H747I-DISCO Cortex-M7: cycles per operation across clocks and cache states, and the ISR-reader contract](../../assets/bench_stm32h747.svg)

**Left panel.** Six fixtures (rows), six bars each: the same code at three clocks, with the data cache off and on. A bar is the number of core cycles one operation costs; shorter is better. Two patterns matter more than any absolute value. Within a row, the cache-on bars stay the same length as the clock rises while the cache-off bars grow — once the core outruns the SRAM every miss costs more cycles, and the ratio between the two is the payoff of the 32-byte node geometry the design doc sized for this core (`docs/design/32-bit-embedded.md` §2.1.4). Across rows, `remove_range` is about a third of the per-key eviction loop: the batched eviction from #578, on hardware.

**Right panel.** The interrupt-handler contract. The main loop mutates a `sync32` map while a SysTick interrupt reads it; rows are how hard the writer works, from flat-out down to a thousand mutations per second. The orange bar is how often the interrupt's single-attempt read came back BUSY (it landed inside a write). The two grey/blue bars are how long the interrupt waited before it could run: blue with Expanse's optimistic reader, grey with the conventional `cpsid`/`cpsie` critical section around each write. Blue is a sliver at every duty; grey is one whole mutation long.

| reading | what it means |
|---|---|
| cache-on bars flat from 160 to 400 MHz; cache-off/cache-on ratio 1.6–1.9× at 2:1 core:bus | the working set fits the 16 KB D-cache; the node layout does its job |
| `remove_range` ≈ ⅓ of the per-key loop | batched ordered eviction holds on the target |
| BUSY 71% flat-out, 15 / 4 / 0.4% at 40k / 10k / 1k mutations/s | single-attempt reads are practical at realistic write rates |
| ISR entry latency ceiling 49–83 cycles vs 1,551–1,728 for the critical section | interrupt latency bounded 18–35× tighter than masking interrupts |

### 2. Expanse against a sorted array, an open-addressing hash table and `tsearch`

![Expanse vs a sorted array, an open-addressing hash table and newlib tsearch on the same Cortex-M7 fixtures, plus bytes per key](../../assets/bench_stm32h747_alternatives.svg)

**Left panel, log scale** — each gridline is 10× the previous, so a bar twice as long is far more than twice as slow. Four fixtures, four bars each: Expanse (blue), a sorted array with `bsearch` + `memmove` (orange), an open-addressing hash table at ≤ 50% load (green), newlib's `tsearch` (grey); every non-Expanse bar is labelled with its ratio to Expanse. Read it as an honest scorecard: the hash table wins point lookups and unordered inserts outright and the sorted array wins them too — Expanse is an ordered structure that allocates as it grows, and loses those on purpose. Bulk expiry (600 of 2,000) goes to the hash table's full scan because most records are expiring anyway. Steady-state expiry (25 of 2,000), the shape a real TTL index lives in, goes to Expanse, because the hash table must scan everything and the sorted array must shuffle memory. Disregard `tsearch`'s win in that row: an unbalanced tree over ascending keys degenerates into a list whose head is the minimum, and the same degeneracy makes its inserts 23× slower.

**Right panel.** Heap bytes per stored key (newlib `mallinfo`, allocator overhead included for all four). Expanse is the densest on dense sequential keys, middle of the pack on random keys, `tsearch` the worst by far. The sorted array and the hash table are pre-sized to capacity; Expanse and `tsearch` grow through `malloc`.

| shape | winner on this core |
|---|---|
| unordered inserts, point lookups | hash table (17× / 4.8×), then sorted array (6.4× / 2.6×) |
| bulk expiry, most records expiring | hash-table scan (6.3×) |
| steady-state expiry, few expiring | Expanse (3.0× over the scan, 1.6× over the sorted array) |
| bytes per key, sequential keys | Expanse (5.7 vs 8.0 / 16.4 / 36.0) |
| interrupt-safe reads | only Expanse offers them |

### 3. The other core, and two cores on one map

![Expanse on the STM32H747's cacheless Cortex-M4, and the M7-writer / M4-reader cells](../../assets/bench_stm32h747_dualcore.svg)

**Left panel.** The Expanse fixtures with a third bar (orange) for the Cortex-M4, which has no cache and runs at half the clock: every operation costs 3.3–4.7× the M7's cache-on cycles. The alternatives pay more for the move (5.2–7.2×), so the scorecard above holds on the small core with narrower gaps, and the interrupt contract holds there with a wider margin (a 91–98-cycle ceiling against 24 µs of masked interrupts).

**Right panel.** The experiment the `sync32` header explicitly marks unsupported, run to find out what happens: the M7 writes the map, the M4 reads it across the D2/D1 bridge, at each writer duty. Three blocks. **Blue**, M7 heap non-cacheable (MPU): correct — no wrong value ever, hits split as the key set dictates — but an M4 read of the M7's memory costs about 9 µs, so it is told BUSY far more often than the same-core interrupt was, and always when the writer is flat-out. **Grey**, the hardware-semaphore twin, the conventional cross-core method with a lock around every access: never BUSY, but a read waits up to about 150 µs when the writer is flat-out. **Orange**, M7 heap cacheable, the unsupported case: every read comes back BUSY and none comes back wrong — it fails safe.

| reading | what it means |
|---|---|
| M4 cycles 3.3–4.7× the M7 cache-on, 2.3–2.5× the M7 cache-off | the simpler in-order pipeline, not memory (its heap runs at core speed) |
| non-cacheable heap: BUSY 100 / 62 / 23 / 2.7% at full / 40k / 10k / 1k mutations/s, 0 corrupted | the protocol is correct across cores; the second core's slow reads make it impractical at high write rates |
| HSEM twin: 0 BUSY, lock wait up to 28.8k cycles (144 µs) flat-out, ≈1.5k (7.6 µs) at ≤ 10k/s | the lock trades BUSY for a bounded-but-long wait |
| cacheable heap: 100% BUSY at every duty, 0 corrupted | the unsupported configuration fails safe; the header's restriction stands, now with numbers |

**In one paragraph.** Expanse runs correctly on both cores; the cache-line node layout measurably pays off on the M7; and its interrupt-safe reads bound interrupt latency in a way that masking interrupts cannot. Against the structures firmware usually reaches for it loses raw point lookups and unordered inserts, and wins steady-state ordered expiry, memory density on dense keys, and the interrupt contract. Across two cores it is correct only with an uncached shared heap, and even then the second core's slow reads limit it to modest write rates.
