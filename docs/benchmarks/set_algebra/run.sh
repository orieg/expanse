#!/usr/bin/env bash
# Reproduce the set-algebra suite's own arms (the `domain` harness).
#
# The other four algebra harnesses are reproduced by the suites that own them:
#   search_boolean / search_instructions -> docs/benchmarks/search_inverted_index/run.sh
#   bench_grammar_masks                  -> docs/benchmarks/llm_inference/run.sh
#   avx512_bitmap                        -> docs/benchmarks/avx512/run.sh
#
# Wall-clock gating is valid only on the quiet reference host (AGENTS.md §8.4).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
QUICK=0
[[ "${1:-}" == "--quick" ]] && QUICK=1

# Host-wide benchmark lock (docs/BENCHMARKING.md, methodology rule 8): one
# suite at a time per machine, across every checkout. `mkdir` is atomic; the
# lock names its owner so a refused start says who holds the host. This suite
# landed without it; the core pin below makes it matter more, because two
# concurrent suites now contend for 16 CPUs rather than 24.
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

# --quick writes to a gitignored scratch path and never touches committed
# results (AGENTS.md §8.5).
if [[ $QUICK -eq 1 ]]; then
    OUT_DIR="$REPO_ROOT/results/quick/set_algebra"
else
    OUT_DIR="$REPO_ROOT/docs/benchmarks/set_algebra/results"
fi
mkdir -p "$OUT_DIR"

echo "==> load snapshot before the run (AGENTS.md §8.4 / BENCHMARKING.md rule 2)"
uptime

echo "==> cargo bench -p expanse-trie --bench domain"
cd "$REPO_ROOT"
if [[ $QUICK -eq 1 ]]; then
    cargo bench -p expanse-trie --bench domain -- --quick
else
    cargo bench -p expanse-trie --bench domain
fi

echo "==> load snapshot after the run"
uptime

echo
echo "Criterion output: target/criterion/"
echo "Harvest intervals with:  python3 scripts/bench_baseline.py --help"
echo "Results directory:       $OUT_DIR"
