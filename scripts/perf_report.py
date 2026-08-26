#!/usr/bin/env python3
"""Render the CI performance and architecture telemetry report from benchmark output.

Turns `cargo bench --bench instructions` output (callgrind counts, see
`docs/BENCHMARKING.md`) into a structured markdown report comparing this branch
against its merge base, plus deterministic memory density and cache simulation.

Why a comparison and not just numbers: callgrind counts are exact and
reproducible, so a delta between two commits is real signal at 0.1%
resolution — unlike wall-clock, where this project has already measured
a ~15-20% noise floor on shared CI runners. The report therefore leads
with deterministic instruction and memory deltas.

Usage:
    perf_report.py --head head.txt [--base base.txt] [--bytes bytes.txt]
                   [--bytes32 bytes32.txt] [--base-ref main]
                   [--vs-stock vs_stock.txt] [--v3 v3.txt]
                   [--head-to-head head_to_head.md] > report.md
"""

from __future__ import annotations

import argparse
import re
import sys
from typing import Any, Dict, List, Optional, Tuple

ANSI = re.compile(r"\x1b\[[0-9;]*m")
# Matches headers like:
# "instructions::cost::map_insert random:"random""
# "smoke_cost::map_insert sequential"
# "cost::set32_insert"
HEADER = re.compile(r"^([\w:]+)::(\w+)(?:\s+([^:\s]+):?.*)?$")
METRIC = re.compile(r"^\s{2,}([\w+ ]+):\s+([\d,]+)\|")

METRICS = ["Instructions", "Estimated Cycles", "L1 Hits", "LL Hits", "RAM Hits"]
NOISE_PCT = 0.1

# Default operations count (N) for each benchmark in deterministic instruction sweeps.
# POP = 50,000 in full benches, POP = 10,000 in smoke benches.
BENCH_N_MAP: Dict[str, int] = {
    # Point Queries & Lookups
    "map_get": 50_000,
    "set_contains": 50_000,
    "map32_get": 500,
    # Range Scans & Ordered Traversal (100 windows * 100 span = 10,000 keys)
    "map_range": 10_000,
    "set_range": 10_000,
    "map_iterate": 50_000,
    "map_nav": 50_000,
    "blobmap32_scan": 2_000,
    # Mutations & Churn
    "map_insert": 50_000,
    "set_insert": 50_000,
    "map_ins_slot": 50_000,
    "map_remove": 50_000,
    "map_churn": 50_000,
    "set32_insert": 10_000,
    # C ABI vs Stock Judy
    "judyl_insert": 50_000,
    "judyl_get": 50_000,
    "judyl_churn": 50_000,
    "judy1_set": 50_000,
    "judy1_test": 50_000,
}

SMOKE_BENCH_N_MAP: Dict[str, int] = {
    "map_get": 10_000,
    "set_contains": 10_000,
    "map_insert": 10_000,
    "set_insert": 10_000,
    "map_ins_slot": 10_000,
    "map_churn": 10_000,
}

# Subsystem categories for structured reporting
CATEGORIES: List[Tuple[str, str, set[str]]] = [
    (
        "point_queries",
        "#### 🔍 Point Queries & Lookups",
        {"map_get", "set_contains", "map32_get", "judyl_get", "judy1_test"},
    ),
    (
        "range_scans",
        "#### ⚡ Range Scans & Ordered Traversal",
        {"map_range", "set_range", "map_iterate", "map_nav", "blobmap32_scan"},
    ),
    (
        "mutations",
        "#### ✍️ Mutations & Churn",
        {
            "map_insert",
            "set_insert",
            "map_ins_slot",
            "map_remove",
            "map_churn",
            "set32_insert",
            "judyl_insert",
            "judy1_set",
            "judyl_churn",
        },
    ),
]


def parse(text: str) -> dict[str, dict[str, int]]:
    """{benchmark id -> {metric -> count}} from one bench run's output."""
    out: dict[str, dict[str, int]] = {}
    current: str | None = None
    for line in ANSI.sub("", text).splitlines():
        head = HEADER.match(line)
        if head:
            _group, bench, arg = head.groups()
            current = f"{bench}/{arg}" if arg else bench
            out[current] = {}
            continue
        metric = METRIC.match(line)
        if metric and current:
            name, value = metric.group(1).strip(), metric.group(2).replace(",", "")
            try:
                out[current][name] = int(value)
            except ValueError:
                pass
    return {k: v for k, v in out.items() if v}


def pct(head: int, base: int) -> float:
    return 0.0 if base == 0 else (head - base) / base * 100.0


def parse_allow_regression(pr_body: str) -> str | None:
    """Extracts a regression-override reason from a PR body.

    Only the strict `allow-regression: <nonempty reason>` form (optionally
    inside an HTML comment) is accepted — colon plus a reason on the same
    line. A bare `allow-regression` substring, `perf-override: approved`, or
    a quoted copy of the policy text (`allow-regression: <reason>`) does NOT
    approve anything: a PR quoting the docs must never silently pass the gate.
    """
    for match in re.finditer(
        r"(?:<!--\s*)?allow-regression:[ \t]*([^\n]+)", pr_body, re.IGNORECASE
    ):
        reason = match.group(1).strip()
        # Strip trailing HTML-comment close / stray backticks from inline quoting.
        reason = re.sub(r"(?:-->|`)+\s*$", "", reason).strip()
        if reason and not reason.lower().startswith("<reason>"):
            return reason
    return None


