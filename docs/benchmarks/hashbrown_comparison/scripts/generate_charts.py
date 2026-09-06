#!/usr/bin/env python3
"""
docs/benchmarks/hashbrown_comparison/scripts/generate_charts.py

Publication-ready, dual-theme SVG chart generator with dynamic scaling and zero overflow:
1. bench_native_throughput.svg (3-panel vertical bar chart)
2. bench_ycsb_workloads.svg (Grouped horizontal bars with badges)
3. bench_memory_footprint.svg (2-panel memory density comparison)
4. bench_key_distributions.svg (Key geometry throughput bars)
5. bench_tail_latency.svg (Tail latency comparison table and chart)
"""

import json
import sys
import math
import xml.etree.ElementTree as ET
from pathlib import Path
from theme import svg_header

# Artifacts written before #732 are a bare JSON array; ones written after are
# `{"provenance": ..., "cells": [...]}`. `body()` accepts both, so a generator
# keeps working against either.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent.parent.parent / "scripts"))
from bench_provenance import body  # noqa: E402

BASE_DIR = Path(__file__).resolve().parent.parent
RESULTS_DIR = BASE_DIR / "results"

def save_and_validate_svg(filepath: Path, content: str):
    """Validates XML syntax before writing to ensure zero parse errors."""
    try:
        ET.fromstring(content)
    except ET.ParseError as err:
        print(f"XML Validation Error in {filepath.name}: {err}")
        raise err

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Generated & Validated: {filepath}")

def generate_native_chart():
    json_path = RESULTS_DIR / "baseline_native.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = body(json.load(f))

    # Use the last (largest) population entry
    item = data[-1]
    pop = item.get("population", 0)
    hit = item.get("lookup_hit", {})
    miss = item.get("lookup_miss", {})
    grow = item.get("insert_growing", {})

    # Extract Mops
    hit_exp = hit.get("expanse", {}).get("mops", 0.0)
    hit_hb = hit.get("hashbrown", {}).get("mops", 0.0)
    hit_bt = hit.get("btree", {}).get("mops", 0.0)

    miss_exp = miss.get("expanse", {}).get("mops", 0.0)
    miss_hb = miss.get("hashbrown", {}).get("mops", 0.0)
    miss_bt = miss.get("btree", {}).get("mops", 0.0)

    grow_exp = grow.get("expanse", {}).get("mops", 0.0)
    grow_hb = grow.get("hashbrown", {}).get("mops", 0.0)
    grow_bt = grow.get("btree", {}).get("mops", 0.0)

    svg = svg_header(width=960, height=290, title="Native Criterion Benchmarks")

    # Legend along the bottom: the top-right band is occupied by the third
    # panel's title/subtitle (translate(660, 20)), so a legend there overlaps it.
    svg += """
  <!-- Legend -->
  <g transform="translate(335, 266)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">ExpanseMap</text>
    <rect x="105" y="0" width="12" height="12" rx="2" class="b-hashbrown"/>
    <text x="123" y="10" class="t-legend">hashbrown</text>
    <rect x="210" y="0" width="12" height="12" rx="2" class="b-btree"/>
    <text x="228" y="10" class="t-legend">BTreeMap</text>
  </g>
"""

    panels = [
        ("Point Lookup Hit", f"{pop:,} keys random query", hit_exp, hit_hb, hit_bt, 30),
        ("Point Lookup Miss", f"{pop:,} absent keys query", miss_exp, miss_hb, miss_bt, 345),
        ("Dynamic Ingestion", f"Growth 0 -&gt; {pop:,} keys", grow_exp, grow_hb, grow_bt, 660),
    ]

    for title, sub, m_exp, m_hb, m_bt, x_off in panels:
        max_measured = max(m_exp, m_hb, m_bt, 1.0)
        max_y = math.ceil(max_measured * 1.25 / 10.0) * 10.0

        svg += f"""
  <!-- Panel: {title} -->
  <g transform="translate({x_off}, 20)">
    <text x="0" y="0" class="t-title">{title}</text>
    <text x="0" y="13" class="t-sub">{sub}</text>
    <text x="0" y="27" class="t-unit">&#9650; Throughput (M ops / sec)</text>

    <!-- Grid lines -->
    <line x1="30" y1="45" x2="260" y2="45" class="grid"/>
    <text x="24" y="48" class="t-axis-label" text-anchor="end">{max_y:.0f}M</text>

    <line x1="30" y1="115" x2="260" y2="115" class="grid"/>
    <text x="24" y="118" class="t-axis-label" text-anchor="end">{max_y/2:.0f}M</text>

    <line x1="30" y1="185" x2="260" y2="185" class="axis"/>
    <text x="24" y="188" class="t-axis-label" text-anchor="end">0</text>
    <line x1="30" y1="40" x2="30" y2="185" class="axis"/>
"""
        # Heights
        h_scale = 135.0 / max_y
        h_exp = min(135.0, max(2.0, m_exp * h_scale))
        h_hb = min(135.0, max(2.0, m_hb * h_scale))
        h_bt = min(135.0, max(2.0, m_bt * h_scale))

        y_exp = 185 - h_exp
        y_hb = 185 - h_hb
        y_bt = 185 - h_bt

        svg += f"""
    <!-- Bar 1: Expanse -->
    <rect x="45" y="{y_exp:.1f}" width="55" height="{h_exp:.1f}" class="b-expanse" rx="2"/>
    <text x="72.5" y="{y_exp - 6:.1f}" class="t-val-accent" text-anchor="middle">{m_exp:.1f}M</text>
    <text x="72.5" y="202" class="t-bar-label" text-anchor="middle">Expanse</text>

    <!-- Bar 2: Hashbrown -->
    <rect x="120" y="{y_hb:.1f}" width="55" height="{h_hb:.1f}" class="b-hashbrown" rx="2"/>
    <text x="147.5" y="{y_hb - 6:.1f}" class="t-val-blue" text-anchor="middle">{m_hb:.1f}M</text>
    <text x="147.5" y="202" class="t-bar-label" text-anchor="middle">hashbrown</text>

    <!-- Bar 3: BTreeMap -->
    <rect x="195" y="{y_bt:.1f}" width="55" height="{h_bt:.1f}" class="b-btree" rx="2"/>
    <text x="222.5" y="{y_bt - 6:.1f}" class="t-val-muted" text-anchor="middle">{m_bt:.1f}M</text>
    <text x="222.5" y="202" class="t-bar-label" text-anchor="middle">BTreeMap</text>
  </g>
"""

    svg += """
  <line x1="320" y1="20" x2="320" y2="260" class="divider"/>
  <line x1="635" y1="20" x2="635" y2="260" class="divider"/>
</svg>
"""
    save_and_validate_svg(RESULTS_DIR / "bench_native_throughput.svg", svg)

