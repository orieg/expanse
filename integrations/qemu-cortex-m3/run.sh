#!/bin/sh
# Run the smoke under QEMU (mps2-an385, Cortex-M3) with semihosting; the
# firmware's semihosting exit code is the process exit code, and the
# transcript must end with PASS. Fails loud on a timeout or a missing PASS.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
OUT="$HERE/qemu.out"
timeout 180 qemu-system-arm -M mps2-an385 -cpu cortex-m3 -nographic \
    -semihosting-config enable=on,target=native \
    -kernel "$HERE/smoke.elf" > "$OUT" 2>&1
code=$?
cat "$OUT"
if [ "$code" -eq 124 ]; then echo "FAIL: qemu timed out" >&2; exit 1; fi
if ! grep -q "^PASS" "$OUT"; then echo "FAIL: no PASS line (qemu exit $code)" >&2; exit 1; fi
exit "$code"
