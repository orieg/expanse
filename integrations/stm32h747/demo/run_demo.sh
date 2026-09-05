#!/bin/sh
# Build, flash and record the two-lanes demo: capture the VCP for the whole
# program, grab a framebuffer PNG at the middle of every step over SWD, harvest.
#
#   run_demo.sh OUTDIR [ENGINE_COMMIT]
#
# The M7 image goes to flash bank 1 over the on-board ST-LINK; the M4 image in
# bank 2 (the measurement harness's) idles. Each grab halts the core for a few
# hundred milliseconds, so both lanes record one blocked burst per grab; the
# grab times are logged to OUTDIR/grabs.txt and the RESULT line of each step
# is read before the grab of that step, never across it.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
CLI=${STM32_CLI:-/Applications/STMicroelectronics/STM32Cube/STM32CubeProgrammer/STM32CubeProgrammer.app/Contents/Resources/bin/STM32_Programmer_CLI}
PORT=${STM32_PORT:-$(ls -t /dev/cu.usbmodem* | grep -v ABC1234567892 | head -1)}
OUT=$1; COMMIT=${2:-$(git -C "$REPO" rev-parse --short=8 HEAD)}
mkdir -p "$OUT"
( cd "$HERE/Makefile/CM7" && make -j8 >/dev/null 2>"$OUT/build.log" ) || { echo "build failed, see $OUT/build.log" >&2; exit 1; }
"$CLI" -c port=SWD mode=UR reset=HWrst -d "$HERE/Makefile/CM7/build/expanse_demo_CM7.elf" -v 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E "File download complete|rror" || true
# program: prologue 6 s, seven steps of 20 s, summary; grab at 4 s into the prologue, 14 s into each step, and in the summary
python3 "$REPO/integrations/stm32h747/capture.py" "$PORT" "$OUT/transcript.txt" 172 > "$OUT/capture.log" &
CAP=$!
sleep 2
"$CLI" -c port=SWD mode=UR reset=HWrst -rst >/dev/null 2>&1 || true
T0=$(date +%s)
: > "$OUT/grabs.txt"
grab() { # $1 = seconds after reset (plus the ~2 s prefill), $2 = name
  while [ $(( $(date +%s) - T0 )) -lt "$1" ]; do sleep 1; done
  sh "$HERE/tools/grab_frame.sh" "$OUT/$2.png" >/dev/null 2>&1 && echo "$2 at +$(( $(date +%s) - T0 )) s" >> "$OUT/grabs.txt"
}
grab 6 frame_0_prologue
grab 22 frame_1_step1
grab 42 frame_2_step2
grab 62 frame_3_step3
grab 82 frame_4_step4
grab 102 frame_5_step5
grab 122 frame_6_step6
grab 142 frame_7_step7
grab 158 frame_8_summary
wait $CAP || true
tr -d '\r' < "$OUT/transcript.txt" > "$OUT/transcript.clean" && mv "$OUT/transcript.clean" "$OUT/transcript.txt"
python3 "$HERE/tools/harvest_demo.py" "$OUT/transcript.txt" "$COMMIT"
cat "$OUT/grabs.txt"
