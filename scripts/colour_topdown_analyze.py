#!/usr/bin/env python3
"""#1682 measurement (a): top-down attribution of libexpanse's extra cycles
per Judy1 call over stock on the colour-map tape.

Reads the `perf_counters.py --workload colour_map` artifacts of the runs
named on the command line (one per independent run) and derives, per run and
population, exactly what `results/colour_map_1682_preregistration.yaml`
(measurement a) fixed before the runs:

- per arm and paired run index i: a category's cycles per call =
  slots(category)_i * cycles_i / slots_i / ops (the arm's own measured
  slots per cycle converts slots to cycles);
- delta_i = libexpanse - stock, per category, per cycles and per
  br_misp_retired.all_branches;
- BCa 95% over the 10 deltas (scripts/bca_bootstrap.py), with the
  construction recorded beside each interval;
- share of the cycle gap = mean delta(category) / mean delta(cycles);
- cost per extra mispredict = mean delta(br-mispredict cycles) /
  mean delta(mispredicts per call).

Usage:
    python3 scripts/colour_topdown_analyze.py --out results/<name>.json \\
        results/counters_colour_topdown_<sha>_run1.json \\
        results/counters_colour_topdown_<sha>_run2.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

CATEGORIES = [
    "topdown-retiring", "topdown-bad-spec", "topdown-fe-bound", "topdown-be-bound",
    "topdown-br-mispredict", "topdown-mem-bound", "topdown-fetch-lat", "topdown-heavy-ops",
]
ARMS = ("stock", "expanse_dl")


def samples(cell: dict, base: str) -> list[float]:
    for name, c in cell["counters"].items():
        if name.split("/")[-2 if name.endswith("/") else -1] == base or name == base:
            if c.get("status") != "ok":
                raise SystemExit(f"{cell['id']}: {name} status {c.get('status')}")
            return [float(x) for x in c["samples"]]
    raise SystemExit(f"{cell['id']}: no {base} counter")


def ci(values: list[float]) -> dict:
    mean, lo, hi, method = bca_bootstrap_ci_with_method(values)
    return {"mean": mean, "ci_lower": lo, "ci_upper": hi, "ci_method": method}


def analyse(path: Path) -> dict:
    d = json.loads(path.read_text())
    cells = {(c["arm"], c["population"]): c for c in d["cells"]}
    out = {"source": str(path), "commit": d["provenance"].get("commit"), "populations": {}}
    for pop in sorted({p for _, p in cells}):
        per_arm = {}
        for arm in ARMS:
            c = cells[(arm, pop)]
            ops = c["distinct_probes"] * c["passes"]
            cyc, slots = samples(c, "cycles"), samples(c, "slots")
            arm_d = {"ops": ops, "cycles": [x / ops for x in cyc],
                     "mispredicts": [x / ops for x in samples(c, "br_misp_retired.all_branches")],
                     "slots_per_cycle": [s / y for s, y in zip(slots, cyc)]}
            for cat in CATEGORIES:
                arm_d[cat] = [t * y / s / ops for t, y, s in zip(samples(c, cat), cyc, slots)]
            per_arm[arm] = arm_d
        n = len(per_arm["stock"]["cycles"])
        if any(len(per_arm[a][k]) != n for a in ARMS for k in ("cycles", *CATEGORIES)):
            raise SystemExit(f"pop={pop}: unequal run counts")
        delta = {k: [per_arm["expanse_dl"][k][i] - per_arm["stock"][k][i] for i in range(n)]
                 for k in ("cycles", "mispredicts", *CATEGORIES)}
        cell = {
            "per_call": {a: {k: sum(v) / n for k, v in per_arm[a].items() if k != "ops"}
                         for a in ARMS},
            "delta": {k: ci(v) for k, v in delta.items()},
        }
        gap = cell["delta"]["cycles"]["mean"]
        cell["share_of_cycle_gap"] = {k: cell["delta"][k]["mean"] / gap for k in CATEGORIES}
        dm = cell["delta"]["mispredicts"]["mean"]
        cell["cycles_per_extra_mispredict"] = (
            cell["delta"]["topdown-br-mispredict"]["mean"] / dm if dm else None)
        out["populations"][str(pop)] = cell
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out", required=True)
    ap.add_argument("runs", nargs="+")
    a = ap.parse_args()
    res = {"workload_id": "capi_colour_map", "measurement": "1682-a top-down attribution",
           "estimators": "BCa 95% over paired per-run deltas, per run",
           "runs": [analyse(Path(p)) for p in a.runs]}
    for r in res["runs"]:
        for pop, c in r["populations"].items():
            dc = c["delta"]["cycles"]
            print(f"{Path(r['source']).name} n={pop}: delta cycles/call {dc['mean']:.2f} "
                  f"[{dc['ci_lower']:.2f}, {dc['ci_upper']:.2f}] {dc['ci_method']}; "
                  f"cyc/extra mispredict {c['cycles_per_extra_mispredict']:.1f}")
            for k in CATEGORIES:
                v = c["delta"][k]
                print(f"   {k:24s} {v['mean']:+7.2f} [{v['ci_lower']:+.2f}, {v['ci_upper']:+.2f}]"
                      f"  share {c['share_of_cycle_gap'][k]:+.2f}")
    Path(a.out).write_text(json.dumps(res, indent=1) + "\n")
    print(f"wrote {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
