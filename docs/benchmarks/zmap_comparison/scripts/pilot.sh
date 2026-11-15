#!/bin/bash
# Pilot (excluded from verdicts): rate selection and receive-thread identification.
set -eu
OUT=/results/pilot; mkdir -p $OUT/out
/scripts/netns_setup.sh > /dev/null
for rate in 50000 100000 200000 400000; do
  for rep in 1 2; do
    for a in NONE:stock:none STOCK:stock:window EXPANSE:expanse:window; do
      IFS=: read n v d <<< "$a"
      python3 /scripts/measure.py $n $v $d 2 $rate 4242 1000000 $OUT/out >> $OUT/raw.jsonl
    done
  done
done
python3 - <<'PY'
import json
for l in open("/results/pilot/raw.jsonl"):
    r = json.loads(l); t = r["thread_cpu_ns"]; z = str(r["zpid"])
    print(r["rate"], r["arm"].ljust(7), r.get("rows"), r.get("pcap_drop"), r["wall_s"],
          round(r["user_s"] + r["sys_s"], 2), r["maxrss_kb"], "main", round(t.get(z, 0) / 1e6),
          "threads(ms)", [round(t[k] / 1e6) for k in t if k != z], round(r["host_busy_core_eq"], 2))
PY
