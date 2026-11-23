#!/usr/bin/env python3
"""stress-ng x Judy analysis (METHODOLOGY.md §6-§8).

Reads `driver.py`'s raw rows and provenance for two runs on one host and
writes `results/baseline_<host>_run<N>.json`: the run's provenance, and per
(workload, W, metric) cell the two-sample BCa 95% interval on E/S with its
`ci_method`, the valid sample counts and `rounds_raw`, every process verbatim.
It then prints the §8 verdict table, the headline H, and the timer cross-check.

With `--short-window=round`, a process whose own load window has no foreign
reading is judged on its round window instead (`short_window.py`); every such
row says so, and the count is printed per run. Without it, nothing changes.

Usage: analyze.py <host_label> <run1_prefix> <run2_prefix> [--short-window=round]
"""
import json
import math
import os
import sys
from collections import defaultdict
from pathlib import Path

SUITE = Path(__file__).resolve().parent.parent
REPO_ROOT = SUITE.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ratio_ci_with_method  # noqa: E402
import short_window

# `SNG_RESULTS_DIR` redirects the artifacts, so a dry run over old rows never
# writes into the committed results directory.
RESULTS = Path(os.environ.get("SNG_RESULTS_DIR", SUITE / "results"))


def shown(path):
    """A results path as printed: relative to the repository root, so console
    output pasted into a committed artifact carries no home directory (the
    `pii` discipline check); outside the repository, the results directory's
    variable name and the file name."""
    p = Path(path).resolve()
    try:
        return str(p.relative_to(REPO_ROOT.resolve()))
    except ValueError:
        return f"$SNG_RESULTS_DIR/{p.name}"

FOREIGN_MAX = 0.5
MIN_VALID = 8
# Set by `--short-window=round` (`take_short_window_flag`). While it is False,
# `valid` reads the process window alone and no artifact gains a field.
SHORT_WINDOW = False
PRIMARY = {
    "sng_judy": ["judy-insert-operations-per-sec", "judy-find-operations-per-sec",
                 "judy-delete-operations-per-sec"],
    "sng_sm_judy": ["judy-puts-per-sec", "judy-gets-per-sec"],
    "sng_sm_hashjudy": ["hashjudy-puts-per-sec", "hashjudy-gets-per-sec"],
}
SECONDARY = ["bogo-ops-per-second-real-time", "max-rss", "wall"]


def family(cell):
    for k in ("sng_sm_hashjudy", "sng_sm_judy", "sng_judy"):
        if cell.startswith(k):
            return k
    raise ValueError(cell)


def value(rec, metric):
    return rec["wall"] if metric == "wall" else rec["metrics"][metric]


def valid(rec):
    foreign = short_window.reading(rec)[0] if SHORT_WINDOW else rec["load"]["foreign_busy_cpus"]
    return (rec["rc"] == 0 and not rec["failed"]
            and foreign is not None
            and foreign <= FOREIGN_MAX)


def take_short_window_flag(args):
    """`args` without the short-window opt-in, which this switches on if present."""
    global SHORT_WINDOW
    unknown = [a for a in args if a.startswith("--short-window") and a != short_window.OPT_IN]
    if unknown:
        sys.exit(f"STOP: {unknown[0]} is not {short_window.OPT_IN}")
    SHORT_WINDOW = short_window.OPT_IN in args
    return [a for a in args if a != short_window.OPT_IN]


def load_rows(recs, prov):
    """The raw rows, with their round-window fallback blocks under the opt-in."""
    if SHORT_WINDOW:
        return short_window.annotate(recs, prov)
    short_window.refuse_prelabelled(recs)
    return recs


def close_short_window(art, recs):
    """Under the opt-in: the artifact's count of fallback rows, after the label check."""
    if SHORT_WINDOW:
        short_window.require_labelled(art["cells"])
        art["provenance"]["short_window"] = short_window.summary(recs, valid, FOREIGN_MAX)
    return art


def write_artifact(dest, art):
    """Writes one run's artifact; the opt-in never replaces one written without it."""
    if SHORT_WINDOW:
        short_window.refuse_overwrite(dest)
    dest.write_text(json.dumps(art, indent=1) + "\n")


def run_void(prov):
    """METHODOLOGY.md §7: a run is void if load1 ever exceeded half the logical CPUs."""
    limit = (prov["host"]["cpus_online"] or 0) / 2
    worst = max(s["load1"] for s in prov["loads"])
    return worst > limit, worst, limit