def generate_ycsb_chart():
    json_path = RESULTS_DIR / "baseline_ycsb.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = body(json.load(f))

    all_vals = []
    for wl_data in data.values():
        all_vals.append(wl_data.get("expanse_mops", 0.0))
        all_vals.append(wl_data.get("btree_mops", 0.0))
        if wl_data.get("hashbrown_mops") is not None:
            all_vals.append(wl_data.get("hashbrown_mops", 0.0))
    max_measured = max(all_vals) if all_vals else 100.0
    max_val = max_measured * 1.25
    bar_max_width = 330.0

    svg = svg_header(width=960, height=380, title="YCSB Workloads A-F Throughput")
    svg += """
  <!-- Header -->
  <text x="30" y="30" class="t-title">YCSB WORKLOADS A–F THROUGHPUT (MOPS/SEC)</text>
  <text x="30" y="46" class="t-sub">500,000 Key Population • Zipfian Skew (s=0.99) • Higher is better</text>

  <!-- Legend -->
  <g transform="translate(630, 20)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">ExpanseMap</text>
    <rect x="105" y="0" width="12" height="12" rx="2" class="b-hashbrown"/>
    <text x="123" y="10" class="t-legend">hashbrown</text>
    <rect x="210" y="0" width="12" height="12" rx="2" class="b-btree"/>
    <text x="228" y="10" class="t-legend">BTreeMap</text>
  </g>
  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>
"""

    workloads = ["workload_a", "workload_b", "workload_c", "workload_d", "workload_e", "workload_f"]
    y_start = 82

    for i, wl in enumerate(workloads):
        item = data.get(wl, {})
        y = y_start + i * 48
        label = item.get("workload", wl)
        desc = item.get("description", "")
        
        m_exp = item.get("expanse_mops", 0.0)
        m_hb = item.get("hashbrown_mops")
        m_bt = item.get("btree_mops", 0.0)

        svg += f"""  <text x="30" y="{y + 13}" class="t-bar-label">{label}</text>
  <text x="30" y="{y + 26}" class="t-sub">{desc}</text>
"""
        w_exp = max(3.0, (m_exp / max_val) * bar_max_width)
        w_bt = max(3.0, (m_bt / max_val) * bar_max_width)

        # Expanse Bar
        svg += f"""  <rect x="310" y="{y}" width="{w_exp:.1f}" height="9" rx="2" class="b-expanse"/>
  <text x="{318 + w_exp:.1f}" y="{y + 8}" class="t-val-accent" text-anchor="start">{m_exp:.1f}M</text>
"""
        # Hashbrown Bar
        if m_hb is not None:
            w_hb = max(3.0, (m_hb / max_val) * bar_max_width)
            svg += f"""  <rect x="310" y="{y + 12}" width="{w_hb:.1f}" height="9" rx="2" class="b-hashbrown"/>
  <text x="{318 + w_hb:.1f}" y="{y + 20}" class="t-val-blue" text-anchor="start">{m_hb:.1f}M</text>
"""
        else:
            svg += f"""  <rect x="310" y="{y + 12}" width="160" height="9" rx="2" class="b-disqualified"/>
  <rect x="485" y="{y + 9}" width="170" height="15" class="badge-disq"/>
  <text x="570" y="{y + 20}" class="badge-disq-text">DISQUALIFIED (No Range Scan)</text>
"""

        # BTreeMap Bar
        svg += f"""  <rect x="310" y="{y + 24}" width="{w_bt:.1f}" height="9" rx="2" class="b-btree"/>
  <text x="{318 + w_bt:.1f}" y="{y + 32}" class="t-val-muted" text-anchor="start">{m_bt:.1f}M</text>
"""
        # Ratio badge vs BTreeMap. A ratio below 1.0 is a LOSS and must not be
        # styled or rounded as a win (Workload E measures 0.96x here, which
        # `{:.1f}x` renders as a green "1.0x").
        if m_bt > 0:
            speedup = m_exp / m_bt
            if speedup >= 1.0:
                badge_text = f"{speedup:.2f}x vs BTree"
                badge_cls, text_cls = "badge-win", "badge-win-text"
            else:
                badge_text = f"BTree {1.0 / speedup:.2f}x faster"
                badge_cls, text_cls = "badge-loss", "badge-loss-text"
            svg += f"""  <rect x="800" y="{y + 10}" width="130" height="18" class="{badge_cls}"/>
  <text x="865" y="{y + 23}" class="{text_cls}">{badge_text}</text>
"""

    svg += "</svg>\n"
    save_and_validate_svg(RESULTS_DIR / "bench_ycsb_workloads.svg", svg)

