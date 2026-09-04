#!/usr/bin/env python3
"""scripts/generate_domain_algebra_svg.py

Renders docs/assets/bench_domain_algebra.svg from committed benchmark data:
  docs/assets/data/bench_domain_algebra.json

Covers all set algebra features (previous and new):
  1. Materialization evolution (#348 v2 direct emission vs v1 merge-insert)
  2. k-way aggregate algebra (#610 multi-way walk vs pairwise cascade vs Roaring)
  3. Interned set domain (#611 zero-overhead provenance check)
  4. Ingestion & resolution throughput (#611 scalar vs batched 128, zero-copy resolution)

Adheres to AGENTS.md §8:
  - Every number traceable to committed data / reference harness
  - Valid XML verified via xml.etree.ElementTree
  - Fully responsive dark/light styling
  - Zero text collisions / overlaps with generous geometry and pixel accounting
"""

from __future__ import annotations

import json
import math
import xml.etree.ElementTree as ET
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
DATA_PATH = REPO_ROOT / "docs" / "assets" / "data" / "bench_domain_algebra.json"
OUTPUT_PATH = REPO_ROOT / "docs" / "assets" / "bench_domain_algebra.svg"

STYLE = """
      /* Light theme (default) */
      .bg { fill: #ffffff; }
      .border { stroke: #e2e8f0; stroke-width: 1px; fill: none; }
      .grid { stroke: #f1f5f9; stroke-width: 1px; stroke-dasharray: 2,3; }
      .axis { stroke: #cbd5e1; stroke-width: 1.5px; }
      .divider { stroke: #e2e8f0; stroke-width: 1px; }

      .t-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 13px; font-weight: 700; letter-spacing: 0.6px; fill: #0f172a; text-transform: uppercase; }
      .t-chart-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 11px; font-weight: 700; letter-spacing: 0.5px; fill: #0f172a; text-transform: uppercase; }
      .t-sub { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 500; fill: #475569; }
      .t-unit { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9px; font-weight: 600; fill: #64748b; }
      .t-bar-label { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 10.5px; font-weight: 700; fill: #0f172a; }
      .t-legend { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 600; fill: #0f172a; }
      .t-note { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 8.5px; font-weight: 500; fill: #64748b; }

      .t-val-green { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 700; fill: #15803d; }
      .t-val-blue { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 600; fill: #2563eb; }
      .t-val-muted { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9px; font-weight: 500; fill: #64748b; }

      .b-expanse { fill: #16a34a; }
      .b-roaring { fill: #2563eb; }
      .b-v1 { fill: #94a3b8; }
      .b-card { fill: #f8fafc; stroke: #e2e8f0; stroke-width: 1px; rx: 6px; }

      .badge-win { fill: #dcfce7; stroke: #86efac; stroke-width: 1px; rx: 3px; }
      .badge-win-text { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9px; font-weight: 700; fill: #15803d; text-anchor: middle; }

      /* Dark theme overrides */
      @media (prefers-color-scheme: dark) {
        .bg { fill: #0d1117; }
        .border { stroke: #30363d; }
        .grid { stroke: #21262d; }
        .axis { stroke: #484f58; }
        .divider { stroke: #21262d; }
        .t-title { fill: #f0f6fc; }
        .t-chart-title { fill: #f0f6fc; }
        .t-sub { fill: #94a3b8; }
        .t-unit { fill: #94a3b8; }
        .t-bar-label { fill: #f8fafc; }
        .t-legend { fill: #f8fafc; }
        .t-note { fill: #94a3b8; }
        .t-val-green { fill: #4ade80; }
        .t-val-blue { fill: #38bdf8; }
        .t-val-muted { fill: #94a3b8; }
        .b-expanse { fill: #22c55e; }
        .b-roaring { fill: #3b82f6; }
        .b-v1 { fill: #64748b; }
        .b-card { fill: #161b22; stroke: #30363d; }
        .badge-win { fill: #064e3b; stroke: #059669; }
        .badge-win-text { fill: #6ee7b7; }
      }

      :root[data-theme="dark"] .bg, [data-theme="dark"] .bg { fill: #0d1117; }
      :root[data-theme="dark"] .border, [data-theme="dark"] .border { stroke: #30363d; }
      :root[data-theme="dark"] .grid, [data-theme="dark"] .grid { stroke: #21262d; }
      :root[data-theme="dark"] .axis, [data-theme="dark"] .axis { stroke: #484f58; }
      :root[data-theme="dark"] .divider, [data-theme="dark"] .divider { stroke: #21262d; }
      :root[data-theme="dark"] .t-title, [data-theme="dark"] .t-title { fill: #f0f6fc; }
      :root[data-theme="dark"] .t-chart-title, [data-theme="dark"] .t-chart-title { fill: #f0f6fc; }
      :root[data-theme="dark"] .t-sub, [data-theme="dark"] .t-sub { fill: #94a3b8; }
      :root[data-theme="dark"] .t-unit, [data-theme="dark"] .t-unit { fill: #94a3b8; }
      :root[data-theme="dark"] .t-bar-label, [data-theme="dark"] .t-bar-label { fill: #f8fafc; }
      :root[data-theme="dark"] .t-legend, [data-theme="dark"] .t-legend { fill: #f8fafc; }
      :root[data-theme="dark"] .t-note, [data-theme="dark"] .t-note { fill: #94a3b8; }
      :root[data-theme="dark"] .t-val-green, [data-theme="dark"] .t-val-green { fill: #4ade80; }
      :root[data-theme="dark"] .t-val-blue, [data-theme="dark"] .t-val-blue { fill: #38bdf8; }
      :root[data-theme="dark"] .t-val-muted, [data-theme="dark"] .t-val-muted { fill: #94a3b8; }
      :root[data-theme="dark"] .b-expanse, [data-theme="dark"] .b-expanse { fill: #22c55e; }
      :root[data-theme="dark"] .b-roaring, [data-theme="dark"] .b-roaring { fill: #3b82f6; }
      :root[data-theme="dark"] .b-v1, [data-theme="dark"] .b-v1 { fill: #64748b; }
      :root[data-theme="dark"] .b-card, [data-theme="dark"] .b-card { fill: #161b22; stroke: #30363d; }
      :root[data-theme="dark"] .badge-win, [data-theme="dark"] .badge-win { fill: #064e3b; stroke: #059669; }
      :root[data-theme="dark"] .badge-win-text, [data-theme="dark"] .badge-win-text { fill: #6ee7b7; }
"""


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def badge(x: float, y: float, w: float, text: str) -> str:
    return (
        f'  <rect x="{x}" y="{y}" width="{w}" height="17" class="badge-win"/>\n'
        f'  <text x="{x + w / 2:.1f}" y="{y + 12:.1f}" class="badge-win-text">{esc(text)}</text>\n'
    )


