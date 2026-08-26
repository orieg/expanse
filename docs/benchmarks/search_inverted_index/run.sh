#!/usr/bin/env bash
# ==============================================================================
# 1-Command reproduction runner for the search / inverted-index suite.
# Evaluates ExpanseSet (Judy1) posting lists vs Roaring bitmaps across Boolean
# algebra, WAND skip-scan, and memory footprint.
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
trap 'rm -rf "${BENCH_LOCK}"' EXIT

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