def generate_memory_chart():
    json_path = RESULTS_DIR / "baseline_memory.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = body(json.load(f))

    last_item = data[-1]
    pop = last_item.get("population", 0)
    rand_data = last_item.get("random_keys_bytes_per_key", {})
    seq_data = last_item.get("sequential_keys_bytes_per_key", {})

    svg = svg_header(width=960, height=280, title="Memory Footprint: Bytes Per Key")
    svg += f"""
  <!-- Header -->
  <text x="30" y="30" class="t-title">MEMORY FOOTPRINT: BYTES PER KEY (LOWER IS BETTER)</text>
  <text x="30" y="46" class="t-sub">Measured via GlobalAlloc Live Heap Tracker • N = {pop:,} Keys</text>

  <!-- Legend -->
  <g transform="translate(630, 20)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">ExpanseMap</text>
    <rect x="105" y="0" width="12" height="12" rx="2" class="b-hashbrown"/>
    <text x="123" y="10" class="t-legend">hashbrown</text>
    <rect x="210" y="0" width="12" height="12" rx="2" class="b-btree"/>
    <text x="228" y="10" class="t-legend">BTreeMap</text>
  </g>
  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>
"""

    panels = [
        ("Dense Sequential Keys (0..N)", "Bitmap &amp; Uncompressed Leaf Packing", seq_data, 30, True),
        ("Uniform Random 64-bit Keys", "High entropy sparse distribution", rand_data, 500, False),
    ]

    max_bpk_scale = 40.0
    panel_bar_max_w = 200.0

    for title, sub, m_dict, x_off, is_seq in panels:
        exp_b = m_dict.get("expanse", 0.0)
        hb_b = m_dict.get("hashbrown", 0.0)
        bt_b = m_dict.get("btree", 0.0)

        w_exp = min(panel_bar_max_w, max(3.0, (exp_b / max_bpk_scale) * panel_bar_max_w))
        w_hb = min(panel_bar_max_w, max(3.0, (hb_b / max_bpk_scale) * panel_bar_max_w))
        w_bt = min(panel_bar_max_w, max(3.0, (bt_b / max_bpk_scale) * panel_bar_max_w))

        svg += f"""
  <!-- Panel: {title} -->
  <g transform="translate({x_off}, 75)">
    <text x="0" y="0" class="t-title">{title}</text>
    <text x="0" y="14" class="t-sub">{sub}</text>

    <!-- Expanse Row -->
    <text x="0" y="44" class="t-bar-label">ExpanseMap</text>
    <rect x="105" y="33" width="{w_exp:.1f}" height="14" rx="2" class="b-expanse"/>
    <text x="{113 + w_exp:.1f}" y="44" class="t-val-accent">{exp_b:.1f} B/key</text>

    <!-- Hashbrown Row -->
    <text x="0" y="74" class="t-bar-label">hashbrown</text>
    <rect x="105" y="63" width="{w_hb:.1f}" height="14" rx="2" class="b-hashbrown"/>
    <text x="{113 + w_hb:.1f}" y="74" class="t-val-blue">{hb_b:.1f} B/key</text>

    <!-- BTreeMap Row -->
    <text x="0" y="104" class="t-bar-label">BTreeMap</text>
    <rect x="105" y="93" width="{w_bt:.1f}" height="14" rx="2" class="b-btree"/>
    <text x="{113 + w_bt:.1f}" y="104" class="t-val-muted">{bt_b:.1f} B/key</text>
"""
        if is_seq:
            ratio = hb_b / exp_b if exp_b > 0 else 1.0
            svg += f"""
    <!-- Speedup Badge -->
    <rect x="0" y="132" width="240" height="24" class="badge-win"/>
    <text x="120" y="148" class="badge-win-text">&#10003; {ratio:.1f}x More Compact than Hashbrown</text>
"""
        else:
            svg += f"""
    <text x="0" y="148" class="t-note">Dynamic Radix tree allocates branch nodes per 8-bit digit.</text>
"""

        svg += "  </g>\n"

    svg += """  <line x1="475" y1="70" x2="475" y2="255" class="divider"/>
</svg>
"""
    save_and_validate_svg(RESULTS_DIR / "bench_memory_footprint.svg", svg)

