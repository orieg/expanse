#!/bin/sh
# Grab the demo's framebuffer over SWD as a PNG, without a camera.
#
#   grab_frame.sh OUT.png
#
# The framebuffer is one ARGB8888 800x480 image at 0xD0000000 (SDRAM, MPU
# non-cacheable, so what the LTDC scans is what memory holds). CubeProgrammer
# connects in HOTPLUG mode (no reset, no halt), uploads the 1,536,000 bytes while
# the firmware keeps running, and fb_to_png.py converts. Halting the core for the
# read was tried and rejected: the debugger connection stalls the core for about
# four seconds, which both lanes' interrupt instruments honestly record as
# blocked time and which would then sit in the step's counters. Reading live
# instead costs nothing on the counters; the price is that anything moving
# during the ~200 ms read (the strip cursor, a changing digit) can smear.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
CLI=${STM32_CLI:-/Applications/STMicroelectronics/STM32Cube/STM32CubeProgrammer/STM32CubeProgrammer.app/Contents/Resources/bin/STM32_Programmer_CLI}
OUT=$1
TMP=$(mktemp -t fb.XXXXXX).bin
"$CLI" -c port=SWD mode=HOTPLUG -u 0xD0000000 0x177000 "$TMP" 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -iE "rror" || true
python3 "$HERE/fb_to_png.py" "$TMP" "$OUT"
rm -f "$TMP"
