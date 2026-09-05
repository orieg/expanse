# STM32H747I-DISCO LCD (NT35510) Setup & Timing Notes

## 1. Purpose

This document captures how the STM32H747I-DISCO board is used as a display coprocessor for SMARS, and the exact LCD timing setup that fixed the horizontal shift / vertical band issue on the MB1166 (NT35510) 800×480 DSI panel.

It is based on ST's `LCD_DSI_VideoMode_DoubleBuffering` example, adapted into the `STM32/smars_disco_hello` project.

---

## 2. Project Layout

Root of the STM32 display project:

- `STM32/smars_disco_hello/`
  - `CM7/` – application for Cortex‑M7 core
    - `Core/Src/main.c` – render loop, double buffering, drawing
    - `Core/Src/stm32h7xx_it.c` – interrupt handlers (incl. LTDC IRQ)
    - `Core/Src/stm32h7xx_hal_msp.c` – LTDC/DSI/DMA2D MSP + NVIC config
  - `Drivers/BSP/STM32H747I-DISCO/stm32h747i_discovery_lcd.c` – LCD BSP
  - `Drivers/BSP/Components/nt35510/` – NT35510 panel driver + timings
  - `Makefile/CM7/` – make-based build for the CM7 image

Build command (from repo root):

```bash
cd STM32/smars_disco_hello/Makefile/CM7
make -j8
```

The resulting binary is `build/smars_disco_hello_CM7.bin`.

---

## 3. Display Pipeline Overview

High-level pipeline for the MB1166/NT35510 panel:

- **Framebuffers in SDRAM**
  - Two full‑screen ARGB8888 buffers allocated in external SDRAM.
  - Addresses tracked in `Buffers[2]` in `main.c`.
- **LTDC**
  - Scans out one buffer ("front").
  - Configured for 800×480 landscape using NT35510 timing macros.
- **DSI Host**
  - Video‑mode, RGB888, timing derived from NT35510 portrait constants.
- **DMA2D (Chrom‑ART)**
  - Used to clear the back buffer and blit the SMARS logo.
- **Double buffering with line events**
  - `HAL_LTDC_ProgramLineEvent(&hlcd_ltdc, 0)` primes a line‑0 event.
  - `HAL_LTDC_LineEventCallback` swaps to the pending buffer at line 0
    using `HAL_LTDC_SetAddress_NoReload` + vertical‑blank reload.

The main loop renders into the non‑visible buffer, then sets `pend_buffer` to request a swap on the next frame boundary.

---

## 4. NT35510 Timing Configuration (Final Working Values)

The NT35510 timing macros live in:

- `STM32/smars_disco_hello/Drivers/BSP/Components/nt35510/nt35510.h`

Current **portrait** timing (used directly in the tear‑free 480×800 PORTRAIT
configuration of `smars_disco_hello`) is:

```c
#define NT35510_480X800_HSYNC ((uint16_t)2)
#define NT35510_480X800_HBP   ((uint16_t)34)
#define NT35510_480X800_HFP   ((uint16_t)34)

#define NT35510_480X800_VSYNC ((uint16_t)120)
#define NT35510_480X800_VBP   ((uint16_t)150)
#define NT35510_480X800_VFP   ((uint16_t)150)
```

Landscape values for 800×480 are derived from these using the macros later in
the same file. Earlier in the project, when we were running the panel as
**800×480 LANDSCAPE**, we also successfully used `HBP = HFP = 68` to fix a
horizontal banding/shift issue (see next section). After switching to a
true 480×800 portrait configuration to remove tearing, the final tuned
values are `HBP = HFP = 34`.

These macros drive **both**:

1. **DSI video timing** in `MX_DSIHOST_DSI_Init`:
   - `VidCfg.HorizontalSyncActive`, `HorizontalBackPorch`, `HorizontalLine`
   - `VidCfg.VerticalSyncActive`, `VerticalBackPorch`, `VerticalFrontPorch`
2. **LTDC timing** in `MX_LTDC_Init`:
   - `hltdc->Init.HorizontalSync`, `AccumulatedHBP`, `TotalWidth`, etc.

The LTDC clock is configured in `MX_LTDC_ClockConfig` to match ST's example:

```c
PLL3M = 5;
PLL3N = 132;   // 660 MHz VCO
PLL3R = 24;    // 660 / 24 ≈ 27.5 MHz LTDC clock
```

This matches the DSI calculations that use an effective pixel clock of ~27.4 MHz.

---

## 5. Horizontal Shift / Vertical Band Issue & Fix (original 800×480 landscape)

**Symptom (before fix):**

- Visible bright vertical band near the right edge.
- Left border slightly offset; overall image appeared shifted to the right.
- A debug pattern that colored framebuffer columns 0–2 (R,G,B) and 797–799 (Y,C,M) showed **both bands on the right side**.

This indicated that the **wrap between logical column 799 and 0 occurred inside the visible area**, meaning the active 800‑pixel window was starting too late in each line.

**Root cause:**

- The NT35510 horizontal porches were too small / mismatched for this exact panel + timing combination, so LTDC/DSI were not aligned with the panel's internal active window.

**Fix (for the original 800×480 LANDSCAPE setup):**

