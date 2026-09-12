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
    categorize_benchmarks,
    check_regressions,
    format_ins_per_op,
    format_n,
    get_bench_n,
    normalize_bench_name,
    parse,
    parse_bytes_32,
    parse_bytes_64,
    render,
    render_cache_simulation,
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

# Captured from `cargo run --release -p expanse-trie --example bytes_per_key_32`;
# each density line carries the guard it is checked against.
SAMPLE_BYTES_32 = """
==========================================================================
Expanse 32-Bit Trie — Real Measured Memory Density (mem_used)
==========================================================================
1. Clustered sensor timestamps (N = 10000): 3152 bytes (0.3152 B/key, guard 1.00)
2. Sparse 29-bit CAN IDs (N = 500): 4960 bytes (9.9200 B/key, guard 20.00)
3. IPv4 subnet routing map (N = 2000): 16832 bytes (8.4160 B/key, guard 24.00)
4. Dense consecutive map (N = 10000): 44240 bytes (4.4240 B/key, guard 12.00)
5. Uniform-random 32-bit keys (N = 5000): 67100 bytes (13.4200 B/key, guard 20.00)
6. OTA firmware inline checksums (N = 1000): 1000 live records

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
    assert format_ins_per_op(2_592_075, 2_000) == "1,296.0 (2k)"
    assert format_ins_per_op(13_663_172, 10_000) == "1,366.3 (10k)"


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


def _enforced_bytes_64_budgets() -> dict[str, tuple[float, float]]:
    """The `(dist, pop, set ceiling, map ceiling)` rows `bytes_per_key.rs` enforces."""
    src = (REPO_ROOT / "crates" / "expanse" / "examples" / "bytes_per_key.rs").read_text(encoding="utf-8")
    row = re.compile(r'\("([a-z-]+)",\s*1_000_000,\s*([0-9.]+),\s*([0-9.]+)\)')
    return {m.group(1): (float(m.group(2)), float(m.group(3))) for m in row.finditer(src)}


def test_parse_bytes_64():
    all_pass, rows = parse_bytes_64(SAMPLE_BYTES_64)
    assert all_pass is True
    assert len(rows) == 5

    # Check canonical distribution order
    dist_order = [r["dist"] for r in rows]
    assert dist_order == [
        "**Sequential**",
        "**Clustered**",
        "**Clustered-Wide**",
        "**Random (Uniform)**",
        "**Sparse (High 24-bit)**",
    ]

    seq = next(r for r in rows if "Sequential" in r["dist"])
    assert seq["pop"] == "1,000,000"
    assert seq["set_bpk"] == "**0.07 B**"
    assert seq["map_bpk"] == "**8.56 B**"
    assert seq["status"] == "🟢 Pass"

    rand = next(r for r in rows if "Random" in r["dist"])
    assert rand["set_bpk"] == "**7.92 B**"
    assert rand["map_bpk"] == "**16.70 B**"
    assert rand["status"] == "🟢 Pass"

    # The displayed ceilings must be the ones the `memory-budget` gate enforces:
    # read them from `examples/bytes_per_key.rs`, so the two tables cannot drift.
    enforced = _enforced_bytes_64_budgets()
    assert len(enforced) == 5, enforced
    labels = {
        "sequential": "**Sequential**",
        "clustered": "**Clustered**",
        "clustered-wide": "**Clustered-Wide**",
        "random": "**Random (Uniform)**",
        "sparse": "**Sparse (High 24-bit)**",
    }
    for dist, (set_max, map_max) in enforced.items():
        row = next(r for r in rows if r["dist"] == labels[dist])
        assert row["ceiling"] == f"set ≤ {set_max:.2f} · map ≤ {map_max:.2f} B", (dist, row["ceiling"])

    # Test failure detection
    failing_text = SAMPLE_BYTES_64 + "\nMEMORY BUDGET EXCEEDED: random set 9.50 > 9.00 B/key"
    all_pass_fail, _ = parse_bytes_64(failing_text)
    assert all_pass_fail is False


def test_parse_bytes_32():
    all_pass, rows = parse_bytes_32(SAMPLE_BYTES_32)
    assert all_pass is True
    assert len(rows) == 5  # five density lines; the OTA line reports records, not bytes

    sensor = next(r for r in rows if "sensor" in r["workload"])
    assert sensor["pop"] == "10,000"
    assert sensor["total_bytes"] == "3,152 B"
    assert sensor["bpk"] == "**0.32 B**"
    assert sensor["status"] == "🟢 Pass"

    routes = next(r for r in rows if "IPv4" in r["workload"])
    assert routes["pop"] == "2,000"
    assert routes["total_bytes"] == "16,832 B"
    assert routes["bpk"] == "**8.42 B**"

    # The ceiling is the guard printed on the same line, never a second copy.
    guards = [float(g) for g in re.findall(r"guard ([0-9.]+)\)", SAMPLE_BYTES_32)]
    assert [r["ceiling"] for r in rows] == [f"≤ {g:.2f} B" for g in guards]

    # A line over its guard fails the table.
    over = SAMPLE_BYTES_32.replace("(0.3152 B/key, guard 1.00)", "(1.3152 B/key, guard 1.00)")
    assert over != SAMPLE_BYTES_32
    assert parse_bytes_32(over)[0] is False


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

    # An override that names no regressed arm is void (AGENTS.md §6).
    has_viol_void, msgs_void = check_regressions(
        head_reg, base, max_regression_pct=5.0, allowed=True, allow_reason="Approved refactor"
    )
    assert has_viol_void is True
    assert any("names no regressed arm" in m for m in msgs_void), msgs_void

    # One that names it is acknowledged.
    has_viol_ovr, msgs_ovr = check_regressions(
        head_reg, base, max_regression_pct=5.0, allowed=True,
        allow_reason="map_insert +9.89% approved refactor; see results/baseline_instructions.json",
    )
    assert has_viol_ovr is False
    assert "override acknowledged" in msgs_ovr[0]


def _duplicate_fn_names(paths: list[Path]) -> int:
    """How many `#[library_benchmark]` fns share a name with one in another harness."""
    names: list[str] = []
    for path in paths:
        if path.exists():
            names += re.findall(r"#\[library_benchmark\][^{]*?fn\s+([a-zA-Z0-9_]+)", path.read_text(encoding="utf-8"), re.S)
    return len(names) - len(set(names))


def test_bench_n_does_not_guess_from_arm_names():
    # The motivating defect: an unanchored digit search took the width in
    # `map32_range` as the operation count.
    assert get_bench_n("map32_range") == 2_000
    assert get_bench_n("map32_range/random") == 1_007  # per-distribution entry wins
    assert get_bench_n("map32_range/sequential") == 2_000
    assert get_bench_n("set32_iterate/clustered") == 2_000
    assert get_bench_n("strmap_churn/routes") == 50_000
    assert get_bench_n("not_a_bench32") == 1  # no explicit count suffix: no guess
    assert get_bench_n("synthetic_10k") == 10_000
    assert get_bench_n("synthetic_2m") == 2_000_000


def test_rust_benchmark_sources_coverage():
    """Extracts benchmark functions from Rust harnesses and asserts 100% mapping and categorization coverage."""
    harness_files = [
        REPO_ROOT / "crates" / "expanse" / "benches" / "instructions.rs",
        REPO_ROOT / "crates" / "expanse" / "benches" / "smoke_instructions.rs",
        REPO_ROOT / "crates" / "expanse-capi" / "benches" / "vs_stock.rs",
    ]

    discovered_benches = set()
    # One unambiguous alternative per attribute or `//` comment line, so a
    # failed match cannot backtrack exponentially (CodeQL py/redos).
    fn_pattern = re.compile(
        r"#\[library_benchmark\]\s*(?:#\[bench::[^\]]*\]\s*|//[^\n]*\n\s*)*fn\s+([a-zA-Z0-9_]+)"
    )

    for path in harness_files:
        if path.exists():
            content = path.read_text(encoding="utf-8")
            for m in fn_pattern.finditer(content):
                fn_name = m.group(1)
                discovered_benches.add(fn_name)

    # Every `#[library_benchmark]` in the harnesses is found, including one
    # whose attributes are interleaved with `//` comment lines.
    total_attrs = sum(
        p.read_text(encoding="utf-8").count("#[library_benchmark]") for p in harness_files if p.exists()
    )
    assert len(discovered_benches) == total_attrs - _duplicate_fn_names(harness_files), (
        len(discovered_benches), total_attrs
    )
    assert "judyl_get_expanse" in discovered_benches

    for bench in discovered_benches:
        # Each benchmark has an explicit N entry: a guessed N (the digit
        # fallback once read 32 out of `map32_range`) is not a mapping.
        assert normalize_bench_name(bench) in BENCH_N_MAP, f"Benchmark {bench} has no BENCH_N_MAP entry"
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
    assert "set ≤ 0.10 · map ≤ 9.00 B" in report  # the enforced sequential ceiling

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
    # Discovers every `test_*` function, so a new test cannot be left out of a
    # hand-kept list, and fails on zero tests as well as on any failure.
    import traceback

    tests = [(name, fn) for name, fn in sorted(globals().items()) if name.startswith("test_") and callable(fn)]
    if not tests:
        sys.exit("test_perf_report.py: no tests found")
    failed = []
    for name, fn in tests:
        try:
            fn()
        except Exception:  # noqa: BLE001 - report every failure, then exit non-zero
            failed.append(name)
            print(f"FAIL {name}")
            traceback.print_exc()
    print(f"test_perf_report.py: {len(tests) - len(failed)}/{len(tests)} passed")
    sys.exit(1 if failed else 0)
