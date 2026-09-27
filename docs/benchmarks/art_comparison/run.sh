#!/usr/bin/env bash
# ==============================================================================
# 1-command reproduction runner for the ART vs Expanse benchmark suite (#387).
# Compares ExpanseMap against blart::TreeMap (Adaptive Radix Tree), BTreeMap,
# and hashbrown::HashMap.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Host-wide benchmark lock (docs/BENCHMARKING.md rule 8): one suite at a
# time per machine, across every checkout. The script re-runs itself under
# scripts/bench_lock.py, which holds a flock the kernel releases when the
# holder dies, so a killed run cannot leave the host locked (#1210).
if [ -z "${EXPANSE_BENCH_LOCK_HELD:-}" ]; then
  exec python3 "${REPO_ROOT}/scripts/bench_lock.py" --suite "$(basename "${SCRIPT_DIR}")" -- \
    bash "${SCRIPT_DIR}/$(basename "${BASH_SOURCE[0]}")" "$@"
fi

# Core pin (docs/BENCHMARKING.md rule 2, #639): confine this shell — and so
# every benchmark process it spawns — to the host's performance cores. A no-op
# on a uniform host; on the hybrid reference host an arm that lands on an
# efficiency core measures 1.576x the P-core time and no interval says so.
# shellcheck source-path=SCRIPTDIR/../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

echo "========================================================================"
echo " ART Comparison Benchmark Suite — Expanse vs blart (ART) vs BTree (#387)"
echo "========================================================================"

python3 "${SCRIPT_DIR}/scripts/run_all.py" "$@"

echo ""
echo " Results and charts written to:"
echo "   docs/benchmarks/art_comparison/results/"
echo "========================================================================"
