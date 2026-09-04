#!/usr/bin/env bash
# ==============================================================================
# 1-command reproduction runner for the WebAssembly fuel benchmark suite.
# Measures exact fuel consumption across wasm32 and wasm64 targets.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# Capability preflight: wasmtime python package is required
if ! python3 -c "import wasmtime" 2>/dev/null; then
  echo "refusing to start: 'wasmtime' python module is not installed." >&2
  echo "Run: pip install wasmtime" >&2
  exit 1
fi

# Host-wide benchmark lock (docs/BENCHMARKING.md, methodology rule 8): one
# suite at a time per machine, across every checkout.
BENCH_LOCK="${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}"
if ! mkdir "${BENCH_LOCK}" 2>/dev/null; then
  echo "refusing to start: benchmark lock ${BENCH_LOCK} is held by:" >&2
  { cat "${BENCH_LOCK}/owner" 2>/dev/null || true; } >&2
  exit 75
fi
printf 'suite=%s pid=%s start=%s\n' "$(basename "${SCRIPT_DIR}")" "$$" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${BENCH_LOCK}/owner"
trap 'rm -rf "${BENCH_LOCK}"' EXIT INT TERM

cd "${REPO_ROOT}"

echo "========================================================================"
echo " WebAssembly Fuel Benchmark Suite (wasm32 vs wasm64)"
echo "========================================================================"
echo "System load at start (docs/BENCHMARKING.md rule 2, non-gating):"
uptime
echo ""

# Measure wasm32 and update baseline
echo "--- Running wasm32 fuel benchmarks ---"
python3 scripts/wasm_fuel.py --build wasm32 --save-baseline results/baseline_wasm_fuel.json "$@"

# Measure wasm64 if rustc nightly with build-std is available
echo "--- Running wasm64 fuel benchmarks ---"
NIGHTLY="${WASM_FUEL_NIGHTLY:-nightly}"
if rustc "+${NIGHTLY}" --version >/dev/null 2>&1; then
  python3 scripts/wasm_fuel.py --build wasm64 --save-baseline results/baseline_wasm_fuel.json "$@"
else
  echo "nightly toolchain (${NIGHTLY}) absent — skipping wasm64 run (requires nightly for -Z build-std)"
fi

echo "--- Regenerating WebAssembly suite charts ---"
python3 "${SCRIPT_DIR}/scripts/generate_charts.py"

echo ""
echo " Results written to:"
echo "   results/baseline_wasm_fuel.json"
echo "   docs/benchmarks/wasm/results/bench_wasm_fuel.svg"
echo "========================================================================"
