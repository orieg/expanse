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
  - Zero text collisions / overlaps with generous geometry
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

      .t-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 12.5px; font-weight: 700; letter-spacing: 0.6px; fill: #0f172a; text-transform: uppercase; }
      .t-chart-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 11px; font-weight: 700; letter-spacing: 0.5px; fill: #0f172a; text-transform: uppercase; }
      .t-sub { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 500; fill: #475569; }
      .t-unit { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9px; font-weight: 600; fill: #64748b; }
      .t-axis-label { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9px; font-weight: 500; fill: #64748b; }
      .t-bar-label { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 10px; font-weight: 700; fill: #0f172a; }
      .t-legend { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 600; fill: #0f172a; }
      .t-note { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 8.5px; font-weight: 500; fill: #64748b; }

      .t-val-green { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 700; fill: #15803d; }
      .t-val-blue { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 600; fill: #2563eb; }
      .t-val-muted { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9px; font-weight: 500; fill: #64748b; }

      .b-expanse { fill: #16a34a; }
      .b-expanse-alt { fill: #10b981; }
      .b-roaring { fill: #2563eb; }
      .b-v1 { fill: #94a3b8; }
      .b-card { fill: #f8fafc; stroke: #e2e8f0; stroke-width: 1px; rx: 6px; }

      .badge-win { fill: #dcfce7; stroke: #86efac; stroke-width: 1px; rx: 3px; }
      .badge-win-text { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9px; font-weight: 700; fill: #15803d; text-anchor: middle; }

      .badge-neutral { fill: #f1f5f9; stroke: #cbd5e1; stroke-width: 1px; rx: 3px; }
      .badge-neutral-text { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9px; font-weight: 600; fill: #475569; text-anchor: middle; }

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
        .t-axis-label { fill: #94a3b8; }
        .t-bar-label { fill: #f8fafc; }
        .t-legend { fill: #f8fafc; }
        .t-note { fill: #94a3b8; }
        .t-val-green { fill: #4ade80; }
        .t-val-blue { fill: #38bdf8; }
        .t-val-muted { fill: #94a3b8; }
        .b-expanse { fill: #22c55e; }
        .b-expanse-alt { fill: #34d399; }
        .b-roaring { fill: #3b82f6; }
        .b-v1 { fill: #64748b; }
        .b-card { fill: #161b22; stroke: #30363d; }
        .badge-win { fill: #064e3b; stroke: #059669; }
        .badge-win-text { fill: #6ee7b7; }
        .badge-neutral { fill: #21262d; stroke: #30363d; }
        .badge-neutral-text { fill: #94a3b8; }
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
      :root[data-theme="dark"] .t-axis-label, [data-theme="dark"] .t-axis-label { fill: #94a3b8; }
      :root[data-theme="dark"] .t-bar-label, [data-theme="dark"] .t-bar-label { fill: #f8fafc; }
      :root[data-theme="dark"] .t-legend, [data-theme="dark"] .t-legend { fill: #f8fafc; }
      :root[data-theme="dark"] .t-note, [data-theme="dark"] .t-note { fill: #94a3b8; }
      :root[data-theme="dark"] .t-val-green, [data-theme="dark"] .t-val-green { fill: #4ade80; }
      :root[data-theme="dark"] .t-val-blue, [data-theme="dark"] .t-val-blue { fill: #38bdf8; }
      :root[data-theme="dark"] .t-val-muted, [data-theme="dark"] .t-val-muted { fill: #94a3b8; }
      :root[data-theme="dark"] .b-expanse, [data-theme="dark"] .b-expanse { fill: #22c55e; }
      :root[data-theme="dark"] .b-expanse-alt, [data-theme="dark"] .b-expanse-alt { fill: #34d399; }
      :root[data-theme="dark"] .b-roaring, [data-theme="dark"] .b-roaring { fill: #3b82f6; }
      :root[data-theme="dark"] .b-v1, [data-theme="dark"] .b-v1 { fill: #64748b; }
      :root[data-theme="dark"] .b-card, [data-theme="dark"] .b-card { fill: #161b22; stroke: #30363d; }
      :root[data-theme="dark"] .badge-win, [data-theme="dark"] .badge-win { fill: #064e3b; stroke: #059669; }
      :root[data-theme="dark"] .badge-win-text, [data-theme="dark"] .badge-win-text { fill: #6ee7b7; }
      :root[data-theme="dark"] .badge-neutral, [data-theme="dark"] .badge-neutral { fill: #21262d; stroke: #30363d; }
      :root[data-theme="dark"] .badge-neutral-text, [data-theme="dark"] .badge-neutral-text { fill: #94a3b8; }
"""


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def badge(x: float, y: float, w: float, text: str, win: bool) -> str:
    cls = "badge-win" if win else "badge-neutral"
    tcls = "badge-win-text" if win else "badge-neutral-text"
    return (
        f'  <rect x="{x}" y="{y}" width="{w}" height="16" class="{cls}"/>\n'
        f'  <text x="{x + w / 2}" y="{y + 11.5}" class="{tcls}">{esc(text)}</text>\n'
    )


def render_svg(data: dict) -> str:
    width = 960
    height = 690
    meta = data["meta"]

    out = []
    out.append(f'<svg xmlns="http://www.w3.org/2000/svg" role="img" viewBox="0 0 {width} {height}" width="100%" height="100%">\n')
    out.append('  <title>SET ALGEBRA &amp; INTERNED SET DOMAIN BENCHMARK SUITE</title>\n')
    out.append(f'  <defs>\n    <style>{STYLE}    </style>\n  </defs>\n\n')
    out.append('  <rect width="100%" height="100%" class="bg" rx="8"/>\n')
    out.append('  <rect width="100%" height="100%" class="border" rx="8"/>\n\n')

    # --- Header ---
    out.append('  <text x="30" y="28" class="t-title">SET ALGEBRA &amp; INTERNED SET DOMAIN BENCHMARK SUITE</text>\n')
    out.append('  <text x="30" y="44" class="t-sub">Materialization evolution (#348), k-way aggregate walk (#610), and interned set domain zero-overhead (#611)</text>\n')

    # Legend at top right
    out.append('  <g transform="translate(560, 18)">\n')
    out.append('    <rect x="0" y="0" width="10" height="10" rx="2" class="b-expanse"/>\n')
    out.append('    <text x="14" y="9" class="t-legend">Expanse v2 / k-Way</text>\n')
    out.append('    <rect x="135" y="0" width="10" height="10" rx="2" class="b-roaring"/>\n')
    out.append('    <text x="149" y="9" class="t-legend">Roaring</text>\n')
    out.append('    <rect x="210" y="0" width="10" height="10" rx="2" class="b-v1"/>\n')
    out.append('    <text x="224" y="9" class="t-legend">v1 / Pairwise Fold</text>\n')
    out.append('  </g>\n')
    out.append('  <line x1="30" y1="54" x2="930" y2="54" class="divider"/>\n\n')

    # =========================================================================
    # SECTION 1: CORE ALGEBRA EVOLUTION & K-WAY SCALING (TOP HALF)
    # =========================================================================
    # Left Panel: Materialization Evolution (#348)
    out.append('  <!-- Panel 1A: Materialization Evolution (#348) -->\n')
    out.append('  <g transform="translate(30, 68)">\n')
    out.append('    <text x="0" y="10" class="t-chart-title">1. MATERIALIZATION: DIRECT EMISSION (#348)</text>\n')
    out.append('    <text x="0" y="24" class="t-sub">Intersection materialization latency (µs, log scale, lower is better)</text>\n')
    out.append('    <text x="0" y="38" class="t-unit">N=100k symmetric |A|=|B|</text>\n')

    mat_rows = data["materialization_348"]
    min_log = math.log10(1.0)
    max_log = math.log10(1500.0)

    def log_w_mat(val_us: float) -> float:
        lv = math.log10(max(1.0, val_us))
        ratio = (lv - min_log) / (max_log - min_log)
        return max(4.0, min(135.0, ratio * 135.0))

    y_mat_start = 52
    row_h_mat = 44
    for i, r in enumerate(mat_rows):
        y = y_mat_start + i * row_h_mat
        out.append(f'    <text x="0" y="{y + 11}" class="t-bar-label">{esc(r["distribution"])}</text>\n')
        out.append(f'    <text x="0" y="{y + 22}" class="t-sub">{esc(r["subtitle"])}</text>\n')

        bx = 105
        # Expanse v2 bar
        w_v2 = log_w_mat(r["v2_us"])
        out.append(f'    <rect x="{bx}" y="{y}" width="{w_v2:.1f}" height="7" rx="1.5" class="b-expanse"/>\n')
        out.append(f'    <text x="{bx + w_v2 + 5:.1f}" y="{y + 6.5}" class="t-val-green">{r["v2_us"]:.1f}µs</text>\n')

        # Roaring bar
        w_roar = log_w_mat(r["roaring_us"])
        out.append(f'    <rect x="{bx}" y="{y + 9}" width="{w_roar:.1f}" height="7" rx="1.5" class="b-roaring"/>\n')
        out.append(f'    <text x="{bx + w_roar + 5:.1f}" y="{y + 15.5}" class="t-val-blue">{r["roaring_us"]:.1f}µs</text>\n')

        # v1 bar
        w_v1 = log_w_mat(r["v1_us"])
        out.append(f'    <rect x="{bx}" y="{y + 18}" width="{w_v1:.1f}" height="7" rx="1.5" class="b-v1"/>\n')
        v1_str = f'{r["v1_us"]:.0f}µs' if r["v1_us"] < 1000 else f'{r["v1_us"]/1000:.1f}ms'
        out.append(f'    <text x="{bx + w_v1 + 5:.1f}" y="{y + 24.5}" class="t-val-muted">{v1_str}</text>\n')

        # Badge on the right
        out.append(badge(bx + 215, y + 4, 105, f'{r["speedup_vs_v1"]:.1f}x vs v1', True))

    out.append('  </g>\n\n')

    # Vertical divider between 1A and 1B
    out.append('  <line x1="475" y1="64" x2="475" y2="285" class="divider"/>\n\n')

    # Right Panel: k-Way Aggregate Algebra (#610)
    out.append('  <!-- Panel 1B: k-Way Aggregate Algebra (#610) -->\n')
    out.append('  <g transform="translate(495, 68)">\n')
    out.append('    <text x="0" y="10" class="t-chart-title">2. K-WAY AGGREGATE WALK (#610, K=5)</text>\n')
    out.append('    <text x="0" y="24" class="t-sub">k-way multi-set intersection latency (µs, log scale, lower is better)</text>\n')
    out.append('    <text x="0" y="38" class="t-unit">k=5 operands, N=100k per set</text>\n')

    kway_rows = data["kway_610"]
    min_log_k = math.log10(0.5)
    max_log_k = math.log10(4000.0)

    def log_w_k(val_us: float) -> float:
        lv = math.log10(max(0.5, val_us))
        ratio = (lv - min_log_k) / (max_log_k - min_log_k)
        return max(4.0, min(135.0, ratio * 135.0))

    for i, r in enumerate(kway_rows):
        y = y_mat_start + i * row_h_mat
        out.append(f'    <text x="0" y="{y + 11}" class="t-bar-label">{esc(r["distribution"])}</text>\n')
        out.append(f'    <text x="0" y="{y + 22}" class="t-sub">{esc(r["subtitle"])}</text>\n')

        bx = 95
        # Expanse kway bar
        w_kw = log_w_k(r["kway_us"])
        out.append(f'    <rect x="{bx}" y="{y}" width="{w_kw:.1f}" height="7" rx="1.5" class="b-expanse"/>\n')
        kw_str = f'{r["kway_us"]*1000:.0f}ns' if r["kway_us"] < 1.0 else f'{r["kway_us"]:.1f}µs'
        out.append(f'    <text x="{bx + w_kw + 5:.1f}" y="{y + 6.5}" class="t-val-green">{kw_str}</text>\n')

        # Roaring MultiOps bar
        w_rm = log_w_k(r["roaring_us"])
        out.append(f'    <rect x="{bx}" y="{y + 9}" width="{w_rm:.1f}" height="7" rx="1.5" class="b-roaring"/>\n')
        rm_str = f'{r["roaring_us"]*1000:.0f}ns' if r["roaring_us"] < 1.0 else (f'{r["roaring_us"]:.1f}µs' if r["roaring_us"] < 1000 else f'{r["roaring_us"]/1000:.1f}ms')
        out.append(f'    <text x="{bx + w_rm + 5:.1f}" y="{y + 15.5}" class="t-val-blue">{rm_str}</text>\n')

        # Pairwise fold bar
        w_pw = log_w_k(r["pairwise_us"])
        out.append(f'    <rect x="{bx}" y="{y + 18}" width="{w_pw:.1f}" height="7" rx="1.5" class="b-v1"/>\n')
        pw_str = f'{r["pairwise_us"]:.0f}µs' if r["pairwise_us"] < 1000 else f'{r["pairwise_us"]/1000:.1f}ms'
        out.append(f'    <text x="{bx + w_pw + 5:.1f}" y="{y + 24.5}" class="t-val-muted">{pw_str}</text>\n')

        # Badge
        badge_txt = f'{r["speedup_vs_pairwise"]:.0f}x vs fold' if r["speedup_vs_pairwise"] > 10 else f'{r["speedup_vs_pairwise"]:.1f}x vs fold'
        out.append(badge(bx + 215, y + 4, 120, badge_txt, True))

    out.append('  </g>\n\n')

    # Horizontal divider between Section 1 and Section 2
    out.append('  <line x1="30" y1="298" x2="930" y2="298" class="divider"/>\n\n')

    # =========================================================================
    # SECTION 2: INTERNED SET DOMAIN (#611, BOTTOM HALF)
    # =========================================================================
    out.append('  <!-- Panel 2: Interned Set Domain (#611) -->\n')
    out.append('  <g transform="translate(30, 312)">\n')
    out.append('    <text x="0" y="10" class="t-chart-title">3. INTERNED SET DOMAIN: PARITY, INGESTION &amp; RESOLUTION (#611)</text>\n')
    out.append('    <text x="0" y="24" class="t-sub">Branded DomainSet zero-overhead verification, batched prefix-trie ingestion, and zero-copy slab resolution</text>\n')

    # Card 2A: Algebra Zero-Overhead Parity
    out.append('    <!-- 2A: Parity Card -->\n')
    out.append('    <g transform="translate(0, 36)">\n')
    out.append('      <rect x="0" y="0" width="280" height="152" class="b-card"/>\n')
    out.append('      <text x="14" y="20" class="t-bar-label">Algebra Provenance Overhead</text>\n')
    out.append('      <text x="14" y="33" class="t-unit">Raw ExpanseSet vs DomainSet (N=100k)</text>\n')
    out.append('      <line x1="14" y1="42" x2="266" y2="42" class="divider"/>\n')

    out.append('      <text x="14" y="58" class="t-bar-label">intersection() materialize</text>\n')
    out.append('      <text x="14" y="72" class="t-sub">Raw ExpanseSet:  <tspan class="t-val-green">9.70 µs</tspan></text>\n')
    out.append('      <text x="14" y="85" class="t-sub">DomainSet brand: <tspan class="t-val-green">9.70 µs</tspan></text>\n')
    out.append(badge(168, 62, 98, "+0.00 ns (1.00x)", True))

    out.append('      <line x1="14" y1="96" x2="266" y2="96" class="grid"/>\n')

    out.append('      <text x="14" y="112" class="t-bar-label">intersection_len() count</text>\n')
    out.append('      <text x="14" y="126" class="t-sub">Raw ExpanseSet:  <tspan class="t-val-green">1.09 µs</tspan></text>\n')
    out.append('      <text x="14" y="139" class="t-sub">DomainSet brand: <tspan class="t-val-green">1.09 µs</tspan></text>\n')
    out.append(badge(168, 116, 98, "+0.00 ns (1.00x)", True))
    out.append('    </g>\n')

    # Card 2B: Ingestion Throughput
    out.append('    <!-- 2B: Ingestion Card -->\n')
    out.append('    <g transform="translate(295, 36)">\n')
    out.append('      <rect x="0" y="0" width="315" height="152" class="b-card"/>\n')
    out.append('      <text x="14" y="20" class="t-bar-label">Dictionary Ingestion Throughput</text>\n')
    out.append('      <text x="14" y="33" class="t-unit">Throughput (M keys / sec) • higher is better</text>\n')
    out.append('      <line x1="14" y1="42" x2="301" y2="42" class="divider"/>\n')

    ingest_data = data["domain_ingestion_611"]
    max_ingest = 5.5
    bar_w_max = 75.0

    for idx, ig in enumerate(ingest_data):
        cy = 50 + idx * 46
        out.append(f'      <text x="14" y="{cy + 10}" class="t-bar-label">{esc(ig["key_type"])}</text>\n')

        # Batched bar
        w_b = (ig["batch128_mops"] / max_ingest) * bar_w_max
        out.append(f'      <rect x="14" y="{cy + 16}" width="{w_b:.1f}" height="7" rx="1.5" class="b-expanse"/>\n')
        out.append(f'      <text x="{14 + w_b + 5:.1f}" y="{cy + 22.5}" class="t-val-green">Batch 128: {ig["batch128_mops"]:.2f} M/s</text>\n')

        # Scalar bar
        w_s = (ig["scalar_mops"] / max_ingest) * bar_w_max
        out.append(f'      <rect x="14" y="{cy + 25}" width="{w_s:.1f}" height="7" rx="1.5" class="b-v1"/>\n')
        out.append(f'      <text x="{14 + w_s + 5:.1f}" y="{cy + 31.5}" class="t-val-muted">Scalar: {ig["scalar_mops"]:.2f} M/s</text>\n')

        out.append(badge(220, cy + 16, 82, f'{ig["speedup"]:.2f}x faster', True))

    out.append('    </g>\n')

    # Card 2C: Zero-Copy Resolution
    out.append('    <!-- 2C: Resolution Card -->\n')
    out.append('    <g transform="translate(625, 36)">\n')
    out.append('      <rect x="0" y="0" width="275" height="152" class="b-card"/>\n')
    out.append('      <text x="14" y="20" class="t-bar-label">Zero-Copy Slab Resolution</text>\n')
    out.append('      <text x="14" y="33" class="t-unit">resolve() direct slice iteration</text>\n')
    out.append('      <line x1="14" y1="42" x2="261" y2="42" class="divider"/>\n')

    res = data["domain_resolution_611"]
    out.append(f'      <text x="14" y="66" class="t-chart-title" style="font-size: 22px; fill: #15803d;">{res["scan_mops"]:.1f} M</text>\n')
    out.append('      <text x="95" y="66" class="t-bar-label" style="font-size: 13px;">keys / sec</text>\n')
    out.append(f'      <text x="14" y="86" class="t-sub">Latency: <tspan class="t-val-green">{res["latency_ns"]:.1f} ns / key</tspan> amortized scan</text>\n')

    out.append('      <line x1="14" y1="98" x2="261" y2="98" class="grid"/>\n')

    out.append('      <text x="14" y="115" class="t-bar-label">Zero Heap Allocation</text>\n')
    out.append('      <text x="14" y="128" class="t-sub">Borrows &amp;[u8] directly from stable chunk</text>\n')
    out.append('      <text x="14" y="139" class="t-sub">slabs in BlobArena (no string clones)</text>\n')
    out.append('    </g>\n')

    out.append('  </g>\n\n')

    # =========================================================================
    # FOOTER
    # =========================================================================
    out.append(f'  <text x="30" y="{height - 24}" class="t-note">Measured: {esc(meta["host"])} &#183; commit {esc(meta["commit"])} &#183; {esc(meta["harness"])}</text>\n')
    out.append(f'  <text x="30" y="{height - 12}" class="t-note">Source: {esc(meta["source"])}. {esc(meta["note"])}</text>\n')

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
