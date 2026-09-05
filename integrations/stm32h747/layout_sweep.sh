#!/bin/sh
# Layout-controlled build of the M7 can_dispatch cell (design doc §8.1.3).
#
# Relinks each engine archive with `_layout_pre` bytes of padding before all
# code and `_layout_gap` bytes between the library's code and the harness's
# (m7.ld with the library placed first; see the sed below; the archive must be
# named libexpanse*.a for the linker-script pattern to select its members), flashes the M7
# image only (the M4 image on flash bank 2 idles: sweep mode never engages
# it), captures the VCP until DONE and harvests. Same board, one sitting.
#
#   layout_sweep.sh OUTDIR "LABEL=ARCHIVE@COMMIT ..." "PRE:GAP PRE:GAP ..."
#
# e.g. layout_sweep.sh results/layout "control=/x/a.a@22908c15 treatment=/x/b.a@a98d8d3c" "0:0 8:0 0:512"
# Every (pad, engine) run writes OUTDIR/<label>_pre<P>_gap<G>.json (+ .txt).
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
CLI=${STM32_CLI:-/Applications/STMicroelectronics/STM32Cube/STM32CubeProgrammer/STM32CubeProgrammer.app/Contents/Resources/bin/STM32_Programmer_CLI}
PORT=${STM32_PORT:-$(ls -t /dev/cu.usbmodem* | grep -v ABC1234567892 | head -1)}
OUT=$1; ARCHIVES=$2; PADS=$3
mkdir -p "$OUT"
CC=arm-none-eabi-gcc
M7="-mcpu=cortex-m7 -mthumb -mfloat-abi=hard -mfpu=fpv5-d16"
COMMON="-O2 -g -ffunction-sections -fdata-sections -Wall -Wextra -Werror -Wno-unused-function -nostartfiles --specs=nosys.specs -Wl,--gc-sections"
SRC="$HERE/startup.c $HERE/main.c $HERE/alts.c"
# The variant script: library code first, then the gap, then everything else.
# Only the order inside .text changes; the default m7.ld is untouched.
LD="$OUT/m7_layout.ld"
sed 's|    \*(.text\*)|    _layout_pre_start = .;\n    . += DEFINED(_layout_pre) ? _layout_pre : 0;\n    *libexpanse*.a:*(.text*)\n    _layout_gap_start = .;\n    . += DEFINED(_layout_gap) ? _layout_gap : 0;\n    _layout_harness_start = .;\n    *(.text*)|' "$HERE/m7.ld" > "$LD"
grep -q "_layout_gap_start" "$LD" || { echo "linker-script hook not applied" >&2; exit 1; }
for pad in $PADS; do
  pre=${pad%%:*}; gap=${pad#*:}
  for spec in $ARCHIVES; do
    label=${spec%%=*}; rest=${spec#*=}; archive=${rest%%@*}; commit=${rest#*@}
    tag="${label}_pre${pre}_gap${gap}"
    echo "[$(date -u +%FT%TZ)] $tag: $archive ($commit)"
    $CC $M7 $COMMON -DCORE_M7 -DLAYOUT_SWEEP -I"$REPO/include" -T "$LD" \
        -Wl,--defsym=_layout_pre="$pre" -Wl,--defsym=_layout_gap="$gap" \
        $SRC "$archive" -lc -lgcc -Wl,-Map="$OUT/$tag.map" -o "$OUT/$tag.elf" 2>&1 | grep -E "error|FAIL" || true
    test -f "$OUT/$tag.elf" || { echo "link failed for $tag" >&2; exit 1; }
    "$CLI" -c port=SWD mode=UR reset=HWrst -d "$OUT/$tag.elf" -v 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E "Error|error" || true
    python3 "$HERE/capture.py" "$PORT" "$OUT/$tag.txt" 120 > /dev/null &
    CAP=$!
    sleep 2
    "$CLI" -c port=SWD mode=UR reset=HWrst -rst >/dev/null 2>&1 || true
    wait $CAP
    tr -d '\r' < "$OUT/$tag.txt" > "$OUT/$tag.clean"
    python3 "$HERE/harvest.py" "$OUT/$tag.clean" "$commit" > /dev/null   # writes $OUT/$tag.json
    rm -f "$OUT/$tag.clean" "$OUT/$tag.elf"
    grep -E "CHECK|FAULT" "$OUT/$tag.txt" && echo "!! $tag reported a CHECK/FAULT line" || true
  done
done
echo "SWEEP_DONE"
