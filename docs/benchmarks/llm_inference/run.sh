#!/usr/bin/env bash
# ==============================================================================
# Master Runner for Expanse LLM Inference & Speculative Decoding Benchmark Suite
#
# Usage:
#   ./docs/benchmarks/llm_inference/run.sh          # Full benchmark run
#   ./docs/benchmarks/llm_inference/run.sh --quick  # Fast smoke run
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
RESULTS_DIR="${SCRIPT_DIR}/results"
DATA_DIR="${SCRIPT_DIR}/data"
mkdir -p "${RESULTS_DIR}" "${DATA_DIR}"

QUICK_FLAG=""
if [[ "${1:-}" == "--quick" ]]; then
    QUICK_FLAG="--quick"
fi

# Acquire atomic host-wide benchmark lock
LOCK_FILE="/tmp/expanse-bench.lock"
exec 200>"${LOCK_FILE}"
if ! flock -n 200; then
    echo "[-] Another benchmark suite is currently running. Waiting for lock..."
    flock 200
fi
echo "[+] Acquired benchmark lock (${LOCK_FILE})"

cleanup() {
    flock -u 200 || true
    echo "[+] Released benchmark lock"
}
trap cleanup EXIT

echo "========================================================================"
echo " Running Expanse LLM Inference Benchmark Suite"
echo " Repo Root: ${REPO_ROOT}"
echo "========================================================================"

# Step 0: Math derivation & ceiling unit tests
echo "==> [0/5] Verifying Step 0 Speedup Ceiling Model..."
python3 "${SCRIPT_DIR}/scripts/ceiling.py"

# Step 1: Generate Runtime Artifacts (ungated corpus + grammar DFAs + reference streams)
echo "==> [1/5] Generating Runtime Datastore Corpus & Reference Streams..."
python3 "${SCRIPT_DIR}/scripts/build_corpus.py"
python3 "${SCRIPT_DIR}/scripts/dump_grammar_dfa.py"
python3 "${SCRIPT_DIR}/scripts/record_streams.py"

# Step 2: Pillar A (Speculative Draft Quality & Alpha)
echo "==> [2/5] Running Pillar A: Reference-Continuation Draft Quality..."
python3 "${SCRIPT_DIR}/benches/bench_draft_quality.py" ${QUICK_FLAG}

# Step 3: Pillar B (Native Rust Dynamic Datastore vs Suffix Array)
echo "==> [3/5] Running Pillar B: Native Rust Dynamic Datastore vs Suffix Array..."
(
    cd "${REPO_ROOT}"
    cargo bench --bench bench_llm_datastore -p expanse-trie -- ${QUICK_FLAG}
)

# Step 4: Pillar D (Native Rust Grammar Masks vs Roaring / Dense)
echo "==> [4/5] Running Pillar D: Native Rust Grammar Mask Cache & Set Algebra..."
(
    cd "${REPO_ROOT}"
    cargo bench --bench bench_grammar_masks -p expanse-trie -- ${QUICK_FLAG}
)

# Step 5: Pillar E (KV-Block Index Appendix)
echo "==> [5/5] Running Pillar E: Prefix-Cache KV-Block Table..."
python3 "${SCRIPT_DIR}/benches/bench_prefix_lru.py" ${QUICK_FLAG}

# Step 6: Generate Dual-Theme SVGs
echo "==> Generating Dual-Theme SVG Comparison Charts..."
python3 "${SCRIPT_DIR}/scripts/generate_charts.py"

echo "========================================================================"
echo " All LLM Inference benchmarks completed successfully!"
echo " Results written to: ${RESULTS_DIR}"
echo "========================================================================"
