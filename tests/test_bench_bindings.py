#!/usr/bin/env python3
"""
Unit tests for scripts/bench_bindings.py baseline regression checking.
"""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from bench_bindings import compare_against_baseline, format_markdown_report


SAMPLE_BASELINE = [
    {
        "runtime": "python",
        "results": [
            {
                "dist": "random",
                "pop": 20000,
                "expanse_map": {
                    "insert_mops": 2.50,
                    "lookup_mops": 6.00,
                    "lookup_ns": 166.67,
                    "iter_mops": 8.00,
                    "bytes_per_key": 24.50,
                },
                "python_dict": {
                    "insert_mops": 1.80,
                    "lookup_mops": 4.50,
                    "lookup_ns": 222.22,
                    "iter_mops": 6.00,
                    "bytes_per_key": 64.00,
                },
            }
        ],
    }
]


def test_compare_against_baseline_passing():
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(SAMPLE_BASELINE, f)
        base_path = f.name

    try:
        current = [
            {
                "runtime": "python",
                "results": [
                    {
                        "dist": "random",
                        "pop": 20000,
                        "expanse_map": {
                            "insert_mops": 2.45,  # -2.0% (within 25% tolerance)
                            "lookup_mops": 6.10,  # +1.6% (improved)
                            "lookup_ns": 163.93,
                            "iter_mops": 8.00,
                            "bytes_per_key": 24.50,
                        },
                    }
                ],
            }
        ]

        has_reg, report = compare_against_baseline(
            current, base_path, max_regression_pct=25.0, max_memory_regression_pct=10.0
        )
        assert has_reg is False
        assert "All binding metrics within baseline tolerance thresholds" in report
    finally:
        Path(base_path).unlink(missing_ok=True)


def test_compare_against_baseline_throughput_regression():
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(SAMPLE_BASELINE, f)
        base_path = f.name

    try:
        current = [
            {
                "runtime": "python",
                "results": [
                    {
                        "dist": "random",
                        "pop": 20000,
                        "expanse_map": {
                            "insert_mops": 1.50,  # -40.0% (exceeds 25% tolerance!)
                            "lookup_mops": 6.00,
                            "lookup_ns": 166.67,
                            "iter_mops": 8.00,
                            "bytes_per_key": 24.50,
                        },
                    }
                ],
            }
        ]

        has_reg, report = compare_against_baseline(
            current, base_path, max_regression_pct=25.0, max_memory_regression_pct=10.0
        )
        assert has_reg is True
        assert "Regressions Detected" in report
        assert "insert throughput regressed -40.0%" in report
    finally:
        Path(base_path).unlink(missing_ok=True)


def test_compare_against_baseline_memory_regression():
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(SAMPLE_BASELINE, f)
        base_path = f.name

    try:
        current = [
            {
                "runtime": "python",
                "results": [
                    {
                        "dist": "random",
                        "pop": 20000,
                        "expanse_map": {
                            "insert_mops": 2.50,
                            "lookup_mops": 6.00,
                            "lookup_ns": 166.67,
                            "iter_mops": 8.00,
                            "bytes_per_key": 32.00,  # +30.6% memory increase (exceeds 10%)
                        },
                    }
                ],
            }
        ]

        has_reg, report = compare_against_baseline(
            current, base_path, max_regression_pct=25.0, max_memory_regression_pct=10.0
        )
        assert has_reg is True
        assert "Regressions Detected" in report
        assert "memory density regressed +30.6%" in report
    finally:
        Path(base_path).unlink(missing_ok=True)


if __name__ == "__main__":
    test_compare_against_baseline_passing()
    test_compare_against_baseline_throughput_regression()
    test_compare_against_baseline_memory_regression()
    print("All bench_bindings tests passed successfully!")
