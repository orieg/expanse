#!/bin/bash
# Inside oqeval:24.04 on the reference host (docker --cpuset-cpus=0-15). $1 = run label, $2 = order seed.
set -euo pipefail
RUN=$1; SEED=$2
export REPO_ROOT=/w/expanse-pin CARGO=/root/.cargo/bin/cargo
. /w/expanse-pin/scripts/bench_pin.sh
echo "bench_pin applied: ${EXPANSE_BENCH_PIN_APPLIED:-unset}"
. /w/scripts/prep_runtime.sh
CELLS=""
for s in er pl grid er_sid; do
  for w in oq_bfs_from oq_dijkstra_from oq_leaves_from oq_bfs_path oq_dijkstra_path oq_bfs_to; do CELLS+="${s}_10000:$w,"; done
  for w in oq_bfs_from oq_dijkstra_from oq_leaves_from oq_bfs_path oq_dijkstra_path; do CELLS+="${s}_100000:$w,"; done
done
export OQ_CELLS=${CELLS%,} OQ_K_10000=16 OQ_K_100000=8 OQ_WARM=1 OQ_SERVER_CPUS=2,4,6,8
mkdir -p /w/results
taskset -c 10 python3 /w/driver/oqdrv.py bench /w/data-bench /w/graphs-bench /w/results/bench-$RUN.json 10 $SEED A B C B1
