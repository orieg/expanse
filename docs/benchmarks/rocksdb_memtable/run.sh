#!/usr/bin/env bash
# ==============================================================================
# 1-command reproduction runner for the RocksDB MemTable benchmark suite.
# Evaluates ExpanseMemTable vs ReferenceSkipListRep vs VectorRep.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

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
