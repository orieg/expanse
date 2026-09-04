#!/usr/bin/env python3
"""integrations/rocksdb/scripts/generate_bench_svg.py

Regenerates ``docs/benchmarks/rocksdb_memtable/results/bench_rocksdb.svg`` (the three-panel MemTable
benchmark chart embedded by ``docs/benchmarks/rocksdb_memtable/README.md``,
``integrations/rocksdb/README.md`` and ``docs/DATABASE.md``) from the measured numbers in
``integrations/rocksdb/benches/results.json``.

Previously the SVG was hand-authored, so re-running the benchmark left it
stale (issue #301). Update ``results.json`` from a fresh
``make -C integrations/rocksdb bench`` run, then:

    python3 integrations/rocksdb/scripts/generate_bench_svg.py

The chart geometry and dual-theme styling mirror the original asset; XML is
validated before writing (same discipline as
``docs/benchmarks/hashbrown_comparison/scripts/generate_charts.py``).
"""

from __future__ import annotations

import json
import math
from typing import Iterable
import xml.etree.ElementTree as ET
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
RESULTS = REPO_ROOT / "integrations" / "rocksdb" / "benches" / "results.json"
OUTPUT = REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable" / "results" / "bench_rocksdb.svg"

# Plot geometry shared by all three panels: the y axis spans axis_max at
# y=45 down to 0 at the baseline y=195 (150 px tall); bars are 50 px wide
# at x = 45 / 115 / 185 within each panel's translate() group.
BASELINE_Y = 195.0
AXIS_TOP_Y = 45.0
PLOT_H = BASELINE_Y - AXIS_TOP_Y
BAR_X = [45, 115, 185]
BAR_W = 50

STYLE = """
      /* Default styles: Light theme by default for maximum contrast on web */
      .bg { fill: #ffffff; }
      .border { stroke: #e2e8f0; stroke-width: 1px; fill: none; }
      .grid { stroke: #f1f5f9; stroke-width: 1px; stroke-dasharray: 2,3; }
      .axis { stroke: #cbd5e1; stroke-width: 1.5px; }
      .divider { stroke: #e2e8f0; stroke-width: 1px; }

      .t-chart-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11.5px; font-weight: 700; letter-spacing: 0.6px; fill: #0f172a; text-transform: uppercase; }
      .t-chart-sub { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10px; font-weight: 500; fill: #334155; }
      .t-unit-header { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 600; fill: #475569; }
      .t-axis-label { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 500; fill: #475569; }
      .t-win { fill: #16a34a; font-weight: 600; }
      .t-bar-label { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11px; font-weight: 700; fill: #0f172a; }

      .t-val-accent { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 700; fill: #16a34a; text-anchor: middle; }
      .t-val-muted { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 600; fill: #334155; text-anchor: middle; }
      .t-val-blue { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 600; fill: #2563eb; text-anchor: middle; }

      .b-expanse { fill: #16a34a; }
      .b-rocksdb { fill: #2563eb; }
      .b-skipmap { fill: #64748b; }

      /* Dark theme overrides */
      @media (prefers-color-scheme: dark) {
        .bg { fill: #0d1117; }
        .border { stroke: #30363d; }
        .grid { stroke: #21262d; }
        .axis { stroke: #484f58; }
        .divider { stroke: #21262d; }
        .t-chart-title { fill: #f0f6fc; }
        .t-chart-sub { fill: #94a3b8; }
        .t-unit-header { fill: #94a3b8; }
        .t-axis-label { fill: #94a3b8; }
        .t-win { fill: #4ade80; font-weight: 600; }
        .t-bar-label { fill: #f8fafc; }
        .t-val-accent { fill: #4ade80; }
        .t-val-blue { fill: #38bdf8; }
        .t-val-muted { fill: #e2e8f0; }
        .b-expanse { fill: #22c55e; }
        .b-rocksdb { fill: #3b82f6; }
        .b-skipmap { fill: #475569; }
      }

      [data-theme="dark"] .bg, :root[data-theme="dark"] .bg { fill: #0d1117; }
      [data-theme="dark"] .border, :root[data-theme="dark"] .border { stroke: #30363d; }
      [data-theme="dark"] .grid, :root[data-theme="dark"] .grid { stroke: #21262d; }
      [data-theme="dark"] .axis, :root[data-theme="dark"] .axis { stroke: #484f58; }
      [data-theme="dark"] .divider, :root[data-theme="dark"] .divider { stroke: #21262d; }
      [data-theme="dark"] .t-chart-title, :root[data-theme="dark"] .t-chart-title { fill: #f0f6fc; }
      [data-theme="dark"] .t-chart-sub, :root[data-theme="dark"] .t-chart-sub { fill: #94a3b8; }
      [data-theme="dark"] .t-unit-header, :root[data-theme="dark"] .t-unit-header { fill: #94a3b8; }
      [data-theme="dark"] .t-axis-label, :root[data-theme="dark"] .t-axis-label { fill: #94a3b8; }
      [data-theme="dark"] .t-win, :root[data-theme="dark"] .t-win { fill: #4ade80; }
      [data-theme="dark"] .t-bar-label, :root[data-theme="dark"] .t-bar-label { fill: #f8fafc; }
      [data-theme="dark"] .t-val-accent, :root[data-theme="dark"] .t-val-accent { fill: #4ade80; }
      [data-theme="dark"] .t-val-blue, :root[data-theme="dark"] .t-val-blue { fill: #38bdf8; }
      [data-theme="dark"] .t-val-muted, :root[data-theme="dark"] .t-val-muted { fill: #e2e8f0; }
      [data-theme="dark"] .b-expanse, :root[data-theme="dark"] .b-expanse { fill: #22c55e; }
      [data-theme="dark"] .b-rocksdb, :root[data-theme="dark"] .b-rocksdb { fill: #3b82f6; }
      [data-theme="dark"] .b-skipmap, :root[data-theme="dark"] .b-skipmap { fill: #475569; }
"""