def analyse_run(prefix):
    recs = [json.loads(line) for line in open(f"{prefix}.jsonl")]
    prov = json.loads(Path(f"{prefix}.provenance.json").read_text())
    load_rows(recs, prov)
    by = defaultdict(list)
    for r in recs:
        by[(r["cell"], r["w"], r["arm"])].append(r)
    cells = []
    for cell, w in sorted({(r["cell"], r["w"]) for r in recs}):
        for metric in PRIMARY[family(cell)] + SECONDARY:
            s = [r for r in by[(cell, w, "S")] if valid(r)]
            e = [r for r in by[(cell, w, "E")] if valid(r)]
            out = {"workload_id": cell, "w": w, "metric": metric,
                   "primary": metric in PRIMARY[family(cell)],
                   "n_S": len(s), "n_E": len(e),
                   "discarded_S": len(by[(cell, w, "S")]) - len(s),
                   "discarded_E": len(by[(cell, w, "E")]) - len(e)}
            if len(s) >= MIN_VALID and len(e) >= MIN_VALID:
                sv, ev = [value(r, metric) for r in s], [value(r, metric) for r in e]
                ratio, lo, hi, ci_method = bca_bootstrap_ratio_ci_with_method(ev, sv, num_resamples=2000, seed=42)
                out.update({"mean_S": math.fsum(sv) / len(sv), "mean_E": math.fsum(ev) / len(ev),
                            "ratio": ratio, "ci_lower": lo, "ci_upper": hi, "ci_method": ci_method})
            else:
                out.update(ratio=None, ci_lower=None, ci_upper=None, ci_method=None)
            out["rounds_raw"] = [
                {"round": r["round"], "arm": r["arm"], "value": value(r, metric) if r["metrics"] else None,
                 "valid": valid(r), "foreign_busy_cpus": r["load"]["foreign_busy_cpus"],
                 "wall_s": r["wall"], "rc": r["rc"],
                 **(short_window.row_fields(r) if SHORT_WINDOW else {})}
                for r in sorted(by[(cell, w, "S")] + by[(cell, w, "E")], key=lambda r: (r["round"], r["arm"]))]
            cells.append(out)
    void, worst, limit = run_void(prov)
    prov["void"] = {"void": void, "max_load1": worst, "limit": limit}
    return close_short_window({"provenance": prov, "cells": cells}, recs), recs


def verdict(cis):
    if all(lo > 1.0 for lo, _ in cis):
        return "FASTER"
    if all(hi < 1.0 for _, hi in cis):
        return "SLOWER"
    if all(lo >= 0.95 and hi <= 1.05 for lo, hi in cis):
        return "EQUIVALENT"
    return "INCONCLUSIVE"


def main():
    args = take_short_window_flag(sys.argv[1:])
    host, prefixes = args[0], args[1:3]
    arts = []
    for n, prefix in enumerate(prefixes, 1):
        art, recs = analyse_run(prefix)
        dest = RESULTS / f"baseline_{host}_run{n}.json"
        write_artifact(dest, art)
        disc = [r for r in recs if not valid(r)]
        v = art["provenance"]["void"]
        print(f"run {n}: {len(recs)} processes, {len(disc)} discarded; max load1 {v['max_load1']} "
              f"(void above {v['limit']}): {'VOID' if v['void'] else 'valid'} -> {shown(dest)}")
        if SHORT_WINDOW:
            print(short_window.summary_line(n, art["provenance"]["short_window"]))
        arts.append(art)
    if any(a["provenance"]["void"]["void"] for a in arts):
        sys.exit("a run is void (METHODOLOGY.md §7); no verdict")
    keyed = [{(c["workload_id"], c["w"], c["metric"]): c for c in a["cells"]} for a in arts]
    print("\n| workload | W | metric | run 1 E/S [BCa 95%] | run 2 E/S [BCa 95%] | verdict |")
    print("|---|---|---|---|---|---|")
    counts, big_loss = defaultdict(int), []
    for key in sorted(keyed[0]):
        cs = [k.get(key) for k in keyed]
        if any(c is None or c["ratio"] is None for c in cs):
            v = "INCONCLUSIVE (n < 8)"
            txt = [("—" if c is None or c["ratio"] is None else f"{c['ratio']:.3f}") for c in cs]
        else:
            cis = [(c["ci_lower"], c["ci_upper"]) for c in cs]
            v = verdict(cis)
            txt = [f"{c['ratio']:.3f} [{c['ci_lower']:.3f}, {c['ci_upper']:.3f}]"
                   + ("" if c["ci_method"] == CI_METHOD_BCA else f" ({c['ci_method']})") for c in cs]
            if cs[0]["primary"] and v == "SLOWER" and all(hi < 0.95 for _, hi in cis):
                big_loss.append(key)
        if not cs[0]["primary"]:
            v = "secondary"
        else:
            counts[v] += 1
        print(f"| `{key[0]}` | {key[1]} | `{key[2]}` | {txt[0]} | {txt[1]} | {v} |")
    print(f"\nprimary cell-metrics: {dict(counts)}")
    print(f"SLOWER with upper < 0.95 in both runs: {len(big_loss)} -> H {'NOT MET' if big_loss else 'MET'}")
    for key in big_loss:
        print(f"  {key[0]} W{key[1]} {key[2]}")
    print("\ntimer cross-check (outside wall E/S against stress-ng bogo-ops/s S/E):")
    for key in sorted(k for k in keyed[0] if k[2] == "wall"):
        for n, k in enumerate(keyed, 1):
            wall, bogo = k.get(key), k.get((key[0], key[1], "bogo-ops-per-second-real-time"))
            if not wall or wall["ratio"] is None or not bogo or bogo["ratio"] is None:
                continue
            flag = "  <-- differs by more than 5%" if abs(wall["ratio"] * bogo["ratio"] - 1) > 0.05 else ""
            print(f"  {key[0]} W{key[1]} run {n}: wall {wall['ratio']:.3f} vs bogo {1 / bogo['ratio']:.3f}{flag}")


if __name__ == "__main__":
    main()
