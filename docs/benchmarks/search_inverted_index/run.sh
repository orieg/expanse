#!/usr/bin/env bash
# ==============================================================================
# 1-Command reproduction runner for the search / inverted-index suite.
# Evaluates ExpanseSet (Judy1) posting lists vs Roaring bitmaps across Boolean
# algebra, WAND skip-scan, and memory footprint.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

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
