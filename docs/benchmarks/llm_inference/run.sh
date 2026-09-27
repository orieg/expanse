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
DATA_DIR="${SCRIPT_DIR}/data"

QUICK_FLAG=""
if [[ "${1:-}" == "--quick" ]]; then
    QUICK_FLAG="--quick"
fi

# A --quick run is a reduced-sweep smoke run: its output is scratch and goes to
# the gitignored results/quick/ so it can never overwrite the committed
# baselines (AGENTS.md §8.5). LLM_RESULTS_DIR carries that decision to every
# harness this script spawns, Python and cargo alike; each harness applies the
# same rule on its own when invoked directly.
if [[ -n "${QUICK_FLAG}" ]]; then
    RESULTS_DIR="${SCRIPT_DIR}/results/quick"
else
    RESULTS_DIR="${SCRIPT_DIR}/results"
fi
export LLM_RESULTS_DIR="${RESULTS_DIR}"
mkdir -p "${RESULTS_DIR}" "${DATA_DIR}"

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

# Step 6: Generate Dual-Theme SVGs — full runs only. The committed charts are
# rendered from the committed results; a --quick run's reduced-sweep data is
# scratch and would render blank or mislabeled SVGs over them.
if [[ -z "${QUICK_FLAG}" ]]; then
    echo "==> Generating Dual-Theme SVG Comparison Charts..."
    python3 "${SCRIPT_DIR}/scripts/generate_charts.py"
else
    echo "==> Skipping chart regeneration (--quick)."
fi

log_system_load
echo "========================================================================"
echo " All LLM Inference benchmarks completed successfully!"
echo " Results written to: ${RESULTS_DIR}"
echo "========================================================================"