- Set both horizontal back porch and front porch to **68** pixels:
  - `NT35510_480X800_HBP = 68`
  - `NT35510_480X800_HFP = 68`
- Rebuilt and reflashed the CM7 firmware.

**Result (landscape):**

- The white 1‑px border appeared exactly on all four physical edges.
- The debug bands (when enabled) showed:
  - R,G,B on the **leftmost** 3 physical columns.
  - Y,C,M on the **rightmost** 3 physical columns.
- The bright vertical band / seam disappeared; no visible wrap or 1‑line offset.

---

## 6. Tearing Behaviour & Portrait vs Command Mode

The panel was originally driven as an **800×480 LANDSCAPE** DSI video‑mode
display (matching ST's `LCD_DSI_VideoMode_DoubleBuffering` example). Under this
configuration we observed a persistent **tearing / split artefact** when doing
full‑screen updates (e.g. switching the solid background color every frame):

- The transition between old and new frames appeared as a **diagonal split**
  across the screen, very similar to the screenshots reported in ST's
  community thread *"STM32H747I-DISCO tearing in double buffering"*.
- Reproducing ST's example project one‑to‑one on the same board produced the
  **same artefact**, suggesting this is a limitation/interaction of the DSI
  video‑mode pipeline rather than a simple bug in our application code.

ST's recommendation in that thread was essentially:

1. Drive the panel in **portrait** (480×800) rather than landscape, *or*
2. Switch to **DSI command‑mode partial refresh**, based on the
   `LCD_DSI_CmdMode_PartialRefresh` example, updating only parts of the frame
   in sync with the panel's TE/vsync signal.

We validated these options in three small steps:

1. **Orientation flag only (still 800×480):**
   - Changed only the BSP orientation to `LCD_ORIENTATION_PORTRAIT` while
     keeping the LTDC/DSI geometry at 800×480 via `BSP_LCD_Init()`.
   - Result: the diagonal split turned into a **vertical tear at ~1/3 of the
     width** of the panel, confirming that the orientation flag alone was not
     enough (the timing/geometry still matched 800×480 landscape).
2. **True 480×800 PORTRAIT geometry:**
   - Switched to `BSP_LCD_InitEx(0, LCD_ORIENTATION_PORTRAIT,
     LCD_PIXEL_FORMAT_RGB888, 480, 800)` so that **both** LTDC and DSI are
     configured for a 480×800 active area.
   - Result: the tearing/split **disappeared** during full‑screen color swaps,
     matching the outcome reported by ST and other users.
   - A 1‑pixel wrap was briefly visible on one edge; this was fixed by tuning
     the NT35510 horizontal porches to **HBP = HFP = 34**, which perfectly
     centers the active window.
3. **Command‑mode partial refresh (future option):**
   - We have **not** yet implemented command‑mode for this project, but the
     ST guidance and example (`LCD_DSI_CmdMode_PartialRefresh`) remain the
     reference if we later need fine‑grained, tear‑free region updates.

In summary, for the SMARS display coprocessor the **recommended baseline** is:

- DSI **video mode**,
- Panel driven as **480×800 PORTRAIT** via `BSP_LCD_InitEx(..., 480, 800)`,
- NT35510 porches set to **HBP = HFP = 34**.

This combination has been verified to be:

- Tear‑free for aggressive full‑screen updates, and
- Pixel‑perfectly aligned with visible panel edges (no 1‑pixel wraps).

On top of this baseline, the SMARS debug/demo firmware keeps the panel and
timings in 480×800 portrait video mode but exposes a **logical 800×480
landscape UI** by writing through a simple 90° clockwise **software rotation**
helper in `main.c` (logical coordinates are rotated into the physical portrait
framebuffer before each pixel write). This gives a landscape‑oriented
experience on the robot while preserving the proven tear‑free portrait
configuration underneath.

Command‑mode partial refresh remains an optional, more complex path if we need
to further optimize bandwidth or update only parts of the screen.

---

## 7. Debug Procedure (If Issues Reappear)

If a future timing change reintroduces a shift:

1. In `DrawFrameToBuffer` (in `main.c`), temporarily add color bands:
   - Columns x=0..2: solid R,G,B.
   - Columns x=width‑3..width‑1: solid Y,C,M.
2. Observe where those bands land on the physical panel.
   - If both bands are on the same side, the active window is wrapped.
3. Adjust `NT35510_480X800_HBP` and `HFP` **symmetrically** until:
   - R,G,B sit on the extreme left edge.
   - Y,C,M sit on the extreme right edge.
4. Once aligned, remove the debug drawing again.

---

## 8. Summary

- The STM32H747I‑DISCO LCD setup is closely aligned with ST's DSI video‑mode double‑buffering example.
- The **critical customisation** is the NT35510 horizontal porches; we have two
  relevant, empirically‑validated configurations:
  - **Original 800×480 LANDSCAPE:** `HBP = HFP = 68` fixed the vertical band /
    wrap and centered the image.
  - **Current 480×800 PORTRAIT (tear‑free):** `HBP = HFP = 34` with
    `BSP_LCD_InitEx(0, LCD_ORIENTATION_PORTRAIT, LCD_PIXEL_FORMAT_RGB888,
    480, 800)` gives a perfectly centered image *and* removes the tearing we
    saw in landscape video mode.


