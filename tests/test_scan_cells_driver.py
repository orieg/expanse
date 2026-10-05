#!/usr/bin/env python3
"""Unit tests for the batch cursor scan instrument in writer_scaling.py (#1142).

Pins the schedule structure, Williams design balance, pin resolution,
P32.3 per-run decision logic, two-run combiner, and artifact schema.
"""

from __future__ import annotations

import json
import subprocess
import tempfile
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
    SCAN_CELL_TIMEOUT_S,
    STR_SCAN_BLOCK,
    STR_SCAN_CELLS_PIN,
    STR_SCAN_CELLS_PROBES,
    STR_SCAN_CELLS_WORKLOAD_ID,
    CellTimeoutError,
    build_scan_cells_artifact,
    build_void_scan_cells_artifact,
    check_reader_counters_row,
    combine_scan_cells_verdict,
    get_binaries,
    p323_paired_ratio,
    p323_report,
    p333_paired_ratio,
    p333_report,
    resolve_scan_cells_pin,
    run_reader_invocation,
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
            "reader_wraps": 0,
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
            "reader_wraps": 0,
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

    def test_p323_report_lb_at_or_above_floor(self):
        t_rows, c_rows = make_synthetic_rows(
            self.sched, scan_w0_speedup=3.5, scan_w1_r4_speedup=2.5
        )
        report = p323_report(t_rows, 8)
        self.assertEqual(report["verdict"], "LB_AT_OR_ABOVE_FLOOR")
        gated_cells = [c for c in report["cells"] if c["gated"]]
        report_only_cells = [c for c in report["cells"] if not c["gated"]]
        self.assertEqual(len(gated_cells), 6)
        self.assertEqual(len(report_only_cells), 2)
        for cell in gated_cells:
            self.assertEqual(cell["verdict"], "LB_AT_OR_ABOVE_FLOOR")
            self.assertGreaterEqual(cell["ratio_ci_lower"], cell["threshold"])

    def test_p323_report_lb_below_floor_on_w0(self):
        # 2.5x speedup at W=0 is below the 3.0x floor
        t_rows, c_rows = make_synthetic_rows(
            self.sched, scan_w0_speedup=2.5, scan_w1_r4_speedup=2.5
        )
        report = p323_report(t_rows, 8)
        self.assertEqual(report["verdict"], "LB_BELOW_FLOOR")

    def test_p323_report_lb_below_floor_on_w1_r4(self):
        # 1.5x speedup at W=1 R=4 is below the 2.0x floor
        t_rows, c_rows = make_synthetic_rows(
            self.sched, scan_w0_speedup=3.5, scan_w1_r4_speedup=1.5
        )
        report = p323_report(t_rows, 8)
        self.assertEqual(report["verdict"], "LB_BELOW_FLOOR")

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
        self.assertEqual(cell["verdict"], "LB_AT_OR_ABOVE_FLOOR")
        self.assertAlmostEqual(cell["ratio_mean"], 3.5, places=1)
        self.assertEqual(len(cell["paired_ratios_raw"]), 8)

    def test_negative_control_mean_vs_lb_discrimination(self):
        """Negative control (a): Discriminates CI lower bound logic from point mean.

        Constructs a cell where point mean >= floor (3.25 >= 3.0) but sampling variance
        pushes the BCa 95% CI lower bound below the floor (LB < 3.0).
        Per §32.4 / Rule 1, the verdict must be LB_BELOW_FLOOR.
        If the logic is broken to test mean >= floor, this test fails red.
        """
        rows: list[dict[str, Any]] = []
        # Mean ratio is (1.5 + 5.0)/2 = 3.25 >= 3.0, but spread drops LB below 3.0
        ratios_pattern = [1.8, 1.9, 2.0, 2.2, 4.8, 4.9, 5.0, 5.2]
        for r, ratio in enumerate(ratios_pattern):
            rows.append({
                "workload_id": SCAN_CELLS_WORKLOAD_ID,
                "role": "throughput",
                "probe": "uniform",
                "writers": 0,
                "readers": 1,
                "read_op": "scan",
                "round": r,
                "reader_mops": ratio,
            })
            rows.append({
                "workload_id": SCAN_CELLS_WORKLOAD_ID,
                "role": "throughput",
                "probe": "uniform",
                "writers": 0,
                "readers": 1,
                "read_op": "next_after_scan",
                "round": r,
                "reader_mops": 1.0,
            })
        cell = p323_paired_ratio(rows, "uniform", 0, 1, 8)
        self.assertGreaterEqual(cell["ratio_mean"], 3.0)
        self.assertLess(cell["ratio_ci_lower"], 3.0)
        self.assertEqual(cell["verdict"], "LB_BELOW_FLOOR")


