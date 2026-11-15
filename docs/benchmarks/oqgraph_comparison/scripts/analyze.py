#!/usr/bin/env python3
"""Apply the PREREG.md decision rule to bench-run*.json. Usage: analyze.py <repo-scripts-dir> <run.json>..."""
import json, math, statistics as st, sys
sys.path.insert(0, sys.argv[1])
from bca_bootstrap import bca_bootstrap_ci_with_method

NCPU = 24
LO, HI = 0.97, 1.03

def busy_cores(a, b):
    dt = b["total_jiffies"] - a["total_jiffies"]
    return NCPU * (b["busy_jiffies"] - a["busy_jiffies"]) / dt if dt else float("nan")

def load_run(path):
    d = json.load(open(path))
    recs = d["rounds"]
    for r in recs:
        r["busy_timed"] = busy_cores(r["snap"][1], r["snap"][2])
        r["load1_start"] = float(r["snap"][0]["loadavg"][0])
    med = st.median(r["busy_timed"] for r in recs)
    void = sorted({r["round"] for r in recs if r["busy_timed"] > med + 1.5 or r["load1_start"] > 12})
    return d, recs, med, void

def verdict(lo, hi):
    v = []
    if LO <= lo and hi <= HI: v.append("EQUIVALENT")
    if lo > 1.0: v.append("SLOWER")
    if hi < 1.0: v.append("FASTER")
    return "+".join(v) or "INCONCLUSIVE"

def analyse(path):
    d, recs, med, void = load_run(path)
    arms = d["meta"]["variants"]
    by = {(r["round"], r["variant"]): r for r in recs if r["round"] not in void}
    rounds = sorted({k[0] for k in by})
    cells = sorted(next(iter(by.values()))["times"])
    out = {"meta": d["meta"], "busy_median": med, "void_rounds": void, "rounds_used": rounds,
           "libs": {a: by[(rounds[0], a)]["libs"] for a in arms}, "cells": {}, "mem": {}}
    for a in arms:
        out["mem"][a] = {k: st.median(by[(r, a)]["mem_end"][k] for r in rounds) for k in ("VmHWM", "VmRSS")}
    for c in cells:
        T = {a: [sum(x[1] for x in by[(r, a)]["times"][c]) for r in rounds] for a in arms}
        row = {"A_ms_median": st.median(T["A"]) / 1e6 / len(by[(rounds[0], "A")]["times"][c]),
               "rows": [x[2] for x in by[(rounds[0], "A")]["times"][c]][:3]}
        for a in arms:
            if a == "A":
                continue
            ell = [math.log(x / y) for x, y in zip(T[a], T["A"])]
            m, lo, hi, meth = bca_bootstrap_ci_with_method(ell, 0.95, 10000, 42)
            row[a] = {"ratio": math.exp(m), "ci": [math.exp(lo), math.exp(hi)], "ci_method": meth,
                      "verdict": verdict(math.exp(lo), math.exp(hi))}
        out["cells"][c] = row
    return out

def main():
    runs = [analyse(p) for p in sys.argv[2:]]
    json.dump(runs, open("analysis.json", "w"), indent=1)
    arms = [a for a in runs[0]["meta"]["variants"] if a != "A"]
    for i, r in enumerate(runs):
        print(f"run{i+1}: rounds used {len(r['rounds_used'])}, void {r['void_rounds']}, busy median {r['busy_median']:.2f} cores, libs {r['libs']}")
        print(f"   mem (kB, median over rounds): {r['mem']}")
    print("\n| cell | A ms/query | " + " | ".join(f"{a} run1 [CI] | {a} run2 [CI]" for a in arms) + " |")
    print("|---|---|" + "---|---|" * len(arms))
    for c in runs[0]["cells"]:
        parts = []
        for a in arms:
            for r in runs:
                x = r["cells"][c][a]
                parts.append(f"{x['ratio']:.3f} [{x['ci'][0]:.3f}, {x['ci'][1]:.3f}] {x['verdict']}")
        print(f"| {c} | {runs[0]['cells'][c]['A_ms_median']:.2f} | " + " | ".join(parts) + " |")
    print()
    for a in arms:
        both = {c: [r["cells"][c][a]["verdict"] for r in runs] for c in runs[0]["cells"]}
        eq = all(all("EQUIVALENT" in v for v in vs) for vs in both.values())
        slow = [c for c, vs in both.items() if all("SLOWER" in v for v in vs)]
        fast = [c for c, vs in both.items() if all("FASTER" in v for v in vs)]
        slow_big = [c for c in slow if all(r["cells"][c][a]["ci"][0] > HI for r in runs)]
        fast_big = [c for c in fast if all(r["cells"][c][a]["ci"][1] < LO for r in runs)]
        not_eq = [c for c, vs in both.items() if not all("EQUIVALENT" in v for v in vs)]
        arm_v = ("PERFORMANCE-NEUTRAL" if eq else "REGRESSES" if slow_big else "IMPROVES" if fast_big and not slow_big else "MIXED/INCONCLUSIVE")
        print(f"{a}: {arm_v}; slower in both runs: {slow}; faster in both runs: {fast}; outside ±3% in some run: {not_eq}")

main()
