#!/usr/bin/env python3
"""
Generates publication-ready, high-contrast, dual-theme SVGs for:
1. bench_native_throughput.svg (Native Criterion Mops/s)
2. bench_ycsb_workloads.svg (YCSB A-F Throughput)
3. bench_tail_latency.svg (P50 to P99.99 Latency Cliff)
4. bench_key_distributions.svg (Ankerl/Tessil Mops/s)
5. bench_memory_footprint.svg (Heap Bytes / Key)
"""

import os
import json
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
RESULTS_DIR = BASE_DIR / "results"

SVG_HEADER_TEMPLATE = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="100%" height="100%">
  <defs>
    <style>
      /* Light theme (default) */
      .bg {{ fill: #ffffff; }}
      .border {{ stroke: #e2e8f0; stroke-width: 1px; fill: none; }}
      .grid {{ stroke: #f1f5f9; stroke-width: 1px; stroke-dasharray: 2,3; }}
      .axis {{ stroke: #cbd5e1; stroke-width: 1.5px; }}
      .divider {{ stroke: #e2e8f0; stroke-width: 1px; }}
      
      .t-title {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11.5px; font-weight: 700; letter-spacing: 0.6px; fill: #0f172a; text-transform: uppercase; }}
      .t-sub {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10px; font-weight: 500; fill: #334155; }}
      .t-unit {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 600; fill: #475569; }}
      .t-tick {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 9.5px; font-weight: 500; fill: #475569; }}
      .t-bar-label {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11px; font-weight: 700; fill: #0f172a; }}
      .t-legend {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10.5px; font-weight: 600; fill: #0f172a; }}

      .b-expanse {{ fill: #16a34a; }}
      .b-hashbrown {{ fill: #2563eb; }}
      .b-btree {{ fill: #64748b; }}
      .b-disqualified {{ fill: #ef4444; stroke: #b91c1c; stroke-width: 1px; stroke-dasharray: 2,2; fill-opacity: 0.15; }}

      .t-val-accent {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10.5px; font-weight: 700; fill: #16a34a; }}
      .t-val-blue {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10.5px; font-weight: 600; fill: #2563eb; }}
      .t-val-muted {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10.5px; font-weight: 600; fill: #334155; }}
      .t-val-warn {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 700; fill: #b91c1c; }}

      .line-exp {{ fill: none; stroke: #16a34a; stroke-width: 2.5px; stroke-linecap: round; }}
      .line-hb {{ fill: none; stroke: #2563eb; stroke-width: 2.5px; stroke-linecap: round; }}
      .line-bt {{ fill: none; stroke: #64748b; stroke-width: 2px; stroke-dasharray: 4,4; }}

      /* Dark theme overrides */
      @media (prefers-color-scheme: dark) {{
        .bg {{ fill: #0d1117; }}
        .border {{ stroke: #30363d; }}
        .grid {{ stroke: #21262d; }}
        .axis {{ stroke: #484f58; }}
        .divider {{ stroke: #21262d; }}
        .t-title {{ fill: #f0f6fc; }}
        .t-sub {{ fill: #94a3b8; }}
        .t-unit {{ fill: #94a3b8; }}
        .t-tick {{ fill: #94a3b8; }}
        .t-bar-label {{ fill: #f8fafc; }}
        .t-legend {{ fill: #f8fafc; }}
        
        .b-expanse {{ fill: #22c55e; }}
        .b-hashbrown {{ fill: #3b82f6; }}
        .b-btree {{ fill: #475569; }}
        .b-disqualified {{ fill: #ef4444; stroke: #f87171; fill-opacity: 0.25; }}

        .t-val-accent {{ fill: #4ade80; }}
        .t-val-blue {{ fill: #38bdf8; }}
        .t-val-muted {{ fill: #e2e8f0; }}
        .t-val-warn {{ fill: #f87171; }}

        .line-exp {{ stroke: #22c55e; }}
        .line-hb {{ stroke: #38bdf8; }}
        .line-bt {{ stroke: #94a3b8; }}
      }}

      [data-theme="dark"] .bg, :root[data-theme="dark"] .bg {{ fill: #0d1117; }}
      [data-theme="dark"] .border, :root[data-theme="dark"] .border {{ stroke: #30363d; }}
      [data-theme="dark"] .grid, :root[data-theme="dark"] .grid {{ stroke: #21262d; }}
      [data-theme="dark"] .axis, :root[data-theme="dark"] .axis {{ stroke: #484f58; }}
      [data-theme="dark"] .divider, :root[data-theme="dark"] .divider {{ stroke: #21262d; }}
      [data-theme="dark"] .t-title, :root[data-theme="dark"] .t-title {{ fill: #f0f6fc; }}
      [data-theme="dark"] .t-sub, :root[data-theme="dark"] .t-sub {{ fill: #94a3b8; }}
      [data-theme="dark"] .t-unit, :root[data-theme="dark"] .t-unit {{ fill: #94a3b8; }}
      [data-theme="dark"] .t-tick, :root[data-theme="dark"] .t-tick {{ fill: #94a3b8; }}
      [data-theme="dark"] .t-bar-label, :root[data-theme="dark"] .t-bar-label {{ fill: #f8fafc; }}
      [data-theme="dark"] .t-legend, :root[data-theme="dark"] .t-legend {{ fill: #f8fafc; }}
      [data-theme="dark"] .b-expanse, :root[data-theme="dark"] .b-expanse {{ fill: #22c55e; }}
      [data-theme="dark"] .b-hashbrown, :root[data-theme="dark"] .b-hashbrown {{ fill: #3b82f6; }}
      [data-theme="dark"] .b-btree, :root[data-theme="dark"] .b-btree {{ fill: #475569; }}
      [data-theme="dark"] .t-val-accent, :root[data-theme="dark"] .t-val-accent {{ fill: #4ade80; }}
      [data-theme="dark"] .t-val-blue, :root[data-theme="dark"] .t-val-blue {{ fill: #38bdf8; }}
      [data-theme="dark"] .t-val-muted, :root[data-theme="dark"] .t-val-muted {{ fill: #e2e8f0; }}
      [data-theme="dark"] .t-val-warn, :root[data-theme="dark"] .t-val-warn {{ fill: #f87171; }}
    </style>
  </defs>
  <rect width="100%" height="100%" rx="6" class="bg"/>
  <rect width="100%" height="100%" rx="6" class="border"/>
"""

def generate_ycsb_chart():
    json_path = RESULTS_DIR / "baseline_ycsb.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = json.load(f)

    svg = SVG_HEADER_TEMPLATE.format(width=880, height=360)
    svg += """
  <!-- Header -->
  <text x="24" y="32" class="t-title">YCSB WORKLOADS A–F THROUGHPUT (MOPS/SEC)</text>
  <text x="24" y="48" class="t-sub">500,000 Key Population • Zipfian Skew (s=0.99) • Higher is better</text>

  <!-- Legend -->
  <g transform="translate(560, 24)">
    <rect x="0" y="0" width="14" height="14" rx="2" class="b-expanse"/>
    <text x="20" y="11" class="t-legend">ExpanseMap</text>
    <rect x="110" y="0" width="14" height="14" rx="2" class="b-hashbrown"/>
    <text x="130" y="11" class="t-legend">hashbrown</text>
    <rect x="220" y="0" width="14" height="14" rx="2" class="b-btree"/>
    <text x="240" y="11" class="t-legend">BTreeMap</text>
  </g>
  <line x1="24" y1="60" x2="856" y2="60" class="divider"/>
"""

    workloads = ["workload_a", "workload_b", "workload_c", "workload_d", "workload_e", "workload_f"]
    y_start = 85
    max_val = 260.0

    for i, wl in enumerate(workloads):
        item = data.get(wl, {})
        y = y_start + i * 44
        label = item.get("workload", wl)
        desc = item.get("description", "")
        
        m_exp = item.get("expanse_mops", 0.0)
        m_hb = item.get("hashbrown_mops")
        m_bt = item.get("btree_mops", 0.0)

        # Draw row label
        svg += f"""  <text x="24" y="{y + 14}" class="t-bar-label">{label}</text>
  <text x="24" y="{y + 26}" class="t-sub">{desc[:38]}</text>
"""
        # Bars region: x=300 to x=720 (width=420)
        w_exp = max(2, int((m_exp / max_val) * 400))
        w_bt = max(2, int((m_bt / max_val) * 400))

        svg += f"""  <rect x="300" y="{y}" width="{w_exp}" height="8" rx="2" class="b-expanse"/>
  <text x="{305 + w_exp}" y="{y + 7}" class="t-val-accent">{m_exp:.1f}M</text>
"""
        if m_hb is not None:
            w_hb = max(2, int((m_hb / max_val) * 400))
            svg += f"""  <rect x="300" y="{y + 10}" width="{w_hb}" height="8" rx="2" class="b-hashbrown"/>
  <text x="{305 + w_hb}" y="{y + 17}" class="t-val-blue">{m_hb:.1f}M</text>
"""
        else:
            svg += f"""  <rect x="300" y="{y + 10}" width="160" height="8" rx="2" class="b-disqualified"/>
  <text x="470" y="{y + 17}" class="t-val-warn">DISQUALIFIED (No Range Scan)</text>
"""

        svg += f"""  <rect x="300" y="{y + 20}" width="{w_bt}" height="8" rx="2" class="b-btree"/>
  <text x="{305 + w_bt}" y="{y + 27}" class="t-val-muted">{m_bt:.1f}M</text>
"""

    svg += "</svg>\n"
    out_file = RESULTS_DIR / "bench_ycsb_workloads.svg"
    with open(out_file, "w") as f:
        f.write(svg)
    print(f"Generated {out_file}")

def generate_memory_chart():
    json_path = RESULTS_DIR / "baseline_memory.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = json.load(f)

    svg = SVG_HEADER_TEMPLATE.format(width=880, height=280)
    svg += """
  <!-- Header -->
  <text x="24" y="32" class="t-title">MEMORY FOOTPRINT: BYTES PER KEY (LOWER IS BETTER)</text>
  <text x="24" y="48" class="t-sub">Measured via GlobalAlloc Live Heap Tracker • Sequential vs. Random 64-bit Keys</text>

  <!-- Legend -->
  <g transform="translate(560, 24)">
    <rect x="0" y="0" width="14" height="14" rx="2" class="b-expanse"/>
    <text x="20" y="11" class="t-legend">ExpanseMap</text>
    <rect x="110" y="0" width="14" height="14" rx="2" class="b-hashbrown"/>
    <text x="130" y="11" class="t-legend">hashbrown</text>
    <rect x="220" y="0" width="14" height="14" rx="2" class="b-btree"/>
    <text x="240" y="11" class="t-legend">BTreeMap</text>
  </g>
  <line x1="24" y1="60" x2="856" y2="60" class="divider"/>
"""

    # We take the largest population measured (e.g. 100k or 500k)
    last_item = data[-1]
    pop = last_item.get("population", 100000)
    rand_data = last_item.get("random_keys_bytes_per_key", {})
    seq_data = last_item.get("sequential_keys_bytes_per_key", {})

    cases = [
        ("Sequential Keys (0..N)", seq_data, 90),
        ("Uniform Random 64-bit Keys", rand_data, 175),
    ]

    max_bpk = 40.0

    for title, m_dict, y in cases:
        exp_b = m_dict.get("expanse", 0.0)
        hb_b = m_dict.get("hashbrown", 0.0)
        bt_b = m_dict.get("btree", 0.0)

        svg += f"""  <text x="24" y="{y + 20}" class="t-bar-label">{title}</text>
  <text x="24" y="{y + 36}" class="t-sub">N = {pop:,} Keys</text>
"""
        w_exp = int((exp_b / max_bpk) * 380)
        w_hb = int((hb_b / max_bpk) * 380)
        w_bt = int((bt_b / max_bpk) * 380)

        svg += f"""  <!-- Expanse -->
  <rect x="260" y="{y}" width="{w_exp}" height="14" rx="2" class="b-expanse"/>
  <text x="{265 + w_exp}" y="{y + 11}" class="t-val-accent">{exp_b:.1f} B/key</text>

  <!-- Hashbrown -->
  <rect x="260" y="{y + 18}" width="{w_hb}" height="14" rx="2" class="b-hashbrown"/>
  <text x="{265 + w_hb}" y="{y + 29}" class="t-val-blue">{hb_b:.1f} B/key</text>

  <!-- BTreeMap -->
  <rect x="260" y="{y + 36}" width="{w_bt}" height="14" rx="2" class="b-btree"/>
  <text x="{265 + w_bt}" y="{y + 47}" class="t-val-muted">{bt_b:.1f} B/key</text>
"""

    svg += "</svg>\n"
    out_file = RESULTS_DIR / "bench_memory_footprint.svg"
    with open(out_file, "w") as f:
        f.write(svg)
    print(f"Generated {out_file}")

def generate_tail_latency_chart():
    json_path = RESULTS_DIR / "baseline_tail_latency.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = json.load(f)

    svg = SVG_HEADER_TEMPLATE.format(width=880, height=280)
    svg += """
  <!-- Header -->
  <text x="24" y="32" class="t-title">INGESTION TAIL LATENCY: P50 TO P99.99 (NANOSECONDS, LOG SCALE)</text>
  <text x="24" y="48" class="t-sub">Dynamic Ingestion without Pre-allocation • Captures SwissTable Rehash Cliff</text>

  <!-- Legend -->
  <g transform="translate(560, 24)">
    <line x1="0" y1="8" x2="16" y2="8" class="line-exp"/>
    <text x="22" y="11" class="t-legend">ExpanseMap</text>
    <line x1="110" y1="8" x2="126" y2="8" class="line-hb"/>
    <text x="132" y="11" class="t-legend">hashbrown</text>
    <line x1="220" y1="8" x2="236" y2="8" class="line-bt"/>
    <text x="242" y="11" class="t-legend">BTreeMap</text>
  </g>
  <line x1="24" y1="60" x2="856" y2="60" class="divider"/>
"""

    exp = data.get("expanse", {})
    hb = data.get("hashbrown", {})
    bt = data.get("btree", {})

    quants = ["p50_ns", "p75_ns", "p90_ns", "p95_ns", "p99_ns", "p99_9_ns", "p99_99_ns"]
    q_labels = ["P50", "P75", "P90", "P95", "P99", "P99.9", "P99.99"]

    # Table layout for clean rendering
    y_hdr = 90
    svg += f"""  <text x="40" y="{y_hdr}" class="t-unit">Percentile</text>
  <text x="180" y="{y_hdr}" class="t-val-accent">ExpanseMap (ns)</text>
  <text x="380" y="{y_hdr}" class="t-val-blue">hashbrown (ns)</text>
  <text x="580" y="{y_hdr}" class="t-val-muted">BTreeMap (ns)</text>
  <line x1="40" y1="{y_hdr + 8}" x2="840" y2="{y_hdr + 8}" class="divider"/>
"""

    for i, (q, ql) in enumerate(zip(quants, q_labels)):
        y = y_hdr + 24 + i * 22
        e_v = exp.get(q, 0)
        h_v = hb.get(q, 0)
        b_v = bt.get(q, 0)

        svg += f"""  <text x="40" y="{y}" class="t-bar-label">{ql}</text>
  <text x="180" y="{y}" class="t-val-accent">{e_v:,} ns</text>
  <text x="380" y="{y}" class="t-val-blue">{h_v:,} ns</text>
  <text x="580" y="{y}" class="t-val-muted">{b_v:,} ns</text>
  <line x1="40" y1="{y + 4}" x2="840" y2="{y + 4}" class="grid"/>
"""

    svg += "</svg>\n"
    out_file = RESULTS_DIR / "bench_tail_latency.svg"
    with open(out_file, "w") as f:
        f.write(svg)
    print(f"Generated {out_file}")

def generate_distributions_chart():
    json_path = RESULTS_DIR / "baseline_distributions.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = json.load(f)

    svg = SVG_HEADER_TEMPLATE.format(width=880, height=300)
    svg += """
  <!-- Header -->
  <text x="24" y="32" class="t-title">MARTIN ANKERL / TESSIL CONTAINER KEY DISTRIBUTIONS (LOOKUP MOPS/SEC)</text>
  <text x="24" y="48" class="t-sub">50,000 Population • Point Query Throughput across Key Geometries</text>

  <!-- Legend -->
  <g transform="translate(560, 24)">
    <rect x="0" y="0" width="14" height="14" rx="2" class="b-expanse"/>
    <text x="20" y="11" class="t-legend">ExpanseMap</text>
    <rect x="110" y="0" width="14" height="14" rx="2" class="b-hashbrown"/>
    <text x="130" y="11" class="t-legend">hashbrown</text>
    <rect x="220" y="0" width="14" height="14" rx="2" class="b-btree"/>
    <text x="240" y="11" class="t-legend">BTreeMap</text>
  </g>
  <line x1="24" y1="60" x2="856" y2="60" class="divider"/>
"""

    dists = ["sequential", "clustered", "zipfian", "uniform"]
    dist_names = {
        "sequential": "Dense Sequential (0..N)",
        "clustered": "Sparse Clustered / Stride",
        "zipfian": "Zipfian Skewed (s=0.99)",
        "uniform": "Uniform Random 64-bit",
    }

    y_start = 85
    max_val = 150.0

    for i, d in enumerate(dists):
        item = data.get(d, {})
        y = y_start + i * 48
        name = dist_names.get(d, d)
        lookups = item.get("lookup_mops", {})

        m_exp = lookups.get("expanse", 0.0)
        m_hb = lookups.get("hashbrown", 0.0)
        m_bt = lookups.get("btree", 0.0)

        svg += f"""  <text x="24" y="{y + 18}" class="t-bar-label">{name}</text>
"""
        w_exp = max(2, int((m_exp / max_val) * 400))
        w_hb = max(2, int((m_hb / max_val) * 400))
        w_bt = max(2, int((m_bt / max_val) * 400))

        svg += f"""  <rect x="280" y="{y}" width="{w_exp}" height="8" rx="2" class="b-expanse"/>
  <text x="{285 + w_exp}" y="{y + 7}" class="t-val-accent">{m_exp:.1f}M</text>

  <rect x="280" y="{y + 10}" width="{w_hb}" height="8" rx="2" class="b-hashbrown"/>
  <text x="{285 + w_hb}" y="{y + 17}" class="t-val-blue">{m_hb:.1f}M</text>

  <rect x="280" y="{y + 20}" width="{w_bt}" height="8" rx="2" class="b-btree"/>
  <text x="{285 + w_bt}" y="{y + 27}" class="t-val-muted">{m_bt:.1f}M</text>
"""

    svg += "</svg>\n"
    out_file = RESULTS_DIR / "bench_key_distributions.svg"
    with open(out_file, "w") as f:
        f.write(svg)
    print(f"Generated {out_file}")

def generate_native_chart():
    json_path = RESULTS_DIR / "baseline_native.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = json.load(f)

    svg = SVG_HEADER_TEMPLATE.format(width=880, height=280)
    svg += """
  <!-- Header -->
  <text x="24" y="32" class="t-title">NATIVE HASHBROWN CRITERION SUITE: LOOKUP & INGESTION (MOPS/SEC)</text>
  <text x="24" y="48" class="t-sub">Ported from hashbrown/benches/bench.rs • 100,000 Key Population</text>

  <!-- Legend -->
  <g transform="translate(560, 24)">
    <rect x="0" y="0" width="14" height="14" rx="2" class="b-expanse"/>
    <text x="20" y="11" class="t-legend">ExpanseMap</text>
    <rect x="110" y="0" width="14" height="14" rx="2" class="b-hashbrown"/>
    <text x="130" y="11" class="t-legend">hashbrown</text>
    <rect x="220" y="0" width="14" height="14" rx="2" class="b-btree"/>
    <text x="240" y="11" class="t-legend">BTreeMap</text>
  </g>
  <line x1="24" y1="60" x2="856" y2="60" class="divider"/>
"""

    last_item = data[-1]
    hit = last_item.get("lookup_hit", {})
    miss = last_item.get("lookup_miss", {})
    grow = last_item.get("insert_growing", {})

    cases = [
        ("Point Lookup Hit (Present Key)", hit, 85),
        ("Point Lookup Miss (Absent Key)", miss, 140),
        ("Insert Growing (Un-preallocated)", grow, 195),
    ]

    max_val = 70.0

    for title, subdict, y in cases:
        exp_m = subdict.get("expanse", {}).get("mops", 0.0) if "mops" in subdict.get("expanse", {}) else subdict.get("expanse", {}).get("mops", 0.0)
        if isinstance(subdict.get("expanse"), (int, float)):
            exp_m = subdict.get("expanse")
        hb_m = subdict.get("hashbrown", {}).get("mops", 0.0) if "mops" in subdict.get("hashbrown", {}) else subdict.get("hashbrown", {}).get("mops", 0.0)
        if isinstance(subdict.get("hashbrown"), (int, float)):
            hb_m = subdict.get("hashbrown")
        bt_m = subdict.get("btree", {}).get("mops", 0.0) if "mops" in subdict.get("btree", {}) else subdict.get("btree", {}).get("mops", 0.0)
        if isinstance(subdict.get("btree"), (int, float)):
            bt_m = subdict.get("btree")

        svg += f"""  <text x="24" y="{y + 14}" class="t-bar-label">{title}</text>
"""
        w_exp = max(2, int((exp_m / max_val) * 380))
        w_hb = max(2, int((hb_m / max_val) * 380))
        w_bt = max(2, int((bt_m / max_val) * 380))

        svg += f"""  <rect x="280" y="{y}" width="{w_exp}" height="8" rx="2" class="b-expanse"/>
  <text x="{285 + w_exp}" y="{y + 7}" class="t-val-accent">{exp_m:.1f}M</text>

  <rect x="280" y="{y + 10}" width="{w_hb}" height="8" rx="2" class="b-hashbrown"/>
  <text x="{285 + w_hb}" y="{y + 17}" class="t-val-blue">{hb_m:.1f}M</text>

  <rect x="280" y="{y + 20}" width="{w_bt}" height="8" rx="2" class="b-btree"/>
  <text x="{285 + w_bt}" y="{y + 27}" class="t-val-muted">{bt_m:.1f}M</text>
"""

    svg += "</svg>\n"
    out_file = RESULTS_DIR / "bench_native_throughput.svg"
    with open(out_file, "w") as f:
        f.write(svg)
    print(f"Generated {out_file}")

def main():
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    generate_ycsb_chart()
    generate_memory_chart()
    generate_tail_latency_chart()
    generate_distributions_chart()
    generate_native_chart()

if __name__ == "__main__":
    main()
