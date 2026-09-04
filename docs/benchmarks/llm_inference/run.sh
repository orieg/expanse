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

# Host-wide benchmark lock (docs/BENCHMARKING.md, methodology rule 8): one
# suite at a time per machine, across every checkout. `mkdir` is atomic and
# portable (flock(1) does not exist on stock macOS), and it is the same
# mechanism the sibling suites use, so they mutually exclude.
BENCH_LOCK="${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}"
if ! mkdir "${BENCH_LOCK}" 2>/dev/null; then
  echo "refusing to start: benchmark lock ${BENCH_LOCK} is held by:" >&2
  { cat "${BENCH_LOCK}/owner" 2>/dev/null || true; } >&2
  exit 75
fi
printf 'suite=%s pid=%s start=%s\n' "$(basename "${SCRIPT_DIR}")" "$$" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${BENCH_LOCK}/owner"
trap 'rm -rf "${BENCH_LOCK}"' EXIT

# Core pin (docs/BENCHMARKING.md rule 2, #639): confine this shell — and so
# every benchmark process it spawns — to the host's performance cores. A no-op
# on a uniform host; on the hybrid reference host an arm that lands on an
# efficiency core measures 1.576x the P-core time and no interval says so.
# shellcheck source-path=SCRIPTDIR/../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

log_system_load() {
    echo "--- System Load Snapshot ---"
    if command -v uptime >/dev/null 2>&1; then
        uptime
    fi
    echo "----------------------------"
}

echo "========================================================================"
echo " Running Expanse LLM Inference Benchmark Suite"
echo " Repo Root: ${REPO_ROOT}"
echo "========================================================================"
log_system_load

# Step 0: Math derivation & ceiling unit tests
echo "==> [0/5] Verifying Step 0 Speedup Ceiling Model..."
python3 "${SCRIPT_DIR}/scripts/ceiling.py"

# Step 1: Materialize runtime artifacts. The gitignored corpus binary is
# regenerated when missing; the reference-token streams are PINNED committed
# snapshots (the committed results were measured on them) and are never
# refreshed here — run scripts/record_streams.py manually to re-record from
# the live sources.
echo "==> [1/5] Materializing Datastore Corpus & Reference Streams..."
if [[ ! -f "${DATA_DIR}/datastore_corpus.bin" ]]; then
    python3 "${SCRIPT_DIR}/scripts/build_corpus.py"
fi
if [[ ! -f "${DATA_DIR}/humaneval_reference_tokens.json" \
   || ! -f "${DATA_DIR}/summary_reference_tokens.json" \
   || ! -f "${DATA_DIR}/json_reference_tokens.json" ]]; then
    python3 "${SCRIPT_DIR}/scripts/record_streams.py"
else
    echo "    (pinned reference streams present; skipping network re-record)"
fi

# Step 2: Pillar A (Speculative Draft Quality & Alpha)
echo "==> [2/5] Running Pillar A: Reference-Continuation Draft Quality..."
log_system_load
python3 "${SCRIPT_DIR}/benches/bench_draft_quality.py" ${QUICK_FLAG}

# Step 3: Pillar B (Native Rust Dynamic Datastore vs Static Sorted Window Index)
echo "==> [3/5] Running Pillar B: Native Rust Dynamic Datastore vs Static Window Index..."
log_system_load
(
    cd "${REPO_ROOT}"
    cargo bench --bench bench_llm_datastore -p expanse-trie -- ${QUICK_FLAG}
)

# Step 4: Pillar D (Native Rust Grammar Masks vs Roaring / Dense)
echo "==> [4/5] Running Pillar D: Native Rust Grammar Mask Cache & Set Algebra..."
log_system_load
(
    cd "${REPO_ROOT}"
    cargo bench --bench bench_grammar_masks -p expanse-trie -- ${QUICK_FLAG}
)

# Step 5: Pillar E (KV-Block Index Appendix)
echo "==> [5/5] Running Pillar E: Prefix-Cache KV-Block Table..."
log_system_load
python3 "${SCRIPT_DIR}/benches/bench_prefix_lru.py" ${QUICK_FLAG}

# Step 6: Generate Dual-Theme SVGs — full runs only. A --quick run writes
# reduced-sweep data into results/*.json, and regenerating the committed
# charts from it would ship blank/mislabeled SVGs.
if [[ -z "${QUICK_FLAG}" ]]; then
    echo "==> Generating Dual-Theme SVG Comparison Charts..."
    python3 "${SCRIPT_DIR}/scripts/generate_charts.py"
else
    echo "==> Skipping chart regeneration (--quick)."
    echo "    WARNING: the quick run rewrote results/*.json with reduced-sweep"
    echo "    smoke data. Restore before committing:"
    echo "      git restore docs/benchmarks/llm_inference/results"
fi

log_system_load
echo "========================================================================"
echo " All LLM Inference benchmarks completed successfully!"
echo " Results written to: ${RESULTS_DIR}"
echo "========================================================================"
