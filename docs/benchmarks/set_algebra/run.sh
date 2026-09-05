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
REPS=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --quick) QUICK=1; shift ;;
    # Independent repetitions of the whole harness. Each repetition's
    # target/criterion is snapshotted so scripts/harvest_domain.py can pair the
    # arms per repetition and bootstrap over repetitions (AGENTS.md §8.4). The
    # committed figures need at least 3.
    --reps) REPS="$2"; shift 2 ;;
    *) echo "usage: run.sh [--quick] [--reps N]" >&2; exit 2 ;;
  esac
done

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

# Raw per-repetition snapshots are scratch (§8.5): only the harvested summary
# JSON is committed, and it carries the per-repetition means.
RAW_DIR="$REPO_ROOT/results/quick/set_algebra/raw"
rm -rf "$RAW_DIR"; mkdir -p "$RAW_DIR"
: > "$RAW_DIR/loads.txt"

cd "$REPO_ROOT"
for ((rep = 1; rep <= REPS; rep++)); do
    echo "==> repetition $rep/$REPS: cargo bench -p expanse-trie --bench domain  (loadavg: $(cut -d' ' -f1-3 /proc/loadavg 2>/dev/null || uptime))"
    cut -d' ' -f1-3 /proc/loadavg >> "$RAW_DIR/loads.txt" 2>/dev/null || true
    rm -rf target/criterion
    if [[ $QUICK -eq 1 ]]; then
        cargo bench -p expanse-trie --bench domain -- --quick
    else
        cargo bench -p expanse-trie --bench domain
    fi
    mkdir -p "$RAW_DIR/rep_$rep"
    cp -r target/criterion "$RAW_DIR/rep_$rep/criterion"
done

echo "==> load snapshot after the run"
uptime

if [[ $REPS -ge 3 ]]; then
    COMMIT="$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo "${EXPANSE_BENCH_COMMIT:-unknown}")"
    HOST_DESC="${EXPANSE_HOST_DESC:-$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //'), $(nproc) logical CPUs, $(uname -sr)}"
    echo "==> harvest: $REPS repetitions -> $OUT_DIR/bench_domain_algebra.json"
    if [[ $QUICK -eq 1 && ! -f "$OUT_DIR/bench_domain_algebra.json" ]]; then
        cp "$REPO_ROOT/docs/benchmarks/set_algebra/results/bench_domain_algebra.json" "$OUT_DIR/"
    fi
    python3 "$SCRIPT_DIR/scripts/harvest_domain.py" --raw "$RAW_DIR" --out "$OUT_DIR/bench_domain_algebra.json" \
        --commit "$COMMIT" --host-desc "$HOST_DESC" --loads "$RAW_DIR/loads.txt" --markdown "$OUT_DIR/domain_tables.md"
else
    echo "==> fewer than 3 repetitions: samples snapshotted under $RAW_DIR, no aggregate written (run with --reps 3 or more to publish)"
fi

echo
echo "Criterion output: target/criterion/"
echo "Harvest intervals with:  python3 scripts/bench_baseline.py --help"
echo "Results directory:       $OUT_DIR"
