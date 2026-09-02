#!/usr/bin/env python3
"""
Dual-theme SVG chart generator for the Adaptive Radix Tree (ART) benchmark suite (#387).

Produces SVG charts:
  1. chart_lookup_hit.svg    (Pillar 1: 100% Hit point lookup, ns/op)
  2. chart_lookup_miss.svg   (Pillar 2: 50/50 rejection miss, ns/op)
  3. chart_insert.svg        (Pillar 3: dynamic growth insertion, ns/op)
  4. chart_scan.svg          (Pillar 4: range scan & iteration, ns/elem)
  5. chart_memory.svg        (Pillar 5: bytes/key census, B/key)
"""

import json
import xml.etree.ElementTree as ET
from pathlib import Path

from theme import svg_header

BASE_DIR = Path(__file__).resolve().parent.parent
RESULTS_DIR = BASE_DIR / "results"


def save_svg(filepath: Path, content: str) -> None:
    try:
        ET.fromstring(content)
    except ET.ParseError as err:
        print(f"XML validation error in {filepath.name}: {err}")
        raise
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Generated & validated: {filepath}")


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def render_four_arm_chart(out_name: str, title: str, sub: str, unit: str, rows: list, lower_is_better: bool = True) -> None:
    """rows: list of (dist_label, sublabel, expanse, blart, btree, hashmap)."""
    if not rows:
        return
    max_val = max([max(r[2], r[3], r[4], r[5]) for r in rows] + [1e-9]) * 1.25
    bar_max = 280.0
    row_h = 72
    top = 96
    height = top + len(rows) * row_h + 24

    svg = svg_header(width=960, height=height, title=esc(title))
    better_text = "lower is better" if lower_is_better else "higher is better"
    svg += f"""
  <text x="30" y="34" class="t-title">{esc(title)}</text>
  <text x="30" y="50" class="t-sub">{esc(sub)} &#183; {esc(unit)} &#183; {better_text}</text>
  <g transform="translate(540, 20)">
    <rect x="0" y="0" width="10" height="10" rx="2" class="b-expanse"/>
    <text x="14" y="9" class="t-legend">ExpanseMap</text>
    <rect x="110" y="0" width="10" height="10" rx="2" class="b-art"/>
    <text x="124" y="9" class="t-legend">blart (ART)</text>
    <rect x="210" y="0" width="10" height="10" rx="2" class="b-btree"/>
    <text x="224" y="9" class="t-legend">BTreeMap</text>
    <rect x="300" y="0" width="10" height="10" rx="2" class="b-hashmap"/>
    <text x="314" y="9" class="t-legend">HashMap</text>
  </g>
  <line x1="30" y1="66" x2="930" y2="66" class="divider"/>
"""
    for i, (label, sublabel, exp, art, bt, hm) in enumerate(rows):
        y = top + i * row_h
        w_exp = max(2.0, (exp / max_val) * bar_max)
        w_art = max(2.0, (art / max_val) * bar_max)
        w_bt = max(2.0, (bt / max_val) * bar_max)
        w_hm = max(2.0, (hm / max_val) * bar_max)

        svg += f"""  <text x="30" y="{y + 12}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 26}" class="t-sub">{esc(sublabel)}</text>
  <rect x="260" y="{y - 8}" width="{w_exp:.1f}" height="10" rx="2" class="b-expanse"/>
  <text x="{268 + w_exp:.1f}" y="{y}" class="t-val-accent">{exp:.1f}</text>
  <rect x="260" y="{y + 6}" width="{w_art:.1f}" height="10" rx="2" class="b-art"/>
  <text x="{268 + w_art:.1f}" y="{y + 14}" class="t-val-blue">{art:.1f}</text>
  <rect x="260" y="{y + 20}" width="{w_bt:.1f}" height="10" rx="2" class="b-btree"/>
  <text x="{268 + w_bt:.1f}" y="{y + 28}" class="t-axis-label">{bt:.1f}</text>
  <rect x="260" y="{y + 34}" width="{w_hm:.1f}" height="10" rx="2" class="b-hashmap"/>
  <text x="{268 + w_hm:.1f}" y="{y + 42}" class="t-note">{hm:.1f}</text>
"""
        # Win / Loss badge vs ART
        if lower_is_better:
            winner = "expanse" if exp <= art else "art"
            ratio = art / exp if exp > 0 and winner == "expanse" else exp / art if art > 0 else 1.0
        else:
            winner = "expanse" if exp >= art else "art"
            ratio = exp / art if art > 0 and winner == "expanse" else art / exp if exp > 0 else 1.0

        if winner == "expanse":
            svg += f"""  <rect x="800" y="{y + 8}" width="130" height="20" rx="3" class="badge-win"/>
  <text x="865" y="{y + 22}" class="badge-win-text">Expanse {ratio:.2f}x</text>
"""
        else:
            svg += f"""  <rect x="800" y="{y + 8}" width="130" height="20" rx="3" class="badge-loss"/>
  <text x="865" y="{y + 22}" class="badge-loss-text">ART {ratio:.2f}x</text>
"""
    svg += "</svg>\n"
    save_svg(RESULTS_DIR / out_name, svg)


