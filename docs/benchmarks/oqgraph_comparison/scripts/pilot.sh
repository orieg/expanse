#!/bin/bash
# Sizing pilot (build host, NOT a verdict run): per-query latency at N=10k/100k, arms A and C, 1 round.
set -euo pipefail
. /w/scripts/prep_runtime.sh >/dev/null
G=/w/graphs-bench; mkdir -p $G
for s in er pl grid er_sid; do for n in 10000 100000; do [ -f $G/${s}_$n.tsv ] || python3 /w/driver/gen_graph.py $s $n 1 $G; done; done
[ -d /w/data-bench ] || python3 /w/driver/oqdrv.py init /w/data-bench $G
OQ_K=4 python3 /w/driver/oqdrv.py bench /w/data-bench $G /w/results/pilot.json 1 1 A C
python3 - <<'PY'
import json, statistics as st
d = json.load(open('/w/results/pilot.json'))
for r in d["rounds"]:
    print(r["variant"], r["libs"], r["mem_warm"], r["mem_end"])
    for cell, xs in sorted(r["times"].items()):
        print(f"  {cell:40s} med {st.median(x[1] for x in xs)/1e6:9.2f} ms  rows {[x[2] for x in xs]}")
PY
