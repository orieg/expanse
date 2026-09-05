# The "two lanes" LCD demo (STM32H747I-DISCO, Cortex-M7)

**Demo, not benchmark.** This firmware shows, on the board's 800×480 panel and
at human timescale, the property the on-target suite measured: an interrupt
that reads an Expanse `sync32` map is never blocked by the writer, while a
hash table protected by masking its reader interrupt holds that interrupt
off for the length of the mask. Every number on the screen is measured in
the run that displays it; the same counters stream over the VCP as `RESULT`
lines once a second. The measured claims live in
[`docs/benchmarks/stm32h747/`](../../../docs/benchmarks/stm32h747/README.md);
the recorded run of this demo and its frame grabs live in
[`docs/benchmarks/stm32h747/demo/`](../../../docs/benchmarks/stm32h747/demo/).
Tracking issue: #605, whose comment thread carries the pre-registration and
its amendment.

## What the screen shows

Two lanes, one core, one workload: a tracker table of 100,000 records with a
60 s time-to-live; the main loop ingests what expires and sweeps expired
records at the step's rate; each lane has its own 1 kHz timer interrupt that
reads its table for four tracked ids and records the outcome. Left lane:
Expanse (by-id index a `sync32` map read single-attempt in the interrupt;
by-time index a plain map swept with `remove_range`). Right lane: an
open-addressing hash table (`../alts.c`, the suite's twin) whose reader
interrupt is masked according to the step.

Per lane, top to bottom:

- the lane name and one line saying how its reader is protected;
- the **heartbeat strip**: the last 4 s in 376 columns of 10.6 ms, swept left
  to right. A column is the lane colour if the interrupt ran and read a value
  in every millisecond of it, red if any millisecond was blocked (a blocked
  millisecond takes precedence, so a 3 ms gap is never averaged away), cyan if
  a `sync32` read was told BUSY, magenta if a read returned a wrong value; the
  legend under the band lists only the outcomes the step can produce;
- four numbers in plain words, the same on both sides: **BLOCKED ms** (how
  long the interrupt could not run this step), **NO VALUE %** (the share of
  milliseconds the reader came away with nothing — blocked on the hash side,
  told BUSY on the Expanse side), **STALE ms** (the longest stretch without a
  fresh value: the worst age of the data the consumer sees), **WRONG** (reads
  that returned a value not matching their id, or not-found for an id that is
  always present);
- the sweep line (milliseconds per sweep and share of the core at the step's
  rate) and a ledger: bytes per record and nanoseconds per lookup, measured
  on the demo's own objects through the same read path the interrupt uses
  (`sync32` reader slot 1 for Expanse), the lower value marked on either side.

The program advances on its own; the blue user button skips a step:

| step | sweeps/s | hash reader | what to watch |
|---|---:|---|---|
| prologue, 6 s | 0 | — | both bands solid: this is normal |
| 1, 20 s | 1 | masked for the whole scan | one red gap per second on the right, ~47 ms wide |
| 2, 20 s | 5 | masked for the whole scan | five gaps per second |
| 3, 20 s | 10 | masked for the whole scan | the right band is striped: blocked ~40% of the time |
| 4, 20 s | 10 | masked around each write only | the competent firmware: no gaps on either side |
| 5, 20 s | 10 | not masked | no gaps; WRONG stays 0 (see below) |
| 6, 20 s | 10 | masked per write, tables filled from empty at 5,000/s | each doubling of the hash table is one write; its cost is timed by DWT and listed |
| summary | — | — | the run's own per-step numbers, held until reset |

## Instruments

Entry latency from a free-running 20 MHz reference (TIM2): each tick's
lateness is `TIM2 − expected`; whole missed ticks are `lateness / 1 ms` and
are written into the time ring as held slots by the tick that finally ran.
The reference is re-armed at every step change from the lane timer's own
remaining count, inside a microsecond-long critical section, so a step
change is never counted as lateness. ISR body cycles by DWT. The hash lane's
mask window by DWT between `NVIC_DisableIRQ` and `NVIC_EnableIRQ`. Sweep cost
by DWT, gross and net of interrupt time. Rehash cost by DWT inside the
masked insert. The two lane timers share one NVIC priority, so neither
lane's ISR body nests inside the other's wait. **The interrupts draw
nothing**: they write one byte per millisecond into a ring; the main loop
draws the strip from the ring, so nothing one lane draws can land in the
other lane's entry wait. Text is composited from 2-bit anti-aliased glyph
atlases (`tools/gen_fonts.py`, DejaVu faces) over a known background and
never reads the framebuffer; every text run has a width budget and an
overflow is reported as a `CHECK` line on the VCP, so a collision cannot
ship unseen. The committed run has no `CHECK` line.

## Deviations from the pre-registration, stated

- **No motion field.** The amendment to #605 asked for interrupt-owned
  motion so that "interrupt did not run" and "dot did not move" are the same
  event. The strip keeps that identity (a slot is filled only when the
  interrupt ran) without a second visual language; the review of the mockup
  found the moving dots unclear and they were dropped.
- **Re-registration is an update in place.** The amendment re-registered
  tracked devices by remove-then-insert, which leaves a gap an unmasked
  reader can fall into. The key does not change, so the correct operation is
  one insert that replaces the value; with that, the amendment's expected
  "phantom not-found reads > 0" for the unmasked hash reader did **not**
  occur: WRONG stayed 0 in step 5. With word-sized slot updates and linear
  probing, an unmasked hash reader is correct at steady state on this core;
  the write that does hazard it is the rehash, which step 6 shows under the
  per-write mask (44 ms for the 65,537-entry doubling).
- **BUSY costs staleness, not latency.** The Expanse reader is told BUSY in
  about 1% of milliseconds, mostly in bursts during its own `remove_range`
  sweep when the writer holds the bracket for consecutive removals; the
  longest such run was 11 ms at 1 sweep/s. The mockup had assumed 2 ms.
- **The rehash projection was optimistic.** 0.35 µs per moved entry was
  projected from the scan cost; measured, the largest doubling took
  0.67 µs per entry (44 ms for 65,537).
- **Memory.** At this population and geometry the hash table (262,144 slots
  × 8 B) is 21–24 B/record and Expanse 18–20 including its by-time index;
  the suite's 16.4 B/record figure is the hash at 128k records. The panel
  prints what the run measures.

## Files

- `CM7/Core/Src/main.c` — screen, program, telemetry, board configuration;
  `lanes.{h,c}` — the two lanes, their tables, the interrupt, the
  instruments; `gfx.{h,c}` — framebuffer drawing and text compositing;
  `CM7/Core/Inc/fonts.h` — generated glyph atlases.
- `Makefile/CM7/` — `make` builds `build/expanse_demo_CM7.elf` against
  `target/thumbv7em-none-eabihf/release/libexpanse.a`; `-O2`, the same as
  the suite's twins.
- `tools/gen_fonts.py` — regenerates `fonts.h`; `tools/grab_frame.sh` —
  reads the live framebuffer over SWD (no halt: a halt costs both lanes
  seconds of honest blocked time) into a PNG via `tools/fb_to_png.py`;
  `tools/harvest_demo.py` — per-step table and JSON from the transcript.
- `run_demo.sh OUTDIR` — build, flash, capture the whole program, grab one
  frame per step, harvest. The M4 image in flash bank 2 (the measurement
  harness's) idles.
- `Drivers/`, `Common/`, `docs/NT35510_LCD_SETUP.md` — the vendored ST
  BSP/HAL and the display bring-up notes (BSD-3), separate from the Cube-free
  measurement harness one directory up.

Done when: the recorded run and frames are committed under
`docs/benchmarks/stm32h747/demo/` and linked from the suite README (this
change). Superseded when the demo moves to another board or the suite's
harness absorbs the display path.
