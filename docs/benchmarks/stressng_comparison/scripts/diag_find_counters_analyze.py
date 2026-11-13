#!/usr/bin/env python3
"""Analysis of the front-end counters on the 1k find cell (README.md, "Diagnostic: counters").

Reads `diag_find_counters.py`'s raw rows and provenance for two runs on one
host and writes `results/diagnostic_find_counters_<host>_run<N>.json`: per
(counter, pair) the two-sample BCa 95% interval on the pair's ratio of means
with its `ci_method`, and `rounds_raw`, every process verbatim. `cycles`,
`instructions` and the find rate are read from every process; each other
counter from the processes of its own event group. It then prints README.md's
reading rule.

Usage: diag_find_counters_analyze.py <host_label> <run1_prefix> <run2_prefix>
"""
import json
import math
import sys
from collections import defaultdict

from analyze import RESULTS, run_void, shown, valid
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ratio_ci_with_method

MIN_VALID = 8
PAIRS = [("O", "N"), ("N", "S"), ("O", "S")]
FIND = "judy-find-operations-per-sec"
FE = "idq_uops_not_delivered.core"
POOLED = ["cycles", "instructions", FIND]


def series(recs, key):
    """Valid processes carrying `key`, as (record, value)."""
    out = []
    for r in recs:
        if not valid(r):
            continue
        if key == FIND:
            out.append((r, r["metrics"][key]))
        elif key in r["counters"]:
            out.append((r, r["counters"][key]))
    return out


def analyse_run(prefix):
    recs = [json.loads(line) for line in open(f"{prefix}.jsonl")]
    prov = json.loads(open(f"{prefix}.provenance.json").read())
    keys = POOLED + sorted({k for r in recs for k in r["counters"]} - set(POOLED))
    by_arm = defaultdict(list)
    for r in recs:
        by_arm[r["arm"]].append(r)
    cells = []
    for key in keys:
        for num, den in PAIRS:
            a, b = series(by_arm[num], key), series(by_arm[den], key)
            out = {"workload_id": recs[0]["cell"], "w": recs[0]["w"], "metric": key,
                   "pair": f"{num}/{den}", f"n_{num}": len(a), f"n_{den}": len(b),
                   "discarded": sum(not valid(r) for r in by_arm[num] + by_arm[den])}
            if len(a) >= MIN_VALID and len(b) >= MIN_VALID:
                av, bv = [v for _, v in a], [v for _, v in b]
                ratio, lo, hi, ci_method = bca_bootstrap_ratio_ci_with_method(
                    av, bv, num_resamples=2000, seed=42)
                out.update({f"mean_{num}": math.fsum(av) / len(av),
                            f"mean_{den}": math.fsum(bv) / len(bv),
                            "ratio": ratio, "ci_lower": lo, "ci_upper": hi, "ci_method": ci_method})
            else:
                out.update(ratio=None, ci_lower=None, ci_upper=None, ci_method=None)
            out["rounds_raw"] = [
                {"round": r["round"], "group": r["group"], "arm": r["arm"], "order": r["order"],
                 "value": v, "foreign_busy_cpus": r["load"]["foreign_busy_cpus"], "wall_s": r["wall"]}
                for r, v in sorted(a + b, key=lambda x: (x[0]["round"], x[0]["group"], x[0]["arm"]))]
            cells.append(out)
    void, worst, limit = run_void(prov)
    prov["void"] = {"void": void, "max_load1": worst, "limit": limit}
    return {"provenance": prov, "cells": cells}, recs


def fe_share(cell_by):
    """README.md's f: the share of N's extra issue slots over O's that the front end left empty."""
    cyc = cell_by[("cycles", "O/N")]
    slots = cell_by[("slots", "O/N")]
    fe = cell_by[(FE, "O/N")]
    width = slots["mean_N"] / cyc["mean_N"]
    d_slots = width * (cyc["mean_N"] - cyc["mean_O"])
    return (fe["mean_N"] - fe["mean_O"]) / d_slots, width


def main():
    host, prefixes = sys.argv[1], sys.argv[2:4]
    arts, keyed = [], []
    for n, prefix in enumerate(prefixes, 1):
        art, recs = analyse_run(prefix)
        dest = RESULTS / f"diagnostic_find_counters_{host}_run{n}.json"
        dest.write_text(json.dumps(art, indent=1) + "\n")
        v = art["provenance"]["void"]
        print(f"run {n}: {len(recs)} processes, {sum(not valid(r) for r in recs)} discarded; "
              f"max load1 {v['max_load1']} (void above {v['limit']}): "
              f"{'VOID' if v['void'] else 'valid'} -> {shown(dest)}")
        arts.append(art)
        keyed.append({(c["metric"], c["pair"]): c for c in art["cells"]})
    if any(a["provenance"]["void"]["void"] for a in arts):
        sys.exit("a run is void (METHODOLOGY.md §7); no reading")
    print("\n| counter | pair | mean num | mean den | run 1 [BCa 95%] | run 2 [BCa 95%] |")
    print("|---|---|---|---|---|---|")
    for key in keyed[0]:
        cs = [k[key] for k in keyed]
        if any(c["ratio"] is None for c in cs):
            print(f"| `{key[0]}` | {key[1]} | n < {MIN_VALID} | | | |")
            continue
        num, den = key[1].split("/")
        txt = [f"{c['ratio']:.4f} [{c['ci_lower']:.4f}, {c['ci_upper']:.4f}]"
               + ("" if c["ci_method"] == CI_METHOD_BCA else f" ({c['ci_method']})") for c in cs]
        print(f"| `{key[0]}` | {key[1]} | {cs[0]['mean_' + num]:.4g} | {cs[0]['mean_' + den]:.4g} "
              f"| {txt[0]} | {txt[1]} |")
    print("\nreading rule (README.md), O/N:")
    ins = [k[("instructions", "O/N")] for k in keyed]
    cyc = [k[("cycles", "O/N")] for k in keyed]
    r0 = all(0.995 <= c["ci_lower"] and c["ci_upper"] <= 1.005 for c in ins)
    r1 = all(c["ci_upper"] < 1.0 for c in cyc)
    print(f"  R0 same work (instructions inside [0.995, 1.005] in both runs): {'MET' if r0 else 'NOT MET'}")
    print(f"  R1 cost reproduced (cycles upper < 1 in both runs): {'MET' if r1 else 'NOT MET'}")
    if not (r0 and r1):
        print("  no front-end reading: a precondition is not met")
        return
    shares = [fe_share(k) for k in keyed]
    for n, (f, width) in enumerate(shares, 1):
        print(f"  run {n}: f = {f:.3f} (measured width {width:.3f} slots per cycle)")
    if all(f >= 0.5 for f, _ in shares):
        verdict = "FRONT-END"
    elif all(f <= 0.2 for f, _ in shares):
        verdict = "NOT FRONT-END"
    else:
        verdict = "PARTIAL"
    print(f"  reading: {verdict}")
    print("\nsymbol shares of user cycles, one perf record per arm per run (n = 1 each):")
    for n, a in enumerate(arts, 1):
        for arm, rows in a["provenance"]["symbol_shares"].items():
            top = ", ".join(f"{r['symbol']} [{r['dso']}] {r['pct']:.1f}%" for r in rows[:6])
            print(f"  run {n} {arm}: {top}")


if __name__ == "__main__":
    main()