def generate_all() -> None:
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)

    # 1. Point Lookup Hit
    hit_path = RESULTS_DIR / "baseline_lookup_hit.json"
    if hit_path.exists():
        with open(hit_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        # Filter for 1M (or max available)
        max_pop = max(r["population"] for r in data["results"])
        rows = [
            (
                r["distribution"].replace("_", " ").title(),
                f"N = {r['population']:,} keys (100% Hit)",
                r["expanse_ns_op"],
                r["blart_art_ns_op"],
                r["btree_ns_op"],
                r["hashmap_ns_op"],
            )
            for r in data["results"]
            if r["population"] == max_pop
        ]
        render_four_arm_chart("chart_lookup_hit.svg", "Point Lookup Latency (100% Hit)", f"Population N = {max_pop:,}", "ns / lookup", rows)

    # 2. Point Lookup Miss
    miss_path = RESULTS_DIR / "baseline_lookup_miss.json"
    if miss_path.exists():
        with open(miss_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        max_pop = max(r["population"] for r in data["results"])
        rows = [
            (
                r["distribution"].replace("_", " ").title(),
                f"N = {r['population']:,} keys (50% Hit / 50% Miss)",
                r["expanse_ns_op"],
                r["blart_art_ns_op"],
                r["btree_ns_op"],
                r["hashmap_ns_op"],
            )
            for r in data["results"]
            if r["population"] == max_pop
        ]
        render_four_arm_chart("chart_lookup_miss.svg", "Point Lookup Latency (50% Hit / 50% Miss)", f"Population N = {max_pop:,}", "ns / lookup", rows)

    # 3. Dynamic Insert
    insert_path = RESULTS_DIR / "baseline_insert.json"
    if insert_path.exists():
        with open(insert_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        max_pop = max(r["population"] for r in data["results"])
        rows = [
            (
                r["distribution"].replace("_", " ").title(),
                f"Dynamic Growth 0 -> {r['population']:,}",
                r["expanse_ns_op"],
                r["blart_art_ns_op"],
                r["btree_ns_op"],
                r["hashmap_ns_op"],
            )
            for r in data["results"]
            if r["population"] == max_pop
        ]
        render_four_arm_chart("chart_insert.svg", "Dynamic Insertion Latency", f"Population N = {max_pop:,}", "ns / insert", rows)

    # 4. Memory Census
    mem_path = RESULTS_DIR / "baseline_memory.json"
    if mem_path.exists():
        with open(mem_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        max_pop = max(r["population"] for r in data["results"])
        rows = [
            (
                r["distribution"].replace("_", " ").title(),
                f"Live Resident Heap (N = {r['population']:,})",
                r["expanse_bpk"],
                r["blart_art_bpk"],
                r["btree_bpk"],
                r["hashmap_bpk"],
            )
            for r in data["results"]
            if r["population"] == max_pop
        ]
        render_four_arm_chart("chart_memory.svg", "Memory Footprint (Live Heap Allocation)", f"Population N = {max_pop:,}", "Bytes / Key", rows)


if __name__ == "__main__":
    generate_all()
