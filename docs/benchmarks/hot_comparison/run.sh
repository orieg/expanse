#!/usr/bin/env bash
# ==============================================================================
# 1-command reproduction runner for the HOT (Height Optimized Trie) comparison
# suite (#660). Compares ExpanseSet and ExpanseMap against HOT's two value
# models, reached through the C++ FFI shim in crates/expanse-hot-bench.
#
# Requires an x86-64 host with AVX2 and BMI2 — HOT does not build otherwise —
# and an initialised submodule:
#     git submodule update --init --depth 1 third_party/hot
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

if [ ! -f "${REPO_ROOT}/third_party/hot/LICENSE" ]; then
  echo "HOT sources are missing. Run:" >&2
  echo "    git submodule update --init --depth 1 third_party/hot" >&2
  exit 1
fi

# The concurrent arm (#692, METHODOLOGY.md §11) links ROWEX, which needs TBB.
# libtbb is built from HOT's own pinned nested submodule into the cargo build
# directory — never a system package — so the nested checkout must exist.
case " $* " in
  *" --concurrent "*|*" --only-concurrent "*)
    if [ ! -f "${REPO_ROOT}/third_party/hot/third-party/tbb/Makefile" ]; then
      echo "TBB sources are missing for the concurrent arm. Run:" >&2
      echo "    git -C third_party/hot submodule update --init --depth 1 third-party/tbb" >&2
      exit 1
    fi
    ;;
esac

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

# Core pin (docs/BENCHMARKING.md rule 2, #639): confine this shell — and so
# every benchmark process it spawns — to the host's performance cores. A no-op
# on a uniform host; on the hybrid reference host an arm that lands on an
# efficiency core measures 1.576x the P-core time and no interval says so.
# shellcheck source-path=SCRIPTDIR/../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

# `run.sh strings [--quick]` drives the string-key arms (#693, METHODOLOGY
# §10); a bare `run.sh [--quick]` drives the integer arms (#660). Both take the
# same lock and the same core pin above.
if [ "${1:-}" = "strings" ]; then
  shift
  echo "========================================================================"
  echo " HOT Comparison Benchmark Suite — string-key arms (#693)"
  echo "========================================================================"
  echo " Arm C: HOT<const char*, IdentityKeyExtractor>   vs ExpanseStrMap (ptr)"
  echo " Arm D: HOT<pair*, PairPointerKeyExtractor>      vs ExpanseStrMap (u64)"
  echo " Arm E: HOT<const char*, IdentityKeyExtractor>   vs ExpanseBytesMap (ptr)"
  echo ""
  echo " Strings are the harness's; the census counts them on neither side and"
  echo " publishes them as their own column. HOT's 255-byte key window is a"
  echo " predicate evaluated per cell, never a restriction (METHODOLOGY §10)."
  echo "========================================================================"
  python3 "${SCRIPT_DIR}/scripts/run_strings.py" "$@"
else
  echo "========================================================================"
  echo " HOT Comparison Benchmark Suite — Expanse vs Height Optimized Trie (#660)"
  echo "========================================================================"
  echo " Arm A: HOT<uint64_t, IdentityKeyExtractor>      vs ExpanseSet  (63-bit)"
  echo " Arm B: HOT<pair*, PairPointerKeyExtractor>      vs ExpanseMap  (64-bit)"
  echo ""
  echo " Memory publishes a curve across expanse occupancy, not a cell per"
  echo " distribution: per-key cost is a sawtooth in density (METHODOLOGY §9.6)."
  echo ""
  echo " --concurrent / --only-concurrent: the HOT-ROWEX arm (#692, §11) —"
  echo " writer throughput vs writer count against SyncExpanseSet/SyncExpanseMap,"
  echo " readers alongside, protocol health, all threads inside the P-core pin."
  echo " --sensitivity / --only-sensitivity: the §12.2 sorted/shuffled pair."
  echo "========================================================================"
  python3 "${SCRIPT_DIR}/scripts/run_all.py" "$@"
fi
echo ""
echo " Results written to:"
echo "   docs/benchmarks/hot_comparison/results/"
echo "========================================================================"