class ScanCellsCombinerTests(unittest.TestCase):
    def setUp(self):
        self.rounds = 8
        self.sched = scan_cells_schedule(self.rounds)
        load = {
            "since": "scan_cells:throughput",
            "wall_s": 1.0,
            "busy_cpus_since_prev": 1.0,
            "own_busy_cpus": 1.0,
            "foreign_busy_cpus": 0.0,
        }
        self.prov = new_provenance(
            suite="concurrency",
            issue=1142,
            ratio="Batch cursor scan over next_after_scan",
            repo_root=REPO_ROOT,
            core_pin=SCAN_CELLS_PIN,
            estimators=estimators("test"),
        )
        t_pass, c_pass = make_synthetic_rows(self.sched, scan_w0_speedup=3.5, scan_w1_r4_speedup=2.5)
        cells_pass = summarize_scan_cells(t_pass, c_pass, self.rounds, load)
        self.art_pass = build_scan_cells_artifact(self.prov, cells_pass, t_pass, self.rounds, SCAN_CELLS_PIN, quick=False)

        t_fail, c_fail = make_synthetic_rows(self.sched, scan_w0_speedup=3.5, scan_w1_r4_speedup=1.5)
        cells_fail = summarize_scan_cells(t_fail, c_fail, self.rounds, load)
        self.art_fail = build_scan_cells_artifact(self.prov, cells_fail, t_fail, self.rounds, SCAN_CELLS_PIN, quick=False)

    def test_combiner_pass_when_both_runs_hold(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f1, f2 = tmp / "run1.json", tmp / "run2.json"
            f1.write_text(json.dumps(self.art_pass))
            f2.write_text(json.dumps(self.art_pass))
            res = combine_scan_cells_verdict(f1, f2)
            self.assertEqual(res["verdict"], "PASS")
            self.assertEqual(len(res["evaluation"]["failing_cells"]), 0)

    def test_negative_control_two_runs_vs_one_discrimination(self):
        """Negative control (b): Discriminates two-run conjunction vs accepting one run.

        Run 1 passes all thresholds, Run 2 fails W=1 R=4.
        Per §32.4 P32.3, the verdict across two runs must be REFUTED.
        If the combiner is broken to check only one run (Run 1), this test fails red.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f1, f2 = tmp / "run1.json", tmp / "run2.json"
            f1.write_text(json.dumps(self.art_pass))
            f2.write_text(json.dumps(self.art_fail))
            res = combine_scan_cells_verdict(f1, f2)
            self.assertEqual(res["verdict"], "REFUTED")
            failing = res["evaluation"]["failing_cells"]
            self.assertGreater(len(failing), 0)
            self.assertTrue(all("run2" in f["failing_runs"] for f in failing))

    def test_negative_control_refuses_quick_void_artifact(self):
        """Negative control (c): Discriminates void enforcement vs un-gated evaluation.

        If an artifact was run with --quick, it carries void notices and quick=True.
        The combiner must refuse to evaluate and raise ValueError.
        If the void check is dropped, this test fails red.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f_quick, f_valid = tmp / "quick.json", tmp / "valid.json"
            art_quick = json.loads(json.dumps(self.art_pass))
            art_quick["scan_cells"]["quick"] = True
            art_quick["scan_cells"]["void"] = ["--quick smoke run"]
            f_quick.write_text(json.dumps(art_quick))
            f_valid.write_text(json.dumps(self.art_pass))
            with self.assertRaises(ValueError) as ctx:
                combine_scan_cells_verdict(f_quick, f_valid)
            self.assertIn("--quick", str(ctx.exception))

    def test_combiner_refuses_commit_mismatch(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f1, f2 = tmp / "run1.json", tmp / "run2.json"
            art_diff = json.loads(json.dumps(self.art_pass))
            art_diff["provenance"]["commit"] = "0123456789abcdef"
            f1.write_text(json.dumps(self.art_pass))
            f2.write_text(json.dumps(art_diff))
            with self.assertRaises(ValueError) as ctx:
                combine_scan_cells_verdict(f1, f2)
            self.assertIn("commit mismatch", str(ctx.exception))

    def test_combiner_refuses_missing_cell(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f1, f2 = tmp / "run1.json", tmp / "run2.json"
            art_missing = json.loads(json.dumps(self.art_pass))
            # Remove one gated cell from run 2
            art_missing["scan_cells"]["p32_3"]["cells"] = [
                c for c in art_missing["scan_cells"]["p32_3"]["cells"]
                if not (c["probe"] == "uniform" and c["writers"] == 0 and c["readers"] == 1)
            ]
            f1.write_text(json.dumps(self.art_pass))
            f2.write_text(json.dumps(art_missing))
            with self.assertRaises(ValueError) as ctx:
                combine_scan_cells_verdict(f1, f2)
            self.assertIn("missing gated cell", str(ctx.exception))


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
        self.assertEqual(sc["p32_3"]["verdict"], "LB_AT_OR_ABOVE_FLOOR")
        self.assertTrue(any("--quick" in v for v in sc["void"]))


class NextAfterScanReaderIdentityTests(unittest.TestCase):
    def setUp(self):
        self.base_row = {
            "workload_id": SCAN_CELLS_WORKLOAD_ID,
            "role": "counters",
            "arm": "expanse",
            "cell": "map_w1_r4_next_after_scan_uniform",
            "keyspace_bits": 64,
            "prefill": 4096,
            "hotspot_prefill": 256,
            "hotspot_base": 0,
            "fresh_keys": 1000,
            "writers": 1,
            "readers": 4,
            "read_op": "next_after_scan",
            "probe": "uniform",
            "round": 0,
            "position": 0,
            "write_ops": 1000,
            "inserts": 1000,
            "reader_ops": 31277057,
            "reader_wraps": 20,
            "cpu_pin": SCAN_CELLS_PIN,
            "tsc_hz": 24000000,
            "lock_fallbacks": 0,
            "quiesce_calls": 0,
            "read_ops": 31277077,
            "read_attempts": 31277077,
            "read_fallbacks": 0,
            "locked_reads": 0,
            "fallback_causes": {cause: 0 for cause in CAUSE_NAMES},
            "population_after": 5096,
        }

    def test_exact_identity_passes(self):
        # Exact mathematical identity: 31277057 + 20 == 31277077
        check_reader_counters_row(self.base_row)
        self.assertEqual(
            self.base_row["read_ops"],
            self.base_row["reader_ops"] + self.base_row["reader_wraps"],
        )

    def test_off_by_one_read_ops_fails(self):
        # Off-by-one above
        row = dict(self.base_row, read_ops=31277078)
        with self.assertRaises(ValueError) as ctx:
            check_reader_counters_row(row)
        self.assertIn("a next_after_scan reader cell needs read_ops == reader_ops + reader_wraps", str(ctx.exception))

        # Off-by-one below
        row_under = dict(self.base_row, read_ops=31277076)
        with self.assertRaises(ValueError) as ctx:
            check_reader_counters_row(row_under)
        self.assertIn("a next_after_scan reader cell needs read_ops == reader_ops + reader_wraps", str(ctx.exception))

    def test_missing_reader_wraps_fails(self):
        row = dict(self.base_row)
        del row["reader_wraps"]
        with self.assertRaises(ValueError) as ctx:
            check_reader_counters_row(row)
        self.assertIn("lacks 'reader_wraps'", str(ctx.exception))

    def test_locked_reads_mismatch_fails(self):
        row = dict(self.base_row, locked_reads=1, quiesce_calls=1)
        with self.assertRaises(ValueError) as ctx:
            check_reader_counters_row(row)
        self.assertIn("locked_reads == read_fallbacks", str(ctx.exception))


def make_synthetic_str_rows(
    sched: list[dict[str, Any]],
    scan_w0_r4_speedup: float = 3.5,
    scan_w1_r4_speedup: float = 3.5,
    scan_w4_r4_speedup: float = 2.5,
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
            if w == 0 and r == 1:
                mops = base_mops
            elif w == 0 and r == 4:
                mops = base_mops * scan_w0_r4_speedup
            elif w == 1 and r == 4:
                mops = base_mops * scan_w1_r4_speedup
            elif w == 4 and r == 4:
                mops = base_mops * scan_w4_r4_speedup
            else:
                mops = base_mops * 2.0
        else:
            mops = base_mops
        t_row = {
            "workload_id": STR_SCAN_CELLS_WORKLOAD_ID,
            "role": "throughput",
            "arm": "str",
            "cell": f"str_w{w}_r{r}_{op}_{probe}",
            "keyspace_bits": 64,
            "prefill": 4096,
            "hotspot_prefill": 0,
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
            "reader_wraps": 0,
            "reader_elapsed_s": 0.001,
            "reader_thread_elapsed_s": [0.001] * r,
            "reader_mops": mops,
            "cpu_pin": STR_SCAN_CELLS_PIN,
            "tsc_hz": 24000000,
            "population_after": 4096,
        }
        c_row = {
            "workload_id": STR_SCAN_CELLS_WORKLOAD_ID,
            "role": "counters",
            "arm": "str",
            "cell": f"str_w{w}_r{r}_{op}_{probe}",
            "keyspace_bits": 64,
            "prefill": 4096,
            "hotspot_prefill": 0,
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
            "reader_wraps": 0,
            "cpu_pin": STR_SCAN_CELLS_PIN,
            "tsc_hz": 24000000,
            "lock_fallbacks": 0,
            "quiesce_calls": 0,
            "read_ops": 0,
            "read_attempts": 0,
            "read_fallbacks": 0,
            "locked_reads": 0,
            "fallback_causes": {cause: 0 for cause in CAUSE_NAMES},
            "population_after": 4096,
        }
        t_rows.append(t_row)
        c_rows.append(c_row)
    return t_rows, c_rows


class StrScanCellsScheduleTests(unittest.TestCase):
    def test_str_schedule_length(self):
        rounds = 8
        sched = scan_cells_schedule(rounds, arm="str")
        expected = rounds * len(STR_SCAN_CELLS_PROBES) * len(STR_SCAN_BLOCK)
        self.assertEqual(len(sched), expected)
        self.assertEqual(len(sched), 128)

    def test_str_williams_permutation_balance(self):
        rounds = 8
        n = len(STR_SCAN_BLOCK)
        sched = scan_cells_schedule(rounds, arm="str")
        for probe in STR_SCAN_CELLS_PROBES:
            positions = {cell: [0] * n for cell in STR_SCAN_BLOCK}
            pairs: dict[tuple[Any, Any], int] = {}
            for r in range(rounds):
                block = sorted(
                    (x for x in sched if x["round"] == r and x["probe"] == probe),
                    key=lambda x: x["position"],
                )
                self.assertEqual([x["position"] for x in block], list(range(n)))
                order = [(x["read_op"], x["writers"], x["readers"]) for x in block]
                self.assertEqual(sorted(order), sorted(STR_SCAN_BLOCK))
                for i, cell in enumerate(order):
                    positions[cell][i] += 1
                for a, b in zip(order, order[1:]):
                    pairs[(a, b)] = pairs.get((a, b), 0) + 1
            # Every cell in every position once across 8 rounds
            self.assertTrue(all(v == [1] * n for v in positions.values()))
            # Every ordered pair adjacent exactly once (n * (n - 1) = 56 pairs)
            self.assertEqual(len(pairs), n * (n - 1))
            self.assertTrue(all(v == 1 for v in pairs.values()))

    def test_str_probe_alternation(self):
        rounds = 8
        sched = scan_cells_schedule(rounds, arm="str")
        firsts = [
            next(x["probe"] for x in sched if x["round"] == r) for r in range(rounds)
        ]
        expected = [STR_SCAN_CELLS_PROBES[r % 2] for r in range(rounds)]
        self.assertEqual(firsts, expected)


class StrScanCellsP333DecisionTests(unittest.TestCase):
    def setUp(self):
        self.sched = scan_cells_schedule(8, arm="str")

    def test_p333_report_lb_at_or_above_floor(self):
        t_rows, c_rows = make_synthetic_str_rows(
            self.sched, scan_w0_r4_speedup=3.5, scan_w1_r4_speedup=3.5, scan_w4_r4_speedup=2.5
        )
        report = p333_report(t_rows, 8)
        self.assertEqual(report["verdict"], "LB_AT_OR_ABOVE_FLOOR")
        gated_cells = [c for c in report["cells"] if c["gated"]]
        report_only_cells = [c for c in report["cells"] if not c["gated"]]
        self.assertEqual(len(gated_cells), 6)
        self.assertEqual(len(report_only_cells), 2)
        for cell in gated_cells:
            self.assertEqual(cell["verdict"], "LB_AT_OR_ABOVE_FLOOR")
            self.assertGreaterEqual(cell["ratio_ci_lower"], cell["threshold"])

    def test_p333_report_lb_below_floor_on_w0_r4(self):
        # 2.0x speedup at (0, 4) is below 2.5x floor
        t_rows, c_rows = make_synthetic_str_rows(
            self.sched, scan_w0_r4_speedup=2.0, scan_w1_r4_speedup=3.5, scan_w4_r4_speedup=2.5
        )
        report = p333_report(t_rows, 8)
        self.assertEqual(report["verdict"], "LB_BELOW_FLOOR")

    def test_p333_report_lb_below_floor_on_w1_r4(self):
        # 2.5x speedup at (1, 4) is below 3.0x floor
        t_rows, c_rows = make_synthetic_str_rows(
            self.sched, scan_w0_r4_speedup=3.5, scan_w1_r4_speedup=2.5, scan_w4_r4_speedup=2.5
        )
        report = p333_report(t_rows, 8)
        self.assertEqual(report["verdict"], "LB_BELOW_FLOOR")

    def test_p333_report_lb_below_floor_on_w4_r4(self):
        # 1.5x speedup at (4, 4) is below 2.0x floor
        t_rows, c_rows = make_synthetic_str_rows(
            self.sched, scan_w0_r4_speedup=3.5, scan_w1_r4_speedup=3.5, scan_w4_r4_speedup=1.5
        )
        report = p333_report(t_rows, 8)
        self.assertEqual(report["verdict"], "LB_BELOW_FLOOR")

    def test_p333_paired_ratio_w0_r4(self):
        t_rows, _ = make_synthetic_str_rows(
            self.sched, scan_w0_r4_speedup=3.5, scan_w1_r4_speedup=3.5, scan_w4_r4_speedup=2.5
        )
        cell = p333_paired_ratio(t_rows, "paths", 0, 4, 8)
        self.assertEqual(cell["probe"], "paths")
        self.assertEqual(cell["writers"], 0)
        self.assertEqual(cell["readers"], 4)
        self.assertTrue(cell["gated"])
        self.assertEqual(cell["threshold"], 2.5)
        self.assertEqual(cell["verdict"], "LB_AT_OR_ABOVE_FLOOR")
        self.assertAlmostEqual(cell["ratio_mean"], 3.5, places=1)
        self.assertEqual(len(cell["paired_ratios_raw"]), 8)

    def test_p333_paired_ratio_w1_r4(self):
        t_rows, _ = make_synthetic_str_rows(
            self.sched, scan_w0_r4_speedup=3.5, scan_w1_r4_speedup=3.5, scan_w4_r4_speedup=2.5
        )
        cell = p333_paired_ratio(t_rows, "paths_dense", 1, 4, 8)
        self.assertEqual(cell["probe"], "paths_dense")
        self.assertEqual(cell["writers"], 1)
        self.assertEqual(cell["readers"], 4)
        self.assertTrue(cell["gated"])
        self.assertEqual(cell["threshold"], 3.0)
        self.assertEqual(cell["verdict"], "LB_AT_OR_ABOVE_FLOOR")
        self.assertAlmostEqual(cell["ratio_mean"], 3.5, places=1)

    def test_negative_control_mean_vs_lb_discrimination_str(self):
        """Negative control (a) for Str: Discriminates CI lower bound logic from point mean.

        Constructs a cell where point mean >= floor (3.25 >= 3.0) but sampling variance
        pushes the BCa 95% CI lower bound below the floor (LB < 3.0).
        Per §33.4 / Rule 1, the verdict must be LB_BELOW_FLOOR.
        """
        rows: list[dict[str, Any]] = []
        ratios_pattern = [1.8, 1.9, 2.0, 2.2, 4.8, 4.9, 5.0, 5.2]
        for r, ratio in enumerate(ratios_pattern):
            rows.append({
                "workload_id": STR_SCAN_CELLS_WORKLOAD_ID,
                "role": "throughput",
                "probe": "paths",
                "writers": 1,
                "readers": 4,
                "read_op": "scan",
                "round": r,
                "reader_mops": ratio,
            })
            rows.append({
                "workload_id": STR_SCAN_CELLS_WORKLOAD_ID,
                "role": "throughput",
                "probe": "paths",
                "writers": 1,
                "readers": 4,
                "read_op": "scan_locked",
                "round": r,
                "reader_mops": 1.0,
            })
        cell = p333_paired_ratio(rows, "paths", 1, 4, 8)
        self.assertGreaterEqual(cell["ratio_mean"], 3.0)
        self.assertLess(cell["ratio_ci_lower"], 3.0)
        self.assertEqual(cell["verdict"], "LB_BELOW_FLOOR")


class StrScanCellsCombinerTests(unittest.TestCase):
    def setUp(self):
        self.rounds = 8
        self.sched = scan_cells_schedule(self.rounds, arm="str")
        load = {
            "since": "scan_cells_str:throughput",
            "wall_s": 1.0,
            "busy_cpus_since_prev": 1.0,
            "own_busy_cpus": 1.0,
            "foreign_busy_cpus": 0.0,
        }
        self.prov = new_provenance(
            suite="concurrency",
            issue=1143,
            ratio="Batch cursor scan over scan_locked",
            repo_root=REPO_ROOT,
            core_pin=STR_SCAN_CELLS_PIN,
            estimators=estimators("test"),
        )
        t_pass, c_pass = make_synthetic_str_rows(self.sched, scan_w0_r4_speedup=3.5, scan_w1_r4_speedup=3.5, scan_w4_r4_speedup=2.5)
        cells_pass = summarize_scan_cells(t_pass, c_pass, self.rounds, load, arm="str")
        self.art_pass = build_scan_cells_artifact(self.prov, cells_pass, t_pass, self.rounds, STR_SCAN_CELLS_PIN, quick=False, arm="str")

        t_fail, c_fail = make_synthetic_str_rows(self.sched, scan_w0_r4_speedup=3.5, scan_w1_r4_speedup=2.0, scan_w4_r4_speedup=2.5)
        cells_fail = summarize_scan_cells(t_fail, c_fail, self.rounds, load, arm="str")
        self.art_fail = build_scan_cells_artifact(self.prov, cells_fail, t_fail, self.rounds, STR_SCAN_CELLS_PIN, quick=False, arm="str")

    def test_combiner_pass_when_both_runs_hold_str(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f1, f2 = tmp / "run1.json", tmp / "run2.json"
            f1.write_text(json.dumps(self.art_pass))
            f2.write_text(json.dumps(self.art_pass))
            res = combine_scan_cells_verdict(f1, f2)
            self.assertEqual(res["verdict"], "PASS")
            self.assertEqual(res["evaluation"]["standard"], "METHODOLOGY.md §33.4 P33.3")
            self.assertEqual(len(res["evaluation"]["failing_cells"]), 0)

    def test_negative_control_two_runs_vs_one_discrimination_str(self):
        """Negative control (b) for Str: Discriminates two-run conjunction vs accepting one run."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f1, f2 = tmp / "run1.json", tmp / "run2.json"
            f1.write_text(json.dumps(self.art_pass))
            f2.write_text(json.dumps(self.art_fail))
            res = combine_scan_cells_verdict(f1, f2)
            self.assertEqual(res["verdict"], "REFUTED")
            failing = res["evaluation"]["failing_cells"]
            self.assertGreater(len(failing), 0)
            self.assertTrue(all("run2" in f["failing_runs"] for f in failing))

    def test_negative_control_refuses_quick_void_artifact_str(self):
        """Negative control (c) for Str: Discriminates void enforcement vs un-gated evaluation."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f_quick, f_valid = tmp / "quick.json", tmp / "valid.json"
            art_quick = json.loads(json.dumps(self.art_pass))
            art_quick["scan_cells"]["quick"] = True
            art_quick["scan_cells"]["void"] = ["--quick smoke run"]
            f_quick.write_text(json.dumps(art_quick))
            f_valid.write_text(json.dumps(self.art_pass))
            with self.assertRaises(ValueError) as ctx:
                combine_scan_cells_verdict(f_quick, f_valid)
            self.assertIn("--quick", str(ctx.exception))

    def test_combiner_refuses_commit_mismatch_str(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f1, f2 = tmp / "run1.json", tmp / "run2.json"
            art_diff = json.loads(json.dumps(self.art_pass))
            art_diff["provenance"]["commit"] = "0123456789abcdef"
            f1.write_text(json.dumps(self.art_pass))
            f2.write_text(json.dumps(art_diff))
            with self.assertRaises(ValueError) as ctx:
                combine_scan_cells_verdict(f1, f2)
            self.assertIn("commit mismatch", str(ctx.exception))

    def test_combiner_refuses_missing_cell_str(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f1, f2 = tmp / "run1.json", tmp / "run2.json"
            art_missing = json.loads(json.dumps(self.art_pass))
            art_missing["scan_cells"]["p33_3"]["cells"] = [
                c for c in art_missing["scan_cells"]["p33_3"]["cells"]
                if not (c["probe"] == "paths" and c["writers"] == 1 and c["readers"] == 4)
            ]
            f1.write_text(json.dumps(self.art_pass))
            f2.write_text(json.dumps(art_missing))
            with self.assertRaises(ValueError) as ctx:
                combine_scan_cells_verdict(f1, f2)
            self.assertIn("missing gated cell", str(ctx.exception))


class StrScanCellsArtifactTests(unittest.TestCase):
    def test_build_str_artifact_structure(self):
        rounds = 8
        sched = scan_cells_schedule(rounds, arm="str")
        t_rows, c_rows = make_synthetic_str_rows(sched)
        load = {
            "since": "scan_cells_str:throughput",
            "wall_s": 1.0,
            "busy_cpus_since_prev": 1.0,
            "own_busy_cpus": 1.0,
            "foreign_busy_cpus": 0.0,
        }
        cells = summarize_scan_cells(t_rows, c_rows, rounds, load, arm="str")
        prov = new_provenance(
            suite="concurrency",
            issue=1143,
            ratio="Batch cursor scan over scan_locked",
            repo_root=REPO_ROOT,
            core_pin=STR_SCAN_CELLS_PIN,
            estimators=estimators("test"),
        )
        art = build_scan_cells_artifact(
            prov, cells, t_rows, rounds, STR_SCAN_CELLS_PIN, quick=True, arm="str"
        )
        self.assertIn("provenance", art)
        self.assertIn("throughput", art)
        self.assertEqual(len(art["throughput"]), len(STR_SCAN_CELLS_PROBES) * len(STR_SCAN_BLOCK))
        self.assertIn("scan_cells", art)
        sc = art["scan_cells"]
        self.assertEqual(sc["issue"], 1143)
        self.assertEqual(sc["arm"], "str")
        self.assertEqual(sc["rounds"], rounds)
        self.assertEqual(sc["pin"]["applied"], STR_SCAN_CELLS_PIN)
        self.assertTrue(sc["pin"]["conforms"])
        self.assertIn("p33_3", sc)
        self.assertEqual(sc["p33_3"]["verdict"], "LB_AT_OR_ABOVE_FLOOR")
        self.assertTrue(any("--quick" in v for v in sc["void"]))


class StrScanCellsRefusalTests(unittest.TestCase):
    def test_str_scan_fails_closed_until_pr2(self):
        """Verify that --arm str with --read-op scan fails closed with non-zero exit code
        and descriptive refusal message until SyncStrMapCursor is implemented in PR 2.
        """
        tp_bin, _ = get_binaries()
        if not tp_bin.exists():
            from writer_scaling import build_binaries
            tp_bin, _ = build_binaries(verbose=False)

        cmd = [
            str(tp_bin),
            "--role", "throughput",
            "--arm", "str",
            "--read-op", "scan",
            "--writers", "0",
            "--readers", "1",
            "--probe", "paths",
            "--quick",
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("SyncStrMapCursor not yet implemented (#1143 PR 2)", proc.stderr)


class ScanCellsTimeoutCapTests(unittest.TestCase):
    def test_timeout_cap_constant_defined(self):
        """Pin the target per-cell wall-clock cap constant."""
        self.assertEqual(SCAN_CELL_TIMEOUT_S, 120.0)

    def test_fake_cell_breaching_cap_is_killed_and_fails_fast(self):
        """Negative control (d): A cell stalling past the per-cell wall-clock cap
        is killed, raises CellTimeoutError, and fails fast (AGENTS.md §8.1).
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            fake_sleep = tmp / "fake_sleep.py"
            fake_sleep.write_text("import time\ntime.sleep(0.5)\n")
            fake_bin = tmp / "fake_cell.sh"
            fake_bin.write_text(f"#!/bin/sh\nexec {sys.executable} {fake_sleep} \"$@\"\n")
            fake_bin.chmod(0o755)
            fake_run = {
                "round": 0,
                "position": 0,
                "read_op": "scan",
                "probe": "paths",
                "writers": 0,
                "readers": 1,
            }
            with self.assertRaises(CellTimeoutError) as ctx:
                run_reader_invocation(
                    fake_bin,
                    "throughput",
                    fake_run,
                    quick=True,
                    arm="str",
                    timeout_s=0.05,
                )
            self.assertEqual(ctx.exception.cap_s, 0.05)
            self.assertIn("breached per-cell wall-clock cap", str(ctx.exception))
            self.assertIn("(target)", str(ctx.exception))

    def test_void_artifact_rejected_by_combiner(self):
        """A timeout-voided artifact is rejected by combine_scan_cells_verdict."""
        rounds = 8
        prov = new_provenance(
            suite="concurrency",
            issue=1143,
            ratio="Batch cursor scan over scan_locked",
            repo_root=REPO_ROOT,
            core_pin=STR_SCAN_CELLS_PIN,
            estimators=estimators("test"),
        )
        void_reason = "cell breached per-cell wall-clock cap of 120.0s (target)"
        void_art = build_void_scan_cells_artifact(
            prov, rounds, STR_SCAN_CELLS_PIN, quick=False, arm="str", void_reasons=[void_reason]
        )
        self.assertIn(void_reason, void_art["scan_cells"]["void"])
        self.assertEqual(void_art["scan_cells"]["p33_3"]["verdict"], "VOID")

        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            f_void, f_pass = tmp / "void.json", tmp / "pass.json"
            f_void.write_text(json.dumps(void_art))

            sched = scan_cells_schedule(rounds, arm="str")
            load = {
                "since": "test",
                "wall_s": 1.0,
                "busy_cpus_since_prev": 1.0,
                "own_busy_cpus": 1.0,
                "foreign_busy_cpus": 0.0,
            }
            t_pass, c_pass = make_synthetic_str_rows(sched)
            cells_pass = summarize_scan_cells(t_pass, c_pass, rounds, load, arm="str")
            art_pass = build_scan_cells_artifact(
                prov, cells_pass, t_pass, rounds, STR_SCAN_CELLS_PIN, quick=False, arm="str"
            )
            f_pass.write_text(json.dumps(art_pass))

            with self.assertRaises(ValueError) as ctx:
                combine_scan_cells_verdict(f_void, f_pass)
            self.assertIn("is void", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()

