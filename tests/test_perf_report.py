#!/usr/bin/env python3
"""
Unit tests for scripts/perf_report.py.

Tests benchmark categorization, N (operations) mapping and plumbing,
memory density parsing (64-bit and 32-bit), Callgrind cache simulation,
regression checks, benchmark source code coverage, and overall telemetry comment rendering.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# Add scripts directory to sys.path
REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from perf_report import (
    BENCH_N_MAP,
    CATEGORIES,
    SMOKE_BENCH_N_MAP,
    categorize_benchmarks,
    check_regressions,
    fmt_delta,
    format_ins_per_op,
    format_n,
    get_bench_n,
    parse,
    parse_bytes_32,
    parse_bytes_64,
    pct,
    render,
    render_cache_simulation,
    verdict,
)


SAMPLE_HEAD_TEXT = """
instructions::cost::map_get sequential:"sequential"
  Instructions: 7,000,213|
  Estimated Cycles: 9,249,128|
  L1 Hits: 8,813,323|
  LL Hits: 86,972|
  RAM Hits: 27|
instructions::cost::map_get random:"random"
  Instructions: 6,312,260|
  Estimated Cycles: 8,647,889|
  L1 Hits: 7,914,874|
  LL Hits: 146,400|
  RAM Hits: 29|
instructions::cost::map_get clustered:"clustered"
  Instructions: 5,669,909|
  Estimated Cycles: 7,803,230|
  L1 Hits: 7,500,000|
  LL Hits: 100,000|
  RAM Hits: 10|
instructions::cost::set_contains random:"random"
  Instructions: 6,189,406|
  Estimated Cycles: 8,317,021|
  L1 Hits: 7,800,000|
  LL Hits: 90,000|
  RAM Hits: 15|
instructions::cost::map32_get can_dispatch:"can_dispatch"
  Instructions: 460,715|
  Estimated Cycles: 656,161|
  L1 Hits: 600,000|
  LL Hits: 5,000|
  RAM Hits: 2|
instructions::range_cost::map_range random:"random"
  Instructions: 79,864|
  Estimated Cycles: 117,865|
  L1 Hits: 111,975|
  LL Hits: 471|
  RAM Hits: 101|
instructions::range_cost::map_range sequential:"sequential"
  Instructions: 420,493|
  Estimated Cycles: 607,353|
  L1 Hits: 580,000|
  LL Hits: 2,000|
  RAM Hits: 50|
instructions::cost::map_insert random:"random"
  Instructions: 27,300,146|
  Estimated Cycles: 40,588,608|
  L1 Hits: 38,578,583|
  LL Hits: 203,576|
  RAM Hits: 28,347|
instructions::cost::set32_insert sensor_timestamps:"sensor_timestamps"
  Instructions: 13,663,172|
  Estimated Cycles: 18,850,153|
  L1 Hits: 18,000,000|
  LL Hits: 50,000|
  RAM Hits: 1,000|
instructions::cost::blobmap32_scan ipv4_routes:"ipv4_routes"
  Instructions: 2,592,075|
  Estimated Cycles: 3,765,775|
  L1 Hits: 3,600,000|
  LL Hits: 10,000|
  RAM Hits: 500|
"""

SAMPLE_BASE_TEXT = """
instructions::cost::map_get sequential:"sequential"
  Instructions: 7,000,000|
  Estimated Cycles: 9,249,000|
instructions::cost::map_get random:"random"
  Instructions: 6,312,260|
  Estimated Cycles: 8,647,889|
instructions::cost::map_get clustered:"clustered"
  Instructions: 5,669,909|
  Estimated Cycles: 7,803,230|
instructions::cost::set_contains random:"random"
  Instructions: 6,189,406|
  Estimated Cycles: 8,317,021|
instructions::cost::map32_get can_dispatch:"can_dispatch"
  Instructions: 460,715|
  Estimated Cycles: 656,161|
instructions::range_cost::map_range random:"random"
  Instructions: 83,210|
  Estimated Cycles: 122,580|
instructions::range_cost::map_range sequential:"sequential"
  Instructions: 428,420|
  Estimated Cycles: 617,850|
instructions::cost::map_insert random:"random"
  Instructions: 27,300,146|
  Estimated Cycles: 40,588,608|
instructions::cost::set32_insert sensor_timestamps:"sensor_timestamps"
  Instructions: 13,663,172|
  Estimated Cycles: 18,850,153|
instructions::cost::blobmap32_scan ipv4_routes:"ipv4_routes"
  Instructions: 2,592,075|
  Estimated Cycles: 3,765,775|
"""

SAMPLE_BYTES_64 = """
bytes/key by distribution and population (set flavor / map flavor)
target from docs/ARCHITECTURE.md: < 9.5 B/key dense+clustered (set)

