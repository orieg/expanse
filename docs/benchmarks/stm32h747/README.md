# STM32H747I-DISCO on-target suite: results and how to read them

*(measured: STM32H747I-DISCO, silicon rev V; Cortex-M7 at 64 MHz HSI, 160 MHz PLL1/VOS3 and 400 MHz PLL1/VOS1, D-cache 4-way × 128 sets × 32-byte lines, off and on; Cortex-M4 at 200 MHz HCLK, no cache; clocks host-verified over 320M cycles at 64.1 / 160.2 / 400.5 / 200.3 MHz; libexpanse staticlib commit `22908c15`; DWT cycles per operation, min of 5 passes, one board; source `docs/benchmarks/stm32h747/results/results.json`, transcript alongside; every chart value is derived from that file by `docs/benchmarks/stm32h747/scripts/generate_charts.py`. The full tables and the verdict against the #598 pre-registration are in [`docs/BENCHMARKING.md`](../../BENCHMARKING.md), "Cortex-M7 on-target"; the pre-registration record is [`METHODOLOGY.md`](METHODOLOGY.md); the firmware and the flash/capture flow are [`integrations/stm32h747/`](../../../integrations/stm32h747/README.md); `run.sh` here reproduces the run with the board attached.)*

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

![Expanse on the STM32H747I-DISCO Cortex-M7: cycles per operation across clocks and cache states, and the ISR-reader contract](results/bench_stm32h747.svg)

**Left panel.** Six fixtures (rows), six bars each: the same code at three clocks, with the data cache off and on. A bar is the number of core cycles one operation costs; shorter is better. Two patterns matter more than any absolute value. Within a row, the cache-on bars stay the same length as the clock rises while the cache-off bars grow — once the core outruns the SRAM every miss costs more cycles, and the ratio between the two is the payoff of the 32-byte node geometry the design doc sized for this core (`docs/design/32-bit-embedded.md` §2.1.4). Across rows, `remove_range` is about a third of the per-key eviction loop: the batched eviction from #578, on hardware.

**Right panel.** The interrupt-handler contract. The main loop mutates a `sync32` map while a SysTick interrupt reads it; rows are how hard the writer works, from flat-out down to a thousand mutations per second. The orange bar is how often the interrupt's single-attempt read came back BUSY (it landed inside a write). The two grey/blue bars are how long the interrupt waited before it could run: blue with Expanse's optimistic reader, grey with the conventional `cpsid`/`cpsie` critical section around each write. Blue is a sliver at every duty; grey is one whole mutation long.

| reading | what it means |
|---|---|
| cache-on bars flat from 160 to 400 MHz; cache-off/cache-on ratio 1.6–1.9× at 2:1 core:bus | the working set fits the 16 KB D-cache; the node layout does its job |
| `remove_range` ≈ ⅓ of the per-key loop | batched ordered eviction holds on the target |
| BUSY 71% flat-out, 15 / 4 / 0.4% at 40k / 10k / 1k mutations/s | single-attempt reads are practical at realistic write rates |
| ISR entry latency ceiling 49–83 cycles vs 1,551–1,728 for the critical section | interrupt latency bounded 18–35× tighter than masking interrupts |

### 2. Expanse against a sorted array, an open-addressing hash table and `tsearch`

![Expanse vs a sorted array, an open-addressing hash table and newlib tsearch on the same Cortex-M7 fixtures, plus bytes per key](results/bench_stm32h747_alternatives.svg)

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

![Expanse on the STM32H747's cacheless Cortex-M4, and the M7-writer / M4-reader cells](results/bench_stm32h747_dualcore.svg)

**Left panel.** The Expanse fixtures with a third bar (orange) for the Cortex-M4, which has no cache and runs at half the clock: every operation costs 3.3–4.7× the M7's cache-on cycles. The alternatives pay more for the move (5.2–7.2×), so the scorecard above holds on the small core with narrower gaps, and the interrupt contract holds there with a wider margin (a 91–98-cycle ceiling against 24 µs of masked interrupts).

**Right panel.** The experiment the `sync32` header explicitly marks unsupported, run to find out what happens: the M7 writes the map, the M4 reads it across the D2/D1 bridge, at each writer duty. Three blocks. **Blue**, M7 heap non-cacheable (MPU): correct — no wrong value ever, hits split as the key set dictates — but an M4 read of the M7's memory costs about 9 µs, so it is told BUSY far more often than the same-core interrupt was, and always when the writer is flat-out. **Grey**, the hardware-semaphore twin, the conventional cross-core method with a lock around every access: never BUSY, but a read waits up to about 150 µs when the writer is flat-out. **Orange**, M7 heap cacheable, the unsupported case: every read comes back BUSY and none comes back wrong — it fails safe.

| reading | what it means |
|---|---|
| M4 cycles 3.3–4.7× the M7 cache-on, 2.3–2.5× the M7 cache-off | the simpler in-order pipeline, not memory (its heap runs at core speed) |
| non-cacheable heap: BUSY 100 / 62 / 23 / 2.7% at full / 40k / 10k / 1k mutations/s, 0 corrupted | the protocol is correct across cores; the second core's slow reads make it impractical at high write rates |
| HSEM twin: 0 BUSY, lock wait up to 28.8k cycles (144 µs) flat-out, ≈1.5k (7.6 µs) at ≤ 10k/s | the lock trades BUSY for a bounded-but-long wait |
| cacheable heap: 100% BUSY at every duty, 0 corrupted | the unsupported configuration fails safe; the header's restriction stands, now with numbers |

**In one paragraph.** Expanse runs correctly on both cores; the cache-line node layout measurably pays off on the M7; and its interrupt-safe reads bound interrupt latency in a way that masking interrupts cannot. Against a sorted array, an open-addressing hash table and `tsearch` it loses raw point lookups and unordered inserts, and wins steady-state ordered expiry, memory density on dense keys, and the interrupt contract. Across two cores it is correct only with an uncached shared heap, and even then the second core's slow reads limit it to modest write rates.

## Cortex-M7 on-target: the full measured record

*(measured: STM32H747I-DISCO, silicon rev V (`DBGMCU_IDCODE` `0x20036450`); Cortex-M7 CPUID `0x411fc271` with D-cache 4-way × 128 sets × 32-byte lines read from CCSIDR, at 64 MHz HSI, 160 MHz PLL1/VOS3 and 400 MHz PLL1/VOS1 over the direct-SMPS supply the board is wired for; Cortex-M4 CPUID `0x410fc241`, no cache, at the 200 MHz HCLK; host-timed calibration over 320M cycles gives 64.1 / 160.2 / 400.5 MHz for the M7 and 200.3 MHz for the M4, and every nanosecond figure below uses those measured clocks; libexpanse staticlib commit `22908c15`, `thumbv7em-none-eabihf`, narrow C ABI; harness [`integrations/stm32h747/`](../../../integrations/stm32h747/README.md); artifacts [`docs/benchmarks/stm32h747/results/transcript.txt`](results/transcript.txt) and [`results.json`](results/results.json); charts [`bench_stm32h747.svg`](results/bench_stm32h747.svg), [`bench_stm32h747_alternatives.svg`](results/bench_stm32h747_alternatives.svg) and [`bench_stm32h747_dualcore.svg`](results/bench_stm32h747_dualcore.svg) from `docs/benchmarks/stm32h747/scripts/generate_charts.py`; #598.)*

The first execution of the engine on ARM, the first on a part with a data cache, and the first across two cores. Numbers are DWT core cycles per operation, min of 5 passes (pass-to-pass spread ≤ 0.5% for every Expanse cell), one board. Every in-firmware check passed on both cores (no `CHECK` line, no corrupted read, every dual-core cell acknowledged). VOS0 / 480 MHz is not reachable on this board: it needs the LDO supply path.

**Provenance.** Every table below is derived from the committed artifact, taken at `22908c15` on one board in one sitting. The `ingest` cells moved in two steps since the previous artifact (`05575498`), both paired on this board with identical harness firmware: `05575498` → `28dd572e` (cap-classed bitmap subarrays, #622) is -15.9% on the M7 (400 MHz, cache on) and -18.3% on the M4; `28dd572e` → `22908c15` (the insert path in #625) is -26.1% and -26.7%. Twin containers in each row bound what is attributable; the suite README carries both paired tables.

| fixture (`embedded_memtable.rs` shape, via the C ABI) | 64 MHz off | 64 MHz on | 160 MHz off | 160 MHz on | 400 MHz off | 400 MHz on (ns) |
|---|---:|---:|---:|---:|---:|---:|
| ingest, 2,000 sequential inserts (per insert) | 1,097 | 718 | 1,438 | 745 | 1,438 | 745 (1,863 ns) |
| CAN dispatch, 500 gets (per get) | 374 | 260 | 483 | 264 | 491 | 264 (659 ns) |
| BLE evict 600 of 2,000, per-key `first`/`remove` loop (per evicted) | 3,905 | 2,720 | 4,930 | 2,846 | 4,929 | 2,847 (7,109 ns) |
| BLE evict 600 of 2,000, `remove_range` (per evicted) | 1,329 | 955 | 1,652 | 995 | 1,649 | 994 (2,483 ns) |
| BLE evict 25 of 2,000, per-key loop (per evicted) | 3,749 | 2,646 | 4,770 | 2,841 | 4,769 | 2,824 (7,051 ns) |
| BLE evict 25 of 2,000, `remove_range` (per evicted) | 1,479 | 1,091 | 1,876 | 1,217 | 1,883 | 1,202 (3,001 ns) |

Reading. The cache-on cycle counts are the same at 160 and 400 MHz (the working set fits the 16 KB D-cache, so nothing is bus-bound once cached), and the cache-off counts are the same at those two clocks too — both run the AXI SRAM at core/2, so the miss cost in cycles is identical; only 64 MHz (bus at core/1) is cheaper. The cache-on/cache-off ratio therefore reads 1.9× (ingest), 1.9× (CAN) and 1.6–1.7× (evictions) at the 2:1 ratio, versus 1.4–1.5× at 1:1 — that ratio is the measurement of the 32-byte-line node geometry in `docs/design/32-bit-embedded.md` §2.1.4, and it is the number a design that straddled lines would lose. `remove_range` is 2.3–2.9× the per-key loop in both eviction shapes, consistent with the host result. At 400 MHz a point lookup is 659 ns and a sequential insert 3.0 µs.

**Expanse against a sorted array, an open-addressing hash table and newlib `tsearch`** (400 MHz, D-cache on; every implementation behind the same vtable and fixture code; the sorted array and the hash table pre-sized to capacity like the host suite's `HashMap::with_capacity`, Expanse and `tsearch` growing through newlib `malloc` as keys arrive):

| fixture | Expanse (C ABI) | sorted array, `bsearch` + `memmove` | open-addressing hash, FNV-1a, ≤ 50% load | newlib `tsearch` (unbalanced BST) |
|---|---:|---:|---:|---:|
| ingest, 2,000 sequential inserts | 745 / 1,863 ns | 185 / 462 ns (4.0× faster) | 71 / 178 ns (10.5× faster) | 29,511 / 73,779 ns (39.6× slower) |
| CAN dispatch, 500 gets | 264 / 659 ns | 100 / 249 ns (2.6× faster) | 55 / 138 ns (4.8× faster) | 5,821 / 14,534 ns (22.1× slower) |
| evict 600 of 2,000, batched (`remove_range`; scan for the hash) | 994 / 2,483 ns | 4,365 / 10,900 ns (4.4× slower) | 157 / 392 ns (6.3× faster) | 2,120 / 5,294 ns (2.1× slower) |
| evict 25 of 2,000, batched | 1,202 / 3,001 ns | 1,932 / 4,825 ns (1.6× slower) | 3,583 / 8,945 ns (3.0× slower) | 885 / 2,209 ns (1.4× faster) |
| evict 600 of 2,000, per-key loop | 2,847 / 7,109 ns | 15,386 / 38,417 ns (5.4× slower) | n/a | 2,178 / 5,438 ns (1.3× faster) |
| evict 25 of 2,000, per-key loop | 2,824 / 7,051 ns | 11,972 / 29,894 ns (4.2× slower) | n/a | 942 / 2,353 ns (3.0× faster) |

| bytes per key, 2,000 keys (newlib heap in use via `mallinfo`, allocator overhead included; requested bytes in parentheses) | sequential keys, one map | BLE index: hash keys, dual index (ordered) or one table (hash) |
|---|---:|---:|
| Expanse (C ABI) | 5.7 (2.6) | 21.2 (16.4) |
| sorted array, `bsearch` + `memmove` | 8.0 (8.0) | 16.0 (16.0) |
| open-addressing hash, FNV-1a, ≤ 50% load | 16.4 (16.4) | 16.4 (16.4) |
| newlib `tsearch` (unbalanced BST) | 36.0 (12.0) | 72.0 (24.0) |

Verdict, stated as plainly as the wins. The hash table is the right structure for point lookups and unordered ingest on this core, by 4.8× and 10×; the sorted array beats Expanse on lookup (2.6×) and on sequential-append ingest (6.4×) and is denser on random keys. Expanse's case on an MCU is the same as on the host: the ordered operations at steady state — the 25-of-2,000 eviction where the hash table must scan 4,096 slots to find 25 expired records (3.0× slower) and the sorted array must `memmove` the survivors (1.6× slower) — plus the densest footprint on dense keys (5.7 heap bytes/key, 2.6 requested) and the interrupt contract below, which none of the three twins offers. Two losses are new relative to the host suite: the sorted array's lookup win (the `bsearch` over 500 keys is 9 probes of contiguous memory) and the 17× ingest gap to the hash table, larger than the host's 14×, because Expanse's inserts allocate through newlib `malloc` while the pre-sized twins never allocate. `tsearch` degenerates on sequential keys (an unbalanced tree over ascending timestamps is a linked list: 28k cycles per insert, 5.8k per lookup) and its eviction wins are an artefact of the same degeneracy — the smallest key is the root, so `first` and its removal are O(1); it should not be read as an ordered-index result.

**The interrupt-handler contract, on real interrupts** (400 MHz, D-cache on): a SysTick ISR every 20,000 cycles calls `expanse_sync32_map_reader_try_get` while the main loop mutates the map, at full duty and paced to three mutation rates with jittered gaps (commensurate periods made the ISR alias into the pacing spin and report 0 BUSY — measured, and discarded). The twin is what bare-metal firmware actually does: `cpsid i`/`cpsie i` around a plain `expanse_map_t`, plain `expanse_map_get` in the ISR. 10,000 interrupts per cell.

| writer duty (mutations/s) | `sync32` single-attempt BUSY | `sync32` ISR entry latency max / mean (max in ns) | critical-section ISR entry latency max / mean (max in ns) | writer cycles per mutation `sync32` / critical section |
|---|---:|---:|---:|---:|
| full duty (~211k/s) | 70.7% | 83 / 25 (207 ns) | 1,551 / 734 (3,873 ns) | 1,893 / 1,479 |
| 40k/s | 14.5% | 54 / 22 (135 ns) | 1,683 / 151 (4,202 ns) | 10,017 / 10,014 |
| 10k/s | 3.7% | 54 / 18 (135 ns) | 1,728 / 54 (4,315 ns) | 40,188 / 40,050 |
| 1k/s | 0.4% | 49 / 15 (122 ns) | 1,671 / 18 (4,172 ns) | 396,227 / 401,085 |

Against the expected-loss matrix pre-registered in #598: the critical section is 22% cheaper per mutation at full duty and never returns BUSY; the optimistic surface holds the ISR entry-latency ceiling at 49–83 cycles (122–207 ns) at every duty, versus 1,551–1,728 cycles (3.9–4.3 µs) for the critical section — an 18–35× bound, and the number an interrupt budget is written against. The price is the BUSY rate, which is the writer's bracket occupancy and falls linearly with duty. Zero corrupted values, zero reclaim refusals, zero arena-full over every cell.

**The other core: Cortex-M4, no cache, 200 MHz** (same fixtures, same four implementations, same ISR arms, run by the M4 into a shared-memory text buffer while the M7 relays its calibration to the host):

| fixture (Expanse, C ABI) | M7 400 MHz, cache on (ns) | M7 400 MHz, cache off | M4 200 MHz (ns) | M4 / M7 cache-on, cycles | M4 / M7 cache-on, time |
|---|---:|---:|---:|---:|---:|
| ingest, 2,000 sequential inserts (per insert) | 745 (1,863 ns) | 1,438 | 3,037 (15,159 ns) | 4.1× | 8.1× |
| CAN dispatch, 500 gets (per get) | 264 (659 ns) | 491 | 1,234 (6,160 ns) | 4.7× | 9.3× |
| BLE evict 600 of 2,000, `remove_range` (per evicted) | 994 (2,483 ns) | 1,649 | 3,844 (19,195 ns) | 3.9× | 7.7× |
| BLE evict 25 of 2,000, `remove_range` (per evicted) | 1,202 (3,001 ns) | 1,883 | 3,994 (19,947 ns) | 3.3× | 6.7× |

The alternatives pay more for the move than Expanse does (5.2–7.2× in cycles for the sorted array and the hash table against Expanse's 3.3–4.7×), so the relative picture on the M4 is the M7's with the gaps narrowed: the hash table's ingest lead is 7.5× (406 vs 3,037) and its lookup lead 4.3× (287 vs 1,234); the sorted array's lookup lead is 2.2× (567) and its batched bulk eviction 8.1× slower (31,252); `tsearch` sits at parity with Expanse on the steady eviction (4,133 vs 3,994) for the degenerate-tree reason above. The like-for-like cacheless comparison is the M7's cache-off column: the M4 needs 2.3–2.5× the cycles of the M7 with its D-cache off, which is the simpler in-order pipeline, not memory (its heap is D2 SRAM at core speed).

| M4 ISR arm, writer duty | `sync32` BUSY | `sync32` ISR entry latency max / mean (max in ns) | critical-section max / mean (max in ns) | writer cycles per mutation `sync32` / critical section |
|---|---:|---:|---:|---:|
| full duty (~35k/s) | 60.7% | 96 / 74 (479 ns) | 4,897 / 2,415 (24,454 ns) | 5,690 / 5,238 |
| 40k/s (not reachable: also full duty, jittered) | 76.6% | 98 / 73 (489 ns) | 4,897 / 2,358 (24,454 ns) | 5,726 / 5,359 |
| 10k/s | 20.2% | 92 / 72 (459 ns) | 4,896 / 617 (24,449 ns) | 19,984 / 20,005 |
| 1k/s | 1.8% | 91 / 72 (454 ns) | 4,879 / 132 (24,365 ns) | 201,216 / 200,516 |

The contract holds on the smaller core with a wider margin: a 91–98-cycle ceiling (454–489 ns) against 4,879–4,897 cycles (24.4 µs) for masking interrupts around a 5,700-cycle mutation — a 50× bound; and the same BUSY-vs-duty curve.

**Two cores on one map** — the M7 mutates a `sync32` map in its own AXI SRAM heap while the M4 reads it across the D2/D1 bridge, per writer duty; three series: the M7 heap **non-cacheable** (MPU), the same with a **hardware-semaphore twin** (HSEM 0 around every mutation and every read, which is what firmware does across cores), and the heap **cacheable** — the configuration the `sync32` header marks unsupported, measured rather than assumed. The first version of this cell put the map in an SRAM4 arena and read zero hits: the `sync32` arena only holds node *handles*, every node body is a heap `Box`, so the M4 was reading the M7's cached heap; that cell was discarded and the design corrected.

| M7 heap | reads | writer duty | M4 reads | OK / not found | BUSY | corrupted (M4 / writer) | M4 read cycles mean / max (max in ns) | lock wait mean / max | writer cycles per mutation |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| non-cacheable | `sync32` single-attempt | full duty | 194,688 | 15 / 7 | 100.0% | 0 / 0 | 373 / 2,218 (11,076 ns) | — | 2,848 |
| non-cacheable | `sync32` single-attempt | 40k/s | 84,160 | 13,702 / 18,045 | 62.3% | 0 / 0 | 1,039 / 2,236 (11,166 ns) | — | 10,018 |
| non-cacheable | `sync32` single-attempt | 10k/s | 58,432 | 22,169 / 22,955 | 22.8% | 0 / 0 | 1,552 / 2,234 (11,156 ns) | — | 39,705 |
| non-cacheable | `sync32` single-attempt | 1k/s | 50,688 | 24,774 / 24,564 | 2.7% | 0 / 0 | 1,811 / 2,232 (11,146 ns) | — | 387,586 |
| non-cacheable | HSEM-locked twin | full duty | 17,664 | 8,934 / 8,730 | 0.0% | 0 / 0 | 5,501 / 30,334 (151,480 ns) | 3,590 / 28,847 | 4,162 |
| non-cacheable | HSEM-locked twin | 40k/s | 35,904 | 18,039 / 17,865 | 0.0% | 0 / 0 | 2,619 / 6,224 (31,081 ns) | 718 / 3,953 | 10,022 |
| non-cacheable | HSEM-locked twin | 10k/s | 44,288 | 22,070 / 22,218 | 0.0% | 0 / 0 | 2,095 / 3,852 (19,236 ns) | 198 / 1,577 | 39,829 |
| non-cacheable | HSEM-locked twin | 1k/s | 46,720 | 23,436 / 23,284 | 0.0% | 0 / 0 | 1,978 / 3,794 (18,946 ns) | 78 / 1,523 | 394,836 |
| cacheable | `sync32` single-attempt | full duty | 213,696 | 0 / 0 | 100.0% | 0 / 0 | 327 / 330 (1,648 ns) | — | 1,834 |
| cacheable | `sync32` single-attempt | 40k/s | 78,272 | 0 / 0 | 100.0% | 0 / 0 | 1,137 / 1,142 (5,703 ns) | — | 10,017 |
| cacheable | `sync32` single-attempt | 10k/s | 78,272 | 0 / 0 | 100.0% | 0 / 0 | 1,137 / 1,137 (5,678 ns) | — | 40,089 |
| cacheable | `sync32` single-attempt | 1k/s | 78,336 | 0 / 0 | 100.0% | 0 / 0 | 1,137 / 1,137 (5,678 ns) | — | 401,758 |

Evaluation, not a claim (this is #598 step 3, and the header keeps saying unsupported). With the M7 heap non-cacheable the protocol is correct across cores — every successful read returned the right value, found and not-found split evenly as the key set dictates, no reclamation refusal — but a single-attempt read from the M4 costs 1,800 cycles (9 µs) at low duty against 264 for the M7 reading its own heap and 1,234 for the M4 reading its own D2 SRAM: that is the D2→D1 bridge, and the 7× longer read window is why BUSY at a given duty is ~4× the same-core ISR arm's (62% vs 14.5% at 40k mutations/s), reaching 100% at full duty. The optimistic reader's worst case is a BUSY answer in 2.2k cycles; the semaphore twin's worst case is a 28.8k-cycle (144 µs) wait at full duty and 1.5k (7.6 µs) at ≤ 10k/s, with reads costing 2.0–2.6k cycles on average including the lock. With the heap cacheable the M4 never sees a consistent version and every read is BUSY, at every duty — the unsupported configuration fails safe rather than silently wrong, and it says nothing about correctness under other cache states. Lifting the restriction would need either a non-cacheable heap for the map (what this cell does, at the M7's cost of running that heap uncached) or explicit clean/invalidate around the version bracket; neither is in the library today.

Not covered: VOS0 / 480 MHz (board wiring), a balanced-tree twin (the toolchain ships none), M4 as writer, external-reviewer replication.

> Moved here verbatim from `docs/BENCHMARKING.md` (#643 step 5), which carried the
> fuller record while this suite held a summary. No figure, provenance tag or caveat
> was altered in the move, and nothing was re-measured.
