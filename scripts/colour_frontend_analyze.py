#!/usr/bin/env python3
"""#1682 step A2: attribute the front-end-bound share of libexpanse's extra
cycles per Judy1 call over stock on the colour-map tape.

Reads, per independent run, the `perf_counters.py --workload colour_map`
artifacts of the event sets that `results/colour_frontend_1682_a2_preregistration.yaml`
fixed before any run (one perf invocation per set, at most four
general-purpose events each), and derives exactly what that file names:

- per arm, paired run index i and node: cycles per call. The top-down level-1
  and level-2 terms convert slots to cycles with the arm's own measured
  slots per cycle (as `colour_topdown_analyze.py` does); the level-3 nodes
  use the Golden Cove TMA expressions perf 6.8.12 ships for `cpu_core`
  (`perf list -j`), multiplied out to cycles with thread cycles standing in
  for core cycles (the SMT sibling is idle, see the pre-registration);
- per call counts (uops by source, frontend_retired.*, BAClears, iTLB);
- delta_i = libexpanse - stock, BCa 95% over the paired deltas, with the
  construction recorded beside each interval;
- share of the front-end delta = mean delta(node) / mean delta(fe_bound) of
  the same run (fe_bound from the top-down set of that run);
- the pre-registered gate per node and population: in this run,
  ci_lower(delta node) >= 0.5 * mean delta(fe_bound).

Usage:
    python3 scripts/colour_frontend_analyze.py --out results/<name>.json \\
        --run run1:results/counters_colour_fe_<sha>_run1_{T,B,U,L,R,D,F1,F2,F3,F4}.json \\
        --run run2:...
    python3 scripts/colour_frontend_analyze.py --self-test

`--run` takes `<label>:<comma-separated artifact paths>`; each artifact's
`counters_requested` decides which set it is.
"""

from __future__ import annotations

import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

ARMS = ("stock", "expanse_dl")

# The event sets, by the GP events that identify them (fixed counters omitted).
SETS = {
    "T": ["slots", "topdown-retiring", "topdown-bad-spec", "topdown-fe-bound",
          "topdown-be-bound", "topdown-br-mispredict", "topdown-mem-bound",
          "topdown-fetch-lat", "topdown-heavy-ops", "br_misp_retired.all_branches"],
    "B": ["idq.dsb_cycles_any", "idq.dsb_cycles_ok", "idq.mite_cycles_any", "idq.mite_cycles_ok"],
    "U": ["idq.dsb_uops", "idq.mite_uops", "idq.ms_uops", "uops_issued.any"],
    "L": ["icache_data.stalls", "icache_tag.stalls", "dsb2mite_switches.penalty_cycles",
          "decode.lcp"],
    "R": ["int_misc.clear_resteer_cycles", "int_misc.unknown_branch_cycles", "baclears.any",
          "decode.ms_busy"],
    "D": ["lsd.uops", "lsd.cycles_active", "lsd.cycles_ok", "itlb_misses.stlb_hit"],
    "F1": ["frontend_retired.dsb_miss", "itlb_misses.walk_completed"],
    "F2": ["frontend_retired.any_dsb_miss", "br_inst_retired.all_branches",
           "br_inst_retired.near_taken"],
    "F3": ["frontend_retired.l1i_miss"],
    "F4": ["frontend_retired.itlb_miss"],
}

# Level-3 front-end nodes, in cycles: (set, function of the per-run counts).
# TMA expressions (perf 6.8.12, cpu_core): tma_mite = (MITE_CYCLES_ANY -
# MITE_CYCLES_OK) / core_clks / 2, likewise tma_dsb and tma_lsd; the
# fetch-latency nodes are <event> / thread_clks. Times clks gives cycles.
NODES = {
    "mite": ("B", lambda c: (c["idq.mite_cycles_any"] - c["idq.mite_cycles_ok"]) / 2),
    "dsb": ("B", lambda c: (c["idq.dsb_cycles_any"] - c["idq.dsb_cycles_ok"]) / 2),
    "lsd": ("D", lambda c: (c["lsd.cycles_active"] - c["lsd.cycles_ok"]) / 2),
    "icache_misses": ("L", lambda c: c["icache_data.stalls"]),
    "itlb_misses": ("L", lambda c: c["icache_tag.stalls"]),
    "dsb_switches": ("L", lambda c: c["dsb2mite_switches.penalty_cycles"]),
    "lcp": ("L", lambda c: c["decode.lcp"]),
    "clear_resteers": ("R", lambda c: c["int_misc.clear_resteer_cycles"]),
    "unknown_branches": ("R", lambda c: c["int_misc.unknown_branch_cycles"]),
}
# Which nodes a code-layout change (BOLT, PGO block order, hot/cold split)
# acts on, as pre-registered. The rest are mispredict recovery, instruction
# encoding or microcode, which layout does not change.
LAYOUT_NODES = {"mite", "dsb", "lsd", "icache_misses", "itlb_misses", "dsb_switches",
                "unknown_branches"}

