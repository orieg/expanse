#!/usr/bin/env python3
"""Analysis of one engine increment on the stress-ng cells (docs/BENCHMARKING.md,
the increment's pre-registration).

Reads `diag_increment.py`'s raw rows and provenance for two runs on one host
and writes `results/diagnostic_increment_<tag>_<host>_run<N>.json`: per (cell,
W, metric, pair) the two-sample BCa 95% interval on the pair's ratio of means
with its `ci_method`, and `rounds_raw`. It then applies the pre-registered rule:

- **Target.** Every `--target cell:W:metric` has its G/M lower bound above 1.0
  in both runs.
- **L1, stock side.** No primary cell-metric whose M/S verdict (METHODOLOGY.md
  §8's rule over both runs) is FASTER or EQUIVALENT has a G/S verdict SLOWER.
- **L2, no slowdown against the base.** No primary cell-metric has its G/M
  upper bound below 0.98 in both runs.

PASS when the target and both bounds hold; otherwise FAIL, naming the cells.

Usage: diag_increment_analyze.py <host> <tag> <run1_prefix> <run2_prefix> [--target cell:W:metric ...]
"""
import json
import math
import sys
from collections import defaultdict

from analyze import PRIMARY, RESULTS, SECONDARY, family, run_void, shown, valid, value, verdict
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ratio_ci_with_method

MIN_VALID = 8
PAIRS = [("G", "S"), ("G", "M"), ("M", "S")]
L2_FLOOR = 0.98


def analyse_run(prefix):
    recs = [json.loads(line) for line in open(f"{prefix}.jsonl")]
    prov = json.loads(open(f"{prefix}.provenance.json").read())
    if prov.get("arm_set") != "increment":
        sys.exit(f"STOP: {prefix} is not an increment run")
    by = defaultdict(list)
    for r in recs:
        by[(r["cell"], r["w"], r["arm"])].append(r)
    cells = []
    for cell, w in sorted({(r["cell"], r["w"]) for r in recs}):
        for metric in PRIMARY[family(cell)] + SECONDARY:
            for num, den in PAIRS:
                a = [r for r in by[(cell, w, num)] if valid(r)]
                b = [r for r in by[(cell, w, den)] if valid(r)]
                out = {"workload_id": cell, "w": w, "metric": metric, "pair": f"{num}/{den}",
                       "primary": metric in PRIMARY[family(cell)],
                       f"n_{num}": len(a), f"n_{den}": len(b),
                       "discarded": len(by[(cell, w, num)]) + len(by[(cell, w, den)]) - len(a) - len(b)}
                if len(a) >= MIN_VALID and len(b) >= MIN_VALID:
                    av, bv = [value(r, metric) for r in a], [value(r, metric) for r in b]
                    ratio, lo, hi, ci_method = bca_bootstrap_ratio_ci_with_method(
                        av, bv, num_resamples=2000, seed=42)
                    out.update({f"mean_{num}": math.fsum(av) / len(av),
                                f"mean_{den}": math.fsum(bv) / len(bv),
                                "ratio": ratio, "ci_lower": lo, "ci_upper": hi, "ci_method": ci_method})
                else:
                    out.update(ratio=None, ci_lower=None, ci_upper=None, ci_method=None)
                out["rounds_raw"] = [
                    {"round": r["round"], "arm": r["arm"], "order": r["order"],
                     "value": value(r, metric) if r["metrics"] else None, "valid": valid(r),
                     "foreign_busy_cpus": r["load"]["foreign_busy_cpus"], "wall_s": r["wall"],
                     "rc": r["rc"]}
                    for r in sorted(by[(cell, w, num)] + by[(cell, w, den)],
                                    key=lambda r: (r["round"], r["arm"]))]
                cells.append(out)
    void, worst, limit = run_void(prov)
    prov["void"] = {"void": void, "max_load1": worst, "limit": limit}
    return {"provenance": prov, "cells": cells}, recs


