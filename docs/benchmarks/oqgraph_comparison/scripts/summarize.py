#!/usr/bin/env python3
"""Summary figures for the README, derived from a record's committed files.

usage: summarize.py <record_dir>

Reads <record_dir>/bench-run{1,2}.json (raw rows) and <record_dir>/analysis.json
(written by analyze.py from those rows). Prints, per arm, the ranges and cell
counts the suite README quotes, and checks that every timed query returned the
same row count in every arm, round and run. Written when the 0.18-era record was
committed; it computes nothing the pre-registration did not already define, it
only aggregates.
"""
import json, os, sys

rec = sys.argv[1]
runs = json.load(open(os.path.join(rec, "analysis.json")))
HI = 1.03

# Row counts: one value per (cell, query index) across every arm, round and run.
rows, cells_bad = {}, set()
for name in ("bench-run1.json", "bench-run2.json"):
    for r in json.load(open(os.path.join(rec, name)))["rounds"]:
        for cell, xs in r["times"].items():
            for i, _t, n in xs:
                rows.setdefault((cell, i), set()).add(n)
for (cell, i), ns in rows.items():
    if len(ns) != 1:
        cells_bad.add(cell)
ncells = len({c for c, _ in rows})
print(f"row counts: {len(rows)} (cell, query) pairs over {ncells} cells; "
      f"cells with any disagreement: {len(cells_bad)}")

for i, r in enumerate(runs, 1):
    print(f"run{i}: void rounds {r['void_rounds']}, rounds used {len(r['rounds_used'])}, "
          f"busy median {r['busy_median']:.2f} core-equivalents")

cells = list(runs[0]["cells"])
for arm in [a for a in runs[0]["meta"]["variants"] if a != "A"]:
    pts = [r["cells"][c][arm]["ratio"] for r in runs for c in cells]
    los = [r["cells"][c][arm]["ci"][0] for r in runs for c in cells]
    his = [r["cells"][c][arm]["ci"][1] for r in runs for c in cells]
    meth = sorted({r["cells"][c][arm]["ci_method"] for r in runs for c in cells})
    slow = [c for c in cells if all("SLOWER" in r["cells"][c][arm]["verdict"] for r in runs)]
    fast = [c for c in cells if all("FASTER" in r["cells"][c][arm]["verdict"] for r in runs)]
    big = [c for c in slow if all(r["cells"][c][arm]["ci"][0] > HI for r in runs)]
    print(f"{arm}: point {min(pts):.3f}-{max(pts):.3f}; interval bounds {min(los):.3f}-{max(his):.3f}; "
          f"ci_method {meth}; slower in both runs {len(slow)}/{len(cells)}; "
          f"faster in both runs {len(fast)} {fast}; lower bound > {HI} in both runs {len(big)} {big}")

for i, r in enumerate(runs, 1):
    print(f"run{i} VmHWM median kB: " + ", ".join(f"{a} {v['VmHWM']:,.0f}" for a, v in r["mem"].items()))
