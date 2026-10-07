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
  python3 scripts/patricia_d2_bounds.py --evaluate <counters_prefix_scan_d2_2m.json>
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

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


def registered_bounds() -> dict[str, float]:
    """The three bounds METHODOLOGY.md §3.4.3 registers for D2a, D2b and D2c.

    D2c pins the D1 L3-stall delta directly (52.50 cycles/entry), not the
    product `surviving_cycle_gap_lower_bound` computes from the cycle gap and
    the stall fraction (52.37): the registered table states 52.50.
    """
    return {
        "dtlb_upper": dtlb_elimination_upper_bound(D1_GEN_DTLB_POINT, 0.95),
        "l3_delta_lower": l3_invariance_lower_bound(D1_GEN_L3_POINT - D1_SORTED_L3_POINT, 0.60),
        "cycle_delta_lower": round(D1_STALLS_DELTA_L3_CYCLES, 2),
    }


GENERATOR_ARM = "strmap_prefix_scan"
SORTED_ARM = "strmap_prefix_scan_sorted"


REGISTERED_RUNS = 10  # A4: `--runs 10` per cell
# A4's two-run protocol names the host condition: `loadavg <= 1.0`, `foreign_busy_cpus == 0`.
REGISTERED_LOAD1_MAX = 1.0
REGISTERED_FOREIGN_MAX = 0.0


def host_precondition(artifact: dict) -> dict:
    """A4's registered host condition, read from the artifact's load snapshots.

    Every snapshot's one-minute load average must be at most 1.0 and every
    recorded foreign-CPU figure must be 0. An artifact with no snapshot does
    not meet it.
    """
    loads = artifact.get("provenance", {}).get("loads") or []
    load1 = [float(x["load1"]) for x in loads if x.get("load1") is not None]
    foreign = [float(x["foreign_busy_cpus_since_prev"]) for x in loads
               if x.get("foreign_busy_cpus_since_prev") is not None]
    unmet = []
    if not load1:
        unmet.append("no load snapshot is recorded")
    elif max(load1) > REGISTERED_LOAD1_MAX:
        unmet.append(f"load1 reached {max(load1):g}, registered `loadavg <= {REGISTERED_LOAD1_MAX:g}`")
    if foreign and max(foreign) > REGISTERED_FOREIGN_MAX:
        unmet.append(f"foreign_busy_cpus reached {max(foreign):g}, registered `foreign_busy_cpus == 0`")
    return {"met": not unmet, "unmet": unmet,
            "load1_max": max(load1) if load1 else None, "foreign_busy_cpus_max": max(foreign) if foreign else None}