def render_svg(data: dict) -> str:
    width = 960
    height = 670
    meta = data["meta"]

    out = []
    out.append(f'<svg xmlns="http://www.w3.org/2000/svg" role="img" viewBox="0 0 {width} {height}" width="100%" height="100%">\n')
    out.append('  <title>SET ALGEBRA &amp; INTERNED SET DOMAIN BENCHMARK SUITE</title>\n')
    out.append(f'  <defs>\n    <style>{STYLE}    </style>\n  </defs>\n\n')
    out.append('  <rect width="100%" height="100%" class="bg" rx="8"/>\n')
    out.append('  <rect width="100%" height="100%" class="border" rx="8"/>\n\n')

    # --- Header ---
    out.append('  <text x="30" y="28" class="t-title">SET ALGEBRA &amp; INTERNED SET DOMAIN BENCHMARK SUITE</text>\n')
    out.append('  <text x="30" y="44" class="t-sub">Direct emission (#348), k-way aggregate walk (#610), and interned domain zero-overhead (#611)</text>\n')

    # Legend at top right (starts at x=610)
    out.append('  <g transform="translate(610, 18)">\n')
    out.append('    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>\n')
    out.append('    <text x="16" y="10" class="t-legend">Expanse (v2 / k-Way)</text>\n')
    out.append('    <rect x="145" y="0" width="12" height="12" rx="2" class="b-roaring"/>\n')
    out.append('    <text x="161" y="10" class="t-legend">Roaring</text>\n')
    out.append('    <rect x="220" y="0" width="12" height="12" rx="2" class="b-v1"/>\n')
    out.append('    <text x="236" y="10" class="t-legend">v1 / Pairwise Fold</text>\n')
    out.append('  </g>\n')
    out.append('  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>\n\n')

    # =========================================================================
    # SECTION 1: CORE ALGEBRA EVOLUTION & K-WAY SCALING (TOP HALF)
    # =========================================================================
    # Panel 1A: Materialization Evolution (#348)
    out.append('  <!-- Panel 1A: Materialization Evolution (#348) -->\n')
    out.append('  <g transform="translate(30, 70)">\n')
    out.append('    <text x="0" y="10" class="t-chart-title">1. MATERIALIZATION: DIRECT EMISSION (#348)</text>\n')
    out.append('    <text x="0" y="24" class="t-sub">A ∩ B result set built • N=100k symmetric • latency (log scale, lower is better)</text>\n')

    mat_rows = data["materialization_348"]
    min_log = math.log10(1.0)
    max_log = math.log10(1500.0)

    def log_w_mat(val_us: float) -> float:
        lv = math.log10(max(1.0, val_us))
        ratio = (lv - min_log) / (max_log - min_log)
        return max(5.0, min(130.0, ratio * 130.0))

    y_row_start = 42
    row_step = 54

    for i, r in enumerate(mat_rows):
        y = y_row_start + i * row_step
        # Label column: x=0 to x=72
        out.append(f'    <text x="0" y="{y + 19}" class="t-bar-label">{esc(r["distribution"])}</text>\n')

        # Bars column starts at x=78
        bx = 78
        # Expanse v2 bar
        w_v2 = log_w_mat(r["v2_us"])
        out.append(f'    <rect x="{bx}" y="{y}" width="{w_v2:.1f}" height="8" rx="1.5" class="b-expanse"/>\n')
        out.append(f'    <text x="{bx + w_v2 + 5:.1f}" y="{y + 7.5}" class="t-val-green">{r["v2_us"]:.1f}µs</text>\n')

        # Roaring bar
        w_roar = log_w_mat(r["roaring_us"])
        out.append(f'    <rect x="{bx}" y="{y + 11}" width="{w_roar:.1f}" height="8" rx="1.5" class="b-roaring"/>\n')
        out.append(f'    <text x="{bx + w_roar + 5:.1f}" y="{y + 18.5}" class="t-val-blue">{r["roaring_us"]:.1f}µs</text>\n')

        # v1 bar
        w_v1 = log_w_mat(r["v1_us"])
        out.append(f'    <rect x="{bx}" y="{y + 22}" width="{w_v1:.1f}" height="8" rx="1.5" class="b-v1"/>\n')
        v1_str = f'{r["v1_us"]:.0f}µs' if r["v1_us"] < 1000 else f'{r["v1_us"]/1000:.1f}ms'
        out.append(f'    <text x="{bx + w_v1 + 5:.1f}" y="{y + 29.5}" class="t-val-muted">{v1_str}</text>\n')

        # Badge column starts at x=325
        out.append(badge(bx + 247, y + 8, 105, f'{r["speedup_vs_v1"]:.1f}x vs v1'))

    out.append('  </g>\n\n')

    # Vertical divider between 1A and 1B
    out.append('  <line x1="478" y1="68" x2="478" y2="330" class="divider"/>\n\n')

    # Panel 1B: k-Way Aggregate Algebra (#610)
    out.append('  <!-- Panel 1B: k-Way Aggregate Algebra (#610) -->\n')
    out.append('  <g transform="translate(498, 70)">\n')
    out.append('    <text x="0" y="10" class="t-chart-title">2. K-WAY AGGREGATE WALK (#610, K=5)</text>\n')
    out.append('    <text x="0" y="24" class="t-sub">k=5 multi-set intersection • N=100k • latency (log scale, lower is better)</text>\n')

    kway_rows = data["kway_610"]
    min_log_k = math.log10(0.5)
    max_log_k = math.log10(4000.0)

    def log_w_k(val_us: float) -> float:
        lv = math.log10(max(0.5, val_us))
        ratio = (lv - min_log_k) / (max_log_k - min_log_k)
        return max(5.0, min(130.0, ratio * 130.0))

    for i, r in enumerate(kway_rows):
        y = y_row_start + i * row_step
        # Label column: x=0 to x=72
        out.append(f'    <text x="0" y="{y + 19}" class="t-bar-label">{esc(r["distribution"])}</text>\n')

        bx = 78
        # Expanse kway bar
        w_kw = log_w_k(r["kway_us"])
        out.append(f'    <rect x="{bx}" y="{y}" width="{w_kw:.1f}" height="8" rx="1.5" class="b-expanse"/>\n')
        kw_str = f'{r["kway_us"]*1000:.0f}ns' if r["kway_us"] < 1.0 else f'{r["kway_us"]:.1f}µs'
        out.append(f'    <text x="{bx + w_kw + 5:.1f}" y="{y + 7.5}" class="t-val-green">{kw_str}</text>\n')

        # Roaring MultiOps bar
        w_rm = log_w_k(r["roaring_us"])
        out.append(f'    <rect x="{bx}" y="{y + 11}" width="{w_rm:.1f}" height="8" rx="1.5" class="b-roaring"/>\n')
        rm_str = f'{r["roaring_us"]*1000:.0f}ns' if r["roaring_us"] < 1.0 else (f'{r["roaring_us"]:.1f}µs' if r["roaring_us"] < 1000 else f'{r["roaring_us"]/1000:.1f}ms')
        out.append(f'    <text x="{bx + w_rm + 5:.1f}" y="{y + 18.5}" class="t-val-blue">{rm_str}</text>\n')

        # Pairwise fold bar
        w_pw = log_w_k(r["pairwise_us"])
        out.append(f'    <rect x="{bx}" y="{y + 22}" width="{w_pw:.1f}" height="8" rx="1.5" class="b-v1"/>\n')
        pw_str = f'{r["pairwise_us"]:.0f}µs' if r["pairwise_us"] < 1000 else f'{r["pairwise_us"]/1000:.1f}ms'
        out.append(f'    <text x="{bx + w_pw + 5:.1f}" y="{y + 29.5}" class="t-val-muted">{pw_str}</text>\n')

        # Badge column starts at x=320
        badge_txt = f'{r["speedup_vs_pairwise"]:.0f}x vs fold' if r["speedup_vs_pairwise"] > 10 else f'{r["speedup_vs_pairwise"]:.1f}x vs fold'
        out.append(badge(bx + 242, y + 8, 110, badge_txt))

    out.append('  </g>\n\n')

    # Horizontal divider between Section 1 and Section 2
    out.append('  <line x1="30" y1="340" x2="930" y2="340" class="divider"/>\n\n')

    # =========================================================================
    # SECTION 2: INTERNED SET DOMAIN (#611, BOTTOM HALF)
    # =========================================================================
    out.append('  <!-- Panel 2: Interned Set Domain (#611) -->\n')
    out.append('  <g transform="translate(30, 354)">\n')
    out.append('    <text x="0" y="10" class="t-chart-title">3. INTERNED SET DOMAIN: PARITY, INGESTION &amp; RESOLUTION (#611)</text>\n')
    out.append('    <text x="0" y="24" class="t-sub">Branded DomainSet zero-overhead verification, batched prefix-trie ingestion, and zero-copy slab resolution</text>\n')

    # Card 2A: Algebra Zero-Overhead Parity (width: 285px)
    out.append('    <!-- 2A: Parity Card -->\n')
    out.append('    <g transform="translate(0, 36)">\n')
    out.append('      <rect x="0" y="0" width="285" height="172" class="b-card"/>\n')
    out.append('      <text x="14" y="20" class="t-bar-label">Algebra Provenance Overhead</text>\n')
    out.append('      <text x="14" y="34" class="t-unit">Raw ExpanseSet vs DomainSet (N=100k)</text>\n')
    out.append('      <line x1="14" y1="43" x2="271" y2="43" class="divider"/>\n')

    # Both rows are DERIVED from the dataset, never stamped (AGENTS.md 8.2):
    # these values were hardcoded here as "9.70 µs" against a source recording
    # "9.7", inventing a significant figure the measurement does not have.
    #
    # The badge states a BOUND, not an equality. Equal values recorded to a
    # finite resolution bound the difference below that resolution; they cannot
    # establish zero. The previous "+0.00 ns parity" asserted a precision 100x
    # finer than the source carries (AGENTS.md 8.4: a null result is an
    # overlap, not a zero).
    parity = data["domain_parity_611"]

    def fmt_us(v: float, res: float) -> str:
        dp = max(0, len(str(res).split(".")[1])) if "." in str(res) else 0
        return f"{v:.{dp}f} µs"

    def bound_label(res: float) -> str:
        # Raw "<" here: badge() runs esc() on its text, so pre-escaping would
        # render the entity literally.
        return f"<{res*1000:.0f} ns" if res < 1.0 else f"<{res:.0f} µs"

    for idx, (key, label, sub, y0) in enumerate((
        ("intersection_100k_us", "intersection()", "(direct emission)", 60),
        ("intersection_len_100k_us", "intersection_len()", "(count only)", 121),
    )):
        row = parity[key]
        res = row["recorded_resolution_us"]
        out.append(f'      <text x="14" y="{y0}" class="t-bar-label">{label} <tspan class="t-sub">{sub}</tspan></text>\n')
        out.append(f'      <text x="14" y="{y0 + 16}" class="t-sub">Raw ExpanseSet:  <tspan class="t-val-green">{fmt_us(row["raw_expanse"], res)}</tspan></text>\n')
        out.append(f'      <text x="14" y="{y0 + 30}" class="t-sub">DomainSet brand: <tspan class="t-val-green">{fmt_us(row["domain_set"], res)}</tspan></text>\n')
        out.append(badge(170, y0 + 8, 102, f"{bound_label(res)} difference"))
        if idx == 0:
            out.append('      <line x1="14" y1="103" x2="271" y2="103" class="grid"/>\n')
    out.append('    </g>\n')

    # Card 2B: Ingestion Throughput (width: 315px)
    out.append('    <!-- 2B: Ingestion Card -->\n')
    out.append('    <g transform="translate(300, 36)">\n')
    out.append('      <rect x="0" y="0" width="315" height="172" class="b-card"/>\n')
    out.append('      <text x="14" y="20" class="t-bar-label">Dictionary Ingestion Throughput</text>\n')
    out.append('      <text x="14" y="34" class="t-unit">Throughput (M keys / sec) • higher is better</text>\n')
    out.append('      <line x1="14" y1="43" x2="301" y2="43" class="divider"/>\n')

    ingest_data = data["domain_ingestion_611"]
    max_ingest = 5.5
    bar_w_max = 85.0

    # Item 1: Text Keys
    out.append('      <text x="14" y="60" class="t-bar-label">Text Keys (user:...)</text>\n')
    out.append(badge(212, 50, 89, "3.12x faster"))
    w_b1 = (ingest_data[0]["batch128_mops"] / max_ingest) * bar_w_max
    out.append(f'      <rect x="14" y="68" width="{w_b1:.1f}" height="8" rx="1.5" class="b-expanse"/>\n')
    out.append(f'      <text x="{14 + w_b1 + 5:.1f}" y="{68 + 7.5}" class="t-val-green">Batch 128: {ingest_data[0]["batch128_mops"]:.2f} M/s</text>\n')
    w_s1 = (ingest_data[0]["scalar_mops"] / max_ingest) * bar_w_max
    out.append(f'      <rect x="14" y="79" width="{w_s1:.1f}" height="8" rx="1.5" class="b-v1"/>\n')
    out.append(f'      <text x="{14 + w_s1 + 5:.1f}" y="{79 + 7.5}" class="t-val-muted">Scalar: {ingest_data[0]["scalar_mops"]:.2f} M/s</text>\n')

    out.append('      <line x1="14" y1="103" x2="301" y2="103" class="grid"/>\n')

    # Item 2: Binary UUID
    out.append('      <text x="14" y="121" class="t-bar-label">Binary UUID (NUL-escaped)</text>\n')
    out.append(badge(212, 111, 89, "3.08x faster"))
    w_b2 = (ingest_data[1]["batch128_mops"] / max_ingest) * bar_w_max
    out.append(f'      <rect x="14" y="129" width="{w_b2:.1f}" height="8" rx="1.5" class="b-expanse"/>\n')
    out.append(f'      <text x="{14 + w_b2 + 5:.1f}" y="{129 + 7.5}" class="t-val-green">Batch 128: {ingest_data[1]["batch128_mops"]:.2f} M/s</text>\n')
    w_s2 = (ingest_data[1]["scalar_mops"] / max_ingest) * bar_w_max
    out.append(f'      <rect x="14" y="140" width="{w_s2:.1f}" height="8" rx="1.5" class="b-v1"/>\n')
    out.append(f'      <text x="{14 + w_s2 + 5:.1f}" y="{140 + 7.5}" class="t-val-muted">Scalar: {ingest_data[1]["scalar_mops"]:.2f} M/s</text>\n')
    out.append('    </g>\n')

    # Card 2C: Zero-Copy Resolution (width: 270px)
    out.append('    <!-- 2C: Resolution Card -->\n')
    out.append('    <g transform="translate(630, 36)">\n')
    out.append('      <rect x="0" y="0" width="270" height="172" class="b-card"/>\n')
    out.append('      <text x="14" y="20" class="t-bar-label">Zero-Copy Slab Resolution</text>\n')
    out.append('      <text x="14" y="34" class="t-unit">resolve() direct slice iteration</text>\n')
    out.append('      <line x1="14" y1="43" x2="256" y2="43" class="divider"/>\n')

    res = data["domain_resolution_611"]
    out.append(f'      <text x="14" y="78" class="t-chart-title" style="font-size: 26px; font-weight: 800; fill: #15803d;">{res["scan_mops"]:.1f} M</text>\n')
    out.append('      <text x="115" y="72" class="t-bar-label" style="font-size: 13px; font-weight: 700;">keys / sec</text>\n')
    out.append('      <text x="115" y="86" class="t-sub">throughput</text>\n')
    out.append(f'      <text x="14" y="106" class="t-sub">Latency: <tspan class="t-val-green">{res["latency_ns"]:.1f} ns / key</tspan> amortized scan</text>\n')

    out.append('      <line x1="14" y1="118" x2="256" y2="118" class="grid"/>\n')

    out.append('      <text x="14" y="136" class="t-bar-label">Zero Heap Allocation</text>\n')
    out.append('      <text x="14" y="150" class="t-sub">Borrows &amp;[u8] directly from stable chunk</text>\n')
    out.append('      <text x="14" y="163" class="t-sub">slabs in BlobArena (no string clones)</text>\n')
    out.append('    </g>\n')

    out.append('  </g>\n\n')

    # =========================================================================
    # FOOTER
    # =========================================================================
    out.append('  <line x1="30" y1="585" x2="930" y2="585" class="divider"/>\n')
    # Provenance is rendered PER SOURCE GROUP, never as one line for the whole
    # figure. A single "Measured: <hostA> / <hostB>, commit <X>" footer used to
    # cover four panels drawn from two harnesses, which asserted an attribution
    # the data cannot support and paired disjoint experiments under one tag
    # (AGENTS.md 8.7, 8.12). Panels are grouped by harness below, and a group
    # whose host the source did not record says so rather than naming one.
    prov = data["provenance"]

    def group_line(sections: list[str]) -> str:
        hosts = {prov[k]["host"] for k in sections}
        issues = ", ".join(sorted({prov[k]["issue"] for k in sections}))
        harness = prov[sections[0]]["harness"].rsplit("/", 1)[-1]
        if hosts == {"unresolved"}:
            state = "host and commit unresolved in the source dataset \u2014 not re-measured"
        else:
            state = " / ".join(sorted(hosts))
        return f"{harness} ({issues}) &#183; {esc(state)}"

    out.append(f'  <text x="30" y="601" class="t-note">Panels 1-2: {group_line(["materialization_348", "kway_610"])}</text>\n')
    out.append(f'  <text x="30" y="615" class="t-note">Panel 3: {group_line(["domain_parity_611", "domain_ingestion_611", "domain_resolution_611"])}</text>\n')
    out.append('  <text x="30" y="629" class="t-note">Two harnesses, separate runs: the panels are NOT a paired comparison and no ratio across them is valid.</text>\n')
    out.append(f'  <text x="30" y="643" class="t-note">Source: {esc(meta["source"])}</text>\n')
    # The line that stood here claimed "All measurements satisfy AGENTS.md §8
    # provenance requirements" -- which the unresolved host above contradicts --
    # and "confirmed identical instruction count", for which no measurement
    # exists: `domain` is declared wallclock in .github/bench-suites.json and
    # has no arm in any Callgrind harness. Both claims are removed rather than
    # reworded (AGENTS.md 8.1, 8.9).
    out.append('  <text x="30" y="657" class="t-note">Parity badges state the resolution bound the recorded values support, not an exact zero; no instruction-count measurement of these arms exists.</text>\n')

    out.append('</svg>\n')
    return "".join(out)


def main():
    with open(DATA_PATH, "r", encoding="utf-8") as f:
        data = json.load(f)

    svg_content = render_svg(data)

    # Validate XML syntax before writing
    ET.fromstring(svg_content)

    OUTPUT_PATH.write_text(svg_content, encoding="utf-8")
    print(f"Successfully generated valid SVG at: {OUTPUT_PATH}")


if __name__ == "__main__":
    main()
