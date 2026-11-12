#!/usr/bin/env python3
"""Analysis of the #1679 delete fix (README.md, "Diagnostic: delete fix").

Reads `diag_delete.py`'s raw rows and provenance for two runs on one host and
writes `results/diagnostic_delete_<host>_run<N>.json`: per (cell, W, metric,
pair) the two-sample BCa 95% interval on the pair's ratio of means with its
`ci_method`, and `rounds_raw`, every process of both arms verbatim. It then
prints README.md's reading rule on the delete rate.

For the arm set `main` (the provenance's `arm_set`; README.md, "The #1679
criterion on main") it writes `results/diagnostic_delete_main_<host>_run<N>.json`
with the pairs N/S, N/D and D/S, reads #1679's criterion on N/S, and lists
every cell-metric whose N/S verdict (METHODOLOGY.md §8's rule) differs from the
published E/S verdict in `results/baseline_<host>_run{1,2}.json`, and every
delete cell whose side of stock differs from the published D/S in
`results/diagnostic_delete_<host>_run{1,2}.json`.

For the arm set `sm_main` (README.md, "The sparsematrix cells on main",
#1680) it writes `results/diagnostic_sm_main_<host>_run<N>.json` with the pairs
N/S, N/E and E/S on the `sparsematrix judy` rates, reads #1680's gets
criterion on N/S, and lists every cell-metric whose N/S verdict differs from
the published E/S verdict.

Usage: diag_delete_analyze.py <host_label> <run1_prefix> <run2_prefix>
"""
import json
import math
import sys
from collections import defaultdict

from analyze import RESULTS, SUITE, run_void, shown, valid, value, verdict
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ratio_ci_with_method

MIN_VALID = 8
METRICS = ["judy-delete-operations-per-sec", "judy-insert-operations-per-sec",
           "judy-find-operations-per-sec", "bogo-ops-per-second-real-time", "wall"]
SM_METRICS = ["judy-gets-per-sec", "judy-puts-per-sec", "bogo-ops-per-second-real-time", "wall"]
PAIR_SETS = {"fix": [("D", "B"), ("D", "S"), ("B", "S")],
             "main": [("N", "S"), ("N", "D"), ("D", "S")],
             "sm_main": [("N", "S"), ("N", "E"), ("E", "S")]}
ARTIFACT = {"fix": "diagnostic_delete", "main": "diagnostic_delete_main", "sm_main": "diagnostic_sm_main"}
GETS = "judy-gets-per-sec"
DELETE = "judy-delete-operations-per-sec"


def analyse_run(prefix):
    recs = [json.loads(line) for line in open(f"{prefix}.jsonl")]
    prov = json.loads(open(f"{prefix}.provenance.json").read())
    pairs = PAIR_SETS[prov.get("arm_set", "fix")]
    metrics = SM_METRICS if prov.get("arm_set") == "sm_main" else METRICS
    by = defaultdict(list)
    for r in recs:
        by[(r["cell"], r["w"], r["arm"])].append(r)
    cells = []
    for cell, w in sorted({(r["cell"], r["w"]) for r in recs}):
        for metric in metrics:
            for num, den in pairs:
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


def fmt(c):
    return (f"{c['ratio']:.3f} [{c['ci_lower']:.3f}, {c['ci_upper']:.3f}]"
            + ("" if c["ci_method"] == CI_METHOD_BCA else f" ({c['ci_method']})"))


def published(name, pair_key):
    """The two runs' published cells, keyed by (workload, W, metric)."""
    out = []
    for n in (1, 2):
        art = json.loads((SUITE / "results" / f"{name}_run{n}.json").read_text())
        out.append({(c["workload_id"], c["w"], c["metric"]): c for c in art["cells"]
                    if pair_key(c)})
    return out


