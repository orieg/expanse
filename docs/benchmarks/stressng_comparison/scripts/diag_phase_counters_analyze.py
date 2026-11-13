#!/usr/bin/env python3
"""Analysis of the phase counters (README.md, "Diagnostic: phase counters").

Reads `diag_phase_counters.py`'s raw rows and provenance for two runs on one
host and writes `results/diagnostic_phase_counters_<host>_run<N>.json`: per
(shape, counter, pair) the two-sample BCa 95% interval on the pair's ratio of
means with its `ci_method`, the per-operation means, and `rounds_raw`, every
process verbatim. `cycles` and `instructions` pool every process of an arm;
each other counter uses its own group's processes. It then prints README.md's
reading rule per shape.

Usage: diag_phase_counters_analyze.py <host_label> <run1_prefix> <run2_prefix>
"""
import json
import math
import sys
from collections import defaultdict

from analyze import RESULTS, run_void, shown, valid
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ratio_ci_with_method

MIN_VALID = 8
# The gets set's pairs; the delete set (`phase_set` "delete", #1679) has one.
PAIRS_BY_SET = {"gets": [("N", "S"), ("N64", "S"), ("N64", "N")], "delete": [("D", "S")],
                "delete_fe": [("D", "S")], "delete_ms": [("D", "S")],
                "delete_acct": [("A", "D"), ("A", "S"), ("D", "S")],
                "promote2": [("P", "M"), ("P", "S"), ("M", "S")],
                "big_main": [("N", "S"), ("N", "E"), ("E", "S")]}
NAME_BY_SET = {"gets": "phase_counters", "delete": "delete_phase", "delete_fe": "delete_fe",
               "delete_ms": "delete_ms", "delete_acct": "delete_acct",
               "promote2": "promote2", "big_main": "big_main"}
# The delete_acct set's rule (README.md, "Diagnostic: unlocked accounting on
# the delete phase"): the share of D's cycle excess over S that A removes, at
# which the locked counters are said to matter.
ACCT_SHARE = 0.25
# The delete_ms set's rule (README.md, "Diagnostic: taken branches and
# microcode on the delete phase"). MS_SWITCH_COST is the cycles charged per
# switch to the microcode sequencer, the constant of Intel's top-down
# microarchitecture analysis model; TB_DENSITY is the taken-branch density
# ratio a TAKEN-BRANCH reading also needs.
MS_SWITCH_COST, TB_DENSITY = 3, 1.10
# The delete_fe set's fetch-latency counters (README.md, "Diagnostic:
# front-end resource on the delete phase"), each in cycles, by the resource
# it names.
LATENCY = {"UOP-CACHE": "dsb2mite_switches.penalty_cycles", "ICACHE": "icache_data.stalls",
           "ITLB": "icache_tag.stalls", "LCP": "decode.lcp"}
SOURCES = ["idq.dsb_uops", "idq.mite_uops", "idq.ms_uops", "lsd.uops"]
NAMED_SHARE, SOURCE_SHIFT = 0.25, 0.10
PAIRS = PAIRS_BY_SET["gets"]
POOLED = ["cycles", "instructions"]
MEM = "cycle_activity.stalls_l1d_miss"
FE = "idq_uops_not_delivered.core"


