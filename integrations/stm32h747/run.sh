#!/bin/sh
# Flash both cores' images over the on-board ST-LINK V3, then capture the
# VCP transcript until the harness prints DONE.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
CLI="/Applications/STMicroelectronics/STM32Cube/STM32CubeProgrammer/STM32CubeProgrammer.app/Contents/Resources/bin/STM32_Programmer_CLI"
OUT=${1:-$HERE/transcript.txt}

before=$(ls /dev/cu.usbmodem* 2>/dev/null || true)
"$CLI" -c port=SWD mode=UR reset=HWrst -d "$HERE/m4.elf" -v 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E "Device ID|Device name|Flash size|File download complete|Verification|Error|error" || true
"$CLI" -c port=SWD mode=UR reset=HWrst -d "$HERE/m7.elf" -v 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E "File download complete|Verification|Error|error" || true

# The ST-LINK V3 VCP: prefer a port whose USB parent is STMicro; fall back to the newest.
PORT=$(ls -t /dev/cu.usbmodem* | head -1)
echo "VCP: $PORT"
# macOS drops stty settings when the port closes, so hold it open from Python (termios).
python3 "$HERE/capture.py" "$PORT" "$OUT" 120 &
CAP=$!
sleep 2
"$CLI" -c port=SWD mode=UR reset=HWrst -rst >/dev/null 2>&1 || true
wait $CAP
tr -d '\r' < "$OUT" > "${OUT%.txt}_clean.txt"
python3 "$HERE/harvest.py" "${OUT%.txt}_clean.txt" "$(git -C "$HERE" rev-parse --short=8 HEAD)"
