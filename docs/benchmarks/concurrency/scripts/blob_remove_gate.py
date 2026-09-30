#!/usr/bin/env python3
"""METHODOLOGY §26's evaluation: the optimistic `SyncExpanseBlobMap` removal (#1280).

Reads `core_concurrency` artifacts written by `mixed_concurrency.py` for the
builds §26.2 names (B, M, C, A), two runs each, and reads every registered
comparison from them. A comparison X ÷ Y is the ratio of mean total ops/s over
one cell's windows, run k of X against run k of Y, with a two-sample BCa 95 %
interval (`scripts/bca_bootstrap.py`). Nothing here re-derives a threshold:
the bounds and floors are §26's, restated as constants below and pinned by the
self-test.

Verdicts, per comparison and per run pair, then across the two:

- a gated ratio is `PASS` when its lower bound clears the floor in both runs,
  `REFUTED` when its upper bound is below the floor in both, `INCONCLUSIVE`
  otherwise;
- the scaling gate reads C's own paired C(16) from each artifact;
- a control cell moved when its interval excludes [1 - CONTROL_BAND,
  1 + CONTROL_BAND]; a control that moved the same way in both runs marks the
  whole comparison `DRIFT` (docs/BENCHMARKING.md rule 18), and its gates are
  then not read.

Usage:
    python3 docs/benchmarks/concurrency/scripts/blob_remove_gate.py \\
        --b RUN1,RUN2 --m RUN1,RUN2 --c RUN1,RUN2 [--a RUN1,RUN2] --out VERDICT.json
    python3 docs/benchmarks/concurrency/scripts/blob_remove_gate.py --self-test
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from bca_bootstrap import (  # noqa: E402
    CI_METHOD_BC,
    CI_METHOD_BCA,
    CI_METHOD_CLAMPED,
    CI_METHOD_DEGENERATE,
    bca_bootstrap_ratio_ci_with_method,
)

CI_METHODS = {CI_METHOD_BCA, CI_METHOD_BC, CI_METHOD_CLAMPED, CI_METHOD_DEGENERATE}

ENGINE = "blob"
MIXED_READ_PCT = 50
# §26.4: the gated comparisons, each (numerator, denominator, threads, floor).
GATES = (
    ("C", "B", 16, 1.0),
    ("C", "B", 4, 1.0),
    ("C", "B", 1, 0.90),
)
# §26.4: reported, not gated.
REPORTED = (
    ("M", "B", 16),
    ("M", "B", 4),
    ("A", "C", 16),
    ("A", "C", 4),
)
# §26.4: the scaling gate on C's own paired C(16).
SCALING_BUILD = "C"
SCALING_THREADS = 16
SCALING_FLOOR = 1.0
# §26.5: cells the change should not move, and the band that defines "moved".
CONTROLS = (
    ("blob", 100, 16),
    ("blob", 100, 1),
    ("map", 50, 16),
    ("map", 100, 16),
)
CONTROL_BAND = 0.05


class GateError(RuntimeError):
    """An input that cannot be evaluated (AGENTS.md §8.1)."""


def load(path: Path) -> dict[str, Any]:
    art = json.loads(path.read_text())
    if "throughput" not in art or "provenance" not in art:
        raise GateError(f"{path}: not a mixed_concurrency.py artifact")
    return art


def cell(art: dict[str, Any], engine: str, read_pct: int, threads: int) -> dict[str, Any]:
    hits = [c for c in art["throughput"]
            if c["engine_key"] == engine and c.get("read_pct") == read_pct and c["threads"] == threads]
    if len(hits) != 1:
        raise GateError(f"{engine} {read_pct}% read, {threads} threads: {len(hits)} cells, expected 1")
    return hits[0]


def window_totals(c: dict[str, Any]) -> list[float]:
    return [(w["read_ops"] + w["write_ops"]) / w["elapsed_s"] for w in c["rounds_raw"]]


def ratio(num: dict[str, Any], den: dict[str, Any]) -> dict[str, Any]:
    r, lo, hi, method = bca_bootstrap_ratio_ci_with_method(window_totals(num), window_totals(den))
    return {"ratio": r, "ci_lower": lo, "ci_upper": hi, "ci_method": method}


def gate_verdict(runs: list[dict[str, Any]], floor: float) -> str:
    if all(x["ci_lower"] > floor for x in runs):
        return "PASS"
    if all(x["ci_upper"] < floor for x in runs):
        return "REFUTED"
    return "INCONCLUSIVE"


def moved(x: dict[str, Any]) -> int:
    """+1 when the interval sits above the band, -1 below, 0 when it overlaps it."""
    if x["ci_lower"] > 1 + CONTROL_BAND:
        return 1
    if x["ci_upper"] < 1 - CONTROL_BAND:
        return -1
    return 0


def void_reasons(name: str, runs: list[dict[str, Any]]) -> list[str]:
    """§26.5's voids that the artifacts themselves show."""
    reasons = []
    commits = {r["provenance"].get("commit") for r in runs}
    if len(commits) != 1:
        reasons.append(f"build {name}: its runs measured different commits {sorted(map(str, commits))}")
    for k, art in enumerate(runs, start=1):
        c = cell(art, ENGINE, MIXED_READ_PCT, SCALING_THREADS)
        if sum(w.get("compactions", 0) for w in c["rounds_raw"]) == 0:
            reasons.append(f"build {name} run {k}: no compaction in the 50% read, 16-thread blob cell")
    return reasons


