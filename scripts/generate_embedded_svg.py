#!/usr/bin/env python3
"""scripts/generate_embedded_svg.py

Regenerates ``docs/assets/bench_embedded.svg`` (the three-panel embedded
storage-engine chart embedded by ``docs/DATABASE.md`` §5.4) from
``docs/benchmarks/embedded/results.json``.

Panels 1-2 (memory) are **derived**, not measured: every byte figure is
computed by importing ``scripts/embedded_envelope.py`` — the committed,
unit-tested derivation whose density constants come from the measured
``bytes_per_key_32`` example — so no constant is retyped here (§8.2).
Panel 3 (wall clock) is **measured**: it renders the BCa-harvested
``baseline-embedded_memtable.json`` a quiet-host bench run produces
(``scripts/bench_baseline.py --harvest``), and the chart footer carries
that run's provenance.

Refresh the committed record from a bench-host artifact, then the SVG:

    python3 scripts/generate_embedded_svg.py --from-baseline baseline-embedded_memtable.json
    python3 scripts/generate_embedded_svg.py           # re-render from committed results.json

XML is validated before writing (same discipline as
``integrations/rocksdb/scripts/generate_bench_svg.py``, whose geometry and
dual-theme styling this mirrors).
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Iterable
import xml.etree.ElementTree as ET

REPO_ROOT = Path(__file__).resolve().parents[1]
RESULTS = REPO_ROOT / "docs" / "benchmarks" / "embedded" / "results.json"
OUTPUT = REPO_ROOT / "docs" / "assets" / "bench_embedded.svg"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
import embedded_envelope as env  # noqa: E402  (single source of the memory model)

N = 2000  # the mid population every #556 table reports

# Criterion full_ids for the measured panel (see benches/embedded_memtable.rs).
MEASURED_ARMS = {
    "expanse": f"embedded_tsdb_ingest_and_flush/expanse_map32_ingest/{N}",
    "hashmap": f"embedded_tsdb_ingest_and_flush/hashmap_ingest/{N}",
    "btreemap": f"embedded_tsdb_ingest_and_flush/btreemap_ingest/{N}",
}

# Plot geometry shared by all three panels (rocksdb chart's layout).
BASELINE_Y = 195.0
AXIS_TOP_Y = 45.0
PLOT_H = BASELINE_Y - AXIS_TOP_Y
BAR_X = [45, 115, 185]
BAR_W = 50

STYLE = """
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
      .b-hashmap { fill: #2563eb; }
      .b-stdmap { fill: #475569; }

      @media (prefers-color-scheme: dark) {
        :root:not([data-theme="light"]) .bg { fill: #0b1220; }
        :root:not([data-theme="light"]) .border { stroke: #1e293b; }
        :root:not([data-theme="light"]) .grid { stroke: #16233b; }
        :root:not([data-theme="light"]) .axis { stroke: #334155; }
        :root:not([data-theme="light"]) .divider { stroke: #1e293b; }
        :root:not([data-theme="light"]) .t-chart-title { fill: #e2e8f0; }
        :root:not([data-theme="light"]) .t-chart-sub { fill: #94a3b8; }
        :root:not([data-theme="light"]) .t-unit-header, :root:not([data-theme="light"]) .t-axis-label { fill: #64748b; }
        :root:not([data-theme="light"]) .t-bar-label { fill: #e2e8f0; }
        :root:not([data-theme="light"]) .t-val-muted { fill: #94a3b8; }
        :root:not([data-theme="light"]) .t-win { fill: #4ade80; }
        :root:not([data-theme="light"]) .t-val-accent { fill: #4ade80; }
        :root:not([data-theme="light"]) .t-val-blue { fill: #60a5fa; }
        :root:not([data-theme="light"]) .b-expanse { fill: #22c55e; }
        :root:not([data-theme="light"]) .b-hashmap { fill: #3b82f6; }
        :root:not([data-theme="light"]) .b-stdmap { fill: #475569; }
      }
      [data-theme="dark"] .bg, :root[data-theme="dark"] .bg { fill: #0b1220; }
      [data-theme="dark"] .border, :root[data-theme="dark"] .border { stroke: #1e293b; }
      [data-theme="dark"] .grid, :root[data-theme="dark"] .grid { stroke: #16233b; }
      [data-theme="dark"] .axis, :root[data-theme="dark"] .axis { stroke: #334155; }
      [data-theme="dark"] .divider, :root[data-theme="dark"] .divider { stroke: #1e293b; }
      [data-theme="dark"] .t-chart-title, :root[data-theme="dark"] .t-chart-title { fill: #e2e8f0; }
      [data-theme="dark"] .t-chart-sub, :root[data-theme="dark"] .t-chart-sub { fill: #94a3b8; }
      [data-theme="dark"] .t-unit-header, :root[data-theme="dark"] .t-unit-header { fill: #64748b; }
      [data-theme="dark"] .t-axis-label, :root[data-theme="dark"] .t-axis-label { fill: #64748b; }
      [data-theme="dark"] .t-bar-label, :root[data-theme="dark"] .t-bar-label { fill: #e2e8f0; }
      [data-theme="dark"] .t-val-muted, :root[data-theme="dark"] .t-val-muted { fill: #94a3b8; }
      [data-theme="dark"] .t-win, :root[data-theme="dark"] .t-win { fill: #4ade80; }
      [data-theme="dark"] .t-val-accent, :root[data-theme="dark"] .t-val-accent { fill: #4ade80; }
      [data-theme="dark"] .t-val-blue, :root[data-theme="dark"] .t-val-blue { fill: #60a5fa; }
      [data-theme="dark"] .b-expanse, :root[data-theme="dark"] .b-expanse { fill: #22c55e; }
      [data-theme="dark"] .b-hashmap, :root[data-theme="dark"] .b-hashmap { fill: #3b82f6; }
      [data-theme="dark"] .b-stdmap, :root[data-theme="dark"] .b-stdmap { fill: #475569; }
"""


def nice_axis_max(values: "Iterable[float]") -> float:
    """1/1.2/1.5/2/2.5/3/4/5/6/7/8/10 ladder ceiling (see the rocksdb
    generator for why the ladder is finer than 1/2/5)."""
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


def bar(x: int, value: float, axis_max: float, css: str, val_text: str, val_css: str,
        label: str, caption: str, caption_win: bool) -> str:
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
          top_label: str, mid_label: str, bars: str) -> str:
    return (
        f'  <g transform="translate({tx}, 20)">\n'
        f'    <text x="0" y="0" class="t-chart-title">{title}</text>\n'
        f'    <text x="0" y="13" class="t-chart-sub">{sub}</text>\n'
        f'    <text x="0" y="29" class="t-unit-header">{unit}</text>\n'
        f'    <line x1="30" y1="45" x2="250" y2="45" class="grid"/>\n'
        f'    <text x="22" y="48" class="t-axis-label" text-anchor="end">{top_label}</text>\n'
        f'    <line x1="30" y1="120" x2="250" y2="120" class="grid"/>\n'
        f'    <text x="22" y="123" class="t-axis-label" text-anchor="end">{mid_label}</text>\n'
        f'    <line x1="30" y1="195" x2="250" y2="195" class="axis"/>\n'
        f'    <text x="22" y="198" class="t-axis-label" text-anchor="end">0</text>\n'
        f'    <line x1="30" y1="40" x2="30" y2="195" class="axis"/>\n'
        f"{bars}"
        f"  </g>\n"
    )


def derive_memory() -> dict:
    """All memory cells straight from the envelope module — never retyped."""
    return {
        "tsdb_bytes_per_key": {
            "expanse": env.mem_expanse_tsdb(N) / N,
            "hashmap": env.mem_unordered_map(N) / N,
            "stdmap": env.mem_std_map(N) / N,
        },
        "ble_tracker_kib": {
            "expanse": env.mem_ble_tracker_expanse_slab(N) / 1024.0,
            "hashmap": env.mem_unordered_map(N, val_size=env.BLE_RECORD_SIZE) / 1024.0,
            "stdmap": env.mem_std_map(N, val_size=env.BLE_RECORD_SIZE) / 1024.0,
        },
    }


def rebuild_results(baseline_path: Path) -> dict:
    doc = json.loads(baseline_path.read_text(encoding="utf-8"))
    by_id = {}
    for a in doc.get("arms", []):
        arm_id = a.get("id") or a.get("arm") or a.get("full_id")
        if arm_id:
            by_id[arm_id] = a
    wall = {}
    for name, full_id in MEASURED_ARMS.items():
        a = by_id.get(full_id)
        if a is None:
            raise SystemExit(
                f"error: arm '{full_id}' absent from {baseline_path} — refusing to "
                f"render a panel with a missing arm (§8.1)."
            )
        wall[name] = {
            "point_ns": a["point_ns"],
            "ci_lower_ns": a.get("ci_lower_ns"),
            "ci_upper_ns": a.get("ci_upper_ns"),
            "status": a.get("status"),
        }
    return {
        "meta": {
            "population": N,
            "memory_model": "scripts/embedded_envelope.py (density constants from the "
                            "measured bytes_per_key_32 example; auxiliary slab arrays "
                            "modeled exactly)",
            "wallclock_provenance": doc.get("provenance", {}),
            "wallclock_statistics": doc.get("statistics", {}),
        },
        "wallclock_ns": wall,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--from-baseline", type=Path,
                    help="baseline-embedded_memtable.json from a bench-host run; "
                         "rebuilds results.json before rendering")
    args = ap.parse_args()

    if args.from_baseline:
        results = rebuild_results(args.from_baseline)
        RESULTS.parent.mkdir(parents=True, exist_ok=True)
        RESULTS.write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {RESULTS}")
    else:
        results = json.loads(RESULTS.read_text(encoding="utf-8"))

    mem = derive_memory()  # always derived live, never persisted numbers
    tsdb = mem["tsdb_bytes_per_key"]
    ble = mem["ble_tracker_kib"]
    wall = {k: v["point_ns"] for k, v in results["wallclock_ns"].items()}
    prov = results["meta"]["wallclock_provenance"]

    tsdb_max = nice_axis_max(tsdb.values())
    ble_max = nice_axis_max(ble.values())
    wall_us = {k: v / 1000.0 for k, v in wall.items()}
    wall_max = nice_axis_max(wall_us.values())

    tsdb_ratio = tsdb["stdmap"] / tsdb["expanse"]
    ble_ratio = ble["expanse"] / ble["hashmap"]
    wall_best_other = min(wall_us["hashmap"], wall_us["btreemap"])
    wall_ratio = wall_best_other / wall_us["expanse"]
    # State a loss as a loss: "N x slower" reads unambiguously where a
    # sub-1.0 "vs best alt" ratio does not (SS 8.7 honest reporting).
    if wall_ratio >= 1.0:
        wall_caption = f"{wall_ratio:.2f}&#215; faster than best alt"
    else:
        wall_caption = f"{1.0 / wall_ratio:.1f}&#215; slower (density trade)"

    p1 = panel(
        30, "Sensor TSDB Density", f"1 kHz timestamps, N={N:,} (lower is better; derived)",
        "&#9660; SRAM (Bytes / key)", tsdb_max, f"{tsdb_max:g} B", f"{tsdb_max / 2:g} B",
        bar(BAR_X[0], tsdb["expanse"], tsdb_max, "b-expanse",
            f'{tsdb["expanse"]:.2f} B', "t-val-accent", "Expanse",
            f"{tsdb_ratio:.1f}&#215; denser", True)
        + bar(BAR_X[1], tsdb["hashmap"], tsdb_max, "b-hashmap",
              f'{tsdb["hashmap"]:.0f} B', "t-val-blue", "HashMap",
              "reserve(peak)", False)
        + bar(BAR_X[2], tsdb["stdmap"], tsdb_max, "b-stdmap",
              f'{tsdb["stdmap"]:.0f} B', "t-val-muted", "std::map",
              "per-node malloc", False),
    )
    p2 = panel(
        340, "BLE Tracker Footprint",
        f"28 B records + TTL index, N={N:,} (derived)",
        "&#9660; SRAM (KiB total)", ble_max, f"{ble_max:g}", f"{ble_max / 2:g}",
        bar(BAR_X[0], ble["expanse"], ble_max, "b-expanse",
            f'{ble["expanse"]:.1f}', "t-val-accent", "Expanse",
            f"{ble_ratio:.2f}&#215; = parity + O(expired) TTL", True)
        + bar(BAR_X[1], ble["hashmap"], ble_max, "b-hashmap",
              f'{ble["hashmap"]:.1f}', "t-val-blue", "HashMap",
              "O(N) TTL sweep", False)
        + bar(BAR_X[2], ble["stdmap"], ble_max, "b-stdmap",
              f'{ble["stdmap"]:.1f}', "t-val-muted", "std::map",
              "ordered", False),
    )
    p3 = panel(
        665, "TSDB Ingest + Flush",
        f"N={N:,} cycle, host reference (lower is better; measured)",
        "&#9660; Wall clock (&#181;s / cycle)", wall_max, f"{wall_max:g}", f"{wall_max / 2:g}",
        bar(BAR_X[0], wall_us["expanse"], wall_max, "b-expanse",
            f'{wall_us["expanse"]:.0f} &#181;s', "t-val-accent", "Expanse",
            wall_caption, wall_ratio >= 1.0)
        + bar(BAR_X[1], wall_us["hashmap"], wall_max, "b-hashmap",
              f'{wall_us["hashmap"]:.0f} &#181;s', "t-val-blue", "HashMap",
              "unordered flush", False)
        + bar(BAR_X[2], wall_us["btreemap"], wall_max, "b-stdmap",
              f'{wall_us["btreemap"]:.0f} &#181;s', "t-val-muted", "BTreeMap",
              "ordered", False),
    )

    run_id = prov.get("run_id", "unrecorded")
    host = prov.get("host_description", "unrecorded host")
    commit = prov.get("commit", "unrecorded")
    footer = (
        f'  <text x="30" y="262" class="t-chart-sub">Panels 1-2 derived by '
        f"scripts/embedded_envelope.py (density constants from the measured bytes_per_key_32 "
        f"example); panel 3 measured: {host},</text>\n"
        f'  <text x="30" y="275" class="t-chart-sub">commit {commit}, run {run_id} '
        f"(BCa 95% CIs in docs/benchmarks/embedded/results.json). On-device ESP32 chart "
        f"pending the first hardware harvest.</text>\n"
    )

    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 960 285" width="100%" height="100%">\n'
        "  <defs>\n    <style>" + STYLE + "    </style>\n  </defs>\n\n"
        '  <rect width="100%" height="100%" class="bg" rx="8"/>\n'
        '  <rect width="100%" height="100%" class="border" rx="8"/>\n\n'
        + p1
        + '\n  <line x1="310" y1="20" x2="310" y2="255" class="divider"/>\n\n'
        + p2
        + '\n  <line x1="635" y1="20" x2="635" y2="255" class="divider"/>\n\n'
        + p3
        + "\n"
        + footer
        + "</svg>\n"
    )

    ET.fromstring(svg)  # refuse to write malformed XML
    OUTPUT.write_text(svg, encoding="utf-8")
    print(f"wrote {OUTPUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