def fmt(c):
    return (f"{c['ratio']:.3f} [{c['ci_lower']:.3f}, {c['ci_upper']:.3f}]"
            + ("" if c["ci_method"] == CI_METHOD_BCA else f" ({c['ci_method']})"))


def main():
    args = sys.argv[1:]
    targets = []
    if "--target" in args:
        i = args.index("--target")
        targets = [tuple(t.split(":")) for t in args[i + 1:]]
        args = args[:i]
    host, tag, prefixes = args[0], args[1], args[2:4]
    arts, keyed = [], []
    for n, prefix in enumerate(prefixes, 1):
        art, recs = analyse_run(prefix)
        dest = RESULTS / f"diagnostic_increment_{tag}_{host}_run{n}.json"
        dest.write_text(json.dumps(art, indent=1) + "\n")
        v = art["provenance"]["void"]
        print(f"run {n}: {len(recs)} processes, {sum(not valid(r) for r in recs)} discarded; "
              f"max load1 {v['max_load1']} (void above {v['limit']}): "
              f"{'VOID' if v['void'] else 'valid'} -> {shown(dest)}")
        arts.append(art)
        keyed.append({(c["workload_id"], c["w"], c["metric"], c["pair"]): c for c in art["cells"]})
    if any(a["provenance"]["void"]["void"] for a in arts):
        sys.exit("a run is void (METHODOLOGY.md §7); no reading")
    if {json.dumps(a["provenance"]["increment"], sort_keys=True) for a in arts} != \
            {json.dumps(arts[0]["provenance"]["increment"], sort_keys=True)}:
        sys.exit("STOP: the two runs measured different increments")

    def pair(cell, w, metric, p):
        cs = [k.get((cell, w, metric, p)) for k in keyed]
        if any(c is None or c["ratio"] is None for c in cs):
            return None
        return cs

    rows = sorted({(k[0], k[1], k[2]) for k in keyed[0] if keyed[0][k]["primary"]})
    print("\n| cell | W | metric | G/M run 1 | G/M run 2 | G/S verdict | M/S verdict |")
    print("|---|---|---|---|---|---|---|")
    failures, missing = [], []
    for cell, w, metric in rows:
        gm, gs, ms = (pair(cell, w, metric, p) for p in ("G/M", "G/S", "M/S"))
        if gm is None or gs is None or ms is None:
            missing.append((cell, w, metric))
            print(f"| {cell} | {w} | `{metric}` | n < {MIN_VALID} | | | |")
            continue
        vgs = verdict([(c["ci_lower"], c["ci_upper"]) for c in gs])
        vms = verdict([(c["ci_lower"], c["ci_upper"]) for c in ms])
        print(f"| {cell} | {w} | `{metric}` | {fmt(gm[0])} | {fmt(gm[1])} | {vgs} | {vms} |")
        if vms in ("FASTER", "EQUIVALENT") and vgs == "SLOWER":
            failures.append(f"L1: {cell} W{w} {metric}: M/S {vms}, G/S SLOWER")
        if all(c["ci_upper"] < L2_FLOOR for c in gm):
            failures.append(f"L2: {cell} W{w} {metric}: G/M upper bound below {L2_FLOOR} in both runs")
    for cell, w, metric in targets:
        gm = pair(cell, int(w), metric, "G/M")
        if gm is None:
            failures.append(f"target {cell} W{w} {metric}: no interval")
        elif not all(c["ci_lower"] > 1.0 for c in gm):
            failures.append(f"target {cell} W{w} {metric}: G/M {fmt(gm[0])} | {fmt(gm[1])}, "
                            "lower bound not above 1.0 in both runs")
        else:
            print(f"target met: {cell} W{w} {metric}: G/M {fmt(gm[0])} | {fmt(gm[1])}")
    if missing:
        failures.append(f"{len(missing)} primary cell-metric(s) without an interval: {missing}")
    print("\nverdict:", "PASS" if not failures else "FAIL")
    for f in failures:
        print("  " + f)


if __name__ == "__main__":
    main()
