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

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
QUICK=0
[[ "${1:-}" == "--quick" ]] && QUICK=1

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
