#!/usr/bin/env python3
"""Turn the harness transcript (RESULT k=v lines) into a summary table + JSON.

Every number in the JSON is copied or derived from the transcript of one run;
nothing is typed in by hand (AGENTS.md §8.2)."""
import json
import statistics
import sys
from collections import defaultdict

path = sys.argv[1] if len(sys.argv) > 1 else "transcript.txt"  # argv[2]: libexpanse commit for provenance
rows, info = [], {"calibration": []}
if len(sys.argv) > 2: info["commit"] = sys.argv[2]
for line in open(path, errors="replace"):
    line = line.strip()
    if line.startswith("RESULT "):
        kv = dict(t.split("=", 1) for t in line[7:].split())
        rows.append({k: (int(v) if v.lstrip("-").isdigit() else v) for k, v in kv.items()})
    elif line.startswith("CALIB "):
        kv = dict(x.split("=", 1) for x in line[6:].split())
        info["calibration"].append({"host_seconds": float(kv["host_seconds"]), "cycles": int(kv["cycles"])})
    elif line.startswith("INFO "):
        for t in line[5:].split():
            if "=" in t:
                k, v = t.split("=", 1)
                info[k] = int(v) if v.isdigit() else v

fix = defaultdict(list)
for r in rows:
    if "pass" in r:
        fix[(r["name"], r["sysclk"], r["dcache"])].append(r["cycles"] / r["ops"])

summary = {"info": info, "fixtures": [], "isr": []}
print(f"{'fixture':<20}{'sysclk':>11}{'dcache':>7}{'n':>3}{'min cyc/op':>12}{'median':>9}{'max':>9}")
for (name, clk, dc), xs in sorted(fix.items()):
    print(f"{name:<20}{clk:>11}{dc:>7}{len(xs):>3}{min(xs):>12.1f}{statistics.median(xs):>9.1f}{max(xs):>9.1f}")
    summary["fixtures"].append({"name": name, "sysclk": clk, "dcache": dc, "min": min(xs),
                                "median": statistics.median(xs), "max": max(xs), "passes": xs})

print(f"\n{'arm':<22}{'period':>8}{'mut/s':>9}{'w cyc/mut':>10}{'isr_n':>7}{'busy%':>7}{'lat max':>8}{'lat mean':>9}{'body max':>9}{'refused':>8}{'bad':>4}")
for r in rows:
    if "isr_n" in r:
        n = max(r["isr_n"], 1)
        rate = r["sysclk"] / r["period"] if r["period"] else r["mutations"] * r["sysclk"] / r["writer_cycles"]
        d = {"name": r["name"], "sysclk": r["sysclk"], "period": r["period"], "mutations": r["mutations"],
             "mutations_per_s": rate, "writer_cycles_per_mutation": r["writer_cycles"] / r["mutations"],
             "isr_n": r["isr_n"], "ok": r["isr_ok"], "not_found": r["isr_nf"], "busy": r["isr_busy"], "bad": r["isr_bad"],
             "busy_rate": r["isr_busy"] / n, "lat_max": r["lat_max"], "lat_mean": r["lat_sum"] / n,
             "body_max": r["dur_max"], "body_mean": r["dur_sum"] / n, "refused": r["refused"], "arena_full": r["arena_full"]}
        summary["isr"].append(d)
        print(f"{d['name']:<22}{d['period']:>8}{rate:>9.0f}{d['writer_cycles_per_mutation']:>10.0f}{d['isr_n']:>7}"
              f"{100*d['busy_rate']:>7.1f}{d['lat_max']:>8}{d['lat_mean']:>9.1f}{d['body_max']:>9}{d['refused']:>8}{d['bad']:>4}")

json.dump(summary, open(path.rsplit(".", 1)[0] + ".json", "w"), indent=1)