dist                   pop      set B/key      map B/key
sequential            1000           0.32           8.70
sequential          100000           0.07           8.57
sequential         1000000           0.07           8.56
random                1000          13.50          24.10
random              100000          14.78          24.62
random             1000000           7.92          16.70
clustered             1000           0.38           8.64
clustered           100000           0.37           8.62
clustered          1000000           0.36           8.61
clustered-wide        1000           0.32           8.70
clustered-wide      100000           0.12           8.60
clustered-wide     1000000           0.12           8.60
sparse                1000          16.83          16.83
sparse              100000          16.32          16.32
sparse             1000000          16.31          16.31

(map B/key includes the 8-byte value per key)
memory budget: all distributions within ceilings
"""

SAMPLE_BYTES_32 = """
==========================================================================
Expanse 32-Bit Trie — Real Measured Memory Density (mem_used)
==========================================================================
1. Clustered sensor timestamps (N = 10000): 6736 bytes (0.6736 B/key)
2. Sparse 29-bit CAN IDs (N = 500): 6304 bytes (12.6080 B/key)
3. IPv4 subnet routing map (N = 2000): 18752 bytes (9.3760 B/key)
4. Dense consecutive map (N = 10000): 52144 bytes (5.2144 B/key)
5. OTA firmware inline checksums (N = 1000): 1000 live records

