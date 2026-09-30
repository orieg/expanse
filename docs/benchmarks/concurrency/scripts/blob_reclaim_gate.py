#!/usr/bin/env python3
"""METHODOLOGY §28.5's evaluation: is the blob map with engine reclamation non-inferior on the published cells (#1290)?

Reads `core_concurrency` artifacts from `mixed_concurrency.py` for §28's two
builds, B (the change's merge base) and R (the change's head), two runs each,
and reads every registered comparison: R ÷ B per cell as the ratio of mean
total ops/s with a two-sample BCa 95 % interval. G3 is non-inferiority at a
floor of 0.95. The verdict rules and the drift controls are
`blob_remove_gate.py`'s, applied to §28's cells.

Usage:
    python3 docs/benchmarks/concurrency/scripts/blob_reclaim_gate.py \\
        --b RUN1,RUN2 --r RUN1,RUN2 --out VERDICT.json
    python3 docs/benchmarks/concurrency/scripts/blob_reclaim_gate.py --self-test
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

from blob_remove_gate import (  # noqa: E402
    CI_METHOD_BCA,
    CI_METHODS,
    CONTROL_BAND,
    GateError,
    cell,
    gate_verdict,
    load,
    moved,
    ratio,
)

# §28.5: the gated comparison (engine, read %, threads, floor).
GATES = (("blob", 50, 16, 0.95),)
# §28.5: reported, not gated.
REPORTED = (("blob", 50, 4), ("blob", 50, 1), ("blob", 100, 16), ("blob_mutex", 50, 16), ("blob_mutex", 50, 4))
# §28.5: cells the change does not reach.
CONTROLS = (("map", 50, 16), ("str", 50, 16))


def void_reasons(name: str, runs: list[dict[str, Any]]) -> list[str]:
    """§28.5's voids that the artifacts show (§27.6 without the compaction clause)."""
    commits = {r["provenance"].get("commit") for r in runs}
    if len(commits) != 1:
        return [f"build {name}: its runs measured different commits {sorted(map(str, commits))}"]
    return []


def evaluate(builds: dict[str, list[dict[str, Any]]]) -> dict[str, Any]:
    for name in ("B", "R"):
        if len(builds.get(name, [])) != 2:
            raise GateError(f"build {name} needs exactly two runs")
    voids = [r for name, runs in builds.items() for r in void_reasons(name, runs)]
    if voids:
        raise GateError("void (§28.5): " + "; ".join(voids))

    def pair(engine: str, pct: int, t: int) -> list[dict[str, Any]]:
        return [ratio(cell(builds["R"][k], engine, pct, t), cell(builds["B"][k], engine, pct, t)) for k in range(2)]

    out: dict[str, Any] = {"gates": [], "reported": [], "controls": []}
    drift = False
    for engine, pct, t in CONTROLS:
        runs = pair(engine, pct, t)
        signs = [moved(x) for x in runs]
        same_way = signs[0] != 0 and signs[0] == signs[1]
        drift |= same_way
        out["controls"].append({"cell": f"{engine} {pct}% read T={t}", "R/B": runs, "moved_both_runs": same_way})
    for engine, pct, t, floor in GATES:
        runs = pair(engine, pct, t)
        out["gates"].append({"id": "G3", "comparison": "R/B", "cell": f"{engine} {pct}% read T={t}", "floor": floor,
                             "runs": runs, "verdict": "DRIFT" if drift else gate_verdict(runs, floor)})
    for engine, pct, t in REPORTED:
        out["reported"].append({"comparison": "R/B", "cell": f"{engine} {pct}% read T={t}", "runs": pair(engine, pct, t)})
    out["drift"] = drift
    out["control_band"] = CONTROL_BAND
    return out


def _synthetic(blob16: float, ctrl: float = 1.0, commit: str = "c0ffee") -> dict[str, Any]:
    levels = {("blob", 50, 1): 12e6, ("blob", 50, 4): 20e6, ("blob", 50, 16): blob16, ("blob", 100, 16): 270e6,
              ("blob_mutex", 50, 4): 6e6, ("blob_mutex", 50, 16): 3e6,
              ("map", 50, 16): 65e6 * ctrl, ("str", 50, 16): 53e6}
    cells = []
    for (engine, pct, t), level in levels.items():
        raw = [{"read_ops": int(level * (1 + 0.01 * (r % 5)) / 2), "write_ops": int(level * (1 + 0.01 * (r % 5)) / 2),
                "elapsed_s": 0.5} for r in range(18)]
        cells.append({"engine_key": engine, "read_pct": pct, "threads": t, "rounds_raw": raw})
    return {"provenance": {"commit": commit}, "throughput": cells}


def self_test() -> int:
    b = [_synthetic(13e6) for _ in range(2)]
    # Equal throughput clears the 0.95 floor: non-inferiority, not improvement.
    same = [_synthetic(13e6, commit="r") for _ in range(2)]
    v = evaluate({"B": b, "R": same})
    assert v["gates"][0]["verdict"] == "PASS" and not v["drift"], v["gates"]
    for g in v["gates"] + v["reported"]:
        for r in g["runs"]:
            assert r["ci_method"] in CI_METHODS and r["ci_lower"] <= r["ratio"] <= r["ci_upper"]
    assert [g["cell"] for g in v["reported"]] == ["blob 50% read T=4", "blob 50% read T=1", "blob 100% read T=16",
                                                   "blob_mutex 50% read T=16", "blob_mutex 50% read T=4"]
    # 10 % slower refutes; about 5 % slower sits on the floor and reads INCONCLUSIVE.
    slower = [_synthetic(11.7e6, commit="r") for _ in range(2)]
    assert evaluate({"B": b, "R": slower})["gates"][0]["verdict"] == "REFUTED"
    edge = [_synthetic(13e6 * 0.95, commit="r") for _ in range(2)]
    assert evaluate({"B": b, "R": edge})["gates"][0]["verdict"] == "INCONCLUSIVE"
    # A control moved the same way in both runs voids the reading.
    drifted = [_synthetic(13e6, ctrl=1.2, commit="r") for _ in range(2)]
    v = evaluate({"B": b, "R": drifted})
    assert v["drift"] and v["gates"][0]["verdict"] == "DRIFT"
    # Mixed commits within a build, and a missing run, fail loudly.
    mixed = [_synthetic(13e6, commit="r"), _synthetic(13e6, commit="r2")]
    for bad in ({"B": b, "R": mixed}, {"B": b, "R": same[:1]}):
        try:
            evaluate(bad)
        except GateError:
            pass
        else:
            raise AssertionError("an invalid input was evaluated")
    assert CI_METHOD_BCA in CI_METHODS
    print("blob_reclaim_gate.py self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--b", help="build B: RUN1,RUN2 artifact paths")
    ap.add_argument("--r", help="build R: RUN1,RUN2 artifact paths")
    ap.add_argument("--out", help="verdict JSON path")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    builds = {"B": [load(Path(p)) for p in args.b.split(",")], "R": [load(Path(p)) for p in args.r.split(",")]}
    verdict = evaluate(builds)
    verdict["inputs"] = {"B": args.b, "R": args.r}
    text = json.dumps(verdict, indent=2)
    if args.out:
        Path(args.out).write_text(text + "\n")
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