def evaluate_d2(artifact_2m: dict, *, expected_runs: int = REGISTERED_RUNS,
                expected_entries: int = D1_OPS) -> dict:
    """Evaluates D2a, D2b and D2c on one `counters_prefix_scan_d2_2m.json` artifact.

    Counts are per entry: each run's `probe - build` count divided by
    `distinct_probes * passes`. D2a reads the generator arm alone. D2b and D2c
    read the difference of the two arms, taken run by run in the order the
    artifact records them. The arms are separate processes run as two blocks,
    so that order pairs nothing: the interval is one of many a re-ordering
    would give. Every clause is decided on a BCa 95% bound (2,000 resamples,
    seed 42), never on the point.

    The run is VOID, with no clause evaluated, when it is not the measurement
    A4 registered: `hugetlb` not recorded, an arm that faulted in no huge page
    or fell back from one, an arm recorded twice, an arm without
    `expected_runs` runs, a per-entry denominator other than
    `expected_entries`, or a counter a clause reads whose status is not `ok`.

    The verdict is FAIL when a clause is not met. When every clause is met it
    is PASS only if A4's host condition is met too (`host_precondition`), and
    INTERMEDIATE otherwise: the clauses hold on a run the registration did
    not admit as written.

    One artifact is one run. A4 registers two runs; a verdict needs both.
    """
    arms = [c["arm"] for c in artifact_2m["cells"]]
    cells = {c["arm"]: c for c in artifact_2m["cells"]}
    missing = [a for a in (GENERATOR_ARM, SORTED_ARM) if a not in cells]
    if missing:
        raise ValueError(f"artifact has no cell for arm(s) {missing}")
    voids = []
    if artifact_2m.get("provenance", {}).get("hugetlb") is not True:
        voids.append("provenance.hugetlb is not true")
    read = ("dTLB-load-misses", "mem_load_retired.l3_miss", "cycles")
    for arm in (GENERATOR_ARM, SORTED_ARM):
        cell = cells[arm]
        if arms.count(arm) != 1:
            voids.append(f"{arm}: recorded {arms.count(arm)} times")
        thp = cell.get("thp_delta", {})
        if thp.get("thp_fault_alloc", 0) <= 0:
            voids.append(f"{arm}: thp_fault_alloc did not increase")
        if thp.get("thp_fault_fallback", 0) > 0:
            voids.append(f"{arm}: {thp['thp_fault_fallback']} huge-page fault(s) fell back to small pages")
        entries = cell["distinct_probes"] * cell["passes"]
        if entries != expected_entries:
            voids.append(f"{arm}: {entries} entries per run, registered {expected_entries}")
        for counter in read:
            c = cell["counters"].get(counter, {})
            if c.get("status") != "ok":
                voids.append(f"{arm}: counter {counter} status is {c.get('status')!r}")
            elif len(c["samples"]) != expected_runs:
                voids.append(f"{arm}: {len(c['samples'])} runs of {counter}, registered {expected_runs}")
    host = host_precondition(artifact_2m)
    if voids:
        return {"verdict": "VOID", "voids": voids, "clauses": {}, "host_precondition": host}

    def per_entry(arm: str, counter: str) -> list[float]:
        cell = cells[arm]
        return [x / expected_entries for x in cell["counters"][counter]["samples"]]

    def diff(counter: str) -> list[float]:
        return [a - b for a, b in zip(per_entry(GENERATOR_ARM, counter), per_entry(SORTED_ARM, counter), strict=True)]

    bounds = registered_bounds()
    clauses = {}
    for name, data, key, side in (
        ("D2a", per_entry(GENERATOR_ARM, "dTLB-load-misses"), "dtlb_upper", "upper"),
        ("D2b", diff("mem_load_retired.l3_miss"), "l3_delta_lower", "lower"),
        ("D2c", diff("cycles"), "cycle_delta_lower", "lower"),
    ):
        point, lo, hi, method = bca_bootstrap_ci_with_method(data, confidence=0.95)
        bound = bounds[key]
        met = hi <= bound if side == "upper" else lo >= bound
        clauses[name] = {
            "n": len(data), "point": point, "ci": [lo, hi], "ci_method": method,
            "bound": bound, "decided_on": side, "met": met,
        }
    if not all(c["met"] for c in clauses.values()):
        verdict = "FAIL"
    else:
        verdict = "PASS" if host["met"] else "INTERMEDIATE"
    return {"verdict": verdict, "voids": [], "clauses": clauses, "host_precondition": host}


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
        # The registered D2c bound is the direct stall delta, not that product.
        self.assertEqual(
            registered_bounds(),
            {"dtlb_upper": 0.0410, "l3_delta_lower": 0.1416, "cycle_delta_lower": 52.50},
        )

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
        self.assertAlmostEqual(d["gen_cycles"], D1_GEN_CYCLES_POINT, places=3)
        self.assertAlmostEqual(d["sorted_cycles"], D1_SORTED_CYCLES_POINT, places=3)
        self.assertAlmostEqual(d["stalls_delta_l3"], D1_STALLS_DELTA_L3_CYCLES, places=3)
        self.assertAlmostEqual(d["stalls_delta_cycles"], D1_STALLS_TOTAL_DELTA_CYCLES, places=3)

    @staticmethod
    def _artifact(gen: dict, srt: dict, hugetlb=True, thp=(100, 100), fallback=(0, 0),
                  loads=((0.4, None), (0.6, 0.0))) -> dict:
        def cell(arm, counters, faults, fell_back):
            return {
                "arm": arm, "distinct_probes": 10, "passes": 10,
                "thp_delta": {"thp_fault_alloc": faults, "thp_fault_fallback": fell_back},
                "counters": {k: {"status": "ok", "samples": v} for k, v in counters.items()},
            }
        return {"provenance": {"hugetlb": hugetlb,
                               "loads": [{"load1": l, "foreign_busy_cpus_since_prev": f} for l, f in loads]},
                "cells": [cell(GENERATOR_ARM, gen, thp[0], fallback[0]), cell(SORTED_ARM, srt, thp[1], fallback[1])]}

    JITTER = [0.0, 1.0, -1.0, 2.0, -2.0, 0.5, -0.5, 1.5, -1.5, 0.0]

    def _good(self):
        # Per entry (100 entries): dTLB 0.02, L3 delta 0.20, cycle delta 60.
        gen = {"dTLB-load-misses": [2.0 + j / 10 for j in self.JITTER],
               "mem_load_retired.l3_miss": [21.0 + j / 10 for j in self.JITTER],
               "cycles": [13000.0 + 10 * j for j in self.JITTER]}
        srt = {"dTLB-load-misses": [1.0] * 10,
               "mem_load_retired.l3_miss": [1.0] * 10,
               "cycles": [7000.0] * 10}
        return gen, srt

    def _eval(self, artifact):
        return evaluate_d2(artifact, expected_entries=100)

    def test_evaluate_d2(self):
        gen, srt = self._good()
        jitter = self.JITTER
        r = self._eval(self._artifact(gen, srt))
        self.assertEqual(r["verdict"], "PASS")
        self.assertTrue(r["host_precondition"]["met"])
        self.assertEqual({k: v["met"] for k, v in r["clauses"].items()},
                         {"D2a": True, "D2b": True, "D2c": True})
        self.assertAlmostEqual(r["clauses"]["D2c"]["point"], 60.0, places=4)
        for c in r["clauses"].values():
            self.assertEqual(c["n"], REGISTERED_RUNS)
            self.assertLessEqual(c["ci"][0], c["point"])
            self.assertLessEqual(c["point"], c["ci"][1])
            self.assertTrue(c["ci_method"])

        # Each clause fails alone: one counter moved across its bound.
        for clause, counter, values in (
            ("D2a", "dTLB-load-misses", [5.0 + j / 10 for j in jitter]),       # 0.05 > 0.0410
            ("D2b", "mem_load_retired.l3_miss", [11.0 + j / 10 for j in jitter]),  # 0.10 < 0.1416
            ("D2c", "cycles", [12000.0 + 10 * j for j in jitter]),             # 50 < 52.50
        ):
            bad = dict(gen)
            bad[counter] = values
            r = self._eval(self._artifact(bad, srt))
            self.assertEqual(r["verdict"], "FAIL", clause)
            self.assertEqual([k for k, v in r["clauses"].items() if not v["met"]], [clause])

        # A point on the right side of its bound with an interval across it fails:
        # the clause is decided on the bound.
        wide = dict(gen)
        wide["cycles"] = [12600.0] * 8 + [11000.0, 14200.0]
        r = self._eval(self._artifact(wide, srt))
        self.assertGreater(r["clauses"]["D2c"]["point"], 52.50)
        self.assertFalse(r["clauses"]["D2c"]["met"])

        # D2a is decided on the UPPER bound: a point under the bound whose
        # interval reaches over it fails.
        over = dict(gen)
        over["dTLB-load-misses"] = [3.0] * 8 + [9.0, 9.0]   # mean 0.042 > 0.0410 at the top
        r = self._eval(self._artifact(over, srt))
        self.assertEqual(r["clauses"]["D2a"]["decided_on"], "upper")
        self.assertFalse(r["clauses"]["D2a"]["met"])

    def test_evaluate_d2_void(self):
        gen, srt = self._good()

        def void(artifact, needle):
            r = self._eval(artifact)
            self.assertEqual(r["verdict"], "VOID", needle)
            self.assertEqual(r["clauses"], {})
            self.assertTrue(any(needle in v for v in r["voids"]), (needle, r["voids"]))

        void(self._artifact(gen, srt, hugetlb=False), "hugetlb is not true")
        void(self._artifact(gen, srt, thp=(0, 100)), "thp_fault_alloc did not increase")
        void(self._artifact(gen, srt, thp=(100, 0)), "thp_fault_alloc did not increase")
        void(self._artifact(gen, srt, fallback=(0, 7)), "fell back to small pages")
        no_key = self._artifact(gen, srt)
        del no_key["provenance"]["hugetlb"]
        void(no_key, "hugetlb is not true")
        no_thp = self._artifact(gen, srt)
        del no_thp["cells"][0]["thp_delta"]
        void(no_thp, "thp_fault_alloc did not increase")

        # Not the registered measurement: too few runs, arms of unequal
        # length, a different denominator on one arm, an arm recorded twice,
        # a counter that did not count.
        few = {k: v[:5] for k, v in gen.items()}
        void(self._artifact(few, {k: v[:5] for k, v in srt.items()}), "5 runs of")
        void(self._artifact(few, srt), "5 runs of")
        halved = self._artifact(gen, srt)
        halved["cells"][1]["passes"] = 20
        void(halved, "200 entries per run, registered 100")
        twice = self._artifact(gen, srt)
        twice["cells"].append(dict(twice["cells"][1]))
        void(twice, "recorded 2 times")
        dead = self._artifact(gen, srt)
        dead["cells"][1]["counters"]["cycles"]["status"] = "multiplexed"
        void(dead, "status is 'multiplexed'")
        # The registered denominator is D1's: the fixture's 100 is not it.
        self.assertEqual(evaluate_d2(self._artifact(gen, srt))["verdict"], "VOID")

        with self.assertRaises(ValueError):
            evaluate_d2({"provenance": {"hugetlb": True}, "cells": []})

    def test_host_precondition(self):
        gen, srt = self._good()
        # Clauses met on a host the registration did not admit as written.
        for loads, needle in (
            (((1.47, None), (1.37, 0.0)), "load1 reached 1.47"),
            (((0.5, None), (0.6, 0.02)), "foreign_busy_cpus reached 0.02"),
            ((), "no load snapshot"),
        ):
            r = self._eval(self._artifact(gen, srt, loads=loads))
            self.assertEqual(r["verdict"], "INTERMEDIATE", needle)
            self.assertTrue(all(c["met"] for c in r["clauses"].values()))
            self.assertTrue(any(needle in u for u in r["host_precondition"]["unmet"]), r["host_precondition"])
        # A failed clause is FAIL whatever the host read.
        bad = dict(gen)
        bad["cycles"] = [12000.0 + 10 * j for j in self.JITTER]
        self.assertEqual(self._eval(self._artifact(bad, srt, loads=((1.9, None),)))["verdict"], "FAIL")
        # Exactly at the registered values is met.
        self.assertTrue(self._eval(self._artifact(gen, srt, loads=((1.0, None), (1.0, 0.0))))["host_precondition"]["met"])

    def test_committed_runs(self):
        # The two runs as committed: every clause met, no void, and A4's host
        # condition not met as written (load1 above 1.0, foreign above 0).
        results = Path(__file__).resolve().parent.parent / "docs/benchmarks/patricia_comparison/results"
        for run, d2c_lower in ((1, 65.28), (2, 66.10)):
            with open(results / f"counters_prefix_scan_d2_2m_run{run}.json", encoding="utf-8") as fh:
                r = evaluate_d2(json.load(fh))
            self.assertEqual(r["verdict"], "INTERMEDIATE", run)
            self.assertEqual(r["voids"], [])
            self.assertTrue(all(c["met"] for c in r["clauses"].values()))
            self.assertAlmostEqual(r["clauses"]["D2c"]["ci"][0], d2c_lower, places=2)
            self.assertFalse(r["host_precondition"]["met"])
            self.assertGreater(r["host_precondition"]["load1_max"], 1.0)