def head_parse_fatals(
    head: dict[str, dict[str, int]],
    base: dict[str, dict[str, int]] | None,
    min_base_fraction: float = 0.5,
) -> list[str]:
    """Fatal conditions for `--fail-on-regression` mode.

    A crashed benchmark run must never render '0 Regressions': an empty head
    parse, or a head that lost most of the base's arms (partial crash), is a
    hard failure rather than a quietly green report.
    """
    fatals: list[str] = []
    if not head:
        fatals.append(
            "head benchmark output parsed to zero benchmarks — the benchmark run "
            "crashed or produced no parseable output; refusing to report a green "
            "regression gate for a run that measured nothing"
        )
    elif base and len(head) < len(base) * min_base_fraction:
        fatals.append(
            f"head parsed only {len(head)} benchmark(s) vs {len(base)} in base "
            f"(<{min_base_fraction:.0%}) — partial benchmark crash suspected; "
            "the missing arms cannot be compared"
        )
    return fatals


def missing_arms(
    head: dict[str, dict[str, int]],
    base: dict[str, dict[str, int]] | None,
) -> list[str]:
    """Arms present in base but absent from head — reported explicitly, never
    silently dropped from the comparison."""
    if not base:
        return []
    return sorted(set(base) - set(head))


def verdict(delta: float) -> str:
    """Marker for one delta. Fewer instructions is better, always."""
    if abs(delta) < NOISE_PCT:
        return "="
    return "🟢" if delta < 0 else "🔴"


def fmt_delta(delta: float, is_bold: bool = True) -> str:
    if abs(delta) < NOISE_PCT:
        return "—"
    if is_bold:
        return f"**{delta:+.2f}%**"
    return f"{delta:+.2f}%"


def normalize_bench_name(bench_name: str) -> str:
    """Strips arms suffixes like _stock, _expanse, _expanse_dl and argument parts."""
    base_name = bench_name.split("/")[0]
    for suffix in ("_expanse_dl", "_expanse", "_stock"):
        if base_name.endswith(suffix):
            base_name = base_name[: -len(suffix)]
            break
    return base_name


def get_bench_n(bench_name: str, is_smoke: bool = False) -> int:
    """Returns the operations count N for a benchmark name (e.g. 'map_get/random' -> 50000)."""
    clean_name = normalize_bench_name(bench_name)
    base_name = bench_name.split("/")[0]
    if is_smoke and clean_name in SMOKE_BENCH_N_MAP:
        return SMOKE_BENCH_N_MAP[clean_name]
    if clean_name in BENCH_N_MAP:
        return BENCH_N_MAP[clean_name]
    if base_name in BENCH_N_MAP:
        return BENCH_N_MAP[base_name]
    if bench_name in BENCH_N_MAP:
        return BENCH_N_MAP[bench_name]
    # Attempt to extract explicit numeric key count from suffix (e.g. 'foo_10k')
    match = re.search(r"(\d+k|\d+m|\d+)", bench_name.lower())
    if match:
        val_str = match.group(1)
        if val_str.endswith("m"):
            return int(val_str[:-1]) * 1_000_000
        elif val_str.endswith("k"):
            return int(val_str[:-1]) * 1_000
        elif val_str.isdigit() and int(val_str) > 1:
            return int(val_str)
    return 1


def format_n(n: int) -> str:
    """Formats N for table display, e.g. 50000 -> '50k', 500 -> '500', 1000000 -> '1M'."""
    if n >= 1_000_000:
        if n % 1_000_000 == 0:
            return f"{n // 1_000_000}M"
        return f"{n / 1_000_000:.1f}M"
    if n >= 1_000:
        if n % 1_000 == 0:
            return f"{n // 1_000}k"
        return f"{n / 1_000:.1f}k"
    return str(n)


def format_ins_per_op(ins: int, n: int, bold: bool = False) -> str:
    """Computes and formats Instructions / Op (N)."""
    ins_per_op = ins / n if n > 0 else float(ins)
    n_str = format_n(n)
    if bold:
        return f"**{ins_per_op:,.1f}** ({n_str})"
    return f"{ins_per_op:,.1f} ({n_str})"


def categorize_benchmarks(
    benchmarks: list[str],
) -> list[tuple[str, str, list[str]]]:
    """Partitions benchmark names into categories in fixed display order, with an 'uncategorized' fallback."""
    categorized: list[tuple[str, str, list[str]]] = []
    seen = set()

    for cat_id, cat_title, prefix_set in CATEGORIES:
        matched = []
        for name in benchmarks:
            clean_name = normalize_bench_name(name)
            base_name = name.split("/")[0]
            if clean_name in prefix_set or base_name in prefix_set or name in prefix_set:
                matched.append(name)
                seen.add(name)
        if matched:
            categorized.append((cat_id, cat_title, matched))

    # Uncategorized fallback: ensures no benchmark is ever dropped from the report!
    uncategorized = [name for name in benchmarks if name not in seen]
    if uncategorized:
        categorized.append(
            ("uncategorized", "#### 📦 Other & General", uncategorized)
        )

    return categorized


