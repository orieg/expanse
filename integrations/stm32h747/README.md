# STM32H747I-DISCO harness (Cortex-M7 + Cortex-M4, bare metal)

The on-target execution lane for the ARM Cortex-M tier: a Cube-free
firmware for both cores of an STM32H747I-DISCO that links `libexpanse.a`
(narrow surface, `thumbv7em-none-eabihf`) and runs the
`benches/embedded_memtable.rs` fixtures — for Expanse and three alternatives
— plus the `sync32` interrupt-handler contract on each core and the
cross-core reader cells, reporting DWT cycle counts over the ST-LINK V3
virtual COM port. Tracking issue: #598. Results, charts and their reading live in
`docs/benchmarks/stm32h747/` (the suite directory, same shape as the other
`docs/benchmarks/` suites), whose README carries the full tables, rendered
from `results.json` by `scripts/render_tables.py`.

## Results

The measured results, the three charts and how to read each live in the suite directory, [`docs/benchmarks/stm32h747/`](../../docs/benchmarks/stm32h747/README.md) (results README with the full tables, `METHODOLOGY.md` pre-registration record, `results.json`, `transcript.txt`, `run.sh`).

**In one paragraph.** Expanse runs correctly on both cores; the cache-line node layout measurably pays off on the M7; and its interrupt-safe reads bound interrupt latency in a way that masking interrupts cannot. Against the structures firmware usually reaches for it loses raw point lookups and unordered inserts, and wins steady-state ordered expiry, memory density on dense keys, and the interrupt contract. Across two cores it is correct only with an uncached shared heap, and even then the second core's slow reads limit it to modest write rates.

## What runs

| stage | what | output line |
|---|---|---|
| calibration | 320M cycles of spin between `TICK`/`TOCK` at each clock; `capture.py` times it on the host, which pins the core clock independently of the firmware's belief and lets `harvest.py` derive nanoseconds | `CALIB` |
| fixtures | ingest (2,000 sequential inserts), CAN dispatch (500 gets), BLE TTL eviction in the bulk (600/2,000) and steady (25/2,000) shapes, each via the per-key `first`/`remove` loop and via `remove_range` (or a full scan for the hash table); 5 passes; at 64 MHz HSI, 160 MHz PLL1 (VOS3) and 400 MHz PLL1 (VOS1), D-cache off and on; for **four implementations** behind one vtable (`alts.c`): Expanse's C ABI, a sorted array with `bsearch`+`memmove`, an open-addressing hash table (same FNV-1a, ≤ 50% load), and newlib's `tsearch` | `RESULT name=… impl=… sysclk=… dcache=… pass=… cycles=… ops=…` |
| bytes/key | newlib heap in use (`mallinfo`) and requested bytes around building each structure with 2,000 keys, sequential-key map and the BLE dual index | `RESULT name=bytes impl=… shape=…` |
| DWT profile | the `ingest` and `can_dispatch` loops again, with the six DWT counters (`CYCCNT`, `CPICNT`, `EXCCNT`, `SLEEPCNT`, `LSUCNT`, `FOLDCNT`) read around **each** operation and accumulated in 32 bits, for all four implementations, plus an empty-bracket `nop` row the harvest subtracts; `cycles = instructions + CPI + EXC + SLEEP + LSU − FOLD` (ARMv7-M ARM C1.8) gives the instruction count, so a cycle movement splits into instructions, fetch/multi-cycle stalls (`CPICNT`) and data stalls (`LSUCNT`). The five event counters are 8-bit: `cpi_max`/`lsu_max` report the largest per-op delta and `suspect` the ops that violate the identity, and the harvest flags a row `wrap_risk` when either reached 255. Skipped when `DWT_CTRL.NOPRFCNT` says the core has no profiling counters (`INFO … dwt_prfcnt=0`) | `RESULT name=dwt fixture=… impl=… cycles=… cpi=… exc=… sleep=… lsu=… fold=… cpi_max=… lsu_max=… suspect=…` |
| ISR arm | SysTick every 20,000 cycles calls `expanse_sync32_map_reader_try_get` while the main loop mutates a `sync32` map at full duty and at three paced rates (jittered gaps, so the writer cannot alias with the timer); counts OK / NOT_FOUND / BUSY, value-corruption checks, reclaim refusals, ISR entry latency (from the SysTick counter at entry) and ISR body cycles | `RESULT name=isr_sync32 …` |
| twin | the same, with `cpsid i`/`cpsie i` around a plain `expanse_map_t` and a plain `expanse_map_get` in the ISR | `RESULT name=isr_critical_section …` |