def evaluate(builds: dict[str, list[dict[str, Any]]]) -> dict[str, Any]:
    for name in ("B", "C"):
        if len(builds.get(name, [])) != 2:
            raise GateError(f"build {name} needs exactly two runs")
    voids = [r for name, runs in builds.items() for r in void_reasons(name, runs)]
    if voids:
        raise GateError("void (§26.5): " + "; ".join(voids))
    out: dict[str, Any] = {"gates": [], "reported": [], "controls": [], "scaling": None}
    drift = False
    for engine, pct, t in CONTROLS:
        runs = [ratio(cell(builds["C"][k], engine, pct, t), cell(builds["B"][k], engine, pct, t)) for k in range(2)]
        signs = [moved(x) for x in runs]
        same_way = signs[0] != 0 and signs[0] == signs[1]
        drift |= same_way
        out["controls"].append({"cell": f"{engine} {pct}% read T={t}", "C/B": runs, "moved_both_runs": same_way})
    for num, den, t, floor in GATES:
        runs = [ratio(cell(builds[num][k], ENGINE, MIXED_READ_PCT, t), cell(builds[den][k], ENGINE, MIXED_READ_PCT, t))
                for k in range(2)]
        out["gates"].append({"comparison": f"{num}/{den}", "threads": t, "floor": floor, "runs": runs,
                             "verdict": "DRIFT" if drift else gate_verdict(runs, floor)})
    for num, den, t in REPORTED:
        if len(builds.get(num, [])) != 2 or len(builds.get(den, [])) != 2:
            continue
        runs = [ratio(cell(builds[num][k], ENGINE, MIXED_READ_PCT, t), cell(builds[den][k], ENGINE, MIXED_READ_PCT, t))
                for k in range(2)]
        out["reported"].append({"comparison": f"{num}/{den}", "threads": t, "runs": runs})
    sc = []
    for art in builds[SCALING_BUILD]:
        c = cell(art, ENGINE, MIXED_READ_PCT, SCALING_THREADS)
        sc.append({"ratio": c["scaling_c_n_mean"], "ci_lower": c["scaling_c_n_ci_lower"],
                   "ci_upper": c["scaling_c_n_ci_upper"], "ci_method": c["scaling_c_n_ci_method"]})
    out["scaling"] = {"build": SCALING_BUILD, "threads": SCALING_THREADS, "floor": SCALING_FLOOR, "runs": sc,
                      "verdict": gate_verdict(sc, SCALING_FLOOR)}
    out["drift"] = drift
    return out


def _synthetic(engine_scale: dict[tuple[str, int, int], float]) -> dict[str, Any]:
    cells = []
    for (engine, pct, t), level in engine_scale.items():
        raw = [{"read_ops": int(level * (1 + 0.01 * (r % 5)) / 2), "write_ops": int(level * (1 + 0.01 * (r % 5)) / 2),
                "elapsed_s": 1.0, "compactions": 3 if engine == "blob" else 0} for r in range(18)]
        c = {"engine_key": engine, "read_pct": pct, "threads": t, "rounds_raw": raw}
        if t == 16:
            base = engine_scale[(engine, pct, 1)]
            c.update(scaling_c_n_mean=level / base, scaling_c_n_ci_lower=level / base * 0.97,
                     scaling_c_n_ci_upper=level / base * 1.03, scaling_c_n_ci_method=CI_METHOD_BCA)
        cells.append(c)
    return {"provenance": {"commit": "c0ffee"}, "throughput": cells}