def analyse_run(prefix):
    global PAIRS
    recs = [json.loads(line) for line in open(f"{prefix}.jsonl")]
    prov = json.loads(open(f"{prefix}.provenance.json").read())
    PAIRS = PAIRS_BY_SET[prov.get("phase_set", "gets")]
    by = defaultdict(list)
    for r in recs:
        by[(r["cell"], r["arm"])].append(r)
    cells = []
    for shape in sorted({r["cell"] for r in recs}):
        keys = POOLED + sorted({k for r in recs if r["cell"] == shape for k in r["counters"]} - set(POOLED))
        for key in keys:
            for num, den in PAIRS:
                a = [r for r in by[(shape, num)] if valid(r) and key in r["counters"]]
                b = [r for r in by[(shape, den)] if valid(r) and key in r["counters"]]
                out = {"workload_id": f"phase_{shape}", "metric": key, "pair": f"{num}/{den}",
                       f"n_{num}": len(a), f"n_{den}": len(b),
                       "discarded": sum(not valid(r) for r in by[(shape, num)] + by[(shape, den)])}
                av, bv = [r["counters"][key] for r in a], [r["counters"][key] for r in b]
                if any(x == 0 for x in bv) or any(x == 0 for x in av):
                    # A zero count has no ratio: an event a shape never
                    # triggers (an L3 miss on a cache-resident array), reported
                    # by its means and never bootstrapped.
                    out.update({f"mean_{num}": math.fsum(av) / max(len(av), 1),
                                f"mean_{den}": math.fsum(bv) / max(len(bv), 1),
                                "ratio": None, "ci_lower": None, "ci_upper": None,
                                "ci_method": None, "no_ratio": "a zero count in an arm"})
                elif len(a) >= MIN_VALID and len(b) >= MIN_VALID:
                    ratio, lo, hi, ci_method = bca_bootstrap_ratio_ci_with_method(
                        av, bv, num_resamples=2000, seed=42)
                    ops = a[0]["ops"]
                    out.update({f"mean_{num}": math.fsum(av) / len(av), f"mean_{den}": math.fsum(bv) / len(bv),
                                f"per_op_{num}": math.fsum(av) / len(av) / ops,
                                f"per_op_{den}": math.fsum(bv) / len(bv) / ops,
                                "ratio": ratio, "ci_lower": lo, "ci_upper": hi, "ci_method": ci_method})
                else:
                    out.update(ratio=None, ci_lower=None, ci_upper=None, ci_method=None)
                out["rounds_raw"] = [
                    {"round": r["round"], "group": r["group"], "arm": r["arm"], "order": r["order"],
                     "value": r["counters"][key], "foreign_busy_cpus": r["load"]["foreign_busy_cpus"],
                     "wall_s": r["wall"]}
                    for r in sorted(a + b, key=lambda r: (r["round"], r["group"], r["arm"]))]
                cells.append(out)
    void, worst, limit = run_void(prov)
    prov["void"] = {"void": void, "max_load1": worst, "limit": limit}
    return {"provenance": prov, "cells": cells}, recs


def shares(k, shape, num="N", den="S"):
    """README.md's shares of the num-over-den cycle difference, from the arms' means."""
    cyc, slots = k[(shape, "cycles", f"{num}/{den}")], k[(shape, "slots", f"{num}/{den}")]
    mem, fe = k[(shape, MEM, f"{num}/{den}")], k[(shape, FE, f"{num}/{den}")]
    d = cyc[f"mean_{num}"] - cyc[f"mean_{den}"]
    width = slots[f"mean_{num}"] / cyc[f"mean_{num}"]
    return d, (mem[f"mean_{num}"] - mem[f"mean_{den}"]) / d, (fe[f"mean_{num}"] - fe[f"mean_{den}"]) / (width * d)


def mean(k, shape, metric, arm, pair):
    return k[(shape, metric, pair)][f"mean_{arm}"]