def parse_bytes_64(text: str | None) -> tuple[bool, list[dict[str, Any]]]:
    """Parses bytes_per_key output table for 64-bit server architecture.
    
    Returns (all_compliant, rows).
    """
    if not text:
        return True, []

    all_compliant = "MEMORY BUDGET EXCEEDED" not in text
    rows_dict: dict[str, dict[str, Any]] = {}

    # Parse 1,000,000 population rows
    # Line pattern: dist pop set_bpk map_bpk
    # Ceilings mirror the enforced maxima in examples/bytes_per_key.rs (the CI
    # `memory-budget` gate); the displayed ceiling is derived from the values
    # this table actually enforces, never a separate hand-written figure.
    budgets = {
        "sequential": ("**Sequential**", 0.10, 9.00),
        "clustered": ("**Clustered**", 0.50, 9.00),
        "clustered-wide": ("**Clustered-Wide**", 0.30, 9.00),
        "random": ("**Random (Uniform)**", 9.00, 18.00),
        "sparse": ("**Sparse (High 24-bit)**", 17.00, 17.00),
    }

    for line in text.splitlines():
        parts = line.strip().split()
        if len(parts) >= 4 and parts[1] == "1000000":
            dist = parts[0].lower()
            if dist in budgets:
                try:
                    set_bpk = float(parts[2])
                    map_bpk = float(parts[3])
                    label, set_max, map_max = budgets[dist]
                    passed = set_bpk <= set_max and map_bpk <= map_max
                    if not passed:
                        all_compliant = False
                    rows_dict[dist] = {
                        "dist": label,
                        "pop": "1,000,000",
                        "set_bpk": f"**{set_bpk:.2f} B**",
                        "map_bpk": f"**{map_bpk:.2f} B**",
                        "ceiling": f"set ≤ {set_max:.2f} · map ≤ {map_max:.2f} B",
                        "status": "🟢 Pass" if passed else "🔴 Fail",
                    }
                except ValueError:
                    pass

    canonical_order = ["sequential", "clustered", "clustered-wide", "random", "sparse"]
    rows = [rows_dict[k] for k in canonical_order if k in rows_dict]
    # Append any remaining parsed rows not in canonical list
    for k, v in rows_dict.items():
        if k not in canonical_order:
            rows.append(v)

    return all_compliant, rows


def render_bytes_64_table(rows: list[dict[str, Any]], raw_fallback: str | None) -> list[str]:
    if not rows:
        if raw_fallback and raw_fallback.strip():
            return [
                "<details open>",
                "<summary><b>64-Bit Server Architecture (Bytes per Key)</b></summary>",
                "",
                "```",
                raw_fallback.strip(),
                "```",
                "",
                "</details>",
                "",
            ]
        return []

    lines = [
        "<details open>",
        "<summary><b>64-Bit Server Architecture (Bytes per Key)</b></summary>",
        "",
        "| Distribution | Population | ExpanseSet (B/key) | ExpanseMap (B/key) | Enforced Ceiling | Status |",
        "|:---|---:|---:|---:|---:|:---:|",
    ]
    for r in rows:
        lines.append(
            f"| {r['dist']} | {r['pop']} | {r['set_bpk']} | {r['map_bpk']} | {r['ceiling']} | {r['status']} |"
        )
    lines += [
        "",
        "*(Map bytes/key include the full 8-byte payload inline).* ",
        "</details>",
        "",
    ]
    return lines


def parse_bytes_32(text: str | None) -> tuple[bool, list[dict[str, Any]]]:
    """Parses bytes_per_key_32 output for 32-bit embedded architecture.
    
    Returns (all_compliant, rows).
    """
    if not text:
        return True, []

    rows: list[dict[str, Any]] = []
    all_compliant = "exceeded guard" not in text.lower()

    # Match lines like:
    # 1. Clustered sensor timestamps (N = 10000): 6736 bytes (0.6736 B/key)
    # 2. Sparse 29-bit CAN IDs (N = 500): 6304 bytes (12.6080 B/key)
    pattern = re.compile(
        r"^\d+\.\s+([^(]+)\s*\(N\s*=\s*(\d+)\):\s*(\d+)\s*bytes\s*\(([0-9.]+)\s*B/key\)"
    )

    # Guards mirror the enforced assertions in examples/bytes_per_key_32.rs;
    # the displayed ceiling is the value this table actually enforces.
    ceilings = {
        "clustered": ("**Clustered Sensor Timestamps**", 1.00),
        "sparse": ("**Sparse 29-bit CAN IDs**", 20.00),
        "ipv4": ("**IPv4 Subnet Routing Map**", 24.00),
        "dense": ("**Dense Consecutive Array**", 12.00),
    }

    for line in text.splitlines():
        m = pattern.match(line.strip())
        if m:
            raw_title, n_str, total_bytes_str, bpk_str = m.groups()
            title_lower = raw_title.lower()
            key = None
            if "sensor" in title_lower or "clustered" in title_lower:
                key = "clustered"
            elif "can" in title_lower or "sparse" in title_lower:
                key = "sparse"
            elif "ipv4" in title_lower or "routing" in title_lower:
                key = "ipv4"
            elif "dense" in title_lower:
                key = "dense"

            if key and key in ceilings:
                label, guard_val = ceilings[key]
                bpk = float(bpk_str)
                total_bytes = int(total_bytes_str)
                n = int(n_str)
                passed = bpk <= guard_val
                if not passed:
                    all_compliant = False
                rows.append({
                    "workload": label,
                    "pop": f"{n:,}",
                    "total_bytes": f"{total_bytes:,} B",
                    "bpk": f"**{bpk:.2f} B**",
                    "ceiling": f"≤ {guard_val:.2f} B",
                    "status": "🟢 Pass" if passed else "🔴 Fail",
                })

    return all_compliant, rows


def render_bytes_32_table(rows: list[dict[str, Any]], raw_fallback: str | None) -> list[str]:
    if not rows:
        if raw_fallback and raw_fallback.strip():
            return [
                "<details>",
                "<summary><b>32-Bit Embedded Architecture (RV32 / ESP32 / Cortex-M): bytes/key</b></summary>",
                "",
                "```",
                raw_fallback.strip(),
                "```",
                "",
                "</details>",
                "",
            ]
        return []

    lines = [
        "<details>",
        "<summary><b>32-Bit Embedded Architecture (RV32 / ESP32 / Cortex-M)</b></summary>",
        "",
        "| Embedded Workload | Population | Total Allocated | Bytes / Key | Enforced Ceiling | Status |",
        "|:---|---:|---:|---:|---:|:---:|",
    ]
    for r in rows:
        lines.append(
            f"| {r['workload']} | {r['pop']} | {r['total_bytes']} | {r['bpk']} | {r['ceiling']} | {r['status']} |"
        )
    lines += ["</details>", ""]
    return lines


