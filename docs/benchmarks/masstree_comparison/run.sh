#!/usr/bin/env bash
# ==============================================================================
# 1-command reproduction runner for the Masstree comparison suite (#661).
# Compares ExpanseMap, ExpanseStrMap, SyncExpanseMap and SyncExpanseStrMap
# against Masstree (kohler/masstree-beta), reached through the C++ FFI shim in
# crates/expanse-hot-bench built with `--features masstree`.
#
# Requires an x86-64 host with AVX2 and BMI2 (the crate also builds the HOT
# shim, and both arms are bound to -march=haswell, METHODOLOGY §3.5) and both
# submodules initialised:
#     git submodule update --init --depth 1 third_party/hot third_party/masstree
#
#   run.sh [--quick]                integer, string and memory cells, after the gate
#   run.sh --concurrent [--quick]   ... plus the concurrent cells (§5, MC1/MC2)
#   run.sh --only-concurrent        the concurrent cells alone
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

for sub in hot masstree; do
  if [ ! -f "${REPO_ROOT}/third_party/${sub}/LICENSE" ]; then
    echo "${sub} sources are missing. Run:" >&2
    echo "    git submodule update --init --depth 1 third_party/${sub}" >&2
    exit 1
  fi
done

# Host-wide benchmark lock (docs/BENCHMARKING.md, methodology rule 8): one
# suite at a time per machine, across every checkout.
BENCH_LOCK="${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}"
if ! mkdir "${BENCH_LOCK}" 2>/dev/null; then
  echo "refusing to start: benchmark lock ${BENCH_LOCK} is held by:" >&2
  { cat "${BENCH_LOCK}/owner" 2>/dev/null || true; } >&2
  exit 75
fi
printf 'suite=%s pid=%s start=%s\n' "$(basename "${SCRIPT_DIR}")" "$$" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${BENCH_LOCK}/owner"
trap 'rm -rf "${BENCH_LOCK}"' EXIT

# Core pin (docs/BENCHMARKING.md rule 2, #639): confine this shell — and so
# every benchmark process it spawns — to the host's performance cores.
# shellcheck source-path=SCRIPTDIR/../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

echo "========================================================================"
echo " Masstree Comparison Benchmark Suite — Expanse vs Masstree (#661)"
echo "========================================================================"
echo " M1:  Masstree u64 -> u64            vs ExpanseMap"
echo " M2:  Masstree string -> u64         vs ExpanseStrMap"
echo " MC1: Masstree, W writers/R readers  vs SyncExpanseMap      (--concurrent)"
echo " MC2: Masstree, W writers/R readers  vs SyncExpanseStrMap   (--concurrent)"
echo ""
echo " The validation gate runs first and is fatal. Memory publishes two"
echo " instruments per cell — the allocator census, slab-quantized on Masstree"
echo " and flagged where the quantum dominates, and each engine's own node"
echo " census — and never mixes them (METHODOLOGY §3.3)."
echo "========================================================================"

python3 "${SCRIPT_DIR}/scripts/run_all.py" "$@"

echo ""
echo " Results written to:"
echo "   docs/benchmarks/masstree_comparison/results/"
echo "========================================================================"
