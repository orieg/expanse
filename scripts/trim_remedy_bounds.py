#!/usr/bin/env python3
"""scripts/trim_remedy_bounds.py — Step 0 bounds for a page cache across tree
drops as a remedy for the stress-ng 4 Mi delete gap (#1680).

AGENTS.md §8.8 commit 1: the bounds are functions with pinned tests, and the
audit (`results/stressng_drop_cache_1680_step0_audit.yaml`) records what they
return for the committed artifacts. Nothing here is a new measurement.

What is bounded. The candidate keeps a dropped tree's memory in a bounded,
process-wide cache so the next tree reuses it and glibc neither trims nor
regrows the heap. Three questions decide whether it can pay, each answered
from runs that already exist:

1. How much of the give-back happens at the drop, where such a cache fills?
   `drop_share` over the trim census.
2. What does removing every trim buy? The runs with glibc trimming fixed off
   are that state for all arms; `rate_ratio` of an arm with trimming off over
   the same arm under the registered settings is the ceiling. Cross-run: the
   two settings were measured in different sessions.
3. Against stock, what is the ceiling? The trimming-off interval of the arm
   over stock, read from the artifact.

Sources:
  - mallopt(3), Linux Programmer's Manual: M_TRIM_THRESHOLD and M_TOP_PAD,
    the documented trigger for a shrinking `brk` inside `free`.
  - docs/benchmarks/stressng_comparison/README.md, "Attribution: where main's
    heap trims come from" and "The `BranchU` size class on the reference
    host: a recorded negative", with the artifacts those sections name.

Usage:
    trim_remedy_bounds.py --self-test
    trim_remedy_bounds.py --audit      # prints the audit's computed values
"""
from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

RESULTS = Path(__file__).resolve().parent.parent / "docs/benchmarks/stressng_comparison/results"
DELETE = "judy-delete-operations-per-sec"


def drop_share(pages_at_drop: int, pages_given_back: int) -> float:
    """Fraction of a process tree's given-back pages that are given back by
    the call that drops the tree: the most a cache filled at the drop can
    keep from glibc."""
    if pages_at_drop < 0 or pages_given_back <= 0 or pages_at_drop > pages_given_back:
        raise ValueError("need 0 <= pages_at_drop <= pages_given_back, pages_given_back > 0")
    return pages_at_drop / pages_given_back


def rate_ratio(rate: float, base_rate: float) -> float:
    """Ratio of two mean rates. A ceiling when `rate` is an arm with no trim
    left and `base_rate` the arm it is compared with."""
    if rate <= 0 or base_rate <= 0:
        raise ValueError("rates must be positive")
    return rate / base_rate


def ceiling_clears(ceiling: float, floor: float) -> bool:
    """A remedy can meet a floor only if its ceiling is above it."""
    if ceiling <= 0 or floor <= 0:
        raise ValueError("ceiling and floor must be positive")
    return ceiling > floor


def census_drop_share(census: dict, arm: str, w: int) -> float:
    """`drop_share` for one census cell; both repeats must agree exactly."""
    shares = []
    for rep in sorted(census["cells"][f"{arm}/W{w}"]):
        phases = census["cells"][f"{arm}/W{w}"][rep]["phases"]
        total = sum(p["pages_trimmed"] for p in phases.values())
        shares.append(drop_share(phases["teardown"]["pages_trimmed"], total))
    if len(set(shares)) != 1:
        raise ValueError(f"census repeats disagree for {arm}/W{w}: {shares}")
    return shares[0]


def delete_cell(artifact: dict, w: int, pair: str) -> dict:
    """The delete-rate row of one (W, pair) in an increment artifact."""
    rows = [c for c in artifact["cells"] if c["w"] == w and c["pair"] == pair and c["metric"] == DELETE]
    if len(rows) != 1:
        raise ValueError(f"expected one delete row for W={w} {pair}, found {len(rows)}")
    return rows[0]


def load(name: str) -> dict:
    return json.loads((RESULTS / name).read_text())


