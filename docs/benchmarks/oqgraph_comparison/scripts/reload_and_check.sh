#!/bin/bash
set -euo pipefail
. /w/scripts/prep_runtime.sh >/dev/null
python3 /w/driver/oqdrv.py init /w/data-small /w/graphs-small
OQ_VERTICES=er_1024,pl_1024,grid_1024,er_sid_1024 python3 /w/driver/oqdrv.py check /w/data-small /w/graphs-small /w/results/check-small.json A B C B1
python3 /w/driver/oqdrv.py init /w/data-bench /w/graphs-bench
python3 /w/driver/oqdrv.py check /w/data-bench /w/graphs-bench /w/results/check-bench.json A B C B1
python3 - <<'PY'
import json, collections
for f in ("check-small", "check-bench"):
    d = json.load(open(f"/w/results/{f}.json"))["A"]["q"]
    zero = sorted(k for k, (n, h) in d.items() if n == 0)
    print(f, "queries", len(d), "zero-row", len(zero), zero[:12])
PY