def nice_axis_max(values: "Iterable[float]") -> float:
    """Smallest round ceiling that leaves the tallest bar ~80-95% of the panel.

    Steps through a 1/1.2/1.5/2/2.5/3/4/5/6/7/8/10 ladder times a power of ten,
    so the axis lands on a label a reader can divide in their head and the
    mid-tick is always exactly half of it. The ladder is deliberately finer
    than 1/2/5: on this data a coarse one jumped 614 M/s to a 1000 axis, which
    wastes as much of the panel as the stamped ceiling it replaced.
    """
    peak = max(values)
    if peak <= 0:
        raise ValueError("axis needs at least one positive value")
    target = peak / 0.9
    exp = math.floor(math.log10(target))
    for mult in (1.0, 1.2, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0):
        candidate = mult * (10.0**exp)
        if candidate >= target:
            return candidate
    return 10.0 ** (exp + 1)


def axis_label(value: float, unit: str) -> str:
    """Renders an axis tick the way each panel's unit is written."""
    if unit == "M":
        return f"{value:g}M"
    if unit == "B":
        return f"{value:g} B"
    return f"{value:g} ns"


def bar(x: int, value: float, axis_max: float, css: str, val_text: str, val_css: str,
        label: str, caption: str, caption_win: bool) -> str:
    """One bar + value label + name label + caption, in panel-local coords."""
    height = round(value / axis_max * PLOT_H, 1)
    y = round(BASELINE_Y - height, 1)
    cx = x + BAR_W // 2
    caption_cls = "t-chart-sub t-win" if caption_win else "t-chart-sub"
    return (
        f'    <rect x="{x}" y="{y}" width="{BAR_W}" height="{height}" class="{css}" rx="2"/>\n'
        f'    <text x="{cx}" y="{y - 8.5:.1f}" class="{val_css}">{val_text}</text>\n'
        f'    <text x="{cx}" y="214" class="t-bar-label" text-anchor="middle">{label}</text>\n'
        f'    <text x="{cx}" y="228" class="{caption_cls}" text-anchor="middle">{caption}</text>\n'
    )


def panel(tx: int, title: str, sub: str, unit: str, axis_max: float,
          axis_top_label: str, axis_mid_label: str, bars: str) -> str:
    return (
        f'  <g transform="translate({tx}, 20)">\n'
        f'    <text x="0" y="0" class="t-chart-title">{title}</text>\n'
        f'    <text x="0" y="13" class="t-chart-sub">{sub}</text>\n'
        f'    <text x="0" y="29" class="t-unit-header">{unit}</text>\n'
        f'    <line x1="30" y1="45" x2="250" y2="45" class="grid"/>\n'
        f'    <text x="22" y="48" class="t-axis-label" text-anchor="end">{axis_top_label}</text>\n'
        f'    <line x1="30" y1="120" x2="250" y2="120" class="grid"/>\n'
        f'    <text x="22" y="123" class="t-axis-label" text-anchor="end">{axis_mid_label}</text>\n'
        f'    <line x1="30" y1="195" x2="250" y2="195" class="axis"/>\n'
        f'    <text x="22" y="198" class="t-axis-label" text-anchor="end">0</text>\n'
        f'    <line x1="30" y1="40" x2="30" y2="195" class="axis"/>\n'
        f"{bars}"
        f"  </g>\n"
    )


