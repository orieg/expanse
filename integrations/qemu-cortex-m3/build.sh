#!/bin/sh
# Build the QEMU Cortex-M3 smoke against the soft-float thumbv7m staticlib.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
LIB="$REPO/target/thumbv7m-none-eabi/release/libexpanse.a"
test -f "$LIB" || { echo "missing $LIB (cargo build --release -p expanse-capi --no-default-features --features embedded-panic-handler --target thumbv7m-none-eabi)" >&2; exit 1; }
arm-none-eabi-gcc -mcpu=cortex-m3 -mthumb -mfloat-abi=soft \
    -O2 -g -ffunction-sections -fdata-sections -Wall -Wextra -Werror \
    -nostartfiles --specs=nosys.specs -Wl,--gc-sections \
    -I"$REPO/include" -T "$HERE/mps2.ld" "$HERE/startup.c" "$HERE/smoke.c" "$LIB" -lc -lgcc \
    -Wl,-Map="$HERE/smoke.map" -o "$HERE/smoke.elf"
arm-none-eabi-size "$HERE/smoke.elf"
# ABI assertion (loud, §8.1): the image must be soft-float ARMv7-M — the
# linker refuses a soft/hard mismatch, and this makes the resolved tags visible.
arm-none-eabi-readelf -A "$HERE/smoke.elf" > "$HERE/smoke.attrs"
grep -E "Tag_CPU_arch|Tag_ABI_VFP_args|Tag_FP_arch" "$HERE/smoke.attrs" || true
grep -q "Tag_CPU_arch: v7$" "$HERE/smoke.attrs" || { echo "FAIL: smoke.elf is not ARMv7-M" >&2; exit 1; }
if grep -q "Tag_ABI_VFP_args" "$HERE/smoke.attrs"; then echo "FAIL: smoke.elf is hard-float; the M3 has no FPU" >&2; exit 1; fi
