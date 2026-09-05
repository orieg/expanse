#!/usr/bin/env python3
"""Summarise a layout-controlled sweep (integrations/stm32h747/layout_sweep.sh).

Reads every ``<label>_pre<P>_gap<G>.json`` in the directory and prints, per
padding point, the Expanse ``can_dispatch`` cell of each engine label: the
timed min-of-5 cycles per get and the DWT split with the cache off and on,
plus the treatment-minus-control deltas. The "unattributed" column is
cycles − CPI − EXC − SLEEP − LSU + FOLD: the instruction count when every stall
is counted, and the instruction count plus whatever the counters miss when it
is not — it is printed for both cache states and marked when they disagree by
more than two cycles, because an instruction count cannot depend on the cache. The closing
lines give each engine's range over the sweep, which is the question the sweep
asks: does the CPI difference between the two engines survive every placement
(code), or does either engine's CPI range cover the other's (placement)?

    python3 docs/benchmarks/stm32h747/scripts/layout_sweep_table.py results/layout_sweep_<sha> [control treatment]
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path


def main() -> int:
    d = Path(sys.argv[1])
    labels = sys.argv[2:4] if len(sys.argv) >= 4 else ["control", "treatment"]
    runs: dict[tuple[int, int], dict[str, dict]] = {}
    for p in sorted(d.glob("*_pre*_gap*.json")):
        m = re.match(r"(.+)_pre(\d+)_gap(\d+)\.json$", p.name)
        if not m:
            continue
        label, pre, gap = m.group(1), int(m.group(2)), int(m.group(3))
        j = json.load(open(p))
        cell = {}
        for f in j["fixtures"]:
            if f["impl"] == "expanse" and f["name"] == "can_dispatch":
                cell[("timed", f["dcache"])] = f["min"]
        for e in j["dwt"]:
            if e["impl"] == "expanse" and e["fixture"] == "can_dispatch":
                cell[("dwt", e["dcache"])] = e["net_per_op"]
                cell[("wrap", e["dcache"])] = e["wrap_risk"]
        cell["layout"] = next((l for l in open(str(p)[:-5] + ".txt", errors="replace") if l.startswith("INFO layout")), "").strip()
        runs.setdefault((pre, gap), {})[label] = cell

    c, t = labels
    hdr = ["pre", "gap"]
    for lab in (c, t):
        hdr += [f"{lab} timed off / on", f"{lab} unattributed off / on", f"{lab} CPI off / on", f"{lab} LSU off / on"]
    hdr += ["Δ timed off / on", "Δ CPI off / on"]
    print("| " + " | ".join(hdr) + " |")
    print("|---:|---:|" + "|".join("---:" for _ in hdr[2:]) + "|")
    ranges: dict[str, dict[str, list[float]]] = {lab: {"cpi0": [], "cpi1": [], "t0": [], "t1": [], "instr": []} for lab in (c, t)}
    for (pre, gap), by in sorted(runs.items()):
        row = [str(pre), str(gap)]
        for lab in (c, t):
            x = by.get(lab)
            if not x or ("dwt", 0) not in x or ("dwt", 1) not in x:
                row += ["—"] * 4
                continue
            d0, d1 = x[("dwt", 0)], x[("dwt", 1)]
            wrap = " (wrap-suspect)" if x.get(("wrap", 0)) or x.get(("wrap", 1)) else ""
            dis = " ⚠" if abs(d0["instr"] - d1["instr"]) > 2 else ""
            row += [f"{x[('timed', 0)]:.1f} / {x[('timed', 1)]:.1f}", f"{d0['instr']:.1f} / {d1['instr']:.1f}{dis}", f"{d0['cpi']:.1f} / {d1['cpi']:.1f}{wrap}", f"{d0['lsu']:.1f} / {d1['lsu']:.1f}"]
            r = ranges[lab]
            r["cpi0"].append(d0["cpi"]); r["cpi1"].append(d1["cpi"]); r["t0"].append(x[("timed", 0)]); r["t1"].append(x[("timed", 1)]); r["instr"].append(d0["instr"])
        if c in by and t in by and ("dwt", 0) in by[c] and ("dwt", 0) in by[t]:
            a, b = by[c], by[t]
            row += [f"{b[('timed', 0)] - a[('timed', 0)]:+.1f} / {b[('timed', 1)] - a[('timed', 1)]:+.1f}",
                    f"{b[('dwt', 0)]['cpi'] - a[('dwt', 0)]['cpi']:+.1f} / {b[('dwt', 1)]['cpi'] - a[('dwt', 1)]['cpi']:+.1f}"]
        else:
            row += ["—", "—"]
        print("| " + " | ".join(row) + " |")
    print()
    for lab in (c, t):
        r = ranges[lab]
        if not r["cpi0"]:
            continue
        print(f"{lab}: {len(r['cpi0'])} placements; CPI cache off {min(r['cpi0']):.1f}–{max(r['cpi0']):.1f}, cache on {min(r['cpi1']):.1f}–{max(r['cpi1']):.1f}; "
              f"timed cache off {min(r['t0']):.1f}–{max(r['t0']):.1f}, cache on {min(r['t1']):.1f}–{max(r['t1']):.1f}; unattributed (cache off) {min(r['instr']):.1f}–{max(r['instr']):.1f}")
    rc, rt = ranges[c], ranges[t]
    if rc["cpi0"] and rt["cpi0"]:
        for dc, key in ((0, "cpi0"), (1, "cpi1")):
            overlap = max(rc[key]) >= min(rt[key])
            print(f"cache {'off' if dc == 0 else 'on'}: control CPI range {'reaches' if overlap else 'never reaches'} the treatment's "
                  f"(control max {max(rc[key]):.1f} vs treatment min {min(rt[key]):.1f})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
