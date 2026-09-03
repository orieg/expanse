"""Pair two STM32 harvests on `min` cycles/op (the suite's published statistic).
Twins (non-expanse impls) share no engine code, so their movement is the drift
ceiling; an expanse cell is attributable only if it moves further than the
largest twin drift in the same (core, sysclk, dcache) row."""
import json, sys
from collections import defaultdict
ctl = json.load(open(sys.argv[1])); trt = json.load(open(sys.argv[2]))
key = lambda r: (r["core"], r["name"], r["sysclk"], r["dcache"], r["impl"])
c = {key(r): r for r in ctl["fixtures"]}; t = {key(r): r for r in trt["fixtures"]}
missing = sorted(set(c) ^ set(t))
if missing: print(f"!! {len(missing)} cells present in only one arm:", missing[:6])
# drift ceiling per (core, fixture, sysclk, dcache) from the twins
ceil = defaultdict(float)
for k in c:
    if k in t and k[4] != "expanse":
        d = abs(t[k]["min"] / c[k]["min"] - 1) * 100
        ceil[k[:4]] = max(ceil[k[:4]], d)
print(f"{'core':4} {'fixture':22} {'clk':>4} {'dc':>2} {'main':>10} {'PR':>10} {'delta':>8} {'noise':>8}  verdict")
for k in sorted(k for k in c if k in t and k[4] == "expanse"):
    core, name, clk, dc, _ = k
    p, q = c[k]["min"], t[k]["min"]; d = (q / p - 1) * 100; m = ceil[k[:4]]
    v = "inside noise -- not claimed" if abs(d) <= m else ("outside noise -- faster" if d < 0 else "outside noise -- slower")
    print(f"{core:4} {name:22} {clk//1_000_000:>4} {dc:>2} {p:10.1f} {q:10.1f} {d:+7.2f}% {m:7.2f}%  {v}")
