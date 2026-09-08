#!/usr/bin/env bash
# The #568 Step 0 measurement campaign, reference host only: every cell the
# pre-registration (../METHODOLOGY.md §3) names, in the order §8.17 wants —
# short and single-threaded first, the concurrent sweeps last, each with its
# own load snapshot — under one host-wide lock and one core pin. The suite
# runners are called directly (not through their run.sh) so the lock is
# taken once here rather than refused by a nested runner.
#
# Two runs of every FFI sweep (rule 18): the first lands in
# results/baseline_concurrent.json, the second in *_run2.json.
#
#   nohup docs/benchmarks/concurrency/scripts/step0_campaign.sh > campaign.log 2>&1 &
#
# Nothing here decides a verdict; the artifacts are read against METHODOLOGY.md.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
cd "${REPO_ROOT}"

BENCH_LOCK="${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}"
if ! mkdir "${BENCH_LOCK}" 2>/dev/null; then
  echo "refusing to start: benchmark lock ${BENCH_LOCK} is held by:" >&2
  { cat "${BENCH_LOCK}/owner" 2>/dev/null || true; } >&2
  exit 75
fi
printf 'suite=concurrency-step0 pid=%s start=%s\n' "$$" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${BENCH_LOCK}/owner"
trap 'rm -rf "${BENCH_LOCK}"' EXIT

# shellcheck source-path=SCRIPTDIR/../../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

stamp() { echo "== $(date -u +%H:%M:%SZ) $*"; uptime; }
CONC="${REPO_ROOT}/docs/benchmarks/concurrency/results"
HOT="${REPO_ROOT}/docs/benchmarks/hot_comparison"
MT="${REPO_ROOT}/docs/benchmarks/masstree_comparison"
mkdir -p "${CONC}"

stamp "1/6 line-transfer matrix + pause calibration"
cargo build --release -p expanse-trie --example line_transfer
python3 scripts/line_transfer_matrix.py --out "${CONC}/line_transfer.json"

stamp "2/6 per-thread hardware counters, ten #568 cells (+ two #730 cells)"
python3 scripts/bench_counters.py --repeats 7 \
  --cell masstree_conc_map_w1_r0 --cell masstree_conc_map_w1_r8 \
  --cell masstree_conc_map_w8_r0 --cell masstree_conc_map_w16_r0 \
  --cell masstree_conc_str_w8_r0 --cell masstree_conc_str_w16_r0 \
  --cell hot_conc_set_w1_r0 --cell hot_conc_set_w1_r8 \
  --cell hot_conc_map_w1_r0 --cell hot_conc_map_w1_r8 \
  --cell masstree_conc_str_w0_r1 --cell masstree_conc_str_w0_r8

stamp "3/6 #789 ablations on C1 W=1 and C2 W=1 R=8"
python3 docs/benchmarks/concurrency/scripts/ablations.py --out "${CONC}/ablations.json"

stamp "4/6 masstree concurrent sweep, run 1 then run 2"
python3 "${MT}/scripts/run_all.py" --only-concurrent
mv "${MT}/results/baseline_concurrent.json" "${MT}/results/baseline_concurrent_run1.tmp.json"
python3 "${MT}/scripts/run_all.py" --only-concurrent
mv "${MT}/results/baseline_concurrent.json" "${MT}/results/baseline_concurrent_run2.json"
mv "${MT}/results/baseline_concurrent_run1.tmp.json" "${MT}/results/baseline_concurrent.json"

stamp "5/6 HOT-ROWEX concurrent sweep, run 1 then run 2"
python3 "${HOT}/scripts/run_all.py" --only-concurrent
mv "${HOT}/results/baseline_concurrent.json" "${HOT}/results/baseline_concurrent_run1.tmp.json"
python3 "${HOT}/scripts/run_all.py" --only-concurrent
mv "${HOT}/results/baseline_concurrent.json" "${HOT}/results/baseline_concurrent_run2.json"
mv "${HOT}/results/baseline_concurrent_run1.tmp.json" "${HOT}/results/baseline_concurrent.json"

stamp "6/6 done"
