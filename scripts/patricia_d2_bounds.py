#!/usr/bin/env python3
"""
scripts/patricia_d2_bounds.py — threshold derivations for Diagnostic D2 (Refs #1096).

Pre-registers the mathematical derivations behind Diagnostic D2 (Characterisation tier:
physical paging sensitivity and cache-line layout isolation for 1M prefix scan,
docs/benchmarks/patricia_comparison/METHODOLOGY.md §3 Amendment A4), as committed,
unit-tested code (AGENTS.md §8.8 commit 1, Rule 12 / §1.3).

Diagnostic D2 evaluates whether 2 MiB transparent huge pages
(GLIBC_TUNABLES=glibc.malloc.hugetlb=1) eliminate the 1M generator-order prefix scan
cycle gap between strmap_prefix_scan and strmap_prefix_scan_sorted, or whether
the gap is dominated by spatial cache-line dispersal across the 45 MB tree.

All clauses are evaluated on BCa 95% bootstrap confidence interval bounds (Rule 1 / §1.1):
  - Clause D2a (dTLB Elimination): BCa_upper(generator_2m.dtlb) <= dtlb_bound
  - Clause D2b (L3 Miss Invariance): BCa_lower(generator_2m.l3 - sorted_2m.l3) >= l3_bound
  - Clause D2c (Surviving Cycle Gap): BCa_lower(generator_2m.cycles - sorted_2m.cycles) >= cycle_bound

Reference values are pinned to committed D1 artifacts:
  - docs/benchmarks/patricia_comparison/results/counters_prefix_scan_d1.json (commit c3e54ec2)
  - docs/benchmarks/patricia_comparison/results/counters_prefix_scan_d1_stalls.json (commit c3e54ec2)

Usage:
  python3 scripts/patricia_d2_bounds.py            # run the pinned tests and print derived bounds
  python3 scripts/patricia_d2_bounds.py --self-test
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

# Pinned baseline figures from D1 artifact counters_prefix_scan_d1.json
# (49,934,000 total ops = 249,670 entries * 200 passes)
D1_OPS = 49_934_000
D1_GEN_DTLB_POINT = 40_962_884.5 / D1_OPS       # 0.820341 misses/entry
D1_SORTED_DTLB_POINT = 486_625.4 / D1_OPS       # 0.009745 misses/entry
D1_GEN_L3_POINT = 11_784_514.5 / D1_OPS         # 0.236002 misses/entry
D1_SORTED_L3_POINT = -2_489.7 / D1_OPS          # -0.000050 misses/entry
D1_GEN_CYCLES_POINT = 7_911_328_314.9 / D1_OPS  # 158.4357 cycles/entry
D1_SORTED_CYCLES_POINT = 3_630_337_417.8 / D1_OPS # 72.7027 cycles/entry
D1_BASELINE_DELTA_CYCLES = D1_GEN_CYCLES_POINT - D1_SORTED_CYCLES_POINT # 85.7330 cycles/entry

# Pinned baseline figures from D1 stalls artifact counters_prefix_scan_d1_stalls.json
D1_STALLS_GEN_L3_CYCLES = 2_623_482_789.9 / D1_OPS # 52.5390 stall cycles/entry
D1_STALLS_SORTED_L3_CYCLES = 1_793_431.3 / D1_OPS  # 0.0359 stall cycles/entry
D1_STALLS_DELTA_L3_CYCLES = D1_STALLS_GEN_L3_CYCLES - D1_STALLS_SORTED_L3_CYCLES # 52.5031 cycles/entry
D1_STALLS_TOTAL_DELTA_CYCLES = (7_921_202_316.5 - 3_629_647_185.4) / D1_OPS # 85.9445 cycles/entry
D1_STALLS_L3_FRACTION = D1_STALLS_DELTA_L3_CYCLES / D1_STALLS_TOTAL_DELTA_CYCLES # 0.610895 (61.1%)


def dtlb_elimination_upper_bound(baseline_gen_dtlb: float, min_reduction: float = 0.95) -> float:
    """Computes the maximum allowable BCa upper bound for dTLB load misses under huge pages.

    A reduction of >= 95% indicates successful elimination of virtual page translation overhead,
    approaching the sorted order baseline (0.0097).
    """
    if baseline_gen_dtlb <= 0:
        raise ValueError(f"baseline_gen_dtlb must be positive, got {baseline_gen_dtlb}")
    if not (0 < min_reduction < 1):
        raise ValueError(f"min_reduction must be between 0 and 1, got {min_reduction}")
    return round(baseline_gen_dtlb * (1.0 - min_reduction), 4)


def l3_invariance_lower_bound(baseline_delta_l3: float, min_retention: float = 0.60) -> float:
    """Computes the minimum allowable BCa lower bound for L3 cache misses delta under huge pages.

    Retaining >= 60% of baseline delta-L3 indicates that spatial cache-line dispersal
    persists independently of page size.
    """
    if baseline_delta_l3 <= 0:
        raise ValueError(f"baseline_delta_l3 must be positive, got {baseline_delta_l3}")
    if not (0 < min_retention <= 1):
        raise ValueError(f"min_retention must be between 0 and 1, got {min_retention}")
    return round(baseline_delta_l3 * min_retention, 4)


def surviving_cycle_gap_lower_bound(baseline_delta_cycles: float, stall_fraction: float) -> float:
    """Computes the minimum allowable BCa lower bound for the surviving cycle gap under huge pages.

    Derived from the hardware stall attribution in D1 stalls, where L3 miss stalls accounted
    for 61.1% of the cycle gap.
    """
    if baseline_delta_cycles <= 0:
        raise ValueError(f"baseline_delta_cycles must be positive, got {baseline_delta_cycles}")
    if not (0 < stall_fraction <= 1):
        raise ValueError(f"stall_fraction must be between 0 and 1, got {stall_fraction}")
    return round(baseline_delta_cycles * stall_fraction, 2)


def load_d1_reference_values(repo_root: Path) -> dict[str, float]:
    """Loads and computes the reference figures directly from committed D1 JSON artifacts."""
    d1_path = repo_root / "docs/benchmarks/patricia_comparison/results/counters_prefix_scan_d1.json"
    stalls_path = repo_root / "docs/benchmarks/patricia_comparison/results/counters_prefix_scan_d1_stalls.json"
    if not d1_path.exists() or not stalls_path.exists():
        raise FileNotFoundError(f"D1 artifacts not found under {repo_root}")

    with open(d1_path, "r", encoding="utf-8") as f:
        d1 = json.load(f)
    c0, c1 = d1["cells"][0], d1["cells"][1]
    ops = c0["distinct_probes"] * c0["passes"]

    with open(stalls_path, "r", encoding="utf-8") as f:
        stalls = json.load(f)
    s0, s1 = stalls["cells"][0], stalls["cells"][1]

    return {
        "ops": ops,
        "gen_dtlb": c0["counters"]["dTLB-load-misses"]["point"] / ops,
        "sorted_dtlb": c1["counters"]["dTLB-load-misses"]["point"] / ops,
        "gen_l3": c0["counters"]["mem_load_retired.l3_miss"]["point"] / ops,
        "sorted_l3": c1["counters"]["mem_load_retired.l3_miss"]["point"] / ops,
        "gen_cycles": c0["counters"]["cycles"]["point"] / ops,
        "sorted_cycles": c1["counters"]["cycles"]["point"] / ops,
        "stalls_delta_l3": (
            s0["counters"]["cycle_activity.stalls_l3_miss"]["point"]
            - s1["counters"]["cycle_activity.stalls_l3_miss"]["point"]
        ) / ops,
        "stalls_delta_cycles": (
            s0["counters"]["cycles"]["point"]
            - s1["counters"]["cycles"]["point"]
        ) / ops,
    }


class TestPatriciaD2Bounds(unittest.TestCase):
    def test_pinned_d1_constants(self):
        self.assertAlmostEqual(D1_GEN_DTLB_POINT, 0.820341, places=5)
        self.assertAlmostEqual(D1_SORTED_DTLB_POINT, 0.009745, places=5)
        self.assertAlmostEqual(D1_GEN_L3_POINT, 0.236002, places=5)
        self.assertAlmostEqual(D1_SORTED_L3_POINT, -0.000050, places=5)
        self.assertAlmostEqual(D1_BASELINE_DELTA_CYCLES, 85.7330, places=3)
        self.assertAlmostEqual(D1_STALLS_DELTA_L3_CYCLES, 52.5031, places=3)
        self.assertAlmostEqual(D1_STALLS_L3_FRACTION, 0.610895, places=5)

    def test_derived_bounds(self):
        # D2a: 95% reduction from 0.8203 -> 0.0410
        dtlb_bound = dtlb_elimination_upper_bound(D1_GEN_DTLB_POINT, 0.95)
        self.assertEqual(dtlb_bound, 0.0410)

        # D2b: 60% retention of delta L3 (0.2360 - (-0.0000)) -> 0.1416
        delta_l3 = D1_GEN_L3_POINT - D1_SORTED_L3_POINT
        l3_bound = l3_invariance_lower_bound(delta_l3, 0.60)
        self.assertEqual(l3_bound, 0.1416)

        # D2c: cycle gap lower bound matching L3 stalls attribution
        cycle_bound = surviving_cycle_gap_lower_bound(D1_BASELINE_DELTA_CYCLES, D1_STALLS_L3_FRACTION)
        self.assertEqual(cycle_bound, 52.37)
        # Direct stall pinning bound
        direct_stall_bound = round(D1_STALLS_DELTA_L3_CYCLES, 2)
        self.assertEqual(direct_stall_bound, 52.50)

    def test_invalid_inputs(self):
        with self.assertRaises(ValueError):
            dtlb_elimination_upper_bound(-1.0)
        with self.assertRaises(ValueError):
            dtlb_elimination_upper_bound(0.82, min_reduction=1.5)
        with self.assertRaises(ValueError):
            l3_invariance_lower_bound(0.0)
        with self.assertRaises(ValueError):
            l3_invariance_lower_bound(0.23, min_retention=-0.1)
        with self.assertRaises(ValueError):
            surviving_cycle_gap_lower_bound(-10.0, 0.5)
        with self.assertRaises(ValueError):
            surviving_cycle_gap_lower_bound(85.0, 0.0)

    def test_against_committed_artifacts(self):
        root = Path(__file__).resolve().parent.parent
        d = load_d1_reference_values(root)
        self.assertEqual(d["ops"], D1_OPS)
        self.assertAlmostEqual(d["gen_dtlb"], D1_GEN_DTLB_POINT, places=5)
        self.assertAlmostEqual(d["sorted_dtlb"], D1_SORTED_DTLB_POINT, places=5)
        self.assertAlmostEqual(d["gen_l3"], D1_GEN_L3_POINT, places=5)
        self.assertAlmostEqual(d["sorted_l3"], D1_SORTED_L3_POINT, places=5)


def main() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestPatriciaD2Bounds)
    runner = unittest.TextTestRunner(stream=sys.stdout, verbosity=2)
    result = runner.run(suite)
    if not result.wasSuccessful():
        return 1

    dtlb_bound = dtlb_elimination_upper_bound(D1_GEN_DTLB_POINT, 0.95)
    delta_l3 = D1_GEN_L3_POINT - D1_SORTED_L3_POINT
    l3_bound = l3_invariance_lower_bound(delta_l3, 0.60)
    cycle_bound = round(D1_STALLS_DELTA_L3_CYCLES, 2) # 52.50 cycles

    print("\n--- Diagnostic D2 Pre-Registered Bounds ---")
    print(f"Clause D2a (dTLB Elimination): BCa_upper(generator_2m.dtlb) <= {dtlb_bound} misses/entry (95% reduction from {D1_GEN_DTLB_POINT:.4f})")
    print(f"Clause D2b (L3 Miss Invariance): BCa_lower(generator_2m.l3 - sorted_2m.l3) >= {l3_bound} misses/entry (60% retention of {delta_l3:.4f})")
    print(f"Clause D2c (Surviving Cycle Gap): BCa_lower(generator_2m.cycles - sorted_2m.cycles) >= {cycle_bound} cycles/entry (pinning L3 stalls delta {D1_STALLS_DELTA_L3_CYCLES:.2f})")
    print("All derivations verified.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
