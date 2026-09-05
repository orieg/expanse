#!/usr/bin/env python3
"""Pair two STM32 harvests (control.json treatment.json): per cell, the Expanse min-of-5 delta and the
largest twin movement in the same (core, clock, dcache, fixture) row — the suite README's noise floor."""
import json, sys
c, t = json.load(open(sys.argv[1])), json.load(open(sys.argv[2]))
def idx(d): return {(f["core"], f["sysclk"], f["dcache"], f["name"], f["impl"]): f for f in d["fixtures"]}
ci, ti = idx(c), idx(t)
print(f"control {c['info']['commit']}  treatment {t['info']['commit']}")
print(f"{'core':4} {'MHz':>4} {'dc':>2} {'fixture':18} {'ctrl':>9} {'treat':>9} {'delta':>8} {'floor':>6}  verdict")
rows = sorted({k[:4] for k in ci})
for core, clk, dc, name in rows:
    def get(i, impl): return i.get((core, clk, dc, name, impl))
    twin_moves = []
    for impl in ("sorted_array", "open_hash", "tsearch"):
        a, b = get(ci, impl), get(ti, impl)
        if a and b: twin_moves.append(abs(b["min"] / a["min"] - 1) * 100)
    a, b = get(ci, "expanse"), get(ti, "expanse")
    if not (a and b): continue
    d = (b["min"] / a["min"] - 1) * 100; floor = max(twin_moves) if twin_moves else float("nan")
    verdict = "inside floor" if abs(d) <= floor else ("attributed" if d < 0 else "REGRESSION?")
    print(f"{core:4} {clk//1000000:>4} {dc:>2} {name:18} {a['min']:9.1f} {b['min']:9.1f} {d:+7.2f}% {floor:5.2f}%  {verdict}")