# Per-call counts reported with intervals but not gated.
COUNTS = {
    "U": ["idq.dsb_uops", "idq.mite_uops", "idq.ms_uops", "uops_issued.any"],
    "R": ["baclears.any", "decode.ms_busy"],
    "D": ["lsd.uops", "itlb_misses.stlb_hit"],
    "F1": ["frontend_retired.dsb_miss", "itlb_misses.walk_completed"],
    "F2": ["frontend_retired.any_dsb_miss", "br_inst_retired.all_branches",
           "br_inst_retired.near_taken"],
    "F3": ["frontend_retired.l1i_miss"],
    "F4": ["frontend_retired.itlb_miss"],
}
GATE_FRACTION = 0.5


def base_name(name: str) -> str:
    if "/" in name:
        parts = [p for p in name.split("/") if p]
        return parts[1] if len(parts) > 1 else parts[0]
    return name


def samples(cell: dict, base: str) -> list[float]:
    for name, c in cell["counters"].items():
        if base_name(name) == base:
            if c.get("status") != "ok":
                raise SystemExit(f"{cell['id']}: {name} status {c.get('status')}")
            if c.get("min_pct_running") is not None and c["min_pct_running"] < 100.0:
                raise SystemExit(f"{cell['id']}: {name} multiplexed "
                                 f"({c['min_pct_running']}% running); not pre-registered")
            return [float(x) for x in c["samples"]]
    raise SystemExit(f"{cell['id']}: no {base} counter")


def which_set(doc: dict) -> str:
    req = {base_name(e) for e in doc["counters_requested"].split(",")}
    hits = [k for k, evs in SETS.items() if set(evs) <= req]
    if len(hits) != 1:
        raise SystemExit(f"artifact requests {sorted(req)}: matches sets {hits}")
    return hits[0]


def ci(values: list[float]) -> dict:
    mean, lo, hi, method = bca_bootstrap_ci_with_method(values)
    return {"mean": mean, "ci_lower": lo, "ci_upper": hi, "ci_method": method}


def per_arm(doc: dict) -> dict:
    """{(arm, pop): {"ops": n, "cycles": [...], <event>: [...]}} per run index."""
    out = {}
    for c in doc["cells"]:
        ops = c["distinct_probes"] * c["passes"]
        d = {"ops": ops}
        for name in c["counters"]:
            d[base_name(name)] = samples(c, base_name(name))
        out[(c["arm"], c["population"])] = d
    return out


