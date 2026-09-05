# STM32H747I-DISCO on-target suite: results and how to read them

*(measured: STM32H747I-DISCO, silicon rev V; Cortex-M7 at 64 MHz HSI, 160 MHz PLL1/VOS3 and 400 MHz PLL1/VOS1, D-cache 4-way × 128 sets × 32-byte lines, off and on; Cortex-M4 at 200 MHz HCLK, no cache; clocks host-verified over 320M cycles at 64.1 / 160.2 / 400.5 / 200.3 MHz; libexpanse staticlib commit `fce563c2`; DWT cycles per operation, min of 5 passes, one board; source `docs/benchmarks/stm32h747/results/results.json`, transcript alongside; every chart value is derived from that file by `docs/benchmarks/stm32h747/scripts/generate_charts.py`, and every table in this README by `docs/benchmarks/stm32h747/scripts/render_tables.py`. The full record is the second half of this page; the pre-registration record is [`METHODOLOGY.md`](METHODOLOGY.md); the firmware and the flash/capture flow are [`integrations/stm32h747/`](../../../integrations/stm32h747/README.md); `run.sh` here reproduces the run with the board attached.)*

> **Paired on the same board: `22908c15` → `fce563c2`.** The engine at `22908c15`
> (the previous artifact) and at `fce563c2` (`main`, carrying #667, #686, #694
> and #698) were built into the same harness firmware from the same toolchain
> and flashed in the order control, treatment, treatment, control, one sitting.
> The harness sources are byte-identical at both commits, so the engine archive
> is the only difference between the two images.
>
> **How to read the verdict.** The three alternatives in a row — sorted array,
> open-addressing hash, `tsearch` — are byte-identical C in both arms and run
> the same board, so any movement *they* show between the two runs is noise:
> layout, thermal, timing. The **noise floor** column is the largest such
> movement in that row. An Expanse movement smaller than it is not claimed; one
> larger than it is attributed to the engine change — a loss as readily as a
> win. Re-flashing the identical image moved no Expanse cell by more than 0.51%.
>
> | core | MHz | D-cache | fixture | `22908c15` | `fce563c2` | Δ | noise floor | verdict |
> |---|---:|---|---|---:|---:|---:|---:|---|
> | `m7` | 400 | on | `evict_steady_range` | 1,285 | **977** | **−24.0%** | 1.3% | outside noise — attributed |
> | `m7` | 400 | on | `evict_bulk_range` | 1,071 | **856** | **−20.1%** | 2.1% | outside noise — attributed |
> | `m7` | 400 | on | `evict_bulk_loop` | 2,923 | **2,580** | **−11.7%** | 2.0% | outside noise — attributed |
> | `m7` | 400 | on | `evict_steady_loop` | 3,000 | **2,668** | **−11.1%** | 3.4% | outside noise — attributed |
> | `m7` | 400 | on | `ingest` | 743 | **789** | **+6.2%** | 5.8% | outside noise — moved up (5.7% against a 5.7% floor in the reversed order) |
> | `m7` | 400 | on | `can_dispatch` | 252 | **272** | **+8.0%** | 4.5% | outside noise — moved up |
> | `m7` | 400 | off | `evict_steady_range` | 1,958 | **1,641** | **−16.2%** | 0.4% | outside noise — attributed |
> | `m7` | 400 | off | `evict_bulk_range` | 1,713 | **1,495** | **−12.7%** | 0.1% | outside noise — attributed |
> | `m7` | 400 | off | `evict_bulk_loop` | 4,864 | **4,513** | **−7.2%** | 0.1% | outside noise — attributed |
> | `m7` | 400 | off | `evict_steady_loop` | 4,903 | **4,584** | **−6.5%** | 0.8% | outside noise — attributed |
> | `m7` | 400 | off | `ingest` | 1,437 | **1,456** | **+1.3%** | 0.0% | outside noise — moved up |
> | `m7` | 400 | off | `can_dispatch` | 436 | **477** | **+9.3%** | 0.0% | outside noise — moved up |
> | `m4` | 200 | off | `evict_steady_range` | 4,050 | **3,260** | **−19.5%** | 0.8% | outside noise — attributed |
> | `m4` | 200 | off | `evict_bulk_range` | 3,866 | **3,234** | **−16.3%** | 0.5% | outside noise — attributed |
> | `m4` | 200 | off | `evict_steady_loop` | 10,594 | **9,177** | **−13.4%** | 0.5% | outside noise — attributed |
> | `m4` | 200 | off | `evict_bulk_loop` | 10,836 | **9,686** | **−10.6%** | 0.3% | outside noise — attributed |
> | `m4` | 200 | off | `ingest` | 3,050 | 2,967 | −2.7% | 7.3% | inside noise (`tsearch` moved 7.3% in this row; the other two twins 0.0% and 0.4%) |
> | `m4` | 200 | off | `can_dispatch` | 1,241 | **1,211** | **−2.4%** | 0.0% | outside noise — attributed |
>
> The 64 and 160 MHz cells, both flash orders and the run-to-run pairs of
> identical images are in
> [`results/pairing_fce563c2/pairing.txt`](results/pairing_fce563c2/pairing.txt);
> the four captures are alongside, with the archive and image hashes in
> `archives.txt`. The 160 MHz cells carry the 400 MHz verdicts to the decimal;
> at 64 MHz `can_dispatch` moved +2.5% (floor 2.3%) cache off and +5.2% (floor
> 4.4%) cache on, the evictions −7.8% to −20.3%.
>
> **What moved and why.** The four eviction fixtures are the removal path #667
> and #698 shortened (`read_rem` and `set_immed_keys` given constant lengths on
> native targets; host Callgrind `set32_remove` −4.31% and −7.66%), and they fall
> 6–24% on both cores at every clock and cache state. The `sync32` writer of the
> ISR arm, which alternates inserts and removes, went from 2,026 to 1,656 cycles
> per mutation at full duty (−18%), and its BUSY rate from 85% to 54%. The two
> read-side movements are recorded, not explained: the M7 `can_dispatch` cell
> rose +9.3% with the cache off and +8.0% with it on while the M4's fell −2.4%;
> the host Callgrind arm for the same fixture shape, `map32_get can_dispatch`,
> retired **106,341 instructions at every engine PR in the range** (#631, #642,
> #667, #686, #694, #698 — no change), and `trie32::map_get` shrank from 1,670
> to 1,540 bytes in the M7 image. Fewer bytes of code and the same instruction
> count moving the M7 up and the M4 down is a code-placement or fetch effect on
> the M7 pipeline; **cause unmeasured** — no DWT `CPICNT`/`LSUCNT` arm exists in
> the harness, and adding one is the candidate measurement. The `ingest` cells
> moved +1.3% cache off (floor 0.0%) and +5.7–6.2% cache on against a 5.7–5.8%
> floor on the M7, −2.7% inside a 7.3% floor on the M4; host `map32_insert
> sensor_timestamps` is −3.34% across the same range (#667). Those cells carry
> no verdict.
>
> **Earlier pairings on this board, kept as history.** The two steps below were
> measured the same way; their artifact (`22908c15`) is superseded by the one
> above, and the numbers here describe those steps, not the current engine.
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
> `28dd572e` is `main` immediately before #625, whose insert path is what the
> `ingest` arm reaches. The artifact before that (`05575498`) predated #622 as
> well; paired against the same control on the same board, that step —
> cap-classing the 32-bit bitmap subarrays — had until then been measured on the
> ESP32 only (#624):
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
| cache-on bars flat from 160 to 400 MHz; cache-off/cache-on ratio 1.7–1.9× at 2:1 core:bus | the working set fits the 16 KB D-cache; the node layout does its job |
| `remove_range` ≈ ⅓ of the per-key loop (the loop costs 2.7–3.0× the range call) | batched ordered eviction holds on the target |
| BUSY 54% flat-out, 13 / 3 / 0.4% at 40k / 10k / 1k mutations/s | single-attempt reads are practical at realistic write rates |
| ISR entry latency ceiling 15–56 cycles vs 1,639–1,743 for the critical section | interrupt latency bounded 29–113× tighter than masking interrupts |

### 2. Expanse against a sorted array, an open-addressing hash table and `tsearch`

![Expanse vs a sorted array, an open-addressing hash table and newlib tsearch on the same Cortex-M7 fixtures, plus bytes per key](results/bench_stm32h747_alternatives.svg)

**Left panel, log scale** — each gridline is 10× the previous, so a bar twice as long is far more than twice as slow. Four fixtures, four bars each: Expanse (blue), a sorted array with `bsearch` + `memmove` (orange), an open-addressing hash table at ≤ 50% load (green), newlib's `tsearch` (grey); every non-Expanse bar is labelled with its ratio to Expanse. Read it as an honest scorecard: the hash table wins point lookups and unordered inserts outright and the sorted array wins them too — Expanse is an ordered structure that allocates as it grows, and loses those on purpose. Bulk expiry (600 of 2,000) goes to the hash table's full scan because most records are expiring anyway. Steady-state expiry (25 of 2,000), the shape a real TTL index lives in, goes to Expanse, because the hash table must scan everything and the sorted array must shuffle memory. Disregard `tsearch`'s win in that row: an unbalanced tree over ascending keys degenerates into a list whose head is the minimum, and the same degeneracy makes its inserts 35× slower than Expanse's.

**Right panel.** Heap bytes per stored key (newlib `mallinfo`, allocator overhead included for all four). Expanse is the densest on dense sequential keys, middle of the pack on random keys, `tsearch` the worst by far. The sorted array and the hash table are pre-sized to capacity; Expanse and `tsearch` grow through `malloc`.

| shape | winner on this core |
|---|---|
| unordered inserts, point lookups | hash table (11.1× / 4.9×), then sorted array (4.3× / 2.7×) |
| bulk expiry, most records expiring | hash-table scan (5.4×) |
| steady-state expiry, few expiring | Expanse (3.7× over the scan, 2.0× over the sorted array) |
| bytes per key, sequential keys | Expanse (5.7 vs 8.0 / 16.4 / 36.0) |
| interrupt-safe reads | only Expanse offers them |

### 3. The other core, and two cores on one map

![Expanse on the STM32H747's cacheless Cortex-M4, and the M7-writer / M4-reader cells](results/bench_stm32h747_dualcore.svg)

**Left panel.** The Expanse fixtures with a third bar (orange) for the Cortex-M4, which has no cache and runs at half the clock: every operation costs 3.3–4.4× the M7's cache-on cycles. The alternatives pay more for the move (4.5–7.2×), so the scorecard above holds on the small core with narrower gaps, and the interrupt contract holds there with a wider margin (an 89–98-cycle ceiling against 26 µs of masked interrupts).

**Right panel.** The experiment the `sync32` header explicitly marks unsupported, run to find out what happens: the M7 writes the map, the M4 reads it across the D2/D1 bridge, at each writer duty. Three blocks. **Blue**, M7 heap non-cacheable (MPU): correct — no wrong value ever, hits split as the key set dictates — but an M4 read of the M7's memory costs about 9 µs, so it is told BUSY far more often than the same-core interrupt was, and always when the writer is flat-out. **Grey**, the hardware-semaphore twin, the conventional cross-core method with a lock around every access: never BUSY, but a read waits up to about 280 µs when the writer is flat-out. **Orange**, M7 heap cacheable, the unsupported case: every read comes back BUSY and none comes back wrong — it fails safe.

| reading | what it means |
|---|---|
| M4 cycles 3.3–4.4× the M7 cache-on, 2.0–2.5× the M7 cache-off | the simpler in-order pipeline, not memory (its heap runs at core speed) |
| non-cacheable heap: BUSY 100 / 61 / 22 / 2.6% at full / 40k / 10k / 1k mutations/s, 0 corrupted | the protocol is correct across cores; the second core's slow reads make it impractical at high write rates |
| HSEM twin: 0 BUSY, lock wait up to 55.4k cycles (277 µs) flat-out, ≈1.5k (7.3 µs) at ≤ 10k/s | the lock trades BUSY for a bounded-but-long wait |
| cacheable heap: 100% BUSY at every duty, 0 corrupted | the unsupported configuration fails safe; the header's restriction stands, now with numbers |

**In one paragraph.** Expanse runs correctly on both cores; the cache-line node layout measurably pays off on the M7; and its interrupt-safe reads bound interrupt latency in a way that masking interrupts cannot. Against a sorted array, an open-addressing hash table and `tsearch` it loses raw point lookups and unordered inserts, and wins steady-state ordered expiry, memory density on dense keys, and the interrupt contract. Across two cores it is correct only with an uncached shared heap, and even then the second core's slow reads limit it to modest write rates.

## Cortex-M7 on-target: the full measured record

*(measured: STM32H747I-DISCO, silicon rev V (`DBGMCU_IDCODE` `0x20036450`); Cortex-M7 CPUID `0x411fc271` with D-cache 4-way × 128 sets × 32-byte lines read from CCSIDR, at 64 MHz HSI, 160 MHz PLL1/VOS3 and 400 MHz PLL1/VOS1 over the direct-SMPS supply the board is wired for; Cortex-M4 CPUID `0x410fc241`, no cache, at the 200 MHz HCLK; host-timed calibration over 320M cycles gives 64.1 / 160.2 / 400.5 MHz for the M7 and 200.3 MHz for the M4, and every nanosecond figure below uses those measured clocks; libexpanse staticlib commit `fce563c2`, `thumbv7em-none-eabihf`, narrow C ABI; harness [`integrations/stm32h747/`](../../../integrations/stm32h747/README.md); artifacts [`docs/benchmarks/stm32h747/results/transcript.txt`](results/transcript.txt) and [`results.json`](results/results.json); charts [`bench_stm32h747.svg`](results/bench_stm32h747.svg), [`bench_stm32h747_alternatives.svg`](results/bench_stm32h747_alternatives.svg) and [`bench_stm32h747_dualcore.svg`](results/bench_stm32h747_dualcore.svg) from `docs/benchmarks/stm32h747/scripts/generate_charts.py`; tables from `docs/benchmarks/stm32h747/scripts/render_tables.py`; #598.)*

The first execution of the engine on ARM, the first on a part with a data cache, and the first across two cores. Numbers are DWT core cycles per operation, min of 5 passes, one board. Pass-to-pass spread is ≤ 0.5% for most Expanse cells; the exceptions in this capture are the M7 cache-on `can_dispatch` cells (4.9% at 160 and 400 MHz), `evict_steady_range` at 64 MHz cache off (2.7%) and `can_dispatch` at 160 MHz cache off (1.9%), which is why the cell is the minimum and not the mean. Every in-firmware check passed on both cores (no `CHECK` line, no corrupted read, every dual-core cell acknowledged). VOS0 / 480 MHz is not reachable on this board: it needs the LDO supply path.

**Provenance.** Every table below is derived from the committed artifact, taken at `fce563c2` on one board in one sitting, by `scripts/render_tables.py`. The previous artifact (`22908c15`) is paired against this one at the top of the page; the ISR and dual-core tables in the previous version of this page were carried over from an older capture and did not match the `22908c15` artifact's own ISR and dual-core blocks (for example its full-duty M7 ISR row read 83 / 25 cycles and 70.7% BUSY where the artifact held 55 / 15 and 69.7%). They are replaced here by the rendered values, and the comparison across the pairing uses the artifacts, not the prose.

| fixture (`embedded_memtable.rs` shape, via the C ABI) | 64 MHz off | 64 MHz on | 160 MHz off | 160 MHz on | 400 MHz off | 400 MHz on (ns) |
|---|---:|---:|---:|---:|---:|---:|
| ingest, 2,000 sequential inserts (per insert) | 1,108 | 732 | 1,456 | 788 | 1,456 | 789 (1,969 ns) |
| CAN dispatch, 500 gets (per get) | 369 | 265 | 477 | 272 | 477 | 272 (680 ns) |
| BLE evict 600 of 2,000, per-key `first`/`remove` loop (per evicted) | 3,564 | 2,456 | 4,515 | 2,581 | 4,513 | 2,580 (6,443 ns) |
| BLE evict 600 of 2,000, `remove_range` (per evicted) | 1,195 | 826 | 1,496 | 854 | 1,495 | 856 (2,136 ns) |
| BLE evict 25 of 2,000, per-key loop (per evicted) | 3,549 | 2,448 | 4,595 | 2,675 | 4,584 | 2,668 (6,661 ns) |
| BLE evict 25 of 2,000, `remove_range` (per evicted) | 1,288 | 900 | 1,639 | 977 | 1,641 | 977 (2,439 ns) |

Reading. The cache-on cycle counts are the same at 160 and 400 MHz (the working set fits the 16 KB D-cache, so nothing is bus-bound once cached), and the cache-off counts are the same at those two clocks too — both run the AXI SRAM at core/2, so the miss cost in cycles is identical; only 64 MHz (bus at core/1) is cheaper. The cache-on/cache-off ratio therefore reads 1.9× (ingest), 1.8× (CAN) and 1.7–1.8× (evictions) at the 2:1 ratio, versus 1.4–1.5× at 1:1 — that ratio is the measurement of the 32-byte-line node geometry in `docs/design/32-bit-embedded.md` §2.1.4, and it is the number a design that straddled lines would lose. The per-key loop costs 2.7–3.0× `remove_range` in both eviction shapes, consistent with the host result. At 400 MHz a point lookup is 680 ns and a sequential insert 2.0 µs.

**Expanse against a sorted array, an open-addressing hash table and newlib `tsearch`** (400 MHz, D-cache on; every implementation behind the same vtable and fixture code; the sorted array and the hash table pre-sized to capacity like the host suite's `HashMap::with_capacity`, Expanse and `tsearch` growing through newlib `malloc` as keys arrive):

| fixture | Expanse (C ABI) | sorted array, `bsearch` + `memmove` | open-addressing hash, FNV-1a, ≤ 50% load | newlib `tsearch` (unbalanced BST) |
|---|---:|---:|---:|---:|
| ingest, 2,000 sequential inserts | 789 / 1,969 ns | 185 / 462 ns (4.3× faster) | 71 / 177 ns (11.1× faster) | 27,793 / 69,395 ns (35.2× slower) |
| CAN dispatch, 500 gets | 272 / 680 ns | 100 / 249 ns (2.7× faster) | 55 / 138 ns (4.9× faster) | 5,817 / 14,525 ns (21.4× slower) |
| evict 600 of 2,000, batched (`remove_range`; scan for the hash) | 856 / 2,136 ns | 4,370 / 10,911 ns (5.1× slower) | 157 / 392 ns (5.4× faster) | 2,204 / 5,504 ns (2.6× slower) |
| evict 25 of 2,000, batched | 977 / 2,439 ns | 1,934 / 4,828 ns (2.0× slower) | 3,585 / 8,950 ns (3.7× slower) | 944 / 2,358 ns (1.0× faster) |
| evict 600 of 2,000, per-key loop | 2,580 / 6,443 ns | 15,384 / 38,411 ns (6.0× slower) | n/a | 2,227 / 5,562 ns (1.2× faster) |
| evict 25 of 2,000, per-key loop | 2,668 / 6,661 ns | 11,921 / 29,765 ns (4.5× slower) | n/a | 1,010 / 2,521 ns (2.6× faster) |

| bytes per key, 2,000 keys (newlib heap in use via `mallinfo`, allocator overhead included; requested bytes in parentheses) | sequential keys, one map | BLE index: hash keys, dual index (ordered) or one table (hash) |
|---|---:|---:|
| Expanse (C ABI) | 5.7 (3.9) | 21.3 (17.2) |
| sorted array, `bsearch` + `memmove` | 8.0 (8.0) | 16.0 (16.0) |
| open-addressing hash, FNV-1a, ≤ 50% load | 16.4 (16.4) | 16.4 (16.4) |
| newlib `tsearch` (unbalanced BST) | 36.0 (12.0) | 72.0 (24.0) |

Verdict, stated as plainly as the wins. The hash table is the right structure for point lookups and unordered ingest on this core, by 4.9× and 11×; the sorted array beats Expanse on lookup (2.7×) and on sequential-append ingest (4.3×) and is denser on random keys. Expanse's case on an MCU is the same as on the host: the ordered operations at steady state — the 25-of-2,000 eviction where the hash table must scan 4,096 slots to find 25 expired records (3.7× slower) and the sorted array must `memmove` the survivors (2.0× slower) — plus the densest footprint on dense keys (5.7 heap bytes/key, 3.9 requested) and the interrupt contract below, which none of the three twins offers. Two losses are new relative to the host suite: the sorted array's lookup win (the `bsearch` over 500 keys is 9 probes of contiguous memory) and the 11× ingest gap to the hash table, because Expanse's inserts allocate through newlib `malloc` while the pre-sized twins never allocate. `tsearch` degenerates on sequential keys (an unbalanced tree over ascending timestamps is a linked list: 27.8k cycles per insert, 5.8k per lookup) and its eviction wins are an artefact of the same degeneracy — the smallest key is the root, so `first` and its removal are O(1); it should not be read as an ordered-index result.

**The interrupt-handler contract, on real interrupts** (400 MHz, D-cache on): a SysTick ISR every 20,000 cycles calls `expanse_sync32_map_reader_try_get` while the main loop mutates the map, at full duty and paced to three mutation rates with jittered gaps (commensurate periods made the ISR alias into the pacing spin and report 0 BUSY — measured, and discarded). The twin is what bare-metal firmware actually does: `cpsid i`/`cpsie i` around a plain `expanse_map_t`, plain `expanse_map_get` in the ISR. 10,000 interrupts per cell.

| writer duty (mutations/s) | `sync32` single-attempt BUSY | `sync32` ISR entry latency max / mean (max in ns) | critical-section ISR entry latency max / mean (max in ns) | writer cycles per mutation `sync32` / critical section |
|---|---:|---:|---:|---:|
| full duty (~242k/s) | 54.2% | 56 / 15 (140 ns) | 1,639 / 735 (4,092 ns) | 1,656 / 1,462 |
| 40k/s | 13.0% | 18 / 15 (45 ns) | 1,743 / 148 (4,352 ns) | 10,017 / 10,014 |
| 10k/s | 3.2% | 16 / 15 (40 ns) | 1,693 / 52 (4,227 ns) | 40,188 / 40,050 |
| 1k/s | 0.4% | 15 / 15 (37 ns) | 1,694 / 19 (4,230 ns) | 396,227 / 401,085 |

Against the expected-loss matrix pre-registered in #598: the critical section is 12% cheaper per mutation at full duty and never returns BUSY; the optimistic surface holds the ISR entry-latency ceiling at 15–56 cycles (37–140 ns) at every duty, versus 1,639–1,743 cycles (4.1–4.4 µs) for the critical section — a 29× bound at full duty and 97–113× at the paced duties, and the number an interrupt budget is written against. The full-duty maximum is the one ISR figure that varies between captures of the same image (55 to 72 cycles across the four captures of this sitting); the paced-duty maxima and every mean do not. The price is the BUSY rate, which is the writer's bracket occupancy and falls linearly with duty. Zero corrupted values, zero reclaim refusals, zero arena-full over every cell.

**The other core: Cortex-M4, no cache, 200 MHz** (same fixtures, same four implementations, same ISR arms, run by the M4 into a shared-memory text buffer while the M7 relays its calibration to the host):

| fixture (Expanse, C ABI) | M7 400 MHz, cache on (ns) | M7 400 MHz, cache off | M4 200 MHz (ns) | M4 / M7 cache-on, cycles | M4 / M7 cache-on, time |
|---|---:|---:|---:|---:|---:|
| ingest, 2,000 sequential inserts (per insert) | 789 (1,969 ns) | 1,456 | 2,967 (14,817 ns) | 3.8× | 7.5× |
| CAN dispatch, 500 gets (per get) | 272 (680 ns) | 477 | 1,211 (6,045 ns) | 4.4× | 8.9× |
| BLE evict 600 of 2,000, `remove_range` (per evicted) | 856 (2,136 ns) | 1,495 | 3,234 (16,151 ns) | 3.8× | 7.6× |
| BLE evict 25 of 2,000, `remove_range` (per evicted) | 977 (2,439 ns) | 1,641 | 3,260 (16,281 ns) | 3.3× | 6.7× |

The alternatives pay more for the move than Expanse does (4.5–7.2× in cycles for the sorted array and the hash table against Expanse's 3.3–4.4×), so the relative picture on the M4 is the M7's with the gaps narrowed: the hash table's ingest lead is 7.3× (406 vs 2,967) and its lookup lead 4.2× (287 vs 1,211); the sorted array's lookup lead is 2.1× (567), its ingest lead 3.1× (971), and its batched bulk eviction 9.7× slower (31,252); `tsearch` is 1.3× slower than Expanse on the steady eviction (4,133 vs 3,260), the row where it sat at parity before the eviction path shortened, for the degenerate-tree reason above. The like-for-like cacheless comparison is the M7's cache-off column: the M4 needs 2.0–2.5× the cycles of the M7 with its D-cache off, which is the simpler in-order pipeline, not memory (its heap is D2 SRAM at core speed).

| M4 ISR arm, writer duty | `sync32` BUSY | `sync32` ISR entry latency max / mean (max in ns) | critical-section max / mean (max in ns) | writer cycles per mutation `sync32` / critical section |
|---|---:|---:|---:|---:|
| full duty (~33k/s) | 82.6% | 97 / 74 (484 ns) | 5,237 / 2,607 (26,152 ns) | 6,078 / 5,642 |
| 40k/s (not reachable: also full duty, jittered) | 56.3% | 98 / 74 (489 ns) | 5,238 / 2,547 (26,157 ns) | 6,274 / 5,763 |
| 10k/s | 22.3% | 98 / 72 (489 ns) | 5,238 / 723 (26,157 ns) | 20,071 / 20,008 |
| 1k/s | 2.2% | 89 / 71 (444 ns) | 5,200 / 149 (25,968 ns) | 197,540 / 200,387 |

The contract holds on the smaller core with a wider margin: an 89–98-cycle ceiling (444–489 ns) against 5,200–5,238 cycles (26.0–26.2 µs) for masking interrupts around a 5,600–6,300-cycle mutation — a 53–58× bound; and the same BUSY-vs-duty curve.

**Two cores on one map** — the M7 mutates a `sync32` map in its own AXI SRAM heap while the M4 reads it across the D2/D1 bridge, per writer duty; three series: the M7 heap **non-cacheable** (MPU), the same with a **hardware-semaphore twin** (HSEM 0 around every mutation and every read, which is what firmware does across cores), and the heap **cacheable** — the configuration the `sync32` header marks unsupported, measured rather than assumed. The first version of this cell put the map in an SRAM4 arena and read zero hits: the `sync32` arena only holds node *handles*, every node body is a heap `Box`, so the M4 was reading the M7's cached heap; that cell was discarded and the design corrected.

| M7 heap | reads | writer duty | M4 reads | OK / not found | BUSY | corrupted (M4 / writer) | M4 read cycles mean / max (max in ns) | lock wait mean / max | writer cycles per mutation |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| non-cacheable | `sync32` single-attempt | full duty | 185,664 | 16 / 16 | 100.0% | 0 / 0 | 398 / 2,250 (11,236 ns) | — | 2,748 |
| non-cacheable | `sync32` single-attempt | 40k/s | 80,448 | 13,823 / 17,767 | 60.7% | 0 / 0 | 1,093 / 2,275 (11,361 ns) | — | 10,018 |
| non-cacheable | `sync32` single-attempt | 10k/s | 56,896 | 21,816 / 22,731 | 21.7% | 0 / 0 | 1,598 / 2,274 (11,356 ns) | — | 39,704 |
| non-cacheable | `sync32` single-attempt | 1k/s | 49,728 | 24,319 / 24,132 | 2.6% | 0 / 0 | 1,850 / 2,268 (11,326 ns) | — | 387,585 |
| non-cacheable | HSEM-locked twin | full duty | 9,088 | 4,557 / 4,531 | 0.0% | 0 / 0 | 10,846 / 57,666 (287,970 ns) | 8,911 / 55,415 | 3,121 |
| non-cacheable | HSEM-locked twin | 40k/s | 36,096 | 18,085 / 18,011 | 0.0% | 0 / 0 | 2,605 / 4,800 (23,970 ns) | 668 / 2,495 | 10,021 |
| non-cacheable | HSEM-locked twin | 10k/s | 43,584 | 21,935 / 21,649 | 0.0% | 0 / 0 | 2,129 / 3,774 (18,846 ns) | 190 / 1,469 | 39,828 |
| non-cacheable | HSEM-locked twin | 1k/s | 45,952 | 22,951 / 23,001 | 0.0% | 0 / 0 | 2,013 / 3,720 (18,577 ns) | 77 / 1,415 | 394,836 |
| cacheable | `sync32` single-attempt | full duty | 95,552 | 0 / 0 | 100.0% | 0 / 0 | 906 / 908 (4,534 ns) | — | 1,598 |
| cacheable | `sync32` single-attempt | 40k/s | 75,712 | 0 / 0 | 100.0% | 0 / 0 | 1,180 / 1,180 (5,893 ns) | — | 10,017 |
| cacheable | `sync32` single-attempt | 10k/s | 75,712 | 0 / 0 | 100.0% | 0 / 0 | 1,180 / 1,217 (6,077 ns) | — | 40,089 |
| cacheable | `sync32` single-attempt | 1k/s | 75,776 | 0 / 0 | 100.0% | 0 / 0 | 1,180 / 1,183 (5,908 ns) | — | 401,757 |

Evaluation, not a claim (this is #598 step 3, and the header keeps saying unsupported). With the M7 heap non-cacheable the protocol is correct across cores — every successful read returned the right value, found and not-found split evenly as the key set dictates, no reclamation refusal — but a single-attempt read from the M4 costs 1,850 cycles (9.3 µs) at low duty against 272 for the M7 reading its own heap and 1,211 for the M4 reading its own D2 SRAM: that is the D2→D1 bridge, and the 7× longer read window is why BUSY at a given duty is ~5× the same-core ISR arm's (61% vs 13% at 40k mutations/s), reaching 100% at full duty. The optimistic reader's worst case is a BUSY answer in 2.3k cycles; the semaphore twin's worst case is a 55.4k-cycle (277 µs) wait at full duty — the writer's mutation got 18% cheaper across the pairing, and the lock-holding M4 reader is starved longer for it (its full-duty read count fell from 17k to 9k) — and 1.5k (7.3 µs) at ≤ 10k/s, with reads costing 2.0–2.6k cycles on average at the paced duties and 10.8k flat-out, including the lock. With the heap cacheable the M4 never sees a consistent version and every read is BUSY, at every duty — the unsupported configuration fails safe rather than silently wrong, and it says nothing about correctness under other cache states. Lifting the restriction would need either a non-cacheable heap for the map (what this cell does, at the M7's cost of running that heap uncached) or explicit clean/invalidate around the version bracket; neither is in the library today.

Not covered: VOS0 / 480 MHz (board wiring), a balanced-tree twin (the toolchain ships none), M4 as writer, external-reviewer replication, a DWT `CPICNT`/`LSUCNT` decomposition of the M7 `can_dispatch` movement recorded above.