def fe_reading(keyed, shape, num, den="S"):
    """README.md's front-end resource rule for one shape, over both runs."""
    pair, named = f"{num}/{den}", []
    per_run = []
    for k in keyed:
        d = mean(k, shape, "cycles", num, pair) - mean(k, shape, "cycles", den, pair)
        width = mean(k, shape, "slots", num, pair) / mean(k, shape, "cycles", num, pair)
        f = (mean(k, shape, FE, num, pair) - mean(k, shape, FE, den, pair)) / (width * d)
        lat = {r: (mean(k, shape, e, num, pair) - mean(k, shape, e, den, pair)) / d for r, e in LATENCY.items()}
        mix = {}
        for arm in (num, den):
            tot = math.fsum(mean(k, shape, e, arm, pair) for e in SOURCES)
            mix[arm] = {e: mean(k, shape, e, arm, pair) / tot for e in SOURCES}
        per_run.append((d, f, lat, mix))
    for n, (d, f, lat, mix) in enumerate(per_run, 1):
        print(f"  {shape} run {n}: {num} - S = {d:.4g} cycles; front-end share {f:.3f}; shares of the extra "
              "cycles: " + ", ".join(f"{r} {v:.3f}" for r, v in lat.items()))
        for arm in (num, den):
            print(f"    {arm} uop sources: " + ", ".join(f"{e} {v:.3f}" for e, v in mix[arm].items()))
    for r in LATENCY:
        if all(lat[r] >= NAMED_SHARE for _, _, lat, _ in per_run):
            named.append(r)
    mite = [mix[num]["idq.mite_uops"] - mix[den]["idq.mite_uops"] for _, _, _, mix in per_run]
    if all(m >= SOURCE_SHIFT for m in mite) and "UOP-CACHE" not in named:
        named.append("UOP-CACHE")
    lsd = [mix[num]["lsd.uops"] - mix[den]["lsd.uops"] for _, _, _, mix in per_run]
    if all(x <= -SOURCE_SHIFT for x in lsd):
        named.append("LOOP-STREAM")
    print(f"  {shape}: {' + '.join(named) if named else 'UNNAMED (no measured front-end resource carries the gap)'}")
    go = any(r in named for r in ("UOP-CACHE", "ICACHE"))
    print(f"  {shape}: option 2 (outline as a code-footprint change) "
          f"{'may proceed' if go else 'does not proceed on this shape: report'}")


def ms_reading(keyed, shape, num, den="S"):
    """README.md's taken-branch and microcode rule for one shape, over both runs."""
    pair, per_run = f"{num}/{den}", []
    for k in keyed:
        def dm(e):
            return mean(k, shape, e, num, pair) - mean(k, shape, e, den, pair)
        d = dm("cycles")
        width = mean(k, shape, "slots", num, pair) / mean(k, shape, "cycles", num, pair)
        f = dm(FE) / (width * d)
        s_ms = MS_SWITCH_COST * dm("idq.ms_switches") / d
        s_ms_cyc = dm("idq.ms_cycles_any") / d
        s_tb = (dm("idq.dsb_cycles_any") - dm("idq.dsb_cycles_ok")) / d
        dens = {a: mean(k, shape, "br_inst_retired.near_taken", a, pair)
                / mean(k, shape, "idq.dsb_uops", a, pair) for a in (num, den)}
        per_run.append((d, f, s_ms, s_ms_cyc, s_tb, dens[num] / dens[den]))
    for n, (d, f, s_ms, s_ms_cyc, s_tb, rho) in enumerate(per_run, 1):
        print(f"  {shape} run {n}: {num} - S = {d:.4g} cycles; front-end share {f:.3f}; "
              f"microcode switch share {s_ms:.3f} (MS-active cycles {s_ms_cyc:.3f}); "
              f"DSB-below-width share {s_tb:.3f}; taken-branch density {num}/S {rho:.3f}")
    named = []
    if all(r[2] >= NAMED_SHARE for r in per_run):
        named.append("MICROCODE")
    if all(r[4] >= NAMED_SHARE and r[5] >= TB_DENSITY for r in per_run):
        named.append("TAKEN-BRANCH")
    verdict = {0: "UNNAMED", 2: "BOTH (MICROCODE + TAKEN-BRANCH)"}.get(len(named), named[0] if named else "")
    print(f"  {shape}: {verdict}")


def side(c):
    """Which side of 1 a ratio's interval lies on: below, above, or across."""
    return "below" if c["ci_upper"] < 1.0 else "above" if c["ci_lower"] > 1.0 else "across"