def main_set_reading(host, keyed):
    """#1679's criterion on N/S, and every cell that changed side against the published tables."""
    cells = sorted({(k[0], k[1]) for k in keyed[0]})
    print("\n#1679 criterion on main (N/S delete lower bound >= 1.0 in both runs):")
    met = True
    for cell, w in cells:
        ns = [k[(cell, w, DELETE, "N/S")] for k in keyed]
        at = all(c["ci_lower"] is not None and c["ci_lower"] >= 1.0 for c in ns)
        met &= at
        print(f"  {cell} W{w}: N/S {fmt(ns[0])} | {fmt(ns[1])} -> "
              f"{'AT OR ABOVE STOCK' if at else 'BELOW STOCK'}")
    print(f"  #1679 criterion on this host: {'MET' if met else 'NOT MET'}")

    base = published(f"baseline_{host}", lambda c: c["workload_id"].startswith("sng_judy_"))
    print(f"\nN/S verdict against the published E/S verdict (results/baseline_{host}_run{{1,2}}.json):")
    print("| cell | W | metric | published E/S run 1 | run 2 | verdict | N/S run 1 | run 2 | verdict | changed |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for cell, w in cells:
        for metric in METRICS[:3]:
            old = [b[(cell, w, metric)] for b in base]
            new = [k[(cell, w, metric, "N/S")] for k in keyed]
            vo = verdict([(c["ci_lower"], c["ci_upper"]) for c in old])
            vn = verdict([(c["ci_lower"], c["ci_upper"]) for c in new])
            print(f"| {cell} | {w} | `{metric}` | {fmt(old[0])} | {fmt(old[1])} | {vo} | "
                  f"{fmt(new[0])} | {fmt(new[1])} | {vn} | {'CHANGED' if vo != vn else ''} |")

    fix = published(f"diagnostic_delete_{host}",
                    lambda c: c["pair"] == "D/S" and c["metric"] == DELETE)
    print(f"\nN/S delete against the published D/S delete (results/diagnostic_delete_{host}_run{{1,2}}.json):")
    print("| cell | W | published D/S run 1 | run 2 | side | N/S run 1 | run 2 | side | changed |")
    print("|---|---|---|---|---|---|---|---|---|")
    for cell, w in cells:
        old = [f[(cell, w, DELETE)] for f in fix]
        new = [k[(cell, w, DELETE, "N/S")] for k in keyed]
        so = "AT OR ABOVE" if all(c["ci_lower"] >= 1.0 for c in old) else "BELOW"
        sn = "AT OR ABOVE" if all(c["ci_lower"] >= 1.0 for c in new) else "BELOW"
        print(f"| {cell} | {w} | {fmt(old[0])} | {fmt(old[1])} | {so} | {fmt(new[0])} | {fmt(new[1])} | "
              f"{sn} | {'CHANGED' if so != sn else ''} |")


def sm_main_reading(host, keyed):
    """#1680's gets criterion on N/S, and every cell-metric that changed side
    against the published E/S table."""
    cells = sorted({(k[0], k[1]) for k in keyed[0]})
    print("\n#1680 gets criterion on main (N/S verdict EQUIVALENT or FASTER, METHODOLOGY.md §8):")
    met = True
    for cell, w in cells:
        ns = [k[(cell, w, GETS, "N/S")] for k in keyed]
        v = verdict([(c["ci_lower"], c["ci_upper"]) for c in ns])
        ok = v in ("EQUIVALENT", "FASTER")
        met &= ok
        print(f"  {cell} W{w}: N/S {fmt(ns[0])} | {fmt(ns[1])} -> {v} ({'PARITY OR BETTER' if ok else 'BELOW PARITY'})")
    print(f"  #1680 gets criterion on this host: {'MET' if met else 'NOT MET'}")
    base = published(f"baseline_{host}", lambda c: c["workload_id"].startswith("sng_sm_judy_"))
    print(f"\nN/S verdict against the published E/S verdict (results/baseline_{host}_run{{1,2}}.json):")
    print("| cell | W | metric | published E/S run 1 | run 2 | verdict | N/S run 1 | run 2 | verdict | changed |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for cell, w in cells:
        for metric in SM_METRICS[:2]:
            old = [b[(cell, w, metric)] for b in base]
            new = [k[(cell, w, metric, "N/S")] for k in keyed]
            vo = verdict([(c["ci_lower"], c["ci_upper"]) for c in old])
            vn = verdict([(c["ci_lower"], c["ci_upper"]) for c in new])
            print(f"| {cell} | {w} | `{metric}` | {fmt(old[0])} | {fmt(old[1])} | {vo} | "
                  f"{fmt(new[0])} | {fmt(new[1])} | {vn} | {'CHANGED' if vo != vn else ''} |")


def main():
    host, prefixes = sys.argv[1], sys.argv[2:4]
    arts, keyed = [], []
    for n, prefix in enumerate(prefixes, 1):
        art, recs = analyse_run(prefix)
        arm_set = art["provenance"].get("arm_set", "fix")
        name = ARTIFACT[arm_set]
        dest = RESULTS / f"{name}_{host}_run{n}.json"
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
        txt = [fmt(c) for c in cs]
        sd = side([(c["ci_lower"], c["ci_upper"]) for c in cs])
        print(f"| {key[0]} | {key[1]} | `{key[2]}` | {key[3]} | {txt[0]} | {txt[1]} | {sd} |")
    sets = {a["provenance"].get("arm_set", "fix") for a in arts}
    if len(sets) != 1:
        sys.exit("the two runs come from different arm sets")
    if sets == {"main"}:
        main_set_reading(host, keyed)
        return
    if sets == {"sm_main"}:
        sm_main_reading(host, keyed)
        return
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
