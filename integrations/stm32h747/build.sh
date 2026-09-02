#!/bin/sh
# Build the M7 harness (flash bank 1) and the M4 harness (flash bank 2)
# against the thumbv7em staticlib. CI runs this as the hard-float link
# assertion; run.sh flashes both images.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
LIB="$REPO/target/thumbv7em-none-eabihf/release/libexpanse.a"
test -f "$LIB" || { echo "missing $LIB" >&2; exit 1; }
CC=arm-none-eabi-gcc
M7="-mcpu=cortex-m7 -mthumb -mfloat-abi=hard -mfpu=fpv5-d16"
M4="-mcpu=cortex-m4 -mthumb -mfloat-abi=hard -mfpu=fpv4-sp-d16"
COMMON="-O2 -g -ffunction-sections -fdata-sections -Wall -Wextra -Werror -nostartfiles --specs=nosys.specs -Wl,--gc-sections"
SRC="$HERE/startup.c $HERE/main.c $HERE/alts.c"
QUICKDEF=${QUICK:+-DQUICK -Wno-unused-function}
$CC $M7 $COMMON $QUICKDEF -DCORE_M7 -I"$REPO/include" -T "$HERE/m7.ld" $SRC "$LIB" -lc -lgcc \
    -Wl,-Map="$HERE/m7.map" -o "$HERE/m7.elf"
$CC $M4 $COMMON $QUICKDEF -DCORE_M4 -I"$REPO/include" -T "$HERE/m4.ld" $SRC "$LIB" -lc -lgcc \
    -Wl,-Map="$HERE/m4.map" -o "$HERE/m4.elf"
arm-none-eabi-size "$HERE/m7.elf" "$HERE/m4.elf"
# Float-ABI assertion (§8.1: loud): the links above already refuse a
# soft/hard mismatch; this makes the resolved ABI visible and fails if an
# image is not hard-float.
for img in m7 m4; do
    arm-none-eabi-readelf -A "$HERE/$img.elf" > "$HERE/$img.attrs"
    grep -E "Tag_CPU_name|Tag_FP_arch|Tag_ABI_VFP_args" "$HERE/$img.attrs" || true
    grep -q "Tag_ABI_VFP_args: VFP registers" "$HERE/$img.attrs" || {
        echo "FAIL: $img.elf is not hard-float (Tag_ABI_VFP_args missing)" >&2
        exit 1
    }
done
