#!/usr/bin/env python3
"""scripts/root_skip_bounds.py — Step 0 bounds for entering a tree below its
one-child top node (#1682, #1680).

AGENTS.md §8.8 commit 1: the bounds are functions with pinned tests, and the
pre-registration (`results/root_skip_1682_preregistration.yaml`) records what
they return for committed artifacts. Nothing here is a new measurement.

What is bounded. A key set whose keys share their top byte puts a one-child
branch at level 8; every lookup, insert and removal crosses it. The candidate
enters the tree below it. Two committed measurements bound what that can buy:

- `results/colour_top_skip_02d1ee099_run{1,2}.json`: cycles and instructions
  saved per Judy1 test by entering at the top node's child, on the reference
  host. An upper bound: it also skips the root match a real change keeps, and
  it was measured on the set lookup, not the map's.
- `docs/benchmarks/stressng_comparison/results/
  diagnostic_increment_hot_layout_reg3_{ref,zen5}_run{1,2}.json`: each cell's
  mean rate for the layout build (G) and stock (S), with the G/S interval.

A rate R (operations per second per worker) is a time per operation 1/R. A
saving of t seconds per operation turns a ratio r against stock into
r / (1 - t*R). Reaching parity needs t >= (1 - r) / R. The saving is known in
cycles, so comparing it with that time needs the core's clock, which no
committed artifact of these runs records: the functions take it as an input
and the audit evaluates a stated bracket.

Usage:
    root_skip_bounds.py --self-test
    root_skip_bounds.py --audit
"""
from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SNG = ROOT / "docs/benchmarks/stressng_comparison/results"
# The reference host's performance-core clock under load is not recorded by
# these runs. The bracket spans the part's documented all-core and single-core
# turbo range; a lower clock makes a cycle longer and the candidate look
# better, so the upper end is the conservative one.
REF_HZ_BRACKET = (4.5e9, 5.1e9)

CELLS = [
    ("sng_sm_judy_default", 1, "judy-gets-per-sec"),
    ("sng_sm_judy_default", 8, "judy-gets-per-sec"),
    ("sng_sm_judy_big", 8, "judy-gets-per-sec"),
    ("sng_judy_4m", 1, "judy-delete-operations-per-sec"),
    ("sng_judy_4m", 2, "judy-delete-operations-per-sec"),
    ("sng_judy_4m", 4, "judy-delete-operations-per-sec"),
    ("sng_judy_4m", 1, "judy-insert-operations-per-sec"),
    ("sng_judy_4m", 8, "judy-insert-operations-per-sec"),
]


def needed_seconds(ratio: float, rate: float) -> float:
    """Time per operation the faster arm must lose for `ratio` to reach 1."""
    if ratio <= 0 or rate <= 0:
        raise ValueError("ratio and rate must be positive")
    return max(0.0, (1.0 - ratio) / rate)


def needed_cycles(ratio: float, rate: float, hz: float) -> float:
    """`needed_seconds` in cycles of a core clocked at `hz`."""
    if hz <= 0:
        raise ValueError("hz must be positive")
    return needed_seconds(ratio, rate) * hz


def ratio_after(ratio: float, rate: float, saved_cycles: float, hz: float) -> float:
    """The ratio against stock after `saved_cycles` leave every operation."""
    if ratio <= 0 or rate <= 0 or hz <= 0 or saved_cycles < 0:
        raise ValueError("inputs must be positive (saving non-negative)")
    share = saved_cycles / hz * rate
    if share >= 1.0:
        raise ValueError("the saving exceeds the whole operation")
    return ratio / (1.0 - share)


def instruction_share(saved: float, per_op: float) -> float:
    """Share of an operation's instructions a saving removes."""
    if saved < 0 or per_op <= 0 or saved > per_op:
        raise ValueError("need 0 <= saved <= per_op")
    return saved / per_op


def top_skip_saving() -> dict:
    """Saved cycles per test, lowest lower end and highest upper end over
    both runs and both populations, and the instructions saved."""
    lo, hi, ins = [], [], []
    for run in (1, 2):
        d = json.loads((ROOT / f"results/colour_top_skip_02d1ee099_run{run}.json").read_text())
        for c in d["cells"]:
            lo.append(c["saved_cycles_per_test"]["ci_lower"])
            hi.append(c["saved_cycles_per_test"]["ci_upper"])
            ins.append(c["saved_instructions_per_test"]["mean"])
    return {"cycles_low": min(lo), "cycles_high": max(hi), "instructions": sum(ins) / len(ins)}


