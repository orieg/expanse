#!/bin/bash
set -euo pipefail
. /w/scripts/prep_runtime.sh >/dev/null
CG=/w/results/cg; rm -rf $CG; mkdir -p $CG
C10=$(for s in er pl grid er_sid; do for w in oq_bfs_from oq_dijkstra_from oq_leaves_from oq_bfs_path oq_dijkstra_path oq_bfs_to; do echo -n "${s}_10000:$w,"; done; done)
C100="er_100000:oq_bfs_from,er_100000:oq_dijkstra_from,pl_100000:oq_bfs_from,grid_100000:oq_bfs_from,er_sid_100000:oq_bfs_from"
OQ_CALLGRIND=$CG OQ_CELLS="${C10}${C100}" python3 /w/driver/oqdrv.py profile /w/data-bench /w/graphs-bench 4 A C B
valgrind --version
