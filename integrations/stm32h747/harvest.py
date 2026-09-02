#!/usr/bin/env python3
"""Turn the harness transcript (RESULT k=v lines) into a summary table + JSON.

Every number in the JSON is copied or derived from the transcript of one run;
nothing is typed in by hand (AGENTS.md §8.2). Nanoseconds are derived from the
host-timed TICK/TOCK calibration at each clock, not from the nominal clock.

    harvest.py transcript.txt [libexpanse-commit]
"""
import json
import statistics
import sys
from collections import defaultdict

path = sys.argv[1] if len(sys.argv) > 1 else "transcript.txt"
rows, info = [], {"calibration": []}
if len(sys.argv) > 2:
    info["commit"] = sys.argv[2]
sysclk_seen = [64000000]  # calibration lines arrive in clock order; INFO sysclk= lines mark switches
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
                if k == "sysclk":
                    sysclk_seen.append(int(v))

# measured Hz per nominal clock: the i-th calibration belongs to the i-th clock seen
measured_hz = {}
for clk, cal in zip(sysclk_seen, info["calibration"]):
    if cal["host_seconds"] > 0:
        measured_hz[clk] = cal["cycles"] / cal["host_seconds"]
info["measured_hz"] = measured_hz


def ns(cycles: float, clk: int) -> float | None:
    hz = measured_hz.get(clk)
    return cycles / hz * 1e9 if hz else None


fix = defaultdict(list)
for r in rows:
    if "pass" in r:
        fix[(r.get("impl", "expanse"), r["name"], r["sysclk"], r["dcache"])].append(r["cycles"] / r["ops"])

summary = {"info": info, "fixtures": [], "bytes": [], "isr": []}
print(f"{'impl':<13}{'fixture':<20}{'sysclk':>10}{'dc':>3}{'n':>3}{'min cyc/op':>11}{'median':>9}{'max':>9}{'min ns':>9}")
for (impl, name, clk, dc), xs in sorted(fix.items()):
    n = ns(min(xs), clk)
    print(f"{impl:<13}{name:<20}{clk:>10}{dc:>3}{len(xs):>3}{min(xs):>11.1f}{statistics.median(xs):>9.1f}{max(xs):>9.1f}"
          f"{(f'{n:.1f}' if n is not None else '-'):>9}")
    summary["fixtures"].append({"impl": impl, "name": name, "sysclk": clk, "dcache": dc, "min": min(xs),
                                "median": statistics.median(xs), "max": max(xs), "passes": xs, "min_ns": n})

for r in rows:
    if r.get("name") == "bytes":
        d = {"impl": r["impl"], "shape": r["shape"], "keys": r["keys"], "heap_bytes": r["heap_bytes"],
             "req_bytes": r["req_bytes"], "heap_bytes_per_key": r["heap_bytes"] / r["keys"],
             "req_bytes_per_key": r["req_bytes"] / r["keys"]}
        summary["bytes"].append(d)
if summary["bytes"]:
    print(f"\n{'impl':<13}{'shape':<10}{'keys':>6}{'heap B':>9}{'B/key':>7}{'req B':>9}{'B/key':>7}")
    for d in summary["bytes"]:
        print(f"{d['impl']:<13}{d['shape']:<10}{d['keys']:>6}{d['heap_bytes']:>9}{d['heap_bytes_per_key']:>7.1f}"
              f"{d['req_bytes']:>9}{d['req_bytes_per_key']:>7.1f}")

print(f"\n{'arm':<22}{'sysclk':>10}{'period':>8}{'mut/s':>9}{'w cyc/mut':>10}{'isr_n':>7}{'busy%':>7}{'lat max':>8}{'lat mean':>9}{'body max':>9}{'refused':>8}{'bad':>4}")
for r in rows:
    if "isr_n" in r:
        n = max(r["isr_n"], 1)
        rate = r["sysclk"] / r["period"] if r["period"] else r["mutations"] * r["sysclk"] / r["writer_cycles"]
        d = {"name": r["name"], "sysclk": r["sysclk"], "period": r["period"], "mutations": r["mutations"],
             "mutations_per_s": rate, "writer_cycles_per_mutation": r["writer_cycles"] / r["mutations"],
             "isr_n": r["isr_n"], "ok": r["isr_ok"], "not_found": r["isr_nf"], "busy": r["isr_busy"], "bad": r["isr_bad"],
             "busy_rate": r["isr_busy"] / n, "lat_max": r["lat_max"], "lat_mean": r["lat_sum"] / n,
             "lat_max_ns": ns(r["lat_max"], r["sysclk"]),
             "body_max": r["dur_max"], "body_mean": r["dur_sum"] / n, "refused": r["refused"], "arena_full": r["arena_full"]}
        summary["isr"].append(d)
        print(f"{d['name']:<22}{d['sysclk']:>10}{d['period']:>8}{rate:>9.0f}{d['writer_cycles_per_mutation']:>10.0f}{d['isr_n']:>7}"
              f"{100*d['busy_rate']:>7.1f}{d['lat_max']:>8}{d['lat_mean']:>9.1f}{d['body_max']:>9}{d['refused']:>8}{d['bad']:>4}")

json.dump(summary, open(path.rsplit(".", 1)[0] + ".json", "w"), indent=1)
