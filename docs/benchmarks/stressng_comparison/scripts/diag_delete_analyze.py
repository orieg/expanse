#!/usr/bin/env python3
"""Analysis of the #1679 delete fix (README.md, "Diagnostic: delete fix").

Reads `diag_delete.py`'s raw rows and provenance for two runs on one host and
writes `results/diagnostic_delete_<host>_run<N>.json`: per (cell, W, metric,
pair) the two-sample BCa 95% interval on the pair's ratio of means with its
`ci_method`, and `rounds_raw`, every process of both arms verbatim. It then
prints README.md's reading rule on the delete rate.

Usage: diag_delete_analyze.py <host_label> <run1_prefix> <run2_prefix>
"""
import json
import math
import sys
from collections import defaultdict

from analyze import RESULTS, run_void, shown, valid, value
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ratio_ci_with_method

MIN_VALID = 8
METRICS = ["judy-delete-operations-per-sec", "judy-insert-operations-per-sec",
           "judy-find-operations-per-sec", "bogo-ops-per-second-real-time", "wall"]
PAIRS = [("D", "B"), ("D", "S"), ("B", "S")]
DELETE = "judy-delete-operations-per-sec"


def analyse_run(prefix):
    recs = [json.loads(line) for line in open(f"{prefix}.jsonl")]
    prov = json.loads(open(f"{prefix}.provenance.json").read())
    by = defaultdict(list)
    for r in recs:
        by[(r["cell"], r["w"], r["arm"])].append(r)
    cells = []
    for cell, w in sorted({(r["cell"], r["w"]) for r in recs}):
        for metric in METRICS:
            for num, den in PAIRS:
                a = [r for r in by[(cell, w, num)] if valid(r)]
                b = [r for r in by[(cell, w, den)] if valid(r)]
                out = {"workload_id": cell, "w": w, "metric": metric, "pair": f"{num}/{den}",
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


def side(cis):
    """Where a pair's intervals sit over both runs."""
    if all(lo >= 0.98 and hi <= 1.02 for lo, hi in cis):
        return "SAME"
    if all(lo > 1.0 for lo, _ in cis):
        return "FASTER"
    if all(hi < 1.0 for _, hi in cis):
        return "SLOWER"
    return "MIXED"


def main():
    host, prefixes = sys.argv[1], sys.argv[2:4]
    arts, keyed = [], []
    for n, prefix in enumerate(prefixes, 1):
        art, recs = analyse_run(prefix)
        dest = RESULTS / f"diagnostic_delete_{host}_run{n}.json"
        dest.write_text(json.dumps(art, indent=1) + "\n")
        v = art["provenance"]["void"]
        print(f"run {n}: {len(recs)} processes, {sum(not valid(r) for r in recs)} discarded; "
              f"max load1 {v['max_load1']} (void above {v['limit']}): "
              f"{'VOID' if v['void'] else 'valid'} -> {shown(dest)}")
        arts.append(art)
        keyed.append({(c["workload_id"], c["w"], c["metric"], c["pair"]): c for c in art["cells"]})
    if any(a["provenance"]["void"]["void"] for a in arts):
        sys.exit("a run is void (METHODOLOGY.md §7); no reading")
    print("\n| cell | W | metric | pair | run 1 [BCa 95%] | run 2 [BCa 95%] | side |")
    print("|---|---|---|---|---|---|---|")
    for key in sorted(keyed[0]):
        cs = [k[key] for k in keyed]
        if any(c["ratio"] is None for c in cs):
            print(f"| {key[0]} | {key[1]} | `{key[2]}` | {key[3]} | n < {MIN_VALID} | | |")
            continue
        txt = [f"{c['ratio']:.3f} [{c['ci_lower']:.3f}, {c['ci_upper']:.3f}]"
               + ("" if c["ci_method"] == CI_METHOD_BCA else f" ({c['ci_method']})") for c in cs]
        sd = side([(c["ci_lower"], c["ci_upper"]) for c in cs])
        print(f"| {key[0]} | {key[1]} | `{key[2]}` | {key[3]} | {txt[0]} | {txt[1]} | {sd} |")
    print("\nreading rule (README.md), delete rate:")
    met = True
    for cell, w in sorted({(k[0], k[1]) for k in keyed[0]}):
        ds = [k[(cell, w, DELETE, "D/S")] for k in keyed]
        db = [k[(cell, w, DELETE, "D/B")] for k in keyed]
        at = all(c["ci_lower"] is not None and c["ci_lower"] >= 1.0 for c in ds)
        met &= at
        print(f"  {cell} W{w}: D/S {'AT OR ABOVE STOCK' if at else 'BELOW STOCK'}, "
              f"D/B {side([(c['ci_lower'], c['ci_upper']) for c in db])}")
    print(f"  #1679 criterion on this host: {'MET' if met else 'NOT MET'}")


if __name__ == "__main__":
    main()
