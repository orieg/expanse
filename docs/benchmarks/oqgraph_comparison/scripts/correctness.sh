#!/bin/bash
# Inside oqeval:24.04: generate graphs, load them once, compare full query results across variants.
set -euo pipefail
cp /w/prefix-expanse/lib/libexpanse.so /usr/local/lib/; ldconfig
G=/w/graphs-small; rm -rf $G; mkdir -p $G
for s in er pl grid er_sid; do for n in 1024 10000; do python3 /w/driver/gen_graph.py $s $n 1 $G; done; done
python3 /w/driver/oqdrv.py init /w/data-small $G
OQ_VERTICES=er_1024,pl_1024,grid_1024,er_sid_1024 python3 /w/driver/oqdrv.py check /w/data-small $G /w/results/check-small.json A B C B1