def generate_key_distributions_chart():
    json_path = RESULTS_DIR / "baseline_distributions.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = body(json.load(f))

    all_vals = []
    for d_data in data.values():
        lookups = d_data.get("lookup_mops", {})
        all_vals.append(lookups.get("expanse", 0.0))
        all_vals.append(lookups.get("hashbrown", 0.0))
        all_vals.append(lookups.get("btree", 0.0))
    max_measured = max(all_vals) if all_vals else 100.0
    max_val = max_measured * 1.25
    bar_max_width = 330.0

    svg = svg_header(width=960, height=340, title="Container Key Distributions")
    svg += """
  <!-- Header -->
  <text x="30" y="30" class="t-title">MARTIN ANKERL &amp; TESSIL KEY DISTRIBUTIONS (LOOKUP MOPS/SEC)</text>
  <text x="30" y="46" class="t-sub">50,000 Key Population • Point Query Throughput across Key Geometries</text>

  <!-- Legend -->
  <g transform="translate(630, 20)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">ExpanseMap</text>
    <rect x="105" y="0" width="12" height="12" rx="2" class="b-hashbrown"/>
    <text x="123" y="10" class="t-legend">hashbrown</text>
    <rect x="210" y="0" width="12" height="12" rx="2" class="b-btree"/>
    <text x="228" y="10" class="t-legend">BTreeMap</text>
  </g>
  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>
"""

    dists = ["sequential", "clustered", "zipfian", "uniform"]
    dist_names = {
        "sequential": ("Dense Sequential (0..N)", "Linear consecutive keys"),
        "clustered": ("Sparse Clustered / Stride", "Bursts of 256 keys with sparse strides"),
        "zipfian": ("Zipfian Skewed (s=0.99)", "Power-law hot key access"),
        "uniform": ("Uniform Random 64-bit", "High entropy pseudo-random keys"),
    }

    y_start = 82

    for i, d in enumerate(dists):
        item = data.get(d, {})
        y = y_start + i * 58
        name, sub = dist_names.get(d, (d, ""))
        lookups = item.get("lookup_mops", {})

        m_exp = lookups.get("expanse", 0.0)
        m_hb = lookups.get("hashbrown", 0.0)
        m_bt = lookups.get("btree", 0.0)

        svg += f"""  <text x="30" y="{y + 14}" class="t-bar-label">{name}</text>
  <text x="30" y="{y + 28}" class="t-sub">{sub}</text>
"""
        w_exp = max(3.0, (m_exp / max_val) * bar_max_width)
        w_hb = max(3.0, (m_hb / max_val) * bar_max_width)
        w_bt = max(3.0, (m_bt / max_val) * bar_max_width)

        # Bars
        svg += f"""  <rect x="310" y="{y}" width="{w_exp:.1f}" height="9" rx="2" class="b-expanse"/>
  <text x="{318 + w_exp:.1f}" y="{y + 8}" class="t-val-accent" text-anchor="start">{m_exp:.1f}M</text>

  <rect x="310" y="{y + 13}" width="{w_hb:.1f}" height="9" rx="2" class="b-hashbrown"/>
  <text x="{318 + w_hb:.1f}" y="{y + 21}" class="t-val-blue" text-anchor="start">{m_hb:.1f}M</text>

  <rect x="310" y="{y + 26}" width="{w_bt:.1f}" height="9" rx="2" class="b-btree"/>
  <text x="{318 + w_bt:.1f}" y="{y + 34}" class="t-val-muted" text-anchor="start">{m_bt:.1f}M</text>
"""
        if m_bt > 0:
            speedup = m_exp / m_bt
            svg += f"""  <rect x="815" y="{y + 12}" width="115" height="18" class="badge-win"/>
  <text x="872.5" y="{y + 25}" class="badge-win-text">{speedup:.1f}x vs BTree</text>
"""

    svg += "</svg>\n"
    save_and_validate_svg(RESULTS_DIR / "bench_key_distributions.svg", svg)