def analyse_run(label: str, docs: dict[str, dict]) -> dict:
    missing = set(SETS) - set(docs)
    if missing:
        raise SystemExit(f"{label}: missing event sets {sorted(missing)}")
    data = {k: per_arm(v) for k, v in docs.items()}
    pops = sorted({p for _, p in data["T"]})
    out = {"label": label, "commit": {k: v["provenance"].get("commit") for k, v in docs.items()},
           "populations": {}}
    for pop in pops:
        series: dict[str, dict[str, list[float]]] = {a: {} for a in ARMS}
        for arm in ARMS:
            t = data["T"][(arm, pop)]
            ops = t["ops"]
            conv = [y / s for y, s in zip(t["cycles"], t["slots"])]
            for cat in ("topdown-fe-bound", "topdown-fetch-lat", "topdown-bad-spec",
                        "topdown-retiring", "topdown-be-bound"):
                series[arm][cat] = [x * k / ops for x, k in zip(t[cat], conv)]
            series[arm]["fetch_bw"] = [a - b for a, b in zip(series[arm]["topdown-fe-bound"],
                                                             series[arm]["topdown-fetch-lat"])]
            for s in SETS:
                series[arm][f"cycles[{s}]"] = [x / data[s][(arm, pop)]["ops"]
                                               for x in data[s][(arm, pop)]["cycles"]]
            for node, (s, fn) in NODES.items():
                d = data[s][(arm, pop)]
                n = len(d["cycles"])
                series[arm][node] = [fn({k: v[i] for k, v in d.items() if k != "ops"}) / d["ops"]
                                     for i in range(n)]
            for s, evs in COUNTS.items():
                d = data[s][(arm, pop)]
                for e in evs:
                    series[arm][e] = [x / d["ops"] for x in d[e]]
            u = data["U"][(arm, pop)]
            series[arm]["dsb_coverage"] = [a / b for a, b in zip(u["idq.dsb_uops"],
                                                                 u["uops_issued.any"])]
        keys = list(series["stock"])
        n = {k: len(series["stock"][k]) for k in keys}
        if any(len(series["expanse_dl"][k]) != n[k] for k in keys):
            raise SystemExit(f"{label} pop={pop}: unequal run counts between arms")
        delta = {k: ci([series["expanse_dl"][k][i] - series["stock"][k][i] for i in range(n[k])])
                 for k in keys}
        fe = delta["topdown-fe-bound"]["mean"]
        floor = GATE_FRACTION * fe
        cell = {
            "per_call": {a: {k: sum(v) / len(v) for k, v in series[a].items()} for a in ARMS},
            "delta": delta,
            "fe_bound_delta_mean": fe,
            "gate_floor": floor,
            "share_of_fe_delta": {k: delta[k]["mean"] / fe
                                  for k in ("fetch_bw", "topdown-fetch-lat", *NODES)},
            "gate": {node: {"met": delta[node]["ci_lower"] >= floor,
                            "point_met": delta[node]["mean"] >= floor,
                            "layout": node in LAYOUT_NODES} for node in NODES},
            "fe_bound_lower_above_zero": delta["topdown-fe-bound"]["ci_lower"] > 0,
            "level3_sum_delta": sum(delta[k]["mean"] for k in NODES),
        }
        out["populations"][str(pop)] = cell
    return out


def verdict(runs: list[dict]) -> dict:
    """Pre-registered decision. A node carries the front-end term at n if its
    gate is met in every run; A5 if a layout node carries it at every n; A1
    if no node carries it at any n, or only non-layout nodes do; anything
    else is mixed and goes to the owner."""
    pops = sorted(runs[0]["populations"], key=int)
    carriers = {}
    for p in pops:
        if not all(r["populations"][p]["fe_bound_lower_above_zero"] for r in runs):
            carriers[p] = None
            continue
        carriers[p] = sorted(node for node in NODES
                             if all(r["populations"][p]["gate"][node]["met"] for r in runs))
    if any(v is None for v in carriers.values()):
        decision = "VOID: the front-end delta's interval contains 0 in some run"
    elif all(carriers[p] and set(carriers[p]) & LAYOUT_NODES for p in pops):
        decision = "A5"
    elif all(not carriers[p] or not (set(carriers[p]) & LAYOUT_NODES) for p in pops):
        decision = "A1"
    else:
        decision = "MIXED: owner decision"
    return {"carriers": carriers, "decision": decision}


def load_runs(specs: list[str]) -> list[dict]:
    runs = []
    for spec in specs:
        label, _, paths = spec.partition(":")
        docs: dict[str, dict] = {}
        for p in paths.split(","):
            d = json.loads(Path(p).read_text())
            s = which_set(d)
            if s in docs:
                raise SystemExit(f"{label}: set {s} given twice")
            docs[s] = d
        runs.append(analyse_run(label, docs))
    return runs