def self_test() -> int:
    def levels(blob16: float, blob4: float, blob1: float, ctrl: float = 1.0) -> dict[tuple[str, int, int], float]:
        return {("blob", 50, 16): blob16, ("blob", 50, 4): blob4, ("blob", 50, 1): blob1,
                ("blob", 100, 16): 300e6 * ctrl, ("blob", 100, 1): 34e6 * ctrl,
                ("map", 50, 16): 100e6, ("map", 100, 16): 350e6, ("map", 50, 1): 28e6, ("map", 100, 1): 38e6}

    b = [_synthetic(levels(8e6, 10e6, 18e6)) for _ in range(2)]
    c = [_synthetic(levels(40e6, 30e6, 17e6)) for _ in range(2)]
    v = evaluate({"B": b, "C": c})
    by = {(g["comparison"], g["threads"]): g["verdict"] for g in v["gates"]}
    assert by[("C/B", 16)] == "PASS" and by[("C/B", 4)] == "PASS", by
    # The single-thread price: 17/18 = 0.94 clears the 0.90 floor.
    assert by[("C/B", 1)] == "PASS", by
    assert v["scaling"]["verdict"] == "PASS" and not v["drift"]
    for g in v["gates"]:
        for r in g["runs"]:
            assert r["ci_method"] in CI_METHODS and r["ci_lower"] <= r["ratio"] <= r["ci_upper"]
    # A slower head refutes; a price below the floor refutes the price gate.
    worse = [_synthetic(levels(6e6, 8e6, 14e6)) for _ in range(2)]
    v = evaluate({"B": b, "C": worse})
    by = {(g["comparison"], g["threads"]): g["verdict"] for g in v["gates"]}
    assert by[("C/B", 16)] == "REFUTED" and by[("C/B", 1)] == "REFUTED", by
    assert v["scaling"]["verdict"] == "REFUTED"
    # A control that moved the same way in both runs voids the gates.
    drifted = [_synthetic(levels(40e6, 30e6, 17e6, ctrl=1.2)) for _ in range(2)]
    v = evaluate({"B": b, "C": drifted})
    assert v["drift"] and all(g["verdict"] == "DRIFT" for g in v["gates"])
    # A control that moved in one run only does not.
    mixed = [_synthetic(levels(40e6, 30e6, 17e6, ctrl=1.2)), _synthetic(levels(40e6, 30e6, 17e6))]
    assert not evaluate({"B": b, "C": mixed})["drift"]
    # §26.5 voids: a blob cell whose trigger never fired, or a build whose runs
    # measured different commits.
    idle = [_synthetic(levels(40e6, 30e6, 17e6)) for _ in range(2)]
    for w in cell(idle[1], "blob", 50, 16)["rounds_raw"]:
        w["compactions"] = 0
    other = [_synthetic(levels(40e6, 30e6, 17e6)) for _ in range(2)]
    other[1]["provenance"]["commit"] = "deadbeef"
    for bad in (idle, other):
        try:
            evaluate({"B": b, "C": bad})
        except GateError as e:
            assert "void" in str(e)
        else:
            raise AssertionError("a void run was evaluated")
    # Missing input fails loudly.
    try:
        evaluate({"B": b[:1], "C": c})
    except GateError:
        pass
    else:
        raise AssertionError("one run of B was accepted")
    try:
        cell(b[0], "blob", 50, 8)
    except GateError:
        pass
    else:
        raise AssertionError("a missing cell was accepted")
    print("blob_remove_gate.py self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    for name in ("b", "m", "c", "a"):
        ap.add_argument(f"--{name}", help=f"build {name.upper()}: RUN1,RUN2 artifact paths")
    ap.add_argument("--out", help="verdict JSON path")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    builds = {}
    for name in ("b", "m", "c", "a"):
        v = getattr(args, name)
        if v:
            builds[name.upper()] = [load(Path(p)) for p in v.split(",")]
    verdict = evaluate(builds)
    verdict["inputs"] = {k: getattr(args, k.lower()) for k in builds}
    text = json.dumps(verdict, indent=2)
    if args.out:
        Path(args.out).write_text(text + "\n")
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
