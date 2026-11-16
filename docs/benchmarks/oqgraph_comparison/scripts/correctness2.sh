#!/bin/bash
set -euo pipefail
. /w/scripts/prep_runtime.sh
G=/w/graphs-small
OQ_VERTICES=er_1024,pl_1024,grid_1024,er_sid_1024 python3 /w/driver/oqdrv.py check /w/data-small $G /w/results/check-small.json A B C B1
O=$(python3 -c "import json;print(json.load(open('$G/er_sid_10000.queries.json'))['origins'][0])")
D=$(python3 -c "import json;print(json.load(open('$G/er_sid_10000.queries.json'))['dests'][0])")
python3 /w/driver/oqdrv.py probe /w/data-small $G /w/results/probe-dijkstra-from.json "SELECT * FROM g_er_sid_10000 WHERE latch='dijkstras' AND origid=$O" A B C
python3 /w/driver/oqdrv.py probe /w/data-small $G /w/results/probe-dijkstra-path.json "SELECT * FROM g_er_sid_10000 WHERE latch='dijkstras' AND origid=$O AND destid=$D" A B C
