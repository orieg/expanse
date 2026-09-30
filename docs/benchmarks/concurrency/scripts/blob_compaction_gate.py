#!/usr/bin/env python3
"""METHODOLOGY §27's evaluation: does in-window compaction set the blob map's four-thread peak (#1280)?

Reads `core_concurrency` artifacts from `mixed_concurrency.py` for §27's two
builds, D (the default trigger, `BLOB_COMPACT_APPENDS = BLOB_POP`) and K (the
trigger at four times that), two runs each, and reads every registered
comparison: K ÷ D per cell as the ratio of mean total ops/s with a two-sample
BCa 95 % interval, and the share of window time each build spent compacting
(`compaction_time_share`, from the per-window `compact_ns`). The verdict rules
and the drift controls are `blob_remove_gate.py`'s, applied to §27's cells.

Usage:
    python3 docs/benchmarks/concurrency/scripts/blob_compaction_gate.py \\
        --d RUN1,RUN2 --k RUN1,RUN2 --out VERDICT.json
    python3 docs/benchmarks/concurrency/scripts/blob_compaction_gate.py --self-test
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

MIXED_READ_PCT = 50
# §27.4: the gated comparison (arm, threads, floor).
GATES = (("blob", 16, 1.0),)
# §27.4: reported, not gated.
REPORTED = (("blob", 4), ("blob", 1), ("blob_mutex", 16), ("blob_mutex", 4))
# §27.5: cells the trigger cannot reach.
CONTROLS = (("blob", 100, 16), ("map", 50, 16), ("str", 50, 16))
SHARE_CELLS = (("blob", 1), ("blob", 4), ("blob", 16), ("blob_mutex", 16))


def window_share(c: dict[str, Any]) -> float:
    rows = c["rounds_raw"]
    if any("compact_ns" not in w for w in rows):
        raise GateError(f"{c['engine_key']} {c['threads']} threads: windows carry no compact_ns")
    return sum(w["compact_ns"] for w in rows) / (1e9 * sum(w["elapsed_s"] for w in rows))


def void_reasons(name: str, runs: list[dict[str, Any]]) -> list[str]:
    """§27.6's voids that the artifacts show."""
    reasons = []
    commits = {r["provenance"].get("commit") for r in runs}
    if len(commits) != 1:
        reasons.append(f"build {name}: its runs measured different commits {sorted(map(str, commits))}")
    for k, art in enumerate(runs, start=1):
        c = cell(art, "blob", MIXED_READ_PCT, 16)
        if sum(w.get("compactions", 0) for w in c["rounds_raw"]) == 0:
            reasons.append(f"build {name} run {k}: no compaction in the 50% read, 16-thread blob cell")
    return reasons


def evaluate(builds: dict[str, list[dict[str, Any]]]) -> dict[str, Any]:
    for name in ("D", "K"):
        if len(builds.get(name, [])) != 2:
            raise GateError(f"build {name} needs exactly two runs")
    voids = [r for name, runs in builds.items() for r in void_reasons(name, runs)]
    if voids:
        raise GateError("void (§27.6): " + "; ".join(voids))
    out: dict[str, Any] = {"gates": [], "reported": [], "controls": [], "shares": []}
    drift = False
    for engine, pct, t in CONTROLS:
        runs = [ratio(cell(builds["K"][k], engine, pct, t), cell(builds["D"][k], engine, pct, t)) for k in range(2)]
        signs = [moved(x) for x in runs]
        same_way = signs[0] != 0 and signs[0] == signs[1]
        drift |= same_way
        out["controls"].append({"cell": f"{engine} {pct}% read T={t}", "K/D": runs, "moved_both_runs": same_way})
    for engine, t, floor in GATES:
        runs = [ratio(cell(builds["K"][k], engine, MIXED_READ_PCT, t), cell(builds["D"][k], engine, MIXED_READ_PCT, t))
                for k in range(2)]
        out["gates"].append({"comparison": "K/D", "cell": f"{engine} 50% read T={t}", "floor": floor, "runs": runs,
                             "verdict": "DRIFT" if drift else gate_verdict(runs, floor)})
    for engine, t in REPORTED:
        runs = [ratio(cell(builds["K"][k], engine, MIXED_READ_PCT, t), cell(builds["D"][k], engine, MIXED_READ_PCT, t))
                for k in range(2)]
        out["reported"].append({"comparison": "K/D", "cell": f"{engine} 50% read T={t}", "runs": runs})
    for engine, t in SHARE_CELLS:
        out["shares"].append({"cell": f"{engine} 50% read T={t}",
                              "D": [window_share(cell(a, engine, MIXED_READ_PCT, t)) for a in builds["D"]],
                              "K": [window_share(cell(a, engine, MIXED_READ_PCT, t)) for a in builds["K"]]})
    out["drift"] = drift
    out["control_band"] = CONTROL_BAND
    return out