def _fake_doc(set_name: str, gen) -> dict:
    events = ["cycles", "instructions", *SETS[set_name]]
    cells = []
    for arm in ARMS:
        for pop in (10000, 100000):
            counters = {f"cpu_core/{e}/": {"status": "ok", "min_pct_running": 100.0,
                                           "samples": [gen(arm, pop, e, i) for i in range(10)]}
                        for e in events}
            cells.append({"id": f"{arm}/pop={pop}/hit=100", "arm": arm, "population": pop,
                          "distinct_probes": 12 * pop + 2, "passes": 1, "counters": counters})
    return {"counters_requested": ",".join(events), "provenance": {"commit": "selftest"},
            "cells": cells}


def self_test() -> int:
    """Synthetic sets in which libexpanse's extra front-end slots are all
    MITE: the gate must name `mite`, and only it, and decide A5; moving the
    same excess to clear_resteers must decide A1."""
    rng = random.Random(7)

    def gen_for(excess_node: str):
        def gen(arm, pop, e, i):
            ops = 12 * pop + 2
            extra = 14.0 if arm == "expanse_dl" else 0.0
            j = 1 + rng.uniform(-0.01, 0.01)
            per_call = {
                "cycles": 50 + extra, "instructions": 150, "slots": 6 * (50 + extra),
                "topdown-fe-bound": 6 * (2 + extra), "topdown-fetch-lat": 6 * 1.0,
                "topdown-retiring": 6 * 40, "topdown-bad-spec": 6 * 5, "topdown-be-bound": 6 * 3,
                "idq.mite_cycles_any": 10 + (2 * extra if excess_node == "mite" else 0),
                "int_misc.clear_resteer_cycles": 2 + (extra if excess_node == "clear" else 0),
                "idq.dsb_uops": 100, "uops_issued.any": 160, "lsd.cycles_active": 1.0,
            }.get(e, 1.0)
            return per_call * ops * j
        return gen

    for node, want in (("mite", "A5"), ("clear", "A1")):
        gen = gen_for(node)
        runs = [analyse_run(f"r{k}", {s: _fake_doc(s, gen) for s in SETS}) for k in (1, 2)]
        v = verdict(runs)
        expect = {"mite": ["mite"], "clear": ["clear_resteers"]}[node]
        assert all(v["carriers"][p] == expect for p in v["carriers"]), v
        assert v["decision"] == want, v
    # A multiplexed counter is refused, not used.
    gen = gen_for("mite")
    doc = _fake_doc("B", gen)
    doc["cells"][0]["counters"]["cpu_core/idq.mite_cycles_any/"]["min_pct_running"] = 80.0
    try:
        per_arm(doc)
    except SystemExit:
        pass
    else:
        raise AssertionError("multiplexed counter accepted")
    print("colour_frontend_analyze self-test: ok")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out")
    ap.add_argument("--run", action="append", default=[])
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    if not a.out or len(a.run) < 2:
        ap.error("--out and two --run are required")
    runs = load_runs(a.run)
    res = {"workload_id": "capi_colour_map", "measurement": "1682-A2 front-end attribution",
           "preregistration": "results/colour_frontend_1682_a2_preregistration.yaml",
           "estimators": "BCa 95% over paired per-run deltas (libexpanse - stock), per run; "
                         "construction in each ci_method",
           "gate": f"ci_lower(delta node) >= {GATE_FRACTION} * mean delta(fe_bound), every run",
           "runs": runs, "verdict": verdict(runs)}
    for r in runs:
        for pop, c in r["populations"].items():
            fe = c["delta"]["topdown-fe-bound"]
            print(f"{r['label']} n={pop}: delta fe_bound {fe['mean']:+.2f} "
                  f"[{fe['ci_lower']:+.2f}, {fe['ci_upper']:+.2f}] {fe['ci_method']}; "
                  f"floor {c['gate_floor']:.2f}")
            for k in ("fetch_bw", "topdown-fetch-lat", *NODES):
                v = c["delta"][k]
                g = c["gate"].get(k, {}).get("met", "")
                print(f"   {k:20s} {v['mean']:+7.2f} [{v['ci_lower']:+.2f}, {v['ci_upper']:+.2f}]"
                      f"  share {c['share_of_fe_delta'][k]:+.2f} {g}")
    print("verdict:", json.dumps(res["verdict"]))
    Path(a.out).write_text(json.dumps(res, indent=1) + "\n")
    print(f"wrote {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
