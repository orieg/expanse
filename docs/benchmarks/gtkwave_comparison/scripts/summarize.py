#!/usr/bin/env python3
"""Summarise run_eval.sh's output directory: correctness verdicts, Callgrind
instruction counts per process and per object, and peak RSS, per workload
and arm. Ratios are arm / A (stock).

The per-object columns are *self* Ir (instructions executed inside that
object), so they add up to the process total exactly; the script checks
that. Allocator work that the Judy provider causes is executed in libc and
is counted there, which is why the libc column is reported beside the
provider's. Callgrind's *inclusive* call costs are not used: in these
profiles, recursion cycles and mislabelled static functions (an LZ4 routine
reported as calling JudyHSIns) give inclusive costs above the process total.

Callgrind counts are exact for the binary and inputs; they are not wall
clock. Peak RSS is one process per (workload, arm), the kernel's
``ru_maxrss`` as GNU time reports it.

Usage: summarize.py OUT_DIR
"""

import os
import re
import sys

ARMS = ("A", "B", "B1")
PROVIDER = re.compile(r"libJudy\.so|libexpanse\.so")


def totals(path: str) -> int:
    with open(path) as f:
        for line in f:
            if line.startswith("summary:") or line.startswith("totals:"):
                return int(line.split()[1])
    raise SystemExit(f"no totals in {path}")


def self_ir_by_object(path: str) -> dict:
    """Self Ir per object (``ob=``) of a Callgrind profile with Ir only."""
    names = {}
    self_ir = {}
    ob = None
    after_call = False

    def resolve(val: str) -> str:
        m = re.match(r"\((\d+)\)(?:\s+(.*))?$", val)
        if not m:
            return val
        if m.group(2) is not None:
            names[m.group(1)] = m.group(2)
        return names.get(m.group(1), "?")

    with open(path) as f:
        for line in f:
            line = line.rstrip("\n")
            if not line:
                continue
            c = line[0]
            if c.isdigit() or c in "+-*":
                fields = line.split()
                if len(fields) < 2:
                    continue
                if after_call:  # the inclusive cost of the preceding call
                    after_call = False
                    continue
                self_ir[ob] = self_ir.get(ob, 0) + int(fields[-1])
            elif line.startswith("ob="):
                ob = resolve(line[3:])
            elif line.startswith("cob="):
                resolve(line[4:])  # cob shares ob's name table
            elif line.startswith("calls="):
                after_call = True
    if sum(self_ir.values()) != totals(path):
        raise SystemExit(f"{path}: self costs {sum(self_ir.values())} != totals {totals(path)}")
    return self_ir


def classify(obj: str) -> str:
    if obj and PROVIDER.search(obj):
        return "judy"
    if obj and re.search(r"/libc\.so", obj):
        return "libc"
    if obj and re.search(r"/src/(helpers/)?[^/]+$", obj):
        return "program"
    return "other"


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = sys.argv[1]
    for name in ("tests/summary.txt", "corr/mapped.txt", "corr/stock_vs_stock.txt",
                 "gui/rc.txt", "gui/compare.txt"):
        p = os.path.join(out, name)
        if os.path.exists(p):
            print(f"== {name}")
            print(open(p).read().rstrip())
    comp = open(os.path.join(out, "corr/compare.txt")).read().splitlines()
    n_diff = sum(1 for c in comp if " DIFFER " in c)
    print(f"== corr/compare.txt: {len(comp)} outputs, {n_diff} differ across arms")
    for c in comp:
        if " DIFFER " in c:
            print("   ", c)

    def ratio(x: int, b: int) -> str:
        return f"{x / b:.4f}" if b else "n/a"

    cg = os.path.join(out, "cg")
    names = sorted({f.split(".")[0] for f in os.listdir(cg) if f.endswith(".out")})
    cols = ("total", "program", "judy", "libc", "other")
    print("== Callgrind self Ir by object, and peak RSS (ratio = arm / A)")
    print("   judy = libJudy.so.1.0.3 (A) or libexpanse.so.1 (B, B1); the columns sum to total")
    print(f"{'workload':<18} {'arm':<3} " + " ".join(f"{c + ' Ir':>14} {'ratio':>7}" for c in cols)
          + f" {'maxrss kB':>9} {'ratio':>7}")
    for n in names:
        base = None
        for arm in ARMS:
            p = os.path.join(cg, f"{n}.{arm}.out")
            by = {c: 0 for c in cols}
            for obj, ir in self_ir_by_object(p).items():
                by[classify(obj)] += ir
            by["total"] = totals(p)
            r = int(open(os.path.join(out, "rss", f"{n}.{arm}.maxrss_kb")).read().split()[-1])
            if base is None:
                base = (dict(by), r)
            print(f"{n:<18} {arm:<3} " + " ".join(f"{by[c]:>14,} {ratio(by[c], base[0][c]):>7}" for c in cols)
                  + f" {r:>9,} {ratio(r, base[1]):>7}")


if __name__ == "__main__":
    main()
