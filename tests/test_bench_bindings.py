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

from bench_bindings import (
    compare_against_baseline,
    format_markdown_report,
    _extract_json_result,
    _parse_go_bench_output,
    DOTNET_JSON_MARKER,
)


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


# --- Go: `go test -bench . -benchmem` text-output parsing --------------------------

# Captured from a real `go test -run '^$' -bench . -benchmem -benchtime=100ms .` run in
# bindings/go against a release-built libexpanse (Apple M1, darwin/arm64).
GO_BENCH_OUTPUT = """goos: darwin
goarch: arm64
pkg: github.com/orieg/expanse/bindings/go
cpu: Apple M1
BenchmarkExpanseMap_Insert_Random-8     \t  656744\t       153.1 ns/op\t       0 B/op\t       0 allocs/op
BenchmarkGoMap_Insert_Random-8          \t 2226757\t        55.49 ns/op\t       0 B/op\t       0 allocs/op
BenchmarkExpanseMap_Lookup_Random-8     \t 1389916\t        86.45 ns/op\t       8 B/op\t       1 allocs/op
BenchmarkGoMap_Lookup_Random-8          \t10879747\t        10.91 ns/op\t       0 B/op\t       0 allocs/op
BenchmarkExpanseSet_Insert_Random-8     \t 1000000\t       129.5 ns/op\t       0 B/op\t       0 allocs/op
BenchmarkGoSet_Insert_Random-8          \t 1850782\t        59.34 ns/op\t       0 B/op\t       0 allocs/op
BenchmarkExpanseSet_Contains_Random-8   \t 1970653\t        62.06 ns/op\t       0 B/op\t       0 allocs/op
BenchmarkGoSet_Contains_Random-8        \t11908010\t        10.51 ns/op\t       0 B/op\t       0 allocs/op
BenchmarkExpanseSet_CountRange-8        \t   63943\t      1830 ns/op\t       0 B/op\t       0 allocs/op
PASS
ok  \tgithub.com/orieg/expanse/bindings/go\t2.121s
"""


def test_parse_go_bench_output():
    stats = _parse_go_bench_output(GO_BENCH_OUTPUT)

    assert stats["ExpanseMap_Insert_Random"]["ns_per_op"] == 153.1
    assert stats["ExpanseMap_Insert_Random"]["bytes_per_op"] == 0.0
    assert stats["ExpanseMap_Insert_Random"]["allocs_per_op"] == 0.0

    assert stats["ExpanseMap_Lookup_Random"]["ns_per_op"] == 86.45
    assert stats["ExpanseMap_Lookup_Random"]["bytes_per_op"] == 8.0
    assert stats["ExpanseMap_Lookup_Random"]["allocs_per_op"] == 1.0

    assert stats["GoMap_Lookup_Random"]["ns_per_op"] == 10.91

    # A benchmark line with no cpu-count suffix at all is still not silently dropped
    # into an unrelated key — every parsed name must come from an actual Benchmark line.
    assert "PASS" not in stats
    assert len(stats) == 9


def test_parse_go_bench_output_empty_on_no_matches():
    assert _parse_go_bench_output("no benchmarks here\n") == {}


# --- Java / .NET: extracting one JSON result line out of noisy build-tool output ---

# `mvn -q -B test-compile exec:java ...` runs ExpanseBenchmark.main() unforked in the
# same JVM, so its stdout is normally the single JSON line below with -q suppressing
# Maven's own logging. This fixture also includes a stray blank line and a warning a
# plugin might emit, to confirm extraction tolerates surrounding noise.
JAVA_MVN_OUTPUT = (
    "[WARNING] Some problem with dependency convergence\n"
    "\n"
    '{"runtime": "java", "results": [{"dist": "random", "pop": 10000, '
    '"expanse_map": {"insert_mops": 5.14, "lookup_mops": 3.87, "lookup_ns": 258.4, "bytes_per_key": 23.89}, '
    '"java_hashmap": {"insert_mops": 5.08, "lookup_mops": 6.67, "lookup_ns": 149.99, "bytes_per_key": 64.0}}]}\n'
)

# `dotnet test --logger "console;verbosity=normal"` interleaves VSTest/xUnit diagnostic
# lines with the marker-prefixed JSON line Console.WriteLine emits (see
# bindings/dotnet/tests/Expanse.NET.Tests/ExpanseBenchmark.cs).
DOTNET_TEST_OUTPUT = (
    "Starting test execution, please wait...\n"
    "[xUnit.net 00:00:00.06]   Starting:    Expanse.NET.Tests\n"
    + DOTNET_JSON_MARKER
    + '{"runtime": "dotnet", "results": [{"dist": "random", "pop": 10000, '
    '"expanse_map": {"insert_mops": 11.40, "lookup_mops": 35.79, "lookup_ns": 27.94, "bytes_per_key": 23.89}, '
    '"dotnet_dictionary": {"insert_mops": 21.67, "lookup_mops": 15.85, "lookup_ns": 63.09, "bytes_per_key": 32.0}}]}\n'
    "[xUnit.net 00:00:00.13]   Finished:    Expanse.NET.Tests\n"
    "  Passed Expanse.Tests.ExpanseBenchmark.RunComparativeBenchmark [53 ms]\n"
    "\n"
    "Test Run Successful.\n"
)