Every `CHECK` line is a failed in-firmware assertion; a clean run prints none
and ends with `DONE`. Core clock is 64 MHz HSI after reset, then 160 MHz from
PLL1 inside the reset VOS3 envelope, then 400 MHz at VOS1 after selecting the
direct-SMPS supply the DISCO is wired for (`PWR_CR3` already reads SDEN=1,
LDOEN=0 on this board). VOS0 / 480 MHz needs the LDO path and a board
modification, so it is not attempted. All numbers are core cycles per
operation; `harvest.py` converts to ns with the host-measured clocks.

The alternatives are deliberately plain: the sorted array and the hash table
are pre-sized to capacity (like the host suite's `HashMap::with_capacity`),
Expanse and `tsearch` grow through `malloc`; `tsearch` degenerates on
sequential keys (unbalanced), which the docs section says out loud.

## Both cores

`main.c` is built twice: `-DCORE_M7` for flash bank 1 and `-DCORE_M4` for
bank 2 (`startup.c` serves both; the M4 variant enables its D2 SRAM and the
D3 SRAM4 clocks with registers only, before its stack exists). After the M7
finishes its own cells at 400 MHz it hands the M4 a turn through the SRAM4
mailbox (`dual.h`): the M4 — 200 MHz HCLK, no cache — runs the same
calibration (relayed by the M7 as `TICK`/`TOCK` so the host times it too),
the same fixtures for all four implementations and the same ISR arms, into a
48 KB text buffer the M7 dumps to the VCP with `core=m4` on every line.

Then the dual-core cells. The M7 allocates a `sync32` map in its own heap
(AXI SRAM — every node body is a heap `Box`, the arena only holds handles,
which is why an SRAM4 bump arena was the wrong shared region the first
time), prefills it, and mutates it at each writer duty while the M4 serves
single-attempt `try_get`s from its reader handle across the D2/D1 bridge,
counting OK / not-found / BUSY / corrupted values, read cycles and lock
waits. Three series: the M7 heap **non-cacheable** (MPU region 2; region 0
keeps all of SRAM4 non-cacheable so the mailbox never sits in the D-cache);
the same with a **hardware-semaphore twin** (HSEM 0 around every mutation
and every read — what firmware does across cores); and the heap
**cacheable** — the configuration the `sync32` header marks unsupported —
measured rather than assumed. `QUICK=1 sh build.sh` skips the fixture
passes so the dual cells can be iterated in under a minute.

## Files

- `main.c` — harness for both cores; `startup.c`; `m7.ld` (flash bank 1,
  DTCM data/stack, 512 KB AXI SRAM heap via `_sbrk`), `m4.ld` (flash bank 2,
  SRAM3 data/stack, SRAM1+2 heap); `dual.h` — mailbox layout and protocol.
- `alts.{h,c}` — the alternatives and the allocation accounting.
- `build.sh` — builds both images against
  `target/thumbv7em-none-eabihf/release/libexpanse.a` and asserts the
  hard-float ABI tag on each; CI runs it. `run.sh` — flashes both banks over
  the on-board ST-LINK (`STM32_Programmer_CLI`), resets, captures the VCP
  with `capture.py`, and summarises with `harvest.py` (table + JSON).

## Running it

```bash
cargo build --release -p expanse-capi --no-default-features \
  --features embedded-panic-handler --target thumbv7em-none-eabihf
sh integrations/stm32h747/build.sh
sh integrations/stm32h747/run.sh docs/benchmarks/stm32h747/transcript.txt
```

Needs the Arm GNU toolchain (`arm-none-eabi-gcc`) and STM32CubeProgrammer.
Board notes that cost a detour the first time: the debugger is the micro-USB
**CN2 (STLK)** on the edge next to the RCA jack, not the micro-USB by the
audio jacks (CN14, 5 V power input only) and not the USB-C (application USB).
When the link is up macOS mounts `DIS_H747XI` and `st-info --probe` finds the
programmer. `run.sh` writes **both** flash banks, replacing whatever was there.
On macOS, `stty` settings are dropped when the port closes, which reads as a
silent board; `capture.py` holds the port open through termios instead.

Superseded when a probe-rs / QEMU lane replaces the CubeProgrammer flow, or
when the harness grows a second board and moves under a shared directory.