def render_cache_simulation(
    head: dict[str, dict[str, int]],
    is_smoke: bool = False,
) -> list[str]:
    """Renders Section 4: Callgrind-Modeled Memory Hierarchy Simulation table."""
    lines = [
        "### 4. 🔬 Callgrind-Modeled Memory Hierarchy Simulation",
        "",
        "<details>",
        "<summary><b>Modeled Cache Accesses & Miss Ratios (Simulation)</b></summary>",
        "",
        "| Benchmark | Total Instructions | Est. Cycles | Modeled L1 Hits | Modeled LL Hits | Modeled RAM Misses | Simulated Hit Ratio |",
        "|:---|---:|---:|---:|---:|---:|---:|",
    ]

    for name, metrics in head.items():
        ins = metrics.get("Instructions")
        cyc = metrics.get("Estimated Cycles")
        l1 = metrics.get("L1 Hits", 0)
        ll = metrics.get("LL Hits", 0)
        ram = metrics.get("RAM Hits", 0)
        if ins is None or cyc is None:
            continue

        total_accesses = l1 + ll + ram
        if total_accesses > 0:
            hit_ratio = (l1 + ll) / total_accesses * 100.0
            hit_ratio_str = f"{hit_ratio:.2f}%"
        else:
            hit_ratio_str = "—"

        lines.append(
            f"| `{name}` | {ins:,} | {cyc:,} | {l1:,} | {ll:,} | {ram:,} | {hit_ratio_str} |"
        )

    lines += ["", "</details>", ""]
    return lines


def render_v3(
    v3: dict[str, dict[str, int]],
    head: dict[str, dict[str, int]],
) -> list[str]:
    lines: list[str] = [
        "<details>",
        "<summary><b>Modern Architecture (<code>x86-64-v3</code>: AVX2 / BMI2 / POPCNT) vs Baseline (<code>x86-64-v1</code>)</b></summary>",
        "",
        "**What this compares:** this branch compiled with `-C target-cpu=x86-64-v3` against the baseline binary. "
        "Demonstrates the hardware acceleration enabled when running on modern CPUs or via `glibc-hwcaps`.",
        "",
        "| | Benchmark | Baseline (v1) | x86-64-v3 | Delta (v3 vs v1) | Est. Cycles Delta |",
        "|---|---|---:|---:|---:|---:|",
    ]
    for name, v3_metrics in v3.items():
        h = head.get(name)
        if not h:
            continue
        v1_ins = h.get("Instructions", 0)
        v3_ins = v3_metrics.get("Instructions", 0)
        v1_cyc = h.get("Estimated Cycles", 0)
        v3_cyc = v3_metrics.get("Estimated Cycles", 0)
        d_ins = pct(v3_ins, v1_ins)
        d_cyc = pct(v3_cyc, v1_cyc)
        lines.append(
            f"| {verdict(d_ins)} | `{name}` | {v1_ins:,} | {v3_ins:,} | {fmt_delta(d_ins)} | {fmt_delta(d_cyc)} |"
        )
    lines += ["", "</details>", ""]
    return lines


def render_vs_stock(counts: dict[str, dict[str, int]]) -> list[str]:
    """The headline comparison: our C ABI against stock libjudy's, in
    instructions retired, paired by benchmark name."""
    sides = (("_expanse_dl", "dl"), ("_expanse", "ours"), ("_stock", "stock"))
    pairs: dict[str, dict[str, dict[str, int]]] = {}
    for name, metrics in counts.items():
        bench, _, arg = name.partition("/")
        for suffix, side in sides:
            if bench.endswith(suffix):
                key = f"{bench[: -len(suffix)]}/{arg}" if arg else bench[: -len(suffix)]
                pairs.setdefault(key, {})[side] = metrics
                break
    pairs = {k: v for k, v in pairs.items() if "stock" in v and ("dl" in v or "ours" in v)}
    if not pairs:
        return []

    have_dl = any("dl" in v for v in pairs.values())
    lines = [
        "### vs stock libjudy (deterministic C ABI instructions)",
        "",
        "The comparison that decides whether libexpanse is a viable drop-in: "
        "**identical C ABI calls, identical key streams**, instructions retired. "
        "Deterministic, so this is reviewable per PR — unlike wall-clock "
        "ratios on shared runners.",
        "",
    ]
    if have_dl:
        lines += [
            "**Read the `.so` column.** `libexpanse.so` and stock are both reached "
            "by `dlopen` + resolved symbols, so they are the same shape; the `rlib` "
            "column is our code linked directly into the harness with LTO. "
            "Ratio = ours ÷ stock. **Below 1.00 means libexpanse does less work.**",
            "",
            "| | Operation | libexpanse `.so` | stock libjudy | **ratio (.so)** "
            "| ratio (rlib) | est. cycles (.so) |",
            "|---|---|---:|---:|---:|---:|---:|",
        ]
    else:
        lines += [
            "Ratio = ours ÷ stock. **Below 1.00 means libexpanse does less work.**",
            "",
            "| | Operation | libexpanse | stock libjudy | ratio | est. cycles ratio |",
            "|---|---|---:|---:|---:|---:|",
        ]
    for name in sorted(pairs):
        sides_here = pairs[name]
        stock = sides_here["stock"]
        s_ins = stock.get("Instructions", 0)
        s_cyc = stock.get("Estimated Cycles", 0)
        if not s_ins:
            continue
        primary = sides_here.get("dl") or sides_here["ours"]
        o_ins = primary.get("Instructions", 0)
        o_cyc = primary.get("Estimated Cycles", 0)
        ratio = o_ins / s_ins
        cyc_ratio = (o_cyc / s_cyc) if s_cyc else 0.0
        mark = "🟢" if ratio < 1.0 else ("🟡" if ratio <= 1.25 else "🔴")
        if have_dl:
            rlib = sides_here.get("ours")
            r_ins = rlib.get("Instructions", 0) if rlib else 0
            rlib_cell = f"{r_ins / s_ins:.2f}x" if r_ins else "—"
            lines.append(
                f"| {mark} | `{name}` | {o_ins:,} | {s_ins:,} | **{ratio:.2f}x** "
                f"| {rlib_cell} | {cyc_ratio:.2f}x |"
            )
        else:
            lines.append(
                f"| {mark} | `{name}` | {o_ins:,} | {s_ins:,} | **{ratio:.2f}x** "
                f"| {cyc_ratio:.2f}x |"
            )
    lines.append("")
    return lines


