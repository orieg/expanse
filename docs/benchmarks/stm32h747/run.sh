#!/bin/sh
# Reproduce the STM32H747I-DISCO suite. Needs the board on its ST-LINK (CN2)
# port, the Arm GNU toolchain and STM32CubeProgrammer; see
# integrations/stm32h747/README.md. Writes transcript.txt + results.json into results/
# and re-renders the three charts under results/.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
cd "$REPO"
cargo build --release -p expanse-capi --no-default-features \
    --features embedded-panic-handler --target thumbv7em-none-eabihf
sh integrations/stm32h747/build.sh
sh integrations/stm32h747/run.sh "$HERE/capture.txt"
# integrations/run.sh leaves capture.txt (raw), capture_clean.txt (CR-stripped)
# and capture_clean.json (harvest); keep the clean pair under the suite's names.
mkdir -p "$HERE/results"
mv "$HERE/capture_clean.txt" "$HERE/results/transcript.txt"
mv "$HERE/capture_clean.json" "$HERE/results/results.json"
rm -f "$HERE/capture.txt"
python3 "$HERE/scripts/generate_charts.py"
