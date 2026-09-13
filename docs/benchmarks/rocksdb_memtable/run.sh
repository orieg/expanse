#!/usr/bin/env bash
# ==============================================================================
# 1-command reproduction runner for the RocksDB MemTable benchmark suite.
# Evaluates ExpanseMemTable vs ReferenceSkipListRep vs VectorRep.
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

# Core pin (docs/BENCHMARKING.md rule 2, #639): confine this shell — and so
# every benchmark process it spawns — to the host's performance cores. A no-op
# on a uniform host; on the hybrid reference host an arm that lands on an
# efficiency core measures 1.576x the P-core time and no interval says so.
# shellcheck source-path=SCRIPTDIR/../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

echo "========================================================================"
echo " Running Expanse RocksDB MemTable Comparative Benchmark Suite"
echo " Repo Root: ${REPO_ROOT}"
echo "========================================================================"

cd "${REPO_ROOT}"
cargo build --release -p expanse-capi
# Builds the binary and prints the human-readable table once. The measured cells
# are the driver's, below: `make bench`'s single invocation times every phase in
# one process and so has no cell boundary for a load snapshot to attach to (#868).
make -C integrations/rocksdb bench

# The measurement. One `bench_memtable --arm <phase>` process per cell, phases
# interleaved within each round, per-cell load attribution and BCa intervals
# (AGENTS.md sections 8.4, 8.17, 8.20.2). `--quick` here: a reproduction run on a
# developer host writes to the gitignored results/quick/ and never to a committed
# baseline (section 8.5). Drop `--quick` on the reference host.
python3 docs/benchmarks/rocksdb_memtable/scripts/single_threaded_bench.py --quick

if [ -f "integrations/rocksdb/scripts/generate_bench_svg.py" ]; then
  python3 integrations/rocksdb/scripts/generate_bench_svg.py
fi

echo ""
echo "========================================================================"
echo " Benchmark suite completed successfully!"
echo " Results written to: docs/benchmarks/rocksdb_memtable/results/"
echo "========================================================================"
