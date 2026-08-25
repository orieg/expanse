#!/usr/bin/env bash
# ==============================================================================
# 1-Command Benchmark Reproduction Runner
# Evaluates ExpanseMap vs hashbrown::HashMap vs std::collections::BTreeMap
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

echo "========================================================================"
echo " Running Expanse vs Hashbrown vs BTreeMap Comparative Benchmark Suite"
echo " Repo Root: ${REPO_ROOT}"
echo "========================================================================"

# Run all benchmarks and regenerate SVG charts
python3 "${SCRIPT_DIR}/scripts/run_all.py" "$@"

echo ""
echo "========================================================================"
echo " Benchmark suite completed successfully!"
echo " Results written to: docs/benchmarks/hashbrown_comparison/results/"
echo "========================================================================"