def cell(host: str, run: int, workload: str, w: int, metric: str) -> dict:
    d = json.loads((SNG / f"diagnostic_increment_hot_layout_reg3_{host}_run{run}.json").read_text())
    rows = [c for c in d["cells"] if (c["workload_id"], c["w"], c["metric"], c["pair"]) == (workload, w, metric, "G/S")]
    if len(rows) != 1:
        raise ValueError(f"expected one G/S row for {workload} W={w} {metric}, found {len(rows)}")
    return rows[0]


def audit() -> dict:
    s = top_skip_saving()
    out = {"saving": {k: round(v, 3) for k, v in s.items()}, "cells": {}}
    for workload, w, metric in CELLS:
        key = f"{workload}/W{w}/{metric.split('-')[1]}"
        row: dict = {}
        for host in ("ref", "zen5"):
            for run in (1, 2):
                c = cell(host, run, workload, w, metric)
                e: dict = {
                    "ratio": round(c["ratio"], 4),
                    "lower": round(c["ci_lower"], 4),
                    # What the interval's lower end needs to reach 1.
                    "needed_ns": round(needed_seconds(c["ci_lower"], c["mean_G"]) * 1e9, 3),
                }
                if host == "ref":
                    # Conservative pairing: the shortest cycle with the least
                    # saving, and the longest cycle with the most.
                    e["needed_cycles"] = [round(needed_cycles(c["ci_lower"], c["mean_G"], hz), 2) for hz in REF_HZ_BRACKET]
                    e["ratio_after"] = [
                        round(ratio_after(c["ratio"], c["mean_G"], s["cycles_low"], REF_HZ_BRACKET[1]), 4),
                        round(ratio_after(c["ratio"], c["mean_G"], s["cycles_high"], REF_HZ_BRACKET[0]), 4),
                    ]
                row[f"{host}{run}"] = e
        out["cells"][key] = row
    return out


class TestRootSkipBounds(unittest.TestCase):
    def test_needed(self) -> None:
        self.assertAlmostEqual(needed_seconds(0.95, 50e6), 1e-9)
        self.assertEqual(needed_seconds(1.02, 50e6), 0.0)
        self.assertAlmostEqual(needed_cycles(0.95, 50e6, 5e9), 5.0)

    def test_ratio_after(self) -> None:
        # 5 cycles at 5 GHz is 1 ns; at 50 M ops/s an operation is 20 ns.
        self.assertAlmostEqual(ratio_after(0.95, 50e6, 5.0, 5e9), 1.0)
        self.assertAlmostEqual(ratio_after(1.0, 50e6, 0.0, 5e9), 1.0)

    def test_instruction_share(self) -> None:
        self.assertAlmostEqual(instruction_share(27.0, 229.9), 0.11744, places=5)

    def test_invalid_inputs(self) -> None:
        for fn, args in (
            (needed_seconds, (0.0, 1.0)),
            (needed_seconds, (1.0, 0.0)),
            (needed_cycles, (0.9, 1.0, 0.0)),
            (ratio_after, (0.9, 1e9, 10.0, 5e9)),
            (ratio_after, (0.9, 1.0, -1.0, 5e9)),
            (instruction_share, (5.0, 4.0)),
        ):
            with self.assertRaises(ValueError):
                fn(*args)

    def test_artifact_pins(self) -> None:
        a = audit()
        self.assertEqual(a["saving"], {"cycles_low": 5.891, "cycles_high": 7.864, "instructions": 26.981})
        d = a["cells"]["sng_sm_judy_default/W8/gets"]["ref2"]
        self.assertEqual((d["ratio"], d["lower"]), (0.9554, 0.9525))
        # A lower end above 1 on this cell needs more than the least saving
        # the upper-bound measurement supports, at the conservative clock.
        self.assertGreater(d["needed_cycles"][1], a["saving"]["cycles_low"])
        self.assertLess(d["needed_cycles"][0], a["saving"]["cycles_high"])
        big = a["cells"]["sng_sm_judy_big/W8/gets"]["ref1"]
        self.assertLess(big["needed_cycles"][1], 1.5)


def main() -> int:
    if "--audit" in sys.argv:
        print(json.dumps(audit(), indent=1))
        return 0
    if "--self-test" in sys.argv:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestRootSkipBounds)
        ok = unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful()
        print("self-test passed" if ok else "self-test FAILED")
        return 0 if ok else 1
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