def acct_reading(keyed, shape):
    """README.md's accounting-ablation rule for one shape, over both runs."""
    ad = [k[(shape, "cycles", "A/D")] for k in keyed]
    as_ = [k[(shape, "cycles", "A/S")] for k in keyed]
    ds = [k[(shape, "cycles", "D/S")] for k in keyed]
    wall = [k[(shape, "phase_ns", "A/D")] for k in keyed]
    if any(c["ci_lower"] is None for c in ad + as_ + ds + wall):
        print(f"  {shape}: NO READING (a run has too few valid processes for an interval)")
        return
    closed = []
    for n, (c, d) in enumerate(zip(ad, ds), 1):
        gap = d["mean_D"] - d["mean_S"]
        share = (c["mean_D"] - c["mean_A"]) / gap if gap > 0 else None
        closed.append(share)
        print(f"  {shape} run {n}: A/D cycles {c['ratio']:.4f} [{c['ci_lower']:.4f}, {c['ci_upper']:.4f}], "
              f"A/D phase wall {wall[n-1]['ratio']:.4f}; D - S = {gap:.4g} cycles; share of it A removes "
              + ("n/a (D is not above S)" if share is None else f"{share:.3f}"))
    sides = [side(c) for c in ad]
    if all(x == "below" for x in sides):
        verdict = ("MATTER" if all(x is not None and x >= ACCT_SHARE for x in closed)
                   else "SMALL (A is faster than D, but removes less than "
                   f"{ACCT_SHARE:.2f} of D's excess over S in a run)")
    elif all(x == "above" for x in sides):
        verdict = "SLOWER (A takes more cycles than D)"
    elif all(x == "across" for x in sides):
        verdict = "NO EFFECT (A/D's interval contains 1 in both runs)"
    else:
        verdict = "MIXED (the runs disagree on A/D's side of 1)"
    if [side(c) for c in wall] != sides:
        verdict += "; WALL DISAGREES (A/D phase wall clock lies on another side of 1 in a run)"
    print(f"  {shape}: {verdict}")
    a_s = [side(c) for c in as_]
    print(f"  {shape}: A against stock: " + ("BELOW STOCK (fewer cycles than S in both runs)" if all(x == "below" for x in a_s)
          else "ABOVE STOCK" if all(x == "above" for x in a_s) else "NOT SEPARATED FROM STOCK"))


# The promote2 rule (README.md, "Promotion, judged on timing"): the two
# delete shapes must be FASTER, and every other shape may not lose more than
# NO_LOSS_TOL, on both the phase wall clock and cycles, in both runs.
P2_TARGETS = ("del_1k", "del_4m")
NO_LOSS_TOL = 1.020


def promote2_reading(keyed):
    """README.md's promote2 verdict over both runs; returns True for PASS."""
    ok = True
    for shape in sorted({k[0] for k in keyed[0]}):
        cells = {m: [k[(shape, m, "P/M")] for k in keyed] for m in ("phase_ns", "cycles")}
        if any(c["ci_upper"] is None for cs in cells.values() for c in cs):
            print(f"  {shape}: NO READING (too few valid processes) -> counts as NOT MET")
            ok = False
            continue
        for m, cs in cells.items():
            print(f"  {shape} {m}: P/M " + "; ".join(
                f"run {n} {c['ratio']:.4f} [{c['ci_lower']:.4f}, {c['ci_upper']:.4f}]" for n, c in enumerate(cs, 1)))
        lk = keyed[0].get((shape, "mem_inst_retired.lock_loads", "P/M"))
        if lk is not None:
            print(f"  {shape} lock_loads per process (run 1): P {lk.get('mean_P')}, M {lk.get('mean_M')}")
        ups = [c["ci_upper"] for cs in cells.values() for c in cs]
        if shape in P2_TARGETS:
            good = all(u < 1.0 for u in ups)
            print(f"  {shape}: target FASTER {'MET' if good else 'NOT MET'}")
        else:
            good = all(u <= NO_LOSS_TOL for u in ups)
            print(f"  {shape}: no-loss bound (upper <= {NO_LOSS_TOL}) {'MET' if good else 'NOT MET'}")
        ok = ok and good
    print(f"promote2 verdict: {'PASS' if ok else 'FAIL'}")
    return ok


