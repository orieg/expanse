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

For the arm sets `hj_main`, `hj_leak` and `hj_perf` (README.md, "The hashjudy
cells on main") it writes `results/diagnostic_hj_{main,leak,perf}_<host>_run<N>.json`.
`hj_main` carries N/S, N/E and E/S on the `hashjudy` rates, bogo-ops/s, wall
clock and max-rss, and lists every cell-metric whose N/S verdict differs from
the published E/S verdict. `hj_leak` and `hj_perf` carry N/S, Nf/Sf, N/Nf and
S/Sf, and the leak factor L = (N/S)/(Nf/Sf) with a four-sample BCa 95%
interval (every arm resampled on its own, the jackknife leaving out one process
of one arm at a time); `hj_perf`'s metrics are its whole-process counters.

Usage: diag_delete_analyze.py <host_label> <run1_prefix> <run2_prefix>
"""
import json
import math
import random
import sys
from collections import defaultdict

from analyze import RESULTS, SUITE, run_void, shown, valid, verdict
from analyze import value as rate_value
from bca_bootstrap import CI_METHOD_BCA, _bca_from_distribution, bca_bootstrap_ratio_ci_with_method

MIN_VALID = 8
METRICS = ["judy-delete-operations-per-sec", "judy-insert-operations-per-sec",
           "judy-find-operations-per-sec", "bogo-ops-per-second-real-time", "wall"]
SM_METRICS = ["judy-gets-per-sec", "judy-puts-per-sec", "bogo-ops-per-second-real-time", "wall"]
HJ_METRICS = ["hashjudy-puts-per-sec", "hashjudy-gets-per-sec", "bogo-ops-per-second-real-time",
              "wall", "max-rss"]
HJ_PERF_METRICS = [f"ctr:{e}" for e in ("cpu_core/cycles/u", "cpu_core/instructions/u",
                                        "cpu_core/cycles/k", "cpu_core/instructions/k",
                                        "page-faults", "minor-faults", "major-faults")]
# A metric where a larger value is better; on every other metric smaller is better.
RATES = {"hashjudy-puts-per-sec", "hashjudy-gets-per-sec", "bogo-ops-per-second-real-time",
         "judy-gets-per-sec", "judy-puts-per-sec", "judy-delete-operations-per-sec",
         "judy-insert-operations-per-sec", "judy-find-operations-per-sec"}
HJ_FOUR_PAIRS = [("N", "S"), ("Nf", "Sf"), ("N", "Nf"), ("S", "Sf")]
PAIR_SETS = {"fix": [("D", "B"), ("D", "S"), ("B", "S")],
             "main": [("N", "S"), ("N", "D"), ("D", "S")],
             "sm_main": [("N", "S"), ("N", "E"), ("E", "S")],
             "hj_main": [("N", "S"), ("N", "E"), ("E", "S")],
             "hj_leak": HJ_FOUR_PAIRS, "hj_perf": HJ_FOUR_PAIRS}
SET_METRICS = {"sm_main": SM_METRICS, "hj_main": HJ_METRICS, "hj_leak": HJ_METRICS,
               "hj_perf": HJ_PERF_METRICS}
ARTIFACT = {"fix": "diagnostic_delete", "main": "diagnostic_delete_main", "sm_main": "diagnostic_sm_main",
            "hj_main": "diagnostic_hj_main", "hj_leak": "diagnostic_hj_leak",
            "hj_perf": "diagnostic_hj_perf"}
LEAK = "L=(N/S)/(Nf/Sf)"
GETS = "judy-gets-per-sec"
DELETE = "judy-delete-operations-per-sec"


def value(rec, metric):
    """A process's value of a metric: a `perf stat` count for `ctr:<event>`, else analyze.py's."""
    if metric.startswith("ctr:"):
        return rec["counters"][metric[4:]]
    return rate_value(rec, metric)


def has_value(rec, metric):
    return bool(rec["counters"]) if metric.startswith("ctr:") else bool(rec["metrics"])


def leak_factor_ci(n, s, nf, sf, num_resamples=2000, seed=42):
    """L = (mean N / mean S) / (mean Nf / mean Sf), with a four-sample BCa 95% interval.

    Each arm is resampled on its own, as the two-sample ratio resamples each of
    its arms; the jackknife leaves out one process of one arm at a time.
    Returns (L, lower, upper, ci_method).
    """
    arms = [list(n), list(s), list(nf), list(sf)]
    if any(len(a) < 3 for a in arms):
        raise ValueError("need at least 3 processes in every arm")

    def stat(means):
        mn, ms, mnf, msf = means
        if ms == 0.0 or mnf == 0.0:
            raise ValueError("a zero mean in a denominator; L is undefined")
        return (mn / ms) / (mnf / msf)

    means = [math.fsum(a) / len(a) for a in arms]
    theta = stat(means)
    rng = random.Random(seed)
    boot = []
    for _ in range(num_resamples):
        boot.append(stat([math.fsum(a[rng.randint(0, len(a) - 1)] for _ in a) / len(a)
                          for a in arms]))
    jack = []
    for i, a in enumerate(arms):
        total = math.fsum(a)
        for v in a:
            m = list(means)
            m[i] = (total - v) / (len(a) - 1)
            jack.append(stat(m))
    lo, hi, method = _bca_from_distribution(theta, boot, jack, 0.95)
    return theta, lo, hi, method


def analyse_run(prefix):
    recs = [json.loads(line) for line in open(f"{prefix}.jsonl")]
    prov = json.loads(open(f"{prefix}.provenance.json").read())
    arm_set = prov.get("arm_set", "fix")
    pairs = PAIR_SETS[arm_set]
    metrics = SET_METRICS.get(arm_set, METRICS)
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
                ratio = None
                if len(a) >= MIN_VALID and len(b) >= MIN_VALID:
                    av, bv = [value(r, metric) for r in a], [value(r, metric) for r in b]
                    out.update({f"mean_{num}": math.fsum(av) / len(av),
                                f"mean_{den}": math.fsum(bv) / len(bv)})
                    if math.fsum(bv) != 0.0:
                        ratio, lo, hi, ci_method = bca_bootstrap_ratio_ci_with_method(
                            av, bv, num_resamples=2000, seed=42)
                        out.update({"ratio": ratio, "ci_lower": lo, "ci_upper": hi,
                                    "ci_method": ci_method})
                    else:
                        out["undefined"] = "the denominator arm's mean is zero"
                if ratio is None:
                    out.update(ratio=None, ci_lower=None, ci_upper=None, ci_method=None)
                out["rounds_raw"] = [
                    {"round": r["round"], "arm": r["arm"], "order": r["order"],
                     "value": value(r, metric) if has_value(r, metric) else None, "valid": valid(r),
                     "foreign_busy_cpus": r["load"]["foreign_busy_cpus"], "wall_s": r["wall"],
                     "rc": r["rc"]}
                    for r in sorted(by[(cell, w, num)] + by[(cell, w, den)],
                                    key=lambda r: (r["round"], r["arm"]))]
                cells.append(out)
            if pairs is HJ_FOUR_PAIRS:
                cells.append(leak_cell(by, cell, w, metric))
    void, worst, limit = run_void(prov)
    prov["void"] = {"void": void, "max_load1": worst, "limit": limit}
    return {"provenance": prov, "cells": cells}, recs


def leak_cell(by, cell, w, metric):
    """The leak factor of one cell-metric over the four arms."""
    arms = {a: [r for r in by[(cell, w, a)] if valid(r)] for a in ("N", "S", "Nf", "Sf")}
    out = {"workload_id": cell, "w": w, "metric": metric, "pair": LEAK,
           **{f"n_{a}": len(v) for a, v in arms.items()},
           "discarded": sum(len(by[(cell, w, a)]) - len(v) for a, v in arms.items()),
           "ratio": None, "ci_lower": None, "ci_upper": None, "ci_method": None}
    if all(len(v) >= MIN_VALID for v in arms.values()):
        vals = [[value(r, metric) for r in arms[a]] for a in ("N", "S", "Nf", "Sf")]
        try:
            ratio, lo, hi, ci_method = leak_factor_ci(*vals)
            out.update({"ratio": ratio, "ci_lower": lo, "ci_upper": hi, "ci_method": ci_method})
        except ValueError as e:
            out["undefined"] = str(e)
    out["rounds_raw"] = [
        {"round": r["round"], "arm": r["arm"], "order": r["order"],
         "value": value(r, metric) if has_value(r, metric) else None, "valid": valid(r),
         "foreign_busy_cpus": r["load"]["foreign_busy_cpus"], "wall_s": r["wall"], "rc": r["rc"]}
        for r in sorted(sum((by[(cell, w, a)] for a in ("N", "S", "Nf", "Sf")), []),
                        key=lambda r: (r["round"], r["arm"]))]
    return out


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
    if c["ratio"] is None:
        return c.get("undefined", f"n < {MIN_VALID}")
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


def hj_main_reading(host, keyed):
    """N/S on the hashjudy cells (METHODOLOGY.md §8's verdict), and every cell-metric
    whose N/S verdict differs from the published E/S verdict."""
    cells = sorted({(k[0], k[1]) for k in keyed[0]})
    base = published(f"baseline_{host}", lambda c: c["workload_id"].startswith("sng_sm_hashjudy_"))
    print(f"\nN/S against the published E/S (results/baseline_{host}_run{{1,2}}.json); "
          "the verdict is METHODOLOGY.md §8's on rates and is not read on wall clock or max-rss:")
    print("| cell | W | metric | published E/S run 1 | run 2 | verdict | N/S run 1 | run 2 | verdict | changed |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for cell, w in cells:
        for metric in HJ_METRICS:
            new = [k[(cell, w, metric, "N/S")] for k in keyed]
            old = [b.get((cell, w, metric)) for b in base]
            rate = metric in RATES and all(c["ratio"] is not None for c in new)
            vn = verdict([(c["ci_lower"], c["ci_upper"]) for c in new]) if rate else ""
            if all(old):
                vo = verdict([(c["ci_lower"], c["ci_upper"]) for c in old]) if rate else ""
                ot = (fmt(old[0]), fmt(old[1]))
            else:
                vo, ot = "", ("not published", "")
            print(f"| {cell} | {w} | `{metric}` | {ot[0]} | {ot[1]} | {vo} | "
                  f"{fmt(new[0])} | {fmt(new[1])} | {vn} | {'CHANGED' if rate and vo != vn else ''} |")


def leak_side(metric, cis):
    """Where the leak factor sits over both runs: whether removing the leak narrows the gap."""
    widen = (lambda lo, hi: hi < 1.0) if metric in RATES else (lambda lo, hi: lo > 1.0)
    narrow = (lambda lo, hi: lo > 1.0) if metric in RATES else (lambda lo, hi: hi < 1.0)
    if all(widen(lo, hi) for lo, hi in cis):
        return "LEAK WIDENS THE GAP"
    if all(lo >= 0.95 and hi <= 1.05 for lo, hi in cis):
        return "NO LEAK EFFECT"
    if all(narrow(lo, hi) for lo, hi in cis):
        return "LEAK NARROWS THE GAP"
    return "INCONCLUSIVE"


def leak_reading(keyed, metrics):
    """README.md's floor-versus-leak reading rule, per cell-metric."""
    cells = sorted({(k[0], k[1]) for k in keyed[0]})
    print("\nFloor versus leak: L = (N/S)/(Nf/Sf), and the share of the log gap the leak carries,"
          " ln L / ln(N/S) (derived, per run):")
    print("| cell | W | metric | N/S run 1 | run 2 | Nf/Sf run 1 | run 2 | L run 1 | run 2 | reading | share run 1 | run 2 |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|")
    for cell, w in cells:
        for metric in metrics:
            ns = [k[(cell, w, metric, "N/S")] for k in keyed]
            fs = [k[(cell, w, metric, "Nf/Sf")] for k in keyed]
            lf = [k[(cell, w, metric, LEAK)] for k in keyed]
            if any(c["ratio"] is None for c in ns + fs + lf):
                print(f"| {cell} | {w} | `{metric}` | undefined | | | | | | | | |")
                continue
            reading = leak_side(metric, [(c["ci_lower"], c["ci_upper"]) for c in lf])
            share = [(math.log(lc["ratio"]) / math.log(nc["ratio"])) if nc["ratio"] != 1.0 else None
                     for lc, nc in zip(lf, ns)]
            st = [f"{v:.2f}" if v is not None else "n/a" for v in share]
            print(f"| {cell} | {w} | `{metric}` | {fmt(ns[0])} | {fmt(ns[1])} | {fmt(fs[0])} | "
                  f"{fmt(fs[1])} | {fmt(lf[0])} | {fmt(lf[1])} | {reading} | {st[0]} | {st[1]} |")


def object_share_table(arts):
    """Each run's `perf record` shares of cycles by object (hj_perf), as recorded."""
    for n, art in enumerate(arts, 1):
        shares = art["provenance"].get("object_shares", {})
        print(f"\nrun {n}: share of cycles by object (perf record, user and kernel):")
        for key in sorted(shares):
            rows = ", ".join(f"{r['row']} {r['pct']:.1f}%" for r in shares[key]["by_object"][:6])
            print(f"  {key}: {rows}")


def self_test():
    """Pins leak_factor_ci: its point is the closed form, its interval holds it, and it is 1
    when the leak changes neither arm; leak_side reads the two metric directions."""
    n = [10.0, 11.0, 9.0, 10.5, 9.5, 10.0, 10.2, 9.8]
    s = [2.0, 2.1, 1.9, 2.05, 1.95, 2.0, 2.02, 1.98]
    nf = [4.0, 4.4, 3.6, 4.2, 3.8, 4.0, 4.1, 3.9]
    sf = [2.0, 2.2, 1.8, 2.1, 1.9, 2.0, 2.05, 1.95]
    lf, lo, hi, method = leak_factor_ci(n, s, nf, sf)
    want = (math.fsum(n) / math.fsum(s)) / (math.fsum(nf) / math.fsum(sf))
    assert abs(lf - want) < 1e-12, (lf, want)
    assert lo < lf < hi and method == CI_METHOD_BCA, (lo, lf, hi, method)
    one, lo1, hi1, _ = leak_factor_ci(n, s, n, s)
    assert abs(one - 1.0) < 1e-12 and lo1 < 1.0 < hi1, (one, lo1, hi1)
    try:
        leak_factor_ci(n, [0.0] * 8, nf, sf)
        raise AssertionError("a zero denominator mean must raise")
    except ValueError:
        pass
    assert leak_side("wall", [(2.0, 2.5), (2.1, 2.4)]) == "LEAK WIDENS THE GAP"
    assert leak_side("hashjudy-puts-per-sec", [(0.4, 0.5), (0.4, 0.6)]) == "LEAK WIDENS THE GAP"
    assert leak_side("max-rss", [(0.97, 1.02), (0.99, 1.04)]) == "NO LEAK EFFECT"
    assert leak_side("max-rss", [(0.5, 0.6), (0.5, 0.7)]) == "LEAK NARROWS THE GAP"
    assert leak_side("max-rss", [(0.9, 1.2), (1.1, 1.3)]) == "INCONCLUSIVE"
    print("self-test ok")


def main():
    if sys.argv[1:] == ["--self-test"]:
        self_test()
        return
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
            why = next((c["undefined"] for c in cs if "undefined" in c), f"n < {MIN_VALID}")
            print(f"| {key[0]} | {key[1]} | `{key[2]}` | {key[3]} | {why} | | |")
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
    if sets == {"hj_main"}:
        hj_main_reading(host, keyed)
        return
    if sets & {"hj_leak", "hj_perf"}:
        leak_reading(keyed, SET_METRICS[next(iter(sets))])
        if sets == {"hj_perf"}:
            object_share_table(arts)
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
