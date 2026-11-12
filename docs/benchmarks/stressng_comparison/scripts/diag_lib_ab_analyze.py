#!/usr/bin/env python3
"""Analysis of the old-vs-new libexpanse A/B (README.md, "Diagnostic").

Reads `diag_lib_ab.py`'s raw rows and provenance for two runs on one host and
writes `results/diagnostic_lib_ab_<host>_run<N>.json`: per (W, metric, pair)
the two-sample BCa 95% interval on the pair's ratio of means with its
`ci_method`, and `rounds_raw`, every process of both arms verbatim. It then
prints the README's reading rule applied to O/N on the find rate.

Usage: diag_lib_ab_analyze.py <host_label> <run1_prefix> <run2_prefix>
"""
import json
import math
import sys
from collections import defaultdict

from analyze import RESULTS, run_void, shown, valid, value
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ratio_ci_with_method

MIN_VALID = 8
METRICS = ["judy-find-operations-per-sec", "judy-insert-operations-per-sec",
           "judy-delete-operations-per-sec", "bogo-ops-per-second-real-time", "wall"]
PAIRS = [("O", "N"), ("N", "S"), ("O", "S")]
FIND = "judy-find-operations-per-sec"


def analyse_run(prefix):
    recs = [json.loads(line) for line in open(f"{prefix}.jsonl")]
    prov = json.loads(open(f"{prefix}.provenance.json").read())
    by = defaultdict(list)
    for r in recs:
        by[(r["w"], r["arm"])].append(r)
    cells = []
    for w in sorted({r["w"] for r in recs}):
        for metric in METRICS:
            for num, den in PAIRS:
                a = [r for r in by[(w, num)] if valid(r)]
                b = [r for r in by[(w, den)] if valid(r)]
                out = {"workload_id": recs[0]["cell"], "w": w, "metric": metric,
                       "pair": f"{num}/{den}", f"n_{num}": len(a), f"n_{den}": len(b),
                       "discarded": len(by[(w, num)]) + len(by[(w, den)]) - len(a) - len(b)}
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
                    for r in sorted(by[(w, num)] + by[(w, den)], key=lambda r: (r["round"], r["arm"]))]
                cells.append(out)
    void, worst, limit = run_void(prov)
    prov["void"] = {"void": void, "max_load1": worst, "limit": limit}
    return {"provenance": prov, "cells": cells}, recs


def reading(cis):
    """README.md's rule for O/N on the find rate at one W, over both runs."""
    if all(lo > 1.0 for lo, _ in cis):
        return "O FASTER"
    if all(lo >= 0.98 and hi <= 1.02 for lo, hi in cis):
        return "SAME"
    if all(hi < 1.0 for _, hi in cis):
        return "O SLOWER"
    return "MIXED"


def main():
    host, prefixes = sys.argv[1], sys.argv[2:4]
    arts = []
    for n, prefix in enumerate(prefixes, 1):
        art, recs = analyse_run(prefix)
        dest = RESULTS / f"diagnostic_lib_ab_{host}_run{n}.json"
        dest.write_text(json.dumps(art, indent=1) + "\n")
        v = art["provenance"]["void"]
        print(f"run {n}: {len(recs)} processes, {sum(not valid(r) for r in recs)} discarded; "
              f"max load1 {v['max_load1']} (void above {v['limit']}): "
              f"{'VOID' if v['void'] else 'valid'} -> {shown(dest)}")
        arts.append(art)
    if any(a["provenance"]["void"]["void"] for a in arts):
        sys.exit("a run is void (METHODOLOGY.md §7); no reading")
    keyed = [{(c["w"], c["metric"], c["pair"]): c for c in a["cells"]} for a in arts]
    print("\n| W | metric | pair | run 1 [BCa 95%] | run 2 [BCa 95%] | reading |")
    print("|---|---|---|---|---|---|")
    for key in sorted(keyed[0]):
        cs = [k[key] for k in keyed]
        if any(c["ratio"] is None for c in cs):
            print(f"| {key[0]} | `{key[1]}` | {key[2]} | n < {MIN_VALID} | | |")
            continue
        txt = [f"{c['ratio']:.3f} [{c['ci_lower']:.3f}, {c['ci_upper']:.3f}]"
               + ("" if c["ci_method"] == CI_METHOD_BCA else f" ({c['ci_method']})") for c in cs]
        rd = reading([(c["ci_lower"], c["ci_upper"]) for c in cs]) if key[2] == "O/N" else ""
        print(f"| {key[0]} | `{key[1]}` | {key[2]} | {txt[0]} | {txt[1]} | {rd} |")
    print("\nper-arm mean rates, both runs pooled (M ops/s):")
    for w in sorted({k[0] for k in keyed[0]}):
        rates = defaultdict(list)
        for a in arts:
            for c in a["cells"]:
                if c["w"] == w and c["metric"] == FIND:
                    for r in c["rounds_raw"]:
                        if r["valid"]:
                            rates[r["arm"]].append(r["value"])
        # Each process appears in two pairs, so every value is counted twice
        # and the mean is unchanged.
        means = {arm: math.fsum(v) / len(v) / 1e6 for arm, v in rates.items()}
        print(f"  W{w} find: " + ", ".join(f"{arm} {means[arm]:.2f}" for arm in sorted(means)))


if __name__ == "__main__":
    main()
