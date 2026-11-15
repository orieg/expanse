#!/usr/bin/env python3
"""H1/H2 from Callgrind dumps (run inside oqeval:24.04, where callgrind_annotate lives).
For each dump: total Ir, bitset Ir (self Ir of every function in libJudy*/libexpanse* plus
open_query::judy_bitset::*), and the top inclusive-free functions."""
import glob, json, os, re, subprocess, sys

LINE = re.compile(r"^\s*([\d,]+)\s+(?:\([^)]*\)\s+)?(.*\S)\s*$")

def parse(path):
    label = None
    for l in open(path, errors="replace"):
        if l.startswith("desc: Trigger:"):
            label = l.split("Trigger:", 1)[1].strip()
        if l.startswith("positions:") or l.startswith("events:"):
            break
    # Mid-run dumps carry "summary: 0"; callgrind_annotate takes that as the total and
    # prints nothing. Annotate a copy whose summary is the file's "totals:" value.
    txt = open(path, errors="replace").read()
    tot = [l for l in txt.splitlines() if l.startswith("totals:")][-1].split()[1]
    fixed = path + ".fixed"
    open(fixed, "w").write(txt.replace("summary: 0\n", f"summary: {tot}\n", 1))
    out = subprocess.run(["callgrind_annotate", "--inclusive=no", "--threshold=100", fixed],
                         capture_output=True, text=True, check=True).stdout
    os.remove(fixed)
    total, lib, wrap, top = None, 0, 0, []
    for l in out.splitlines():
        m = LINE.match(l)
        if not m:
            continue
        n, what = int(m.group(1).replace(",", "")), m.group(2)
        if "PROGRAM TOTALS" in what:
            total = n; continue
        if total is None:
            continue
        if re.search(r"libJudy|libexpanse", what):
            lib += n
        elif "judy_bitset" in what:
            wrap += n
        if len(top) < 8:
            top.append([n, what[:110]])
    return label, total, lib, wrap, top

res = {}
for f in sorted(glob.glob(sys.argv[1] + "/*.*.*")):
    if f.endswith(".fixed"):
        continue
    variant = os.path.basename(f).split(".")[0]
    label, total, lib, wrap, top = parse(f)
    if not label or not label.startswith("dump ") or not total:
        continue
    cell = label[len("dump "):].strip()
    res.setdefault(cell, {})[variant] = {"Ir": total, "lib_Ir": lib, "wrap_Ir": wrap,
                                         "share": (lib + wrap) / total, "top": top}
json.dump(res, open(sys.argv[2], "w"), indent=1)
print(f"{'cell':34s} {'A share':>8s} {'A lib%':>7s} {'C share':>8s} {'C/A Ir':>7s} {'B/A Ir':>7s}")
for cell, v in sorted(res.items()):
    a, c, b = v.get("A"), v.get("C"), v.get("B")
    cs = f"{100*c['share']:7.2f}% {c['Ir']/a['Ir']:7.4f}" if c else f"{'-':>8s} {'-':>7s}"
    bs = f"{b['Ir']/a['Ir']:7.4f}" if b else f"{'-':>7s}"
    print(f"{cell:34s} {100*a['share']:7.2f}% {100*a['lib_Ir']/a['Ir']:6.2f}% {cs} {bs}  A Ir={a['Ir']:,}")
