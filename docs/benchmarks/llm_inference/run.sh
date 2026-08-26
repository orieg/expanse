#!/usr/bin/env bash
# ==============================================================================
# 1-Command reproduction runner for the LLM inference & speculative decoding suite.
# Evaluates Expanse across speculative draft quality, multi-million token scale,
# native C++ llama.cpp cache integration, and prefix-cache LRU block eviction.
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
echo " Running Expanse LLM Inference & Speculative Decoding Benchmark Suite"
echo " Repo Root: ${REPO_ROOT}"
echo "========================================================================"

# 1. Ensure expanse-capi release library is compiled
if [ ! -f "$REPO_ROOT/target/release/libexpanse.dylib" ] && [ ! -f "$REPO_ROOT/target/release/libexpanse.so" ]; then
    echo "==> Building expanse-capi release library..."
    (cd "$REPO_ROOT" && cargo build --release -p expanse-capi)
fi

# 2. Build native C++ harness
echo "==> Compiling native C++ harnesses..."
make -C "$SCRIPT_DIR"

# 3. Execute master benchmark runner
echo "==> Running benchmarks..."
python3 "$SCRIPT_DIR/scripts/run_all.py" "$@"

echo ""
echo "========================================================================"
echo " Suite completed. Results in:"
echo "   docs/benchmarks/llm_inference/results/"
echo "========================================================================"