def audit() -> dict:
    """Every value the Step 0 audit records, computed from the artifacts.
    Arms: M main `4af9e927f`, G the `BranchU` size class `f4b78d1bf`, S stock.
    """
    census = load("diagnostic_trim_census_m1_amd64.json")
    out: dict = {"drop_share": {}, "runs": {}}
    for arm in ("N", "P"):
        for w in (1, 4, 8):
            out["drop_share"][f"{arm}/W{w}"] = round(census_drop_share(census, arm, w), 4)
    for run in (1, 2):
        reg = load(f"diagnostic_increment_heap_trim_class_ref_run{run}.json")
        off = load(f"diagnostic_increment_heap_trim_class_trimoff_ref_run{run}.json")
        for w in (8, 4):
            m_reg = delete_cell(reg, w, "G/M")["mean_M"]
            g_reg = delete_cell(reg, w, "G/M")["mean_G"]
            m_off = delete_cell(off, w, "G/M")["mean_M"]
            g_off = delete_cell(off, w, "G/M")["mean_G"]
            gs_off = delete_cell(off, w, "G/S")
            ms_off = delete_cell(off, w, "M/S")
            out["runs"][f"run{run}/W{w}"] = {
                # no trim left, main's code otherwise unchanged
                "main_no_trim_over_main": round(rate_ratio(m_off, m_reg), 4),
                # the pooled class with no trim left, over main as registered
                "class_no_trim_over_main": round(rate_ratio(g_off, m_reg), 4),
                # what full retention adds to the pooled class
                "class_no_trim_over_class": round(rate_ratio(g_off, g_reg), 4),
                "class_no_trim_over_stock_upper": round(gs_off["ci_upper"], 4),
                "main_no_trim_over_stock_upper": round(ms_off["ci_upper"], 4),
            }
    return out


class TestTrimRemedyBounds(unittest.TestCase):
    def test_drop_share(self) -> None:
        self.assertAlmostEqual(drop_share(1, 4), 0.25)
        self.assertEqual(drop_share(0, 9), 0.0)
        self.assertEqual(drop_share(9, 9), 1.0)

    def test_rate_ratio_and_floor(self) -> None:
        self.assertAlmostEqual(rate_ratio(3.0, 2.0), 1.5)
        self.assertTrue(ceiling_clears(1.01, 1.0))
        self.assertFalse(ceiling_clears(1.0, 1.0))
        self.assertFalse(ceiling_clears(0.996, 1.0))

    def test_invalid_inputs(self) -> None:
        for args in ((-1, 4), (5, 4), (1, 0)):
            with self.assertRaises(ValueError):
                drop_share(*args)
        for args in ((0.0, 1.0), (1.0, 0.0)):
            with self.assertRaises(ValueError):
                rate_ratio(*args)
            with self.assertRaises(ValueError):
                ceiling_clears(*args)

    def test_census_pins(self) -> None:
        # Exact counts of results/diagnostic_trim_census_m1_amd64.json.
        self.assertAlmostEqual(drop_share(7231, 202477), 0.0357, places=4)
        a = audit()["drop_share"]
        self.assertEqual(a["N/W8"], 0.0357)
        self.assertEqual(a["N/W4"], 0.0026)
        self.assertEqual(a["N/W1"], 0.2517)
        self.assertEqual((a["P/W1"], a["P/W4"], a["P/W8"]), (1.0, 1.0, 1.0))

    def test_rate_pins(self) -> None:
        r = audit()["runs"]
        self.assertEqual(r["run1/W8"]["main_no_trim_over_main"], 1.0188)
        self.assertEqual(r["run2/W8"]["main_no_trim_over_main"], 1.018)
        self.assertEqual(r["run1/W8"]["class_no_trim_over_main"], 0.9959)
        self.assertEqual(r["run2/W8"]["class_no_trim_over_main"], 0.9958)
        self.assertEqual(r["run1/W8"]["class_no_trim_over_class"], 1.0016)
        # The verdict's two premises, on every cell of both runs.
        for cell, v in r.items():
            self.assertLess(v["class_no_trim_over_stock_upper"], 1.0, cell)
            if cell.endswith("W8"):
                self.assertFalse(ceiling_clears(v["class_no_trim_over_main"], 1.0), cell)
                self.assertLess(v["main_no_trim_over_stock_upper"], 1.0, cell)


def main() -> int:
    if "--audit" in sys.argv:
        print(json.dumps(audit(), indent=2))
        return 0
    if "--self-test" in sys.argv:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestTrimRemedyBounds)
        ok = unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful()
        print("self-test passed" if ok else "self-test FAILED")
        return 0 if ok else 1
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