def generate_tail_latency_chart():
    json_path = RESULTS_DIR / "baseline_tail_latency.json"
    if not json_path.exists():
        return
    with open(json_path) as f:
        data = body(json.load(f))

    total_inserts = data.get("total_inserts", 0)
    svg = svg_header(width=960, height=300, title="Ingestion Tail Latency Percentiles")
    svg += f"""
  <!-- Header -->
  <text x="30" y="30" class="t-title">INGESTION TAIL LATENCY: P50 TO P99.99 (NANOSECONDS)</text>
  <text x="30" y="46" class="t-sub">Dynamic Ingestion without Pre-allocation • {total_inserts:,} Inserts • Lower is better</text>

  <!-- Legend -->
  <g transform="translate(630, 20)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">ExpanseMap</text>
    <rect x="105" y="0" width="12" height="12" rx="2" class="b-hashbrown"/>
    <text x="123" y="10" class="t-legend">hashbrown</text>
    <rect x="210" y="0" width="12" height="12" rx="2" class="b-btree"/>
    <text x="228" y="10" class="t-legend">BTreeMap</text>
  </g>
  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>
"""

    exp = data.get("expanse", {})
    hb = data.get("hashbrown", {})
    bt = data.get("btree", {})

    quants = ["p50_ns", "p75_ns", "p90_ns", "p95_ns", "p99_ns", "p99_9_ns", "p99_99_ns"]
    q_labels = ["P50 (Median)", "P75", "P90", "P95", "P99", "P99.9", "P99.99 (Tail Cliff)"]

    y_hdr = 82
    svg += f"""  <text x="40" y="{y_hdr}" class="t-unit">Percentile</text>
  <text x="240" y="{y_hdr}" class="t-val-accent" text-anchor="start">ExpanseMap</text>
  <text x="460" y="{y_hdr}" class="t-val-blue" text-anchor="start">hashbrown</text>
  <text x="680" y="{y_hdr}" class="t-val-muted" text-anchor="start">BTreeMap</text>
  <line x1="30" y1="{y_hdr + 8}" x2="930" y2="{y_hdr + 8}" class="divider"/>
"""

    for i, (q, ql) in enumerate(zip(quants, q_labels)):
        y = y_hdr + 22 + i * 23
        e_v = exp.get(q, 0)
        h_v = hb.get(q, 0)
        b_v = bt.get(q, 0)

        svg += f"""  <text x="40" y="{y}" class="t-bar-label">{ql}</text>
  <text x="240" y="{y}" class="t-val-accent" text-anchor="start">{e_v:,} ns</text>
  <text x="460" y="{y}" class="t-val-blue" text-anchor="start">{h_v:,} ns</text>
  <text x="680" y="{y}" class="t-val-muted" text-anchor="start">{b_v:,} ns</text>
  <line x1="30" y1="{y + 4}" x2="930" y2="{y + 4}" class="grid"/>
"""

    svg += "</svg>\n"
    save_and_validate_svg(RESULTS_DIR / "bench_tail_latency.svg", svg)

def main():
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    generate_native_chart()
    generate_ycsb_chart()
    generate_memory_chart()
    generate_key_distributions_chart()
    generate_tail_latency_chart()

if __name__ == "__main__":
    main()
