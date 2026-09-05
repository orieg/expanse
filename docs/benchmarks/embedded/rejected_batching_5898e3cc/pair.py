"""Pair two ESP32 harvests on median cycles/op, per-arm twin drift as the noise floor.

The twins (hash_open_addressing, linear_scan, ring_buffer, sorted_array...) are
byte-identical C in both arms, so their movement between the two runs is the
floor below which nothing is attributable to the engine or component change.
"""
import json, sys
ctl = json.load(open(sys.argv[1])); trt = json.load(open(sys.argv[2]))
EXPANSE = {"expanse_slab", "expanse_memtable", "expanse_blobmap", "expanse"}
c, t = ctl["benchmarks"], trt["benchmarks"]
rows, floors = [], {}
for name in sorted(set(c) & set(t)):
    for arm in sorted(set(c[name]["arms"]) & set(t[name]["arms"])):
        cv = c[name]["arms"][arm]["cycles_per_op"]["median"]
        tv = t[name]["arms"][arm]["cycles_per_op"]["median"]
        d = (tv / cv - 1) * 100 if cv else float("nan")
        if arm not in EXPANSE:
            floors[name] = max(floors.get(name, 0.0), abs(d))
        rows.append((name, arm, cv, tv, d))
print(f"{'benchmark':44} {'arm':22} {'ctl':>10} {'trt':>10} {'delta':>8} {'noise':>7}  verdict")
for name, arm, cv, tv, d in rows:
    if arm not in EXPANSE:
        continue
    f = floors.get(name, 0.0)
    v = "inside noise -- not claimed" if abs(d) <= f else ("outside noise -- faster" if d < 0 else "outside noise -- slower")
    print(f"{name:44} {arm:22} {cv:10.1f} {tv:10.1f} {d:+7.2f}% {f:6.2f}%  {v}")
tw = [abs(d) for _, arm, _, _, d in rows if arm not in EXPANSE]
tw.sort()
print(f"\ntwins: {len(tw)} cells, max drift {max(tw):.2f}%, median {tw[len(tw)//2]:.2f}%")
