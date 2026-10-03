#!/usr/bin/env python3
"""
Unit tests for sync32 step checks and CPU sample keys (#1292).

Tests:
1. Constants and keys verification (no hard-coded medians in source).
2. Wilson score confidence interval calculation (matching #1292 values).
3. Empty runs and single-run unreferenced handling.
4. Single-run fallback against a reference artifact (commit and governor printed).
5. Batch-relative mode on clean runs (10 runs, 0/30 stepped, Wilson CI [0.000, 0.114]).
6. Batch-relative uniform shift invariance (all runs -20% -> 0 stepped).
7. Batch-relative stepped run detection (10k/s duty stepped -> 1/30 stepped).
8. Committed baseline artifact as reference.
9. CPU sample keys presence in sync32 rounds_raw.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# Add concurrency script directory to sys.path
REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "scripts"))

from mixed_concurrency import (
    CPU_SAMPLE_KEYS,
    PLACEMENT_KEYS,
    RAW_KEYS,
    SYNC32,
    SYNC32_PACED_RATES,
    SYNC32_STEP_THRESHOLD_FACTOR,
    check_sync32_steps,
    format_sync32_step_checks,
    summarize_cell,
    wilson_interval,
)


def _make_sync32_cell(rate: int | None, workload: str, threads: int, read_mean: float) -> dict:
    return {
        "workload_id": "core_concurrency",
        "engine_key": SYNC32,
        "engine": "SyncExpanseMap32",
        "workload": workload,
        "read_pct": None,
        "write_rate": rate,
        "threads": threads,
        "read_ops_s_mean": read_mean,
        "read_ops_s_median": read_mean,
        "write_ops_s_mean": float(rate or 0),
        "total_ops_s_mean": read_mean + float(rate or 0),
        "busy_pct": 0.05,
        "refused_writes": 0,
    }


def _make_paced_run(rates: dict[int, float]) -> list[dict]:
    labels = {1_000_000: "1M/s", 100_000: "100k/s", 10_000: "10k/s"}
    return [
        _make_sync32_cell(rate, f"writer {labels[rate]} / N readers try_get", 1, read_mean)
        for rate, read_mean in rates.items()
    ]


def test_constants():
    assert SYNC32_STEP_THRESHOLD_FACTOR == 0.85
    assert SYNC32_PACED_RATES == (1_000_000, 100_000, 10_000)
    assert CPU_SAMPLE_KEYS == ("reader_cpu_samples", "writer_cpu_samples")
    assert PLACEMENT_KEYS == ("reader_placement", "writer_placement")
    # Verify no hardcoded medians dictionary in mixed_concurrency module:
    import mixed_concurrency
    assert not hasattr(mixed_concurrency, "SYNC32_PACED_DUTIES_MEDIAN_READ_OPS_S"), \
        "no hardcoded median constants permitted in source (#1292)"


def test_wilson_interval():
    # Empty / zero bounds
    assert wilson_interval(0, 0) == (0.0, 0.0)

    # #1292 reference value: k=0, n=30 -> [0.000, 0.114]
    lo_0, hi_0 = wilson_interval(0, 30)
    assert lo_0 == 0.0
    assert abs(hi_0 - 0.114) < 0.001, f"got {hi_0}, expected ~0.114"

    # #1292 reference value: k=6, n=18 -> [0.163, 0.562]
    lo_6, hi_6 = wilson_interval(6, 18)
    assert abs(lo_6 - 0.163) < 0.001, f"got {lo_6}, expected ~0.163"
    assert abs(hi_6 - 0.562) < 0.001, f"got {hi_6}, expected ~0.562"


def test_empty_and_unreferenced_runs():
    # Empty input
    res_empty = check_sync32_steps([])
    assert res_empty["mode"] == "empty"
    assert res_empty["stepped_count"] == 0
    assert res_empty["total_duties"] == 0
    assert format_sync32_step_checks(res_empty) == "sync32 step check (#1292): no paced duties present"

    # Single run without reference artifact
    run = _make_paced_run({1_000_000: 39.4e6, 100_000: 45.3e6, 10_000: 46.2e6})
    res_unref = check_sync32_steps([run])
    assert res_unref["mode"] == "single-run-unreferenced"
    assert res_unref["stepped_count"] == 0
    text = format_sync32_step_checks(res_unref)
    assert "single run provided without --reference-artifact" in text


def test_single_run_reference_fallback():
    ref_artifact = {
        "provenance": {
            "commit": "c0ffee123",
            "host": {"governor": "performance"},
        },
        "throughput": _make_paced_run({1_000_000: 40.0e6, 100_000: 45.0e6, 10_000: 46.0e6}),
    }

    # Clean run: matches reference
    clean_run = _make_paced_run({1_000_000: 39.5e6, 100_000: 44.5e6, 10_000: 45.5e6})
    res_clean = check_sync32_steps([clean_run], reference=ref_artifact)
    assert res_clean["mode"] == "reference-fallback"
    assert res_clean["stepped_count"] == 0
    assert res_clean["total_duties"] == 3
    assert res_clean["reference_commit"] == "c0ffee123"
    assert res_clean["reference_governor"] == "performance"
    text_clean = format_sync32_step_checks(res_clean)
    assert "reference fallback: commit c0ffee123, governor performance" in text_clean
    assert "0 of 3 paced duties stepped" in text_clean
    assert "NOT STEPPED" in text_clean
    assert ": STEPPED" not in text_clean

    # Stepped run (10k/s steps to 35.0 M ops/s < 0.85 * 46.0 M = 39.1 M)
    stepped_run = _make_paced_run({1_000_000: 39.5e6, 100_000: 44.5e6, 10_000: 35.0e6})
    res_stepped = check_sync32_steps([stepped_run], reference=ref_artifact)
    assert res_stepped["mode"] == "reference-fallback"
    assert res_stepped["stepped_count"] == 1
    assert res_stepped["total_duties"] == 3
    text_stepped = format_sync32_step_checks(res_stepped)
    assert "10k/s: STEPPED" in text_stepped
    assert "1M/s: NOT STEPPED" in text_stepped
    assert "100k/s: NOT STEPPED" in text_stepped
    assert "1 of 3 paced duties stepped" in text_stepped
    assert "commit c0ffee123" in text_stepped


def test_batch_relative_clean_runs():
    # 10 clean runs: small variations around 40M, 45M, 46M
    runs = []
    for i in range(10):
        f = 1.0 + (i - 4.5) * 0.005  # within +/- 2.5%
        runs.append(_make_paced_run({
            1_000_000: 40.0e6 * f,
            100_000: 45.0e6 * f,
            10_000: 46.0e6 * f,
        }))

    res = check_sync32_steps(runs)
    assert res["mode"] == "batch-relative"
    assert res["runs_count"] == 10
    assert res["stepped_count"] == 0
    assert res["total_duties"] == 30
    assert res["wilson_ci"][0] == 0.0
    assert abs(res["wilson_ci"][1] - 0.114) < 0.001
    text = format_sync32_step_checks(res)
    assert "batch-relative over 10 runs" in text
    assert "0 of 30 paced duties stepped" in text


def test_batch_relative_uniform_shift_invariance():
    # Base 10 clean runs
    base_runs = []
    for i in range(10):
        f = 1.0 + (i - 4.5) * 0.005
        base_runs.append(_make_paced_run({
            1_000_000: 40.0e6 * f,
            100_000: 45.0e6 * f,
            10_000: 46.0e6 * f,
        }))

    # Uniform -20% shift on all runs (scale = 0.80)
    shifted_runs = [
        _make_paced_run({
            1_000_000: 40.0e6 * (1.0 + (i - 4.5) * 0.005) * 0.80,
            100_000: 45.0e6 * (1.0 + (i - 4.5) * 0.005) * 0.80,
            10_000: 46.0e6 * (1.0 + (i - 4.5) * 0.005) * 0.80,
        })
        for i in range(10)
    ]

    res = check_sync32_steps(shifted_runs)
    assert res["mode"] == "batch-relative"
    # Because batch median drops by -20%, all runs remain ~1.0x batch median (>= 0.85x),
    # so under batch-relative rule, exactly 0 duties step:
    assert res["stepped_count"] == 0, (
        f"uniform -20% shift must report 0 stepped duties under batch-relative, got {res['stepped_count']}"
    )
    assert res["total_duties"] == 30


def test_batch_relative_stepped_run_detection():
    # 9 clean runs and 1 run where 10k/s steps to 0.5x
    runs = []
    for i in range(9):
        f = 1.0 + (i - 4.0) * 0.005
        runs.append(_make_paced_run({
            1_000_000: 40.0e6 * f,
            100_000: 45.0e6 * f,
            10_000: 46.0e6 * f,
        }))
    # Run 10: 10k/s stepped down to 20M ops/s (batch median ~ 46M ops/s, floor ~ 39.1M ops/s)
    runs.append(_make_paced_run({
        1_000_000: 40.0e6,
        100_000: 45.0e6,
        10_000: 20.0e6,
    }))

    res = check_sync32_steps(runs)
    assert res["mode"] == "batch-relative"
    assert res["runs_count"] == 10
    assert res["stepped_count"] == 1
    assert res["total_duties"] == 30
    text = format_sync32_step_checks(res)
    assert "1 of 30 paced duties stepped" in text
    assert "run 10 10k/s: STEPPED" in text


def test_committed_baseline_artifact_as_reference():
    baseline_path = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "baseline_concurrent_mixed.json"
    if not baseline_path.is_file():
        return
    data = json.loads(baseline_path.read_text())
    cells = data.get("throughput", [])
    # Evaluated against itself as reference:
    res = check_sync32_steps([cells], reference=data)
    assert res["mode"] == "reference-fallback"
    assert res["stepped_count"] == 0
    assert res["total_duties"] == 3
    assert res["reference_commit"] == data.get("provenance", {}).get("commit", "unknown")


def test_cpu_samples_in_rounds_raw():
    load = {"scope": "test", "foreign_busy_cpus": 0.0}
    rows_with_samples = [
        {
            "workload_id": "core_concurrency",
            "engine_key": SYNC32,
            "engine": "SYNC32",
            "workload": "writer 10k/s / N readers try_get",
            "read_pct": None,
            "write_rate": 10000,
            "threads": 2,
            "round": r,
            "position": 0,
            "elapsed_s": 0.5,
            "read_ops": 1000 + r * 10,
            "write_ops": 500 + r * 5,
            "busy": 0,
            "ok": 1000,
            "refused": 0,
            "remove_hits": 0,
            "compactions": 0,
            "arena_bytes": 0,
            "compact_ns": 0,
            "reader_placement": [[2, 1, 2, 1], [4, 2, 4, 2]],
            "writer_placement": [0, 0, 0, 0],
            "reader_cpu_samples": [[2, 2, 2], [4, 4, 4]],
            "writer_cpu_samples": [0, 0, 0],
        }
        for r in range(3)
    ]
    cell = summarize_cell(rows_with_samples, None, load)
    assert set(cell["rounds_raw"][0]) == set(RAW_KEYS + PLACEMENT_KEYS + CPU_SAMPLE_KEYS)
    assert cell["rounds_raw"][0]["reader_cpu_samples"] == [[2, 2, 2], [4, 4, 4]]
    assert cell["rounds_raw"][0]["writer_cpu_samples"] == [0, 0, 0]


if __name__ == "__main__":
    import traceback

    tests = [(name, fn) for name, fn in sorted(globals().items()) if name.startswith("test_") and callable(fn)]
    if not tests:
        sys.exit("test_sync32_step.py: no tests found")
    failed = []
    for name, fn in tests:
        try:
            fn()
            print(f"PASS {name}")
        except Exception:
            failed.append(name)
            print(f"FAIL {name}")
            traceback.print_exc()
    print(f"test_sync32_step.py: {len(tests) - len(failed)}/{len(tests)} passed")
    sys.exit(1 if failed else 0)
