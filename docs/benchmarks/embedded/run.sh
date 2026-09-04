#!/usr/bin/env bash
# ==============================================================================
# 1-command reproduction runner for the embedded memtable benchmark suite.
# Evaluates ExpanseMap32 against std::BTreeMap and hashbrown::HashMap.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Host-wide benchmark lock (docs/BENCHMARKING.md, methodology rule 8): one
# suite at a time per machine, across every checkout. `mkdir` is atomic; the
# lock names its owner so a refused start says who holds the host.
BENCH_LOCK="${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}"
if ! mkdir "${BENCH_LOCK}" 2>/dev/null; then
  echo "refusing to start: benchmark lock ${BENCH_LOCK} is held by:" >&2
  { cat "${BENCH_LOCK}/owner" 2>/dev/null || true; } >&2
  exit 75
fi
printf 'suite=%s pid=%s start=%s\n' "$(basename "${SCRIPT_DIR}")" "$$" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${BENCH_LOCK}/owner"
trap 'rm -rf "${BENCH_LOCK}"' EXIT INT TERM

# Core pin (docs/BENCHMARKING.md rule 2, #639): confine this shell — and so
# every benchmark process it spawns — to the host's performance cores. A no-op
# on a uniform host; on the hybrid reference host an arm that lands on an
# efficiency core measures 1.576x the P-core time and no interval says so.
# shellcheck source-path=SCRIPTDIR/../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

cd "${REPO_ROOT}"

echo "========================================================================"
echo " Embedded MemTable Benchmark Suite — Expanse vs BTree vs Hashbrown"
echo "========================================================================"
echo "System load at start (docs/BENCHMARKING.md rule 2, non-gating):"
uptime

cargo bench -p expanse-trie --bench embedded_memtable "$@"

python3 "${SCRIPT_DIR}/scripts/generate_charts.py"
if [ -f "${SCRIPT_DIR}/results/esp32.json" ]; then
  python3 "${SCRIPT_DIR}/scripts/generate_charts.py" --on-device
fi

echo ""
echo " Results and charts written to:"
echo "   docs/benchmarks/embedded/results/"
echo "========================================================================"