def check_regressions(
    head: dict[str, dict[str, int]],
    base: dict[str, dict[str, int]] | None,
    max_regression_pct: float = 1.5,
    max_regressed_count: int = 1,
    noise_floor: float = 0.5,
    allowed: bool = False,
    allow_reason: str | None = None,
) -> tuple[bool, list[str]]:
    """Evaluates if head introduces unacceptable instruction regressions vs base.

    Returns (has_unacceptable_regression, list_of_error_messages).
    """
    if not base or not head:
        return False, []

    regressions: list[tuple[str, float, int, int]] = []
    for name, metrics in head.items():
        b = base.get(name)
        if not b:
            continue
        ins = metrics.get("Instructions", 0)
        b_ins = b.get("Instructions", 0)
        d_ins = pct(ins, b_ins)
        if d_ins > noise_floor:
            regressions.append((name, d_ins, ins, b_ins))

    if not regressions:
        return False, []

    worst = max(r[1] for r in regressions)
    is_violating = worst > max_regression_pct or len(regressions) > max_regressed_count

    if not is_violating:
        return False, []

    messages = []
    if allowed:
        messages.append(f"> [!NOTE]\n> **Performance regression override acknowledged**: {allow_reason or 'Approved'}\n>")
        for name, d_ins, ins, b_ins in sorted(regressions, key=lambda x: -x[1]):
            messages.append(f"> - `{name}`: {fmt_delta(d_ins, is_bold=False)} ({ins:,} vs {b_ins:,})")
        return False, messages

    messages.append(
        f"> [!CAUTION]\n"
        f"> **Performance regression detected**: {len(regressions)} benchmark(s) regressed > {noise_floor}% "
        f"(worst: {worst:+.2f}%, threshold: {max_regression_pct}%).\n"
        f"> To approve an intentional regression, add `allow-regression: <reason>` to the PR body.\n>"
    )
    for name, d_ins, ins, b_ins in sorted(regressions, key=lambda x: -x[1]):
        messages.append(f"> - `{name}`: {fmt_delta(d_ins, is_bold=False)} ({ins:,} vs {b_ins:,})")

    return True, messages