All 32-bit memory-density regression guards held.
"""


def test_parse_callgrind_output():
    parsed = parse(SAMPLE_HEAD_TEXT)
    assert "map_get/sequential" in parsed
    assert parsed["map_get/sequential"]["Instructions"] == 7000213
    assert parsed["map_get/sequential"]["Estimated Cycles"] == 9249128
    assert parsed["map_get/sequential"]["L1 Hits"] == 8813323
    assert parsed["map_get/sequential"]["LL Hits"] == 86972
    assert parsed["map_get/sequential"]["RAM Hits"] == 27
    assert "map_range/random" in parsed
    assert parsed["map_range/random"]["Instructions"] == 79864


def test_bench_n_mapping_and_formatting():
    # Verify exact N counts for primary benchmark types
    assert get_bench_n("map_get/sequential") == 50_000
    assert get_bench_n("map_get/random") == 50_000
    assert get_bench_n("set_contains/random") == 50_000
    assert get_bench_n("map32_get/can_dispatch") == 500
    assert get_bench_n("map_range/random") == 10_000
    assert get_bench_n("set_range/sequential") == 10_000
    assert get_bench_n("blobmap32_scan/ipv4_routes") == 2_000
    assert get_bench_n("map_insert/random") == 50_000
    assert get_bench_n("set32_insert/sensor_timestamps") == 10_000

    # Smoke mode N counts
    assert get_bench_n("map_get/random", is_smoke=True) == 10_000
    assert get_bench_n("map_insert/sequential", is_smoke=True) == 10_000

    # Formatting of N
    assert format_n(50_000) == "50k"
    assert format_n(10_000) == "10k"
    assert format_n(2_000) == "2k"
    assert format_n(500) == "500"
    assert format_n(1_000_000) == "1M"

    # Ins / Op string formatting
    assert format_ins_per_op(7_000_213, 50_000) == "140.0 (50k)"
    assert format_ins_per_op(79_864, 10_000, bold=True) == "**8.0** (10k)"
    assert format_ins_per_op(460_715, 500) == "921.4 (500)"


def test_categorization_and_uncategorized_fallback():
    bench_list = [
        "map_get/random",
        "set_contains/random",
        "map_range/random",
        "blobmap32_scan/ipv4_routes",
        "map_insert/random",
        "set32_insert/sensor_timestamps",
        "custom_experimental_bench/test",  # Unknown benchmark
    ]

    cats = categorize_benchmarks(bench_list)
    cat_dict = {cat_id: items for cat_id, title, items in cats}

    assert "point_queries" in cat_dict
    assert "map_get/random" in cat_dict["point_queries"]
    assert "set_contains/random" in cat_dict["point_queries"]

    assert "range_scans" in cat_dict
    assert "map_range/random" in cat_dict["range_scans"]
    assert "blobmap32_scan/ipv4_routes" in cat_dict["range_scans"]

    assert "mutations" in cat_dict
    assert "map_insert/random" in cat_dict["mutations"]
    assert "set32_insert/sensor_timestamps" in cat_dict["mutations"]

    # Uncategorized fallback MUST contain the unknown benchmark
    assert "uncategorized" in cat_dict
    assert "custom_experimental_bench/test" in cat_dict["uncategorized"]


def test_parse_bytes_64():
    all_pass, rows = parse_bytes_64(SAMPLE_BYTES_64)
    assert all_pass is True
    assert len(rows) == 5

    seq = next(r for r in rows if "Sequential" in r["dist"])
    assert seq["pop"] == "1,000,000"
    assert seq["set_bpk"] == "**0.07 B**"
    assert seq["map_bpk"] == "**8.56 B**"
    assert seq["status"] == "🟢 Pass"
    assert seq["ceiling"] == "< 9.50 B"

    rand = next(r for r in rows if "Random" in r["dist"])
    assert rand["set_bpk"] == "**7.92 B**"
    assert rand["map_bpk"] == "**16.70 B**"
    assert rand["status"] == "🟢 Pass"
    assert rand["ceiling"] == "< 28.00 B"

    # Test failure detection
    failing_text = SAMPLE_BYTES_64 + "\nMEMORY BUDGET EXCEEDED: random set 9.50 > 9.00 B/key"
    all_pass_fail, _ = parse_bytes_64(failing_text)
    assert all_pass_fail is False


def test_parse_bytes_32():
    all_pass, rows = parse_bytes_32(SAMPLE_BYTES_32)
    assert all_pass is True
    assert len(rows) == 4

    sensor = next(r for r in rows if "Sensor" in r["workload"])
    assert sensor["pop"] == "10,000"
    assert sensor["total_bytes"] == "6,736 B"
    assert sensor["bpk"] == "**0.67 B**"
    assert sensor["status"] == "🟢 Pass"
    assert sensor["ceiling"] == "< 1.50 B"

    routes = next(r for r in rows if "IPv4" in r["workload"])
    assert routes["pop"] == "2,000"
    assert routes["total_bytes"] == "18,752 B"
    assert routes["bpk"] == "**9.38 B**"
    assert routes["status"] == "🟢 Pass"
    assert routes["ceiling"] == "< 12.00 B"


def test_cache_simulation_table():
    head = parse(SAMPLE_HEAD_TEXT)
    lines = render_cache_simulation(head)
    rendered = "\n".join(lines)
    assert "### 4. 🔬 Callgrind-Modeled Memory Hierarchy Simulation" in rendered
    assert "Simulated Hit Ratio" in rendered
    assert "map_get/sequential" in rendered
    assert "99.99%" in rendered  # Hit ratio for map_get/sequential


def test_check_regressions():
    head = parse(SAMPLE_HEAD_TEXT)
    base = parse(SAMPLE_BASE_TEXT)

    # Clean case
    has_viol, msgs = check_regressions(head, base, max_regression_pct=5.0)
    assert has_viol is False

    # Introduce synthetic regression in head
    head_reg = {k: dict(v) for k, v in head.items()}
    head_reg["map_insert/random"]["Instructions"] = 30_000_000  # ~9.89% increase vs base

    has_viol, msgs = check_regressions(head_reg, base, max_regression_pct=5.0)
    assert has_viol is True
    assert len(msgs) > 0
    assert "Performance regression detected" in msgs[0]

    # Test override
    has_viol_ovr, msgs_ovr = check_regressions(
        head_reg, base, max_regression_pct=5.0, allowed=True, allow_reason="Approved refactor"
    )
    assert has_viol_ovr is False
    assert "override acknowledged" in msgs_ovr[0]


def test_rust_benchmark_sources_coverage():
    """Extracts benchmark functions from Rust harnesses and asserts 100% mapping and categorization coverage."""
    harness_files = [
        REPO_ROOT / "crates" / "expanse" / "benches" / "instructions.rs",
        REPO_ROOT / "crates" / "expanse" / "benches" / "smoke_instructions.rs",
        REPO_ROOT / "crates" / "expanse-capi" / "benches" / "vs_stock.rs",
    ]

    discovered_benches = set()
    fn_pattern = re.compile(r"#\[library_benchmark\]\s*(?:#\[bench::\w+[^\]]*\]\s*)*fn\s+([a-zA-Z0-9_]+)")

    for path in harness_files:
        if path.exists():
            content = path.read_text(encoding="utf-8")
            for m in fn_pattern.finditer(content):
                fn_name = m.group(1)
                discovered_benches.add(fn_name)

    assert len(discovered_benches) > 0

    for bench in discovered_benches:
        # Assert each benchmark has an explicit N operations mapping
        n = get_bench_n(bench)
        assert n > 1, f"Benchmark {bench} has missing or unmapped N operations (got {n})"

        # Assert each benchmark maps to a primary category (not uncategorized)
        cats = categorize_benchmarks([bench])
        cat_ids = [c[0] for c in cats]
        assert "uncategorized" not in cat_ids, f"Benchmark {bench} fell into uncategorized fallback"
        assert any(c in {"point_queries", "range_scans", "mutations"} for c in cat_ids), (
            f"Benchmark {bench} did not map to a primary category"
        )


def test_full_rendered_report_structure():
    head = parse(SAMPLE_HEAD_TEXT)
    base = parse(SAMPLE_BASE_TEXT)

    report = render(
        head=head,
        base=base,
        bytes_table=SAMPLE_BYTES_64,
        base_ref="origin/main",
        bytes32_table=SAMPLE_BYTES_32,
        bindings_status="⚡ **0 Native Heap Allocs**",
    )

    # Check Executive Header Table
    assert "## 📊 Expanse CI Performance & Architecture Telemetry" in report
    assert "| Regression Gate | Top Optimization | 64-Bit Memory Density | 32-Bit Embedded Density | Bindings Invariants |" in report
    assert "🟢 **0 Regressions**" in report
    assert "✅ **100% Compliant**" in report
    assert "⚡ **0 Native Heap Allocs**" in report

    # Check Context Note
    assert "against **merge base `origin/main` (expanse's own previous code)**" in report

    # Check Structured Subsystems
    assert "#### 🔍 Point Queries & Lookups" in report
    assert "#### ⚡ Range Scans & Ordered Traversal" in report
    assert "#### ✍️ Mutations & Churn" in report

    # Check Ins / Op column
    assert "Ins / Op ($N$)" in report
    assert "140.0 (50k)" in report

    # Check Memory Ledgers
    assert "### 3. 💾 Memory Density Ledgers (Allocator Accounting)" in report
    assert "64-Bit Server Architecture (Bytes per Key)" in report
    assert "32-Bit Embedded Architecture (RV32 / ESP32 / Cortex-M)" in report
    assert "< 9.50 B" in report

    # Check Cache Simulation
    assert "### 4. 🔬 Callgrind-Modeled Memory Hierarchy Simulation" in report

    # Check Bindings Layered Visibility
    assert "### 5. 🌐 Cross-Language Bindings & FFI Invariants" in report
    assert "Native / FFI Engine Core" in report


def test_chip_regression_logic_consistency():
    head = parse(SAMPLE_HEAD_TEXT)
    base = parse(SAMPLE_BASE_TEXT)

    # 1. No baseline
    report_no_base = render(head=head, base=None, bytes_table=None, base_ref="origin/main")
    assert "⚪ **No Baseline**" in report_no_base

    # 2. Clean: 0 regressions
    report_clean = render(head=head, base=base, bytes_table=None, base_ref="origin/main")
    assert "🟢 **0 Regressions**" in report_clean

    # 3. Sub-threshold regression (e.g. +0.4% which is <= 1.5% max_regression_pct)
    head_sub = {k: dict(v) for k, v in head.items()}
    head_sub["map_get/sequential"]["Instructions"] = int(base["map_get/sequential"]["Instructions"] * 1.004)
    report_sub = render(head=head_sub, base=base, bytes_table=None, base_ref="origin/main", has_violation=False)
    assert "🟡 **1 Regressed (< threshold)**" in report_sub

    # 4. Unacceptable regression without override
    head_viol = {k: dict(v) for k, v in head.items()}
    head_viol["map_get/sequential"]["Instructions"] = int(base["map_get/sequential"]["Instructions"] * 1.10)
    report_viol = render(head=head_viol, base=base, bytes_table=None, base_ref="origin/main", has_violation=True)
    assert "🔴 **1 Regressions**" in report_viol

    # 5. Unacceptable regression with approved override
    report_ovr = render(
        head=head_viol,
        base=base,
        bytes_table=None,
        base_ref="origin/main",
        has_violation=False,
        is_allowed_override=True,
    )
    assert "🟡 **1 Regressed (Approved)**" in report_ovr


def test_bindings_status_default_and_custom():
    head = parse(SAMPLE_HEAD_TEXT)
    base = parse(SAMPLE_BASE_TEXT)

    # Honest default when not supplied by caller
    report_default = render(head=head, base=base, bytes_table=None, base_ref="origin/main")
    assert "⚪ **Not measured (see nightly)**" in report_default

    # Custom caller-supplied status
    report_custom = render(
        head=head,
        base=base,
        bytes_table=None,
        base_ref="origin/main",
        bindings_status="⚡ **0 Native Heap Allocs**",
    )
    assert "⚡ **0 Native Heap Allocs**" in report_custom


if __name__ == "__main__":
    test_parse_callgrind_output()
    test_bench_n_mapping_and_formatting()
    test_categorization_and_uncategorized_fallback()
    test_parse_bytes_64()
    test_parse_bytes_32()
    test_cache_simulation_table()
    test_check_regressions()
    test_rust_benchmark_sources_coverage()
    test_chip_regression_logic_consistency()
    test_bindings_status_default_and_custom()
    test_full_rendered_report_structure()
    print("All perf_report tests passed successfully!")