def main():
    host, prefixes = sys.argv[1], sys.argv[2:4]
    arts, keyed = [], []
    for n, prefix in enumerate(prefixes, 1):
        art, recs = analyse_run(prefix)
        name = NAME_BY_SET[art["provenance"].get("phase_set", "gets")]
        dest = RESULTS / f"diagnostic_{name}_{host}_run{n}.json"
        dest.write_text(json.dumps(art, indent=1) + "\n")
        v = art["provenance"]["void"]
        print(f"run {n}: {len(recs)} processes, {sum(not valid(r) for r in recs)} discarded; "
              f"max load1 {v['max_load1']} (void above {v['limit']}): "
              f"{'VOID' if v['void'] else 'valid'} -> {shown(dest)}")
        arts.append(art)
        keyed.append({(c["workload_id"][6:], c["metric"], c["pair"]): c for c in art["cells"]})
    if any(a["provenance"]["void"]["void"] for a in arts):
        sys.exit("a run is void (METHODOLOGY.md §7); no reading")
    fe_set = arts[0]["provenance"].get("phase_set") == "delete_fe"
    ms_set = arts[0]["provenance"].get("phase_set") == "delete_ms"
    acct_set = arts[0]["provenance"].get("phase_set") == "delete_acct"
    promote2_set = arts[0]["provenance"].get("phase_set") == "promote2"
    print("\n| shape | counter | pair | per op (num) | per op (den) | run 1 [BCa 95%] | run 2 [BCa 95%] |")
    print("|---|---|---|---|---|---|---|")
    for key in keyed[0]:
        cs = [k[key] for k in keyed]
        if any(c["ratio"] is None for c in cs):
            why = next((c["no_ratio"] for c in cs if c.get("no_ratio")), f"n < {MIN_VALID}")
            print(f"| {key[0]} | `{key[1]}` | {key[2]} | {why} | | | |")
            continue
        num, den = key[2].split("/")
        txt = [f"{c['ratio']:.4f} [{c['ci_lower']:.4f}, {c['ci_upper']:.4f}]"
               + ("" if c["ci_method"] == CI_METHOD_BCA else f" ({c['ci_method']})") for c in cs]
        print(f"| {key[0]} | `{key[1]}` | {key[2]} | {cs[0]['per_op_' + num]:.4g} | "
              f"{cs[0]['per_op_' + den]:.4g} | {txt[0]} | {txt[1]} |")
    if promote2_set:
        print("\nreading rule (README.md), promote2, P against M per shape:")
        promote2_reading(keyed)
        return
    if acct_set:
        print("\nreading rule (README.md), A against D and S per shape:")
        for shape in sorted({k[0] for k in keyed[0]}):
            acct_reading(keyed, shape)
        return
    num = PAIRS[0][0]
    print(f"\nreading rule (README.md), {num} against S per shape:")
    for shape in sorted({k[0] for k in keyed[0]}):
        cyc = [k[(shape, "cycles", f"{num}/S")] for k in keyed]
        ins = [k[(shape, "instructions", f"{num}/S")] for k in keyed]
        if any(c["ci_lower"] is None for c in cyc + ins):
            print(f"  {shape}: NO READING (a run has too few valid processes for an interval)")
            continue
        if fe_set and all(c["ci_lower"] > 1.0 for c in cyc):
            fe_reading(keyed, shape, num)
            continue
        if ms_set and all(c["ci_lower"] > 1.0 for c in cyc):
            ms_reading(keyed, shape, num)
            continue
        if all(c["ci_lower"] > 1.0 for c in cyc):
            sh = [shares(k, shape, num) for k in keyed]
            for n, (d, m, f) in enumerate(sh, 1):
                print(f"  {shape} run {n}: {num} - S = {d:.4g} cycles; memory share {m:.3f}, front-end share {f:.3f}")
            mem_all, fe_all = all(m >= 0.5 for _, m, _ in sh), all(f >= 0.5 for _, _, f in sh)
            verdict = ("MEMORY" if mem_all and not fe_all else "FRONT-END" if fe_all and not mem_all
                       else "BOTH" if mem_all and fe_all else "MIXED")
            # The delete set's extra verdict (README.md, "Diagnostic: delete-phase
            # counters"): more instructions retired than stock, and neither
            # memory nor the front end carrying half the extra cycles in either
            # run.
            if (num == "D" and all(c["ci_lower"] > 1.0 for c in ins)
                    and all(m < 0.5 and f < 0.5 for _, m, f in sh)):
                verdict += " (CALL-STRUCTURE)"
        elif all(c["ci_upper"] < 1.0 for c in cyc):
            verdict = f"NO GAP ({num} takes fewer cycles than S)"
        else:
            verdict = "UNRESOLVED (the cycle ratio's intervals do not agree on a side)"
        print(f"  {shape}: {verdict}")


if __name__ == "__main__":
    main()