def main() -> int:
    if sys.argv[1:2] == ["--evaluate"]:
        if len(sys.argv) != 3:
            print("usage: patricia_d2_bounds.py --evaluate <counters_prefix_scan_d2_2m.json>", file=sys.stderr)
            return 2
        with open(sys.argv[2], "r", encoding="utf-8") as fh:
            print(json.dumps(evaluate_d2(json.load(fh)), indent=2))
        return 0
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestPatriciaD2Bounds)
    runner = unittest.TextTestRunner(stream=sys.stdout, verbosity=2)
    result = runner.run(suite)
    if not result.wasSuccessful():
        return 1

    bounds = registered_bounds()
    dtlb_bound = bounds["dtlb_upper"]
    delta_l3 = D1_GEN_L3_POINT - D1_SORTED_L3_POINT
    l3_bound = bounds["l3_delta_lower"]
    cycle_bound = bounds["cycle_delta_lower"]

    print("\n--- Diagnostic D2 Pre-Registered Bounds ---")
    print(f"Clause D2a (dTLB Elimination): BCa_upper(generator_2m.dtlb) <= {dtlb_bound} misses/entry (95% reduction from {D1_GEN_DTLB_POINT:.4f})")
    print(f"Clause D2b (L3 Miss Invariance): BCa_lower(generator_2m.l3 - sorted_2m.l3) >= {l3_bound} misses/entry (60% retention of {delta_l3:.4f})")
    print(f"Clause D2c (Surviving Cycle Gap): BCa_lower(generator_2m.cycles - sorted_2m.cycles) >= {cycle_bound} cycles/entry (pinning L3 stalls delta {D1_STALLS_DELTA_L3_CYCLES:.2f})")
    print("All derivations verified.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
