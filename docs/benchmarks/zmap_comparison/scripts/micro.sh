#!/bin/sh
# Microbenchmark run: ROUNDS rounds, arms alternated, workloads rotated,
# one process per measurement, pinned to one CPU of the container's cpuset.
set -eu
RUN=$1; ROUNDS=$2; CPU=$3
OUT=/results/micro_run$RUN; mkdir -p $OUT
python3 - "$RUN" "$ROUNDS" "$CPU" <<'PY'
import json, os, subprocess, sys
run, rounds, cpu = sys.argv[1], int(sys.argv[2]), sys.argv[3]
def busy():
    v=[int(x) for x in open("/proc/stat").readline().split()[1:]]
    return sum(v[:8])-v[3]-v[4], sum(v[:8])
wls = ["m1", "m2", "m3"]; arms = ["stock", "expanse"]
with open(f"/results/micro_run{run}/raw.jsonl", "a") as f:
    for r in range(rounds):
        for wl in wls[r % 3:] + wls[:r % 3]:
            for arm in (arms if r % 2 == 0 else arms[::-1]):
                load = open("/proc/loadavg").read().split()[:3]
                b0, t0 = busy()
                p = subprocess.run(["taskset", "-c", cpu, f"/build/{arm}/cachehash_bench", wl, "4242"],
                                   capture_output=True, text=True)
                b1, t1 = busy()
                rec = json.loads(p.stdout); rec.update(run=int(run), round=r, arm=arm.upper(), rc=p.returncode,
                    load_before=load, host_busy_core_eq=(b1-b0)/max(1,t1-t0)*os.cpu_count())
                f.write(json.dumps(rec) + "\n"); f.flush()
                print(r, wl, arm, rec["ns_per_response"], rec["ok"], round(rec["host_busy_core_eq"],2), flush=True)
PY
