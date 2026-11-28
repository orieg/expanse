#!/bin/sh
# One end-to-end run: ROUNDS rounds, every (workload, arm) cell once per round,
# arm order rotated per round. One zmap process per measurement.
set -eu
RUN=$1; ROUNDS=$2; RATE=$3
OUT=/results/e2e_run$RUN; mkdir -p $OUT/out
/scripts/netns_setup.sh
python3 - "$RUN" "$ROUNDS" "$RATE" <<'PY'
import json, os, subprocess, sys
run, rounds, rate = sys.argv[1], int(sys.argv[2]), sys.argv[3]
arms = [("NONE", "stock", "none"), ("STOCK", "stock", "window"), ("EXPANSE", "expanse", "window")]
if "main" in os.environ.get("ZMAP_ARMS", "").split():
    arms.insert(2, ("MAIN", "main", "window"))
wls = [("e1", "1"), ("e2", "2")]
with open(f"/results/e2e_run{run}/raw.jsonl", "a") as f:
    for r in range(rounds):
        for wi, (wl, probes) in enumerate(wls[r % 2:] + wls[:r % 2]):
            k = (r + wi) % len(arms)
            for name, variant, dedup in arms[k:] + arms[:k]:
                out = subprocess.run(["python3", "/scripts/measure.py", name, variant, dedup, probes,
                                      rate, "4242", "1000000", f"/results/e2e_run{run}/out"],
                                     capture_output=True, text=True).stdout.strip()
                rec = json.loads(out); rec.update(run=int(run), round=r, workload=wl)
                f.write(json.dumps(rec) + "\n"); f.flush()
                print(r, wl, name, rec.get("thread_cpu_ns"), rec.get("pcap_drop"), rec.get("rows"), flush=True)
PY