def render(
    head: dict[str, dict[str, int]],
    base: dict[str, dict[str, int]] | None,
    bytes_table: str | None,
    base_ref: str,
    vs_stock: dict[str, dict[str, int]] | None = None,
    v3: dict[str, dict[str, int]] | None = None,
    bytes32_table: str | None = None,
    head_to_head: str | None = None,
    bindings_status: str | None = None,
    is_smoke: bool = False,
    has_violation: bool = False,
    is_allowed_override: bool = False,
    base_requested: bool = False,
    gate_armed: bool = False,
    head_fatals: list[str] | None = None,
) -> str:
    # 1. Parse memory density tables and verify compliance
    compliant_64, rows_64 = parse_bytes_64(bytes_table)
    compliant_32, rows_32 = parse_bytes_32(bytes32_table)

    # 2. Evaluate overall instruction delta stats
    regressed = improved = unchanged = 0
    worst = float("-inf")
    best = float("inf")
    best_bench = ""

    all_bench_names = list(head.keys())

    if base:
        for name, metrics in head.items():
            ins = metrics.get("Instructions")
            if ins is None:
                continue
            b = base.get(name)
            if not b or "Instructions" not in b:
                continue
            b_ins = b["Instructions"]
            d_ins = pct(ins, b_ins)
            if abs(d_ins) < NOISE_PCT:
                unchanged += 1
            elif d_ins < 0:
                improved += 1
                if d_ins < best:
                    best = d_ins
                    best_bench = name
            else:
                regressed += 1
                if d_ins > worst:
                    worst = d_ins

    # 3. Build Executive Header Chips
    if not base:
        if base_requested or gate_armed:
            reg_chip = "⚠️ **NO BASELINE — gate did not run**"
        else:
            reg_chip = "⚪ **No Baseline**"
        opt_chip = "—"
    elif has_violation:
        reg_chip = f"🔴 **{regressed} Regressions**"
        opt_chip = f"🚀 **{best:+.2f}% ins** (`{best_bench}`)" if improved else "—"
    elif is_allowed_override and regressed > 0:
        reg_chip = f"🟡 **{regressed} Regressed (Approved)**"
        opt_chip = f"🚀 **{best:+.2f}% ins** (`{best_bench}`)" if improved else "—"
    elif regressed > 0:
        reg_chip = f"🟡 **{regressed} Regressed (< threshold)**"
        opt_chip = f"🚀 **{best:+.2f}% ins** (`{best_bench}`)" if improved else "—"
    else:
        reg_chip = "🟢 **0 Regressions**"
        opt_chip = f"🚀 **{best:+.2f}% ins** (`{best_bench}`)" if improved else "—"

    # An empty parse is a parse failure, never silent compliance: a supplied
    # bytes table that yields zero rows must not render "✅ 100% Compliant".
    if not bytes_table:
        chip_64 = "—"
    elif not rows_64:
        chip_64 = "⚠️ **Unparsed (no rows)**"
    elif compliant_64:
        chip_64 = "✅ **100% Compliant**"
    else:
        chip_64 = "🔴 **Exceeded Ceiling**"

    if not bytes32_table:
        chip_32 = "—"
    elif not rows_32:
        chip_32 = "⚠️ **Unparsed (no rows)**"
    elif compliant_32:
        chip_32 = "✅ **100% Compliant**"
    else:
        chip_32 = "🔴 **Exceeded Ceiling**"

    chip_bindings = bindings_status or "⚪ **Not measured (see nightly)**"

    lines: list[str] = [
        "## 📊 Expanse CI Performance & Architecture Telemetry",
        "",
        "| Regression Gate | Top Optimization | 64-Bit Memory Density | 32-Bit Embedded Density | Bindings Invariants |",
        "|:---:|:---:|:---:|:---:|:---:|",
        f"| {reg_chip} | {opt_chip} | {chip_64} | {chip_32} | {chip_bindings} |",
        "",
        f"> **Context & Baseline**: Compares this branch against **merge base `{base_ref}` (expanse's own previous code)**. "
        "Instructions measure *computational work/cost*, not wall-clock time. Fewer instructions is strictly better.",
        "",
    ]

    # Loud, honest degradation: a baseline that failed to materialize means the
    # regression gate did not run — say so prominently, not via a quiet chip.
    if not base and (base_requested or gate_armed):
        if base_requested:
            detail = (
                "A base benchmark file was supplied but parsed to **zero benchmarks** "
                "(the base pass failed to build, crashed, or produced no output). "
            )
        else:
            detail = "No base benchmark output was supplied to `--base`. "
        lines += [
            "> [!WARNING]",
            "> ### ⚠️ NO BASELINE — regression gate did not run",
            f"> {detail}Head numbers below are reported **without comparison**; "
            "this change has NOT been checked for instruction regressions.",
            "",
        ]

    for fatal in head_fatals or []:
        lines += [
            "> [!CAUTION]",
            f"> **Benchmark output integrity failure**: {fatal}",
            "",
        ]

    lines += [
        "---",
        "",
    ]

    # Section 1: Deterministic Instructions vs Merge Base
    lines.append(f"### 1. Deterministic Instructions vs Merge Base (`{base_ref}`)")
    lines.append("")

    categories = categorize_benchmarks(all_bench_names)

    if base:
        for cat_id, cat_title, bench_list in categories:
            lines.append(cat_title)
            lines.append(
                "| Status | Benchmark | Instructions | vs `main` | Ins / Op ($N$) | Est. Cycles | vs `main` |"
            )
            lines.append(
                "|:---:|:---|---:|---:|---:|---:|---:|"
            )
            for name in bench_list:
                metrics = head[name]
                ins = metrics.get("Instructions", 0)
                cyc = metrics.get("Estimated Cycles", 0)
                n = get_bench_n(name, is_smoke=is_smoke)
                b = base.get(name)

                if not b or "Instructions" not in b:
                    ins_op_str = format_ins_per_op(ins, n, bold=False)
                    lines.append(f"| 🆕 | `{name}` | {ins:,} | — | {ins_op_str} | {cyc:,} | — |")
                    continue

                b_ins = b.get("Instructions", 0)
                b_cyc = b.get("Estimated Cycles", cyc)
                d_ins = pct(ins, b_ins)
                d_cyc = pct(cyc, b_cyc)

                is_improved = d_ins < -NOISE_PCT
                is_regressed = d_ins > NOISE_PCT
                status_icon = verdict(d_ins)

                ins_op_str = format_ins_per_op(ins, n, bold=is_improved or is_regressed)
                lines.append(
                    f"| {status_icon} | `{name}` | {ins:,} | {fmt_delta(d_ins)} | {ins_op_str} | {cyc:,} | {fmt_delta(d_cyc)} |"
                )
            lines.append("")

        # Arms measured on base but absent from head are reported, not
        # silently dropped from the comparison.
        dropped = missing_arms(head, base)
        if dropped:
            arm_list = ", ".join(f"`{name}`" for name in dropped)
            lines += [
                "> [!WARNING]",
                f"> **{len(dropped)} arm(s) present in base `{base_ref}` but missing from head** — "
                f"not compared (benchmark removed, renamed, or crashed): {arm_list}",
                "",
            ]
    else:
        for cat_id, cat_title, bench_list in categories:
            lines.append(cat_title)
            lines.append("| Benchmark | Instructions | Ins / Op ($N$) | Est. Cycles |")
            lines.append("|:---|---:|---:|---:|")
            for name in bench_list:
                metrics = head[name]
                ins = metrics.get("Instructions", 0)
                cyc = metrics.get("Estimated Cycles", 0)
                n = get_bench_n(name, is_smoke=is_smoke)
                ins_op_str = format_ins_per_op(ins, n, bold=False)
                lines.append(f"| `{name}` | {ins:,} | {ins_op_str} | {cyc:,} |")
            lines.append("")

    lines.append("---")
    lines.append("")

    # Section 2: Comparative Wall-Clock Throughput vs Industry Standards
    if head_to_head and head_to_head.strip():
        lines.append("### 2. ⚔️ Comparative Wall-Clock Throughput vs Industry Standards")
        lines.append("")
        lines.append(
            "> **Wall-Clock Notice**: Indicative throughput measured on CI host via `scripts/bench_report.py` (job: `instructions / comparative`). "
            "Shared CI runners have ~15-20% thermal noise floor; no CI gating is performed on wall-clock metrics. "
            "Formal quiet-host measurements come from the dedicated-host `/bench` bare-metal runs (`docs/BENCHMARKING.md`)."
        )
        lines.append("")
        lines.append(head_to_head.strip())
        lines.append("")
        lines.append("---")
        lines.append("")

    # vs stock libjudy table (deterministic C ABI instructions)
    if vs_stock:
        stock_lines = render_vs_stock(vs_stock)
        if stock_lines:
            lines.extend(stock_lines)
            lines.append("---")
            lines.append("")

    # Modern architecture (x86-64-v3) table
    if v3:
        v3_lines = render_v3(v3, head)
        if v3_lines:
            lines.extend(v3_lines)

    # Section 3: Memory Density Ledgers
    if bytes_table or bytes32_table:
        lines.append("### 3. 💾 Memory Density Ledgers (Allocator Accounting)")
        lines.append("")
        if bytes_table:
            lines.extend(render_bytes_64_table(rows_64, bytes_table))
        if bytes32_table:
            lines.extend(render_bytes_32_table(rows_32, bytes32_table))
        lines.append("---")
        lines.append("")

    # Section 4: Callgrind-Modeled Memory Hierarchy Simulation
    lines.extend(render_cache_simulation(head, is_smoke=is_smoke))
    lines.append("---")
    lines.append("")

    # Section 5: Cross-Language Bindings & FFI Invariants
    lines.extend([
        "### 5. 🌐 Cross-Language Bindings & FFI Invariants",
        "",
        "- **Native / FFI Engine Core**: 0 heap allocations during point lookup & ordered scan (zero-allocation engine contract). All 9 language bindings funnel through the same `libexpanse.so` (`cdylib`), covered deterministically by the C ABI instruction gate above.",
        "- **Runtime Allocation Budgets**: Go reports 0 B/op & 0 allocs/op natively in `go test -bench`; Python/Node/.NET/Java allocation budgets constrained to runtime-level primitive boxing only (no intermediate buffer or array allocations).",
        "- **FFI & Marshalling Overhead**: Tracked against historical baseline in nightly verification runs (`scripts/bench_bindings.py --check-baseline`).",
        "",
        "---",
        "<sub>🟢 less work · 🔴 more work · = within noise ($< 0.1\\%$) · 🆕 new benchmark. "
        "Instructions reflect exact deterministic computational work vs previous code. Full cross-language benchmarks: <code>docs/BINDINGS_BENCHMARKS.md</code>.</sub>\n",
    ])

    return "\n".join(lines)