def _synthetic(blob16: float, share16: float, ctrl: float = 1.0, commit: str = "c0ffee") -> dict[str, Any]:
    levels = {("blob", 50, 1): 12e6, ("blob", 50, 4): 20e6, ("blob", 50, 16): blob16,
              ("blob_mutex", 50, 4): 6e6, ("blob_mutex", 50, 16): 3e6,
              ("blob", 100, 16): 270e6 * ctrl, ("map", 50, 16): 65e6, ("str", 50, 16): 53e6}
    cells = []
    for (engine, pct, t), level in levels.items():
        share = share16 if t == 16 else share16 / 2
        raw = [{"read_ops": int(level * (1 + 0.01 * (r % 5)) / 2), "write_ops": int(level * (1 + 0.01 * (r % 5)) / 2),
                "elapsed_s": 0.5, "compactions": 9 if engine.startswith("blob") else 0,
                "compact_ns": int(share * 0.5e9) if engine.startswith("blob") else 0} for r in range(18)]
        cells.append({"engine_key": engine, "read_pct": pct, "threads": t, "rounds_raw": raw})
    return {"provenance": {"commit": commit}, "throughput": cells}


def self_test() -> int:
    d = [_synthetic(13e6, 0.20) for _ in range(2)]
    k = [_synthetic(17e6, 0.05, commit="k") for _ in range(2)]
    v = evaluate({"D": d, "K": k})
    assert v["gates"][0]["verdict"] == "PASS" and not v["drift"], v["gates"]
    for g in v["gates"] + v["reported"]:
        for r in g["runs"]:
            assert r["ci_method"] in CI_METHODS and r["ci_lower"] <= r["ratio"] <= r["ci_upper"]
    blob16 = [s for s in v["shares"] if s["cell"] == "blob 50% read T=16"][0]
    assert abs(blob16["D"][0] - 0.20) < 1e-9 and abs(blob16["K"][0] - 0.05) < 1e-9, blob16
    # No effect refutes nothing; a slower K refutes; a moved control voids the gate.
    same = [_synthetic(13e6, 0.05, commit="k") for _ in range(2)]
    assert evaluate({"D": d, "K": same})["gates"][0]["verdict"] == "INCONCLUSIVE"
    slower = [_synthetic(10e6, 0.05, commit="k") for _ in range(2)]
    assert evaluate({"D": d, "K": slower})["gates"][0]["verdict"] == "REFUTED"
    drifted = [_synthetic(17e6, 0.05, ctrl=1.2, commit="k") for _ in range(2)]
    v = evaluate({"D": d, "K": drifted})
    assert v["drift"] and v["gates"][0]["verdict"] == "DRIFT"
    # §27.6 voids and missing inputs fail loudly.
    mixed_commits = [_synthetic(17e6, 0.05, commit="k"), _synthetic(17e6, 0.05, commit="k2")]
    no_compact = [_synthetic(17e6, 0.05, commit="k") for _ in range(2)]
    for w in cell(no_compact[0], "blob", 50, 16)["rounds_raw"]:
        w["compactions"] = 0
    for bad in (mixed_commits, no_compact):
        try:
            evaluate({"D": d, "K": bad})
        except GateError as e:
            assert "void" in str(e)
        else:
            raise AssertionError("a void run was evaluated")
    old = [_synthetic(13e6, 0.2) for _ in range(2)]
    for w in cell(old[0], "blob", 50, 16)["rounds_raw"]:
        del w["compact_ns"]
    try:
        evaluate({"D": old, "K": k})
    except GateError:
        pass
    else:
        raise AssertionError("an artifact without compact_ns was evaluated")
    assert CI_METHOD_BCA in CI_METHODS
    print("blob_compaction_gate.py self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--d", help="build D: RUN1,RUN2 artifact paths")
    ap.add_argument("--k", help="build K: RUN1,RUN2 artifact paths")
    ap.add_argument("--out", help="verdict JSON path")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    builds = {"D": [load(Path(p)) for p in args.d.split(",")], "K": [load(Path(p)) for p in args.k.split(",")]}
    verdict = evaluate(builds)
    verdict["inputs"] = {"D": args.d, "K": args.k}
    text = json.dumps(verdict, indent=2)
    if args.out:
        Path(args.out).write_text(text + "\n")
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
