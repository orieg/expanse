#!/usr/bin/env bash
# ==============================================================================
# 1-Command reproduction runner for the search / inverted-index suite.
# Evaluates ExpanseSet (Judy1) posting lists vs Roaring bitmaps across Boolean
# algebra, WAND skip-scan, and memory footprint.
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
echo " Running ExpanseSet vs Roaring inverted-index benchmark suite"
echo " Repo Root: ${REPO_ROOT}"
echo "========================================================================"

python3 "${SCRIPT_DIR}/scripts/run_all.py" "$@"

echo ""
echo "========================================================================"
echo " Suite completed. Results in:"
echo "   docs/benchmarks/search_inverted_index/results/"
echo ""
echo " Deterministic instruction counts (Linux + valgrind only):"
echo "   cargo bench -p expanse-trie --bench search_instructions"
echo "========================================================================"
