#!/bin/sh
# Build the M7 harness and the M4 idle image against the thumbv7em staticlib.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
LIB="$REPO/target/thumbv7em-none-eabihf/release/libexpanse.a"
test -f "$LIB" || { echo "missing $LIB" >&2; exit 1; }
CC=arm-none-eabi-gcc
M7="-mcpu=cortex-m7 -mthumb -mfloat-abi=hard -mfpu=fpv5-d16"
M4="-mcpu=cortex-m4 -mthumb -mfloat-abi=hard -mfpu=fpv4-sp-d16"
COMMON="-O2 -g -ffunction-sections -fdata-sections -Wall -Wextra -Werror -nostartfiles --specs=nosys.specs -Wl,--gc-sections"
$CC $M7 $COMMON -I"$REPO/include" -T "$HERE/m7.ld" \
    "$HERE/startup_m7.c" "$HERE/main.c" "$LIB" -lc -lgcc \
    -Wl,-Map="$HERE/m7.map" -o "$HERE/m7.elf"
$CC $M4 $COMMON -T "$HERE/m4.ld" "$HERE/m4_idle.c" -o "$HERE/m4.elf"
arm-none-eabi-size "$HERE/m7.elf" "$HERE/m4.elf"
arm-none-eabi-readelf -A "$HERE/m7.elf" | grep -E "Tag_ABI_VFP_args|Tag_CPU_name|Tag_FP_arch"
