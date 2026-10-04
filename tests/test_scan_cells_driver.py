#!/usr/bin/env python3
"""Unit tests for the batch cursor scan instrument in writer_scaling.py (#1142).

Pins the schedule structure, Williams design balance, pin resolution,
P32.3 decision logic, and artifact schema.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "scripts"))
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from bench_provenance import estimators, new_provenance  # noqa: E402
from writer_scaling import (  # noqa: E402
    CAUSE_NAMES,
    SCAN_BLOCK,
    SCAN_CELLS_PIN,
    SCAN_CELLS_PROBES,
    SCAN_CELLS_WORKLOAD_ID,
    build_scan_cells_artifact,
    p323_paired_ratio,
    p323_report,
    resolve_scan_cells_pin,
    scan_cells_schedule,
    summarize_scan_cells,
)


def make_synthetic_rows(
    sched: list[dict[str, Any]],
    scan_w0_speedup: float = 3.5,
    scan_w1_r4_speedup: float = 2.5,
    scan_w4_r4_speedup: float = 1.5,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    t_rows: list[dict[str, Any]] = []
    c_rows: list[dict[str, Any]] = []
    for run in sched:
        op = run["read_op"]
        w = run["writers"]
        r = run["readers"]
        probe = run["probe"]
        rnd = run["round"]
        base_mops = 1.0 + 0.01 * rnd
        if op == "scan":
            factor = (
                scan_w0_speedup
                if w == 0
                else (scan_w1_r4_speedup if (w == 1 and r == 4) else scan_w4_r4_speedup)
            )
            mops = base_mops * factor
        else:
            mops = base_mops
        t_row = {
            "workload_id": SCAN_CELLS_WORKLOAD_ID,
            "role": "throughput",
            "arm": "expanse",
            "cell": f"map_w{w}_r{r}_{op}_{probe}",
            "keyspace_bits": 64,
            "prefill": 4096,
            "hotspot_prefill": 256,
            "hotspot_base": 0,
            "fresh_keys": 0 if w == 0 else 1000,
            "writers": w,
            "readers": r,
            "read_op": op,
            "probe": probe,
            "round": rnd,
            "position": run["position"],
            "write_ops": 0 if w == 0 else 1000,
            "writer_elapsed_s": None if w == 0 else 0.01,
            "writer_mops": None if w == 0 else 1.0,
            "reader_ops": 4096,
            "reader_elapsed_s": 0.001,
            "reader_thread_elapsed_s": [0.001] * r,
            "reader_mops": mops,
            "cpu_pin": SCAN_CELLS_PIN,
            "tsc_hz": 24000000,
            "population_after": 4096,
        }
        c_row = {
            "workload_id": SCAN_CELLS_WORKLOAD_ID,
            "role": "counters",
            "arm": "expanse",
            "cell": f"map_w{w}_r{r}_{op}_{probe}",
            "keyspace_bits": 64,
            "prefill": 4096,
            "hotspot_prefill": 256,
            "hotspot_base": 0,
            "fresh_keys": 0 if w == 0 else 1000,
            "writers": w,
            "readers": r,
            "read_op": op,
            "probe": probe,
            "round": rnd,
            "position": run["position"],
            "write_ops": 0 if w == 0 else 1000,
            "inserts": 0 if w == 0 else 1000,
            "reader_ops": 4096,
            "cpu_pin": SCAN_CELLS_PIN,
            "tsc_hz": 24000000,
            "lock_fallbacks": 0,
            "quiesce_calls": 0,
            "read_ops": 16 if op == "scan" else 4096,
            "read_attempts": 16 if op == "scan" else 4096,
            "read_fallbacks": 0,
            "locked_reads": 0,
            "fallback_causes": {cause: 0 for cause in CAUSE_NAMES},
            "population_after": 4096,
        }
        t_rows.append(t_row)
        c_rows.append(c_row)
    return t_rows, c_rows


class ScanCellsScheduleTests(unittest.TestCase):
    def test_schedule_length(self):
        rounds = 8
        sched = scan_cells_schedule(rounds)
        expected = rounds * len(SCAN_CELLS_PROBES) * len(SCAN_BLOCK)
        self.assertEqual(len(sched), expected)
        self.assertEqual(len(sched), 128)

    def test_williams_permutation_balance(self):
        rounds = 8
        n = len(SCAN_BLOCK)
        sched = scan_cells_schedule(rounds)
        for probe in SCAN_CELLS_PROBES:
            positions = {cell: [0] * n for cell in SCAN_BLOCK}
            pairs: dict[tuple[Any, Any], int] = {}
            for r in range(rounds):
                block = sorted(
                    (x for x in sched if x["round"] == r and x["probe"] == probe),
                    key=lambda x: x["position"],
                )
                self.assertEqual([x["position"] for x in block], list(range(n)))
                order = [(x["read_op"], x["writers"], x["readers"]) for x in block]
                self.assertEqual(sorted(order), sorted(SCAN_BLOCK))
                for i, cell in enumerate(order):
                    positions[cell][i] += 1
                for a, b in zip(order, order[1:]):
                    pairs[(a, b)] = pairs.get((a, b), 0) + 1
            # Every cell in every position once across 8 rounds
            self.assertTrue(all(v == [1] * n for v in positions.values()))
            # Every ordered pair adjacent exactly once (n * (n - 1) = 56 pairs)
            self.assertEqual(len(pairs), n * (n - 1))
            self.assertTrue(all(v == 1 for v in pairs.values()))

    def test_probe_alternation(self):
        rounds = 8
        sched = scan_cells_schedule(rounds)
        firsts = [
            next(x["probe"] for x in sched if x["round"] == r) for r in range(rounds)
        ]
        expected = [SCAN_CELLS_PROBES[r % 2] for r in range(rounds)]
        self.assertEqual(firsts, expected)


class ScanCellsPinTests(unittest.TestCase):
    def test_valid_pin_resolves(self):
        res = resolve_scan_cells_pin(
            {"EXPANSE_BENCH_PIN": SCAN_CELLS_PIN}, smoke=False
        )
        self.assertIsNone(res)

    def test_invalid_pin_raises(self):
        with self.assertRaises(ValueError) as ctx:
            resolve_scan_cells_pin({"EXPANSE_BENCH_PIN": "0-15"}, smoke=False)
        self.assertIn("0-15", str(ctx.exception))

    def test_smoke_notice_when_not_pinned(self):
        notice = resolve_scan_cells_pin({"EXPANSE_BENCH_PIN": "off"}, smoke=True)
        self.assertIsNotNone(notice)
        self.assertIn("off", notice)


class ScanCellsP323DecisionTests(unittest.TestCase):
    def setUp(self):
        self.sched = scan_cells_schedule(8)

    def test_p323_report_single_run_pass(self):
        t_rows, c_rows = make_synthetic_rows(
            self.sched, scan_w0_speedup=3.5, scan_w1_r4_speedup=2.5
        )
        report = p323_report(t_rows, 8)
        self.assertEqual(report["verdict"], "SINGLE_RUN_PASS")
        gated_cells = [c for c in report["cells"] if c["gated"]]
        report_only_cells = [c for c in report["cells"] if not c["gated"]]
        self.assertEqual(len(gated_cells), 6)
        self.assertEqual(len(report_only_cells), 2)
        for cell in gated_cells:
            self.assertGreaterEqual(cell["ratio_ci_lower"], cell["threshold"])

    def test_p323_report_rejects_when_w0_below_floor(self):
        # 2.5x speedup at W=0 is below the 3.0x floor
        t_rows, c_rows = make_synthetic_rows(
            self.sched, scan_w0_speedup=2.5, scan_w1_r4_speedup=2.5
        )
        report = p323_report(t_rows, 8)
        self.assertEqual(report["verdict"], "REJECTED")

    def test_p323_report_rejects_when_w1_r4_below_floor(self):
        # 1.5x speedup at W=1 R=4 is below the 2.0x floor
        t_rows, c_rows = make_synthetic_rows(
            self.sched, scan_w0_speedup=3.5, scan_w1_r4_speedup=1.5
        )
        report = p323_report(t_rows, 8)
        self.assertEqual(report["verdict"], "REJECTED")

    def test_p323_paired_ratio_computation(self):
        t_rows, _ = make_synthetic_rows(
            self.sched, scan_w0_speedup=3.5, scan_w1_r4_speedup=2.5
        )
        cell = p323_paired_ratio(t_rows, "uniform", 0, 1, 8)
        self.assertEqual(cell["probe"], "uniform")
        self.assertEqual(cell["writers"], 0)
        self.assertEqual(cell["readers"], 1)
        self.assertTrue(cell["gated"])
        self.assertEqual(cell["threshold"], 3.0)
        self.assertEqual(cell["verdict"], "SINGLE_RUN_PASS")
        self.assertAlmostEqual(cell["ratio_mean"], 3.5, places=1)
        self.assertEqual(len(cell["paired_ratios_raw"]), 8)


class ScanCellsArtifactTests(unittest.TestCase):
    def test_build_artifact_structure(self):
        rounds = 8
        sched = scan_cells_schedule(rounds)
        t_rows, c_rows = make_synthetic_rows(sched)
        load = {
            "since": "scan_cells:throughput",
            "wall_s": 1.0,
            "busy_cpus_since_prev": 1.0,
            "own_busy_cpus": 1.0,
            "foreign_busy_cpus": 0.0,
        }
        cells = summarize_scan_cells(t_rows, c_rows, rounds, load)
        prov = new_provenance(
            suite="concurrency",
            issue=1142,
            ratio="Batch cursor scan over next_after_scan",
            repo_root=REPO_ROOT,
            core_pin=SCAN_CELLS_PIN,
            estimators=estimators("test"),
        )
        art = build_scan_cells_artifact(
            prov, cells, t_rows, rounds, SCAN_CELLS_PIN, quick=True
        )
        self.assertIn("provenance", art)
        self.assertIn("throughput", art)
        self.assertEqual(len(art["throughput"]), len(SCAN_CELLS_PROBES) * len(SCAN_BLOCK))
        self.assertIn("scan_cells", art)
        sc = art["scan_cells"]
        self.assertEqual(sc["issue"], 1142)
        self.assertEqual(sc["rounds"], rounds)
        self.assertEqual(sc["pin"]["applied"], SCAN_CELLS_PIN)
        self.assertTrue(sc["pin"]["conforms"])
        self.assertIn("p32_3", sc)
        self.assertEqual(sc["p32_3"]["verdict"], "SINGLE_RUN_PASS")
        self.assertTrue(any("--quick" in v for v in sc["void"]))


if __name__ == "__main__":
    unittest.main()
