#!/usr/bin/env bash
# Concurrency instruments (#568): the host's cross-core line-transfer matrix
# and the `pause` calibration that converts an `occ_stats` spin count into
# time. The `Sync*` scaling sweep itself (`benches/concurrency.rs`) is the
# `/benchmark concurrency` builtin runner in `.github/workflows/bench_baremetal.yml`;
# the health and per-thread counter cells live in the two FFI suites
# (`docs/benchmarks/{hot,masstree}_comparison/run.sh --only-concurrent` and
# `scripts/bench_counters.py`). METHODOLOGY.md is the pre-registration all
# of them are read against.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
cd "${REPO_ROOT}"

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
trap 'rm -rf "${BENCH_LOCK}"' EXIT

# Core pin (docs/BENCHMARKING.md rule 2, #639). The matrix driver pins each
# pair itself inside this mask; the pin here keeps the driver process and the
# build off the efficiency cores.
# shellcheck source-path=SCRIPTDIR/../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

QUICK=""
OUT="${SCRIPT_DIR}/results/line_transfer.json"
if [ "${1:-}" = "--quick" ]; then
  QUICK="--quick"
  OUT="${SCRIPT_DIR}/results/quick/line_transfer.json"
fi

echo "========================================================================"
echo " Concurrency instruments (#568) — cross-core line transfer + pause"
echo "========================================================================"
cargo build --release -p expanse-trie --example line_transfer
python3 "${REPO_ROOT}/scripts/line_transfer_matrix.py" --out "${OUT}" ${QUICK}
