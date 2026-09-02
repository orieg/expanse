# STM32H747I-DISCO harness (Cortex-M7, bare metal)

The first on-target execution lane for the ARM Cortex-M tier: a Cube-free
firmware that links `libexpanse.a` (narrow surface, `thumbv7em-none-eabihf`)
and runs the `benches/embedded_memtable.rs` fixtures plus the `sync32`
interrupt-handler contract on the M7 of an STM32H747I-DISCO, reporting DWT
cycle counts over the ST-LINK V3 virtual COM port. Tracking issue: #598.
Measured results and their reading live in `docs/BENCHMARKING.md`
("Cortex-M7 on-target"); the committed artifacts are
`docs/benchmarks/stm32h747/{transcript.txt,results.json}` and the derived
chart `docs/assets/bench_stm32h747.svg` (`scripts/generate_stm32_svg.py`).

## What runs

| stage | what | output line |
|---|---|---|
| calibration | 320M cycles of spin between `TICK`/`TOCK`; `capture.py` times it on the host, which pins the core clock independently of the firmware's belief | `CALIB` |
| fixtures | ingest (2,000 sequential inserts), CAN dispatch (500 gets), BLE TTL eviction in the bulk (600/2,000) and steady (25/2,000) shapes, each via the per-key `first`/`remove` loop and via `expanse_map_remove_range`; 5 passes; at 64 MHz HSI and 160 MHz PLL1, D-cache off and on | `RESULT name=… sysclk=… dcache=… pass=… cycles=… ops=…` |
| ISR arm | SysTick every 20,000 cycles calls `expanse_sync32_map_reader_try_get` while the main loop mutates a `sync32` map at full duty and at three paced rates (jittered gaps, so the writer cannot alias with the timer); counts OK / NOT_FOUND / BUSY, value-corruption checks, reclaim refusals, ISR entry latency (from the SysTick counter at entry) and ISR body cycles | `RESULT name=isr_sync32 …` |
| twin | the same, with `cpsid i`/`cpsie i` around a plain `expanse_map_t` and a plain `expanse_map_get` in the ISR | `RESULT name=isr_critical_section …` |

Every `CHECK` line is a failed in-firmware assertion; a clean run prints none
and ends with `DONE`. Core clock is 64 MHz HSI after reset and 160 MHz from
PLL1 (HCLK 80 MHz) afterwards, chosen because it stays inside the reset VOS3
envelope on every silicon revision, so no PWR supply reconfiguration is needed.
All numbers are core cycles per operation, not nanoseconds.

## Files

- `main.c` — harness; `startup_m7.c`, `m7.ld` — M7 startup and memory map
  (flash bank 1, DTCM data/stack, 512 KB AXI SRAM heap via `_sbrk`).
- `m4_idle.c`, `m4.ld` — a 72-byte image for flash bank 2 that parks the
  Cortex-M4 in WFI so the factory demo does not run alongside the harness.
- `build.sh` — builds both images against
  `target/thumbv7em-none-eabihf/release/libexpanse.a`; CI runs it as the
  hard-float link assertion. `run.sh` — flashes both banks over the on-board
  ST-LINK (`STM32_Programmer_CLI`), resets, captures the VCP with
  `capture.py`, and summarises with `harvest.py` (table + JSON).

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