def main() -> int:
    data = json.loads(RESULTS.read_text(encoding="utf-8"))
    scan = data["sequential_scan_mops"]
    ram = data["ram_bytes_per_key"]
    lat = data["readrandom_ns"]
    keys = data["meta"]["keys"]

    scan_speedup = scan["expanse"] / scan["skiplist"]
    ram_ratio = ram["skiplist"] / ram["expanse"]
    ram_saving = (1.0 - ram["expanse"] / ram["skiplist"]) * 100.0
    lat_speedup = lat["skiplist"] / lat["expanse"]

    # Axis maxima are derived from the data, not stamped. They used to be
    # hardcoded, and the RAM panel's 160 B ceiling was sized for the strawman
    # skiplist node's ~146.7 B/entry (#372). Once that baseline was retracted
    # and re-measured at 18.7 B/entry the tallest bar filled about an eighth of
    # the panel, so the chart read as mostly white space and the three
    # candidates were visually indistinguishable. A stamped axis silently
    # outlives the numbers it was scaled for; a derived one cannot.
    scan_max, ram_max, lat_max = (nice_axis_max(v.values()) for v in (scan, ram, lat))

    p1 = panel(
        30, "Sequential Scan", f"{keys:,} keys prefixscan (higher is better)",
        "&#9650; Throughput (M ops / sec)", scan_max, axis_label(scan_max, "M"), axis_label(scan_max / 2, "M"),
        bar(BAR_X[0], scan["expanse"], scan_max, "b-expanse",
            f'{scan["expanse"]:.1f} M/s', "t-val-accent", "Expanse",
            f"{scan_speedup:.1f}&#215; speedup", True)
        + bar(BAR_X[1], scan["skiplist"], scan_max, "b-rocksdb",
              f'{scan["skiplist"]:.1f} M/s', "t-val-blue", "SkipList",
              "1.0&#215; baseline", False)
        + bar(BAR_X[2], scan["vector"], scan_max, "b-skipmap",
              f'{scan["vector"]:.1f} M/s', "t-val-muted", "Vector",
              "read-only", False),
    )
    p2 = panel(
        340, "RAM Footprint per Entry",
        f'{keys:,} keys ({data["meta"]["key_bytes"]}B key / {data["meta"]["value_bytes"]}B val) (lower is better)',
        "&#9660; Indexing Overhead (Bytes / key)", ram_max, axis_label(ram_max, "B"), axis_label(ram_max / 2, "B"),
        bar(BAR_X[0], ram["expanse"], ram_max, "b-expanse",
            f'{ram["expanse"]:.1f} B', "t-val-accent", "Expanse",
            f"-{ram_saving:.1f}% RAM", True)
        + bar(BAR_X[1], ram["skiplist"], ram_max, "b-rocksdb",
              f'{ram["skiplist"]:.1f} B', "t-val-blue", "SkipList",
              f"{ram_ratio:.1f}&#215; memory", False)
        + bar(BAR_X[2], ram["vector"], ram_max, "b-skipmap",
              f'{ram["vector"]:.1f} B', "t-val-muted", "Vector",
              "read-only", False),
    )
    p3 = panel(
        665, "Point Lookup Latency", "readrandom single-key query (lower is better)",
        "&#9660; Query Latency (ns / op)", lat_max, axis_label(lat_max, "ns"), axis_label(lat_max / 2, "ns"),
        bar(BAR_X[0], lat["expanse"], lat_max, "b-expanse",
            f'{lat["expanse"]:.0f} ns', "t-val-accent", "Expanse",
            f"{lat_speedup:.2f}&#215; faster", True)
        + bar(BAR_X[1], lat["skiplist"], lat_max, "b-rocksdb",
              f'{lat["skiplist"]:.0f} ns', "t-val-blue", "SkipList",
              "1.0&#215; baseline", False)
        + bar(BAR_X[2], lat["vector"], lat_max, "b-skipmap",
              f'{lat["vector"]:.0f} ns', "t-val-muted", "Vector",
              "1.0&#215;", False),
    )

    # The wall-clock panels were measured against the retracted strawman
    # skiplist node (#372); results.json records that and the chart must carry
    # it too, since the asset is inlined standalone into the Pages portal where
    # the surrounding DATABASE.md prose does not travel with it.
    caveat = ""
    if data["meta"].get("retraction_372"):
        caveat = (
            '  <text x="30" y="262" class="t-chart-sub">Panels 1 and 3 (wall clock) were measured'
            " against the retracted strawman skiplist node (#372):</text>\n"
            '  <text x="30" y="275" class="t-chart-sub">every vs-SkipList ratio there awaits a'
            " quiet-host re-run. Panel 2 uses the fair variable-height baseline.</text>\n"
        )

    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 960 285" width="100%" height="100%">\n'
        "  <defs>\n    <style>" + STYLE + "    </style>\n  </defs>\n\n"
        '  <rect width="100%" height="100%" class="bg" rx="8"/>\n'
        '  <rect width="100%" height="100%" class="border" rx="8"/>\n\n'
        "  <!-- ================= CHART 1: SEQUENTIAL SCAN ================= -->\n"
        + p1
        + '\n  <line x1="310" y1="20" x2="310" y2="255" class="divider"/>\n\n'
        "  <!-- ================= CHART 2: RAM FOOTPRINT ================= -->\n"
        + p2
        + '\n  <line x1="635" y1="20" x2="635" y2="255" class="divider"/>\n\n'
        "  <!-- ================= CHART 3: POINT LOOKUP LATENCY ================= -->\n"
        + p3
        + "\n"
        + caveat
        + "</svg>\n"
    )

    ET.fromstring(svg)  # validate XML before touching the asset
    OUTPUT.write_text(svg, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(REPO_ROOT)} from {RESULTS.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