def test_extract_json_result_java_from_noisy_maven_output():
    data = _extract_json_result(JAVA_MVN_OUTPUT, "java")
    assert data is not None
    assert data["runtime"] == "java"
    assert data["results"][0]["expanse_map"]["bytes_per_key"] == 23.89


def test_extract_json_result_dotnet_requires_marker():
    data = _extract_json_result(DOTNET_TEST_OUTPUT, "dotnet", marker=DOTNET_JSON_MARKER)
    assert data is not None
    assert data["runtime"] == "dotnet"
    assert data["results"][0]["expanse_map"]["insert_mops"] == 11.40

    # Without the marker, unrelated JSON-shaped log lines must not be mistaken for
    # the result (there are none in this fixture, so extraction correctly finds nothing).
    assert _extract_json_result(DOTNET_TEST_OUTPUT, "dotnet") is None


def test_extract_json_result_returns_none_when_absent():
    assert _extract_json_result("no json here\njust logs\n", "java") is None
    assert _extract_json_result("no json here\njust logs\n", "dotnet", marker=DOTNET_JSON_MARKER) is None


# --- Baseline comparison for the three new runtimes ---------------------------------

def test_compare_against_baseline_go_java_dotnet_passing():
    baseline = [
        {
            "runtime": "go",
            "results": [{
                "dist": "random", "pop": 100_000,
                "expanse_map": {"insert_mops": 3.00, "lookup_mops": 3.10, "lookup_ns": 322.0, "bytes_per_key": 0.0},
            }],
        },
        {
            "runtime": "java",
            "results": [{
                "dist": "random", "pop": 10_000,
                "expanse_map": {"insert_mops": 5.00, "lookup_mops": 4.00, "lookup_ns": 250.0, "bytes_per_key": 23.89},
            }],
        },
        {
            "runtime": "dotnet",
            "results": [{
                "dist": "random", "pop": 10_000,
                "expanse_map": {"insert_mops": 11.00, "lookup_mops": 35.00, "lookup_ns": 28.5, "bytes_per_key": 23.89},
            }],
        },
    ]

    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(baseline, f)
        base_path = f.name

    try:
        # Small, within-tolerance improvements/variation across all three new runtimes.
        current = [
            {"runtime": "go", "results": [{"dist": "random", "pop": 100_000,
                "expanse_map": {"insert_mops": 2.95, "lookup_mops": 3.11, "lookup_ns": 321.9, "bytes_per_key": 0.0}}]},
            {"runtime": "java", "results": [{"dist": "random", "pop": 10_000,
                "expanse_map": {"insert_mops": 5.14, "lookup_mops": 3.87, "lookup_ns": 258.4, "bytes_per_key": 23.89}}]},
            {"runtime": "dotnet", "results": [{"dist": "random", "pop": 10_000,
                "expanse_map": {"insert_mops": 11.40, "lookup_mops": 35.79, "lookup_ns": 27.94, "bytes_per_key": 23.89}}]},
        ]

        has_reg, report = compare_against_baseline(current, base_path, max_regression_pct=25.0, max_memory_regression_pct=10.0)
        assert has_reg is False
        assert "`GO`" in report
        assert "`JAVA`" in report
        assert "`DOTNET`" in report
    finally:
        Path(base_path).unlink(missing_ok=True)


def test_compare_against_baseline_dotnet_regression():
    baseline = [{
        "runtime": "dotnet",
        "results": [{
            "dist": "random", "pop": 10_000,
            "expanse_map": {"insert_mops": 11.00, "lookup_mops": 35.00, "lookup_ns": 28.5, "bytes_per_key": 23.89},
        }],
    }]

    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(baseline, f)
        base_path = f.name

    try:
        current = [{
            "runtime": "dotnet",
            "results": [{
                "dist": "random", "pop": 10_000,
                # -50% lookup throughput: exceeds the 25% default tolerance.
                "expanse_map": {"insert_mops": 11.00, "lookup_mops": 17.50, "lookup_ns": 57.0, "bytes_per_key": 23.89},
            }],
        }]

        has_reg, report = compare_against_baseline(current, base_path, max_regression_pct=25.0, max_memory_regression_pct=10.0)
        assert has_reg is True
        assert "dotnet random lookup throughput regressed -50.0%" in report
    finally:
        Path(base_path).unlink(missing_ok=True)


if __name__ == "__main__":
    test_compare_against_baseline_passing()
    test_compare_against_baseline_throughput_regression()
    test_compare_against_baseline_memory_regression()
    test_parse_go_bench_output()
    test_parse_go_bench_output_empty_on_no_matches()
    test_extract_json_result_java_from_noisy_maven_output()
    test_extract_json_result_dotnet_requires_marker()
    test_extract_json_result_returns_none_when_absent()
    test_compare_against_baseline_go_java_dotnet_passing()
    test_compare_against_baseline_dotnet_regression()
    print("All bench_bindings tests passed successfully!")