def read(path: str | None) -> str | None:
    if not path:
        return None
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError as exc:
        print(f"warning: {exc}", file=sys.stderr)
        return None


SELF_TEST_HEAD = """\
instructions::cost::map_get random:"random"
  Instructions:              1000|1000            (No change)
  Estimated Cycles:          2000|2000            (No change)
instructions::cost::map_insert
  Instructions:              3000|3000            (No change)
  Estimated Cycles:          6000|6000            (No change)
"""


def self_test() -> int:
    """Unit-style checks for the parsing and gating helpers. No cargo required."""
    # 1. Parser basics.
    head = parse(SELF_TEST_HEAD)
    assert set(head) == {"map_get/random", "map_insert"}, head
    assert head["map_get/random"]["Instructions"] == 1000

    # 2. Empty / near-empty head parse is fatal for the armed gate.
    assert head_parse_fatals({}, None), "empty head parse must be fatal"
    assert head_parse_fatals({}, head), "empty head parse must be fatal with base too"
    assert not head_parse_fatals(head, head), "identical head/base must not be fatal"
    base_big = {f"bench_{i}": {"Instructions": 100} for i in range(6)}
    partial_head = {"bench_0": {"Instructions": 100}}
    assert head_parse_fatals(partial_head, base_big), "losing most base arms must be fatal"

    # 3. Base-present/head-missing arms are reported explicitly.
    base = dict(head)
    base["map_remove"] = {"Instructions": 500, "Estimated Cycles": 900}
    assert missing_arms(head, base) == ["map_remove"]
    out = render(head=head, base=base, bytes_table=None, base_ref="origin/main")
    assert "missing from head" in out and "`map_remove`" in out, "missing arms must be rendered"

    # 4. allow-regression: strict form only.
    assert parse_allow_regression("allow-regression: intentional SIMD trade-off") == "intentional SIMD trade-off"
    assert parse_allow_regression("<!-- allow-regression: hardening cost -->") == "hardening cost"
    assert parse_allow_regression("we may need allow-regression at some point") is None
    assert parse_allow_regression("add `allow-regression: <reason>` to the PR body.") is None
    assert parse_allow_regression("allow-regression:") is None
    assert parse_allow_regression("perf-override: approved") is None
    # A quoted policy line must not shadow a real approval further down.
    assert parse_allow_regression(
        "docs say `allow-regression: <reason>`\n\nallow-regression: OCC hardening cost"
    ) == "OCC hardening cost"

    # 5. Baseline degradation is loud, not a quiet chip.
    out = render(head=head, base=None, bytes_table=None, base_ref="origin/main", base_requested=True)
    assert "NO BASELINE — regression gate did not run" in out
    out = render(head=head, base=None, bytes_table=None, base_ref="origin/main", gate_armed=True)
    assert "NO BASELINE — regression gate did not run" in out
    out = render(head=head, base=None, bytes_table=None, base_ref="origin/main")
    assert "NO BASELINE" not in out and "⚪ **No Baseline**" in out

    # 6. Empty bytes-per-key parse renders a warning chip, never compliance.
    out = render(head=head, base=head, bytes_table="garbage with no rows", base_ref="origin/main")
    assert "⚠️ **Unparsed (no rows)**" in out
    assert "100% Compliant" not in out

    # 7. The memory table prints the ceilings it actually enforces.
    compliant, rows = parse_bytes_64("random 1000000 8.50 17.50\n")
    assert compliant and len(rows) == 1
    assert rows[0]["ceiling"] == "set ≤ 9.00 · map ≤ 18.00 B", rows[0]["ceiling"]
    compliant, rows = parse_bytes_64("random 1000000 9.50 17.50\n")
    assert not compliant, "8.50->9.50 must exceed the enforced set ceiling of 9.00"

    # 8. Regression detection still fires.
    regressed_head = {"map_get/random": {"Instructions": 1200}, "map_insert": {"Instructions": 3000}}
    has_violation, msgs = check_regressions(regressed_head, head, max_regression_pct=5.0)
    assert has_violation and msgs, "a +20% single-arm regression must violate the 5% gate"

    print("perf_report.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="Render Expanse CI Performance & Architecture Telemetry Report")
    ap.add_argument("--head", help="bench output for this branch (required unless --self-test)")
    ap.add_argument("--base", help="bench output for the merge base")
    ap.add_argument("--bytes", help="bytes_per_key output")
    ap.add_argument("--bytes32", help="bytes_per_key_32 output")
    ap.add_argument("--base-ref", default="main")
    ap.add_argument("--vs-stock", help="vs_stock bench output")
    ap.add_argument("--v3", help="bench output for x86-64-v3")
    ap.add_argument("--head-to-head", help="head-to-head comparison markdown report (bench_report.py)")
    ap.add_argument("--bindings-status", help="summary status chip for bindings invariants")
    ap.add_argument("--smoke", action="store_true", help="indicate smoke benchmark run (POP = 10,000)")
    ap.add_argument("--fail-on-regression", action="store_true", help="fail if regressions exceed threshold")
    ap.add_argument("--max-regression-pct", type=float, default=1.5, help="maximum single-benchmark regression pct allowed")
    ap.add_argument("--allow-regression", action="store_true", help="override/approve intentional regressions")
    ap.add_argument("--allow-regression-reason", help="reason for approving regression")
    ap.add_argument("--pr-body-file", help="path to PR body markdown to check for override markers")
    ap.add_argument("--self-test", action="store_true", help="run unit-style checks on the parsing/gating helpers and exit")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if not args.head:
        ap.error("--head is required unless --self-test is given")

    head_text = read(args.head)
    if head_text is None:
        print("## 📊 Expanse CI Performance & Architecture Telemetry\n\nBenchmarks did not run.\n")
        if args.fail_on_regression:
            print(
                "::error::head benchmark output is missing/unreadable — the "
                "benchmark run produced nothing to gate on; failing instead of "
                "reporting a green regression gate",
                file=sys.stderr,
            )
            return 1
        return 0
    base_text = read(args.base)
    stock_text = read(args.vs_stock)
    v3_text = read(args.v3)

    head_parsed = parse(head_text)
    base_parsed = parse(base_text) if base_text else None
    base_requested = bool(args.base)

    fatals = head_parse_fatals(head_parsed, base_parsed)

    # Check for PR body override markers (strict `allow-regression: <reason>`
    # form only; a bare substring or quoted policy text approves nothing).
    allowed = args.allow_regression
    allow_reason = args.allow_regression_reason
    if args.pr_body_file:
        pr_body = read(args.pr_body_file) or ""
        reason = parse_allow_regression(pr_body)
        if reason:
            allowed = True
            allow_reason = reason

    has_violation, reg_messages = check_regressions(
        head_parsed,
        base_parsed,
        max_regression_pct=args.max_regression_pct,
        allowed=allowed,
        allow_reason=allow_reason,
    )

    is_smoke = args.smoke or "smoke" in args.head.lower()
    is_allowed_override = allowed and bool(reg_messages) and not has_violation

    rendered = render(
        head=head_parsed,
        base=base_parsed,
        bytes_table=read(args.bytes),
        base_ref=args.base_ref,
        vs_stock=parse(stock_text) if stock_text else None,
        v3=parse(v3_text) if v3_text else None,
        bytes32_table=read(args.bytes32),
        head_to_head=read(args.head_to_head),
        bindings_status=args.bindings_status,
        is_smoke=is_smoke,
        has_violation=has_violation,
        is_allowed_override=is_allowed_override,
        base_requested=base_requested,
        gate_armed=args.fail_on_regression,
        head_fatals=fatals,
    )

    if reg_messages:
        rendered += "\n### 🛡️ Regression Guard\n\n" + "\n".join(reg_messages) + "\n"

    print(rendered, end="")

    # A crashed/empty head parse is a hard failure when the gate is armed: a
    # run that measured nothing must never render a green regression gate.
    if args.fail_on_regression and fatals:
        for fatal in fatals:
            print(f"::error::{fatal}", file=sys.stderr)
        return 1

    if args.fail_on_regression and has_violation:
        for msg in reg_messages:
            # Strip markdown blockquote prefix for stderr log annotations
            clean_msg = re.sub(r"^>\s*\[!(?:CAUTION|WARNING|NOTE)\]\n?>\s*", "", msg)
            clean_msg = re.sub(r"^>\s*[-*]?\s*", "  - ", clean_msg, flags=re.MULTILINE)
            clean_msg = clean_msg.replace("**", "").replace("`", "").strip()
            print(f"::error::{clean_msg}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
