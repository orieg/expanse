#!/usr/bin/env python3
"""
Dual-theme SVG chart generator for the Redis ZSET engine benchmark suite (#330).

Produces four charts:
  1. bench_zadd_throughput.svg    (Pillar 1, grouped bars, M ops/sec)
  2. bench_range_throughput.svg   (Pillar 2, grouped bars, M elem/sec)
  3. bench_rank_throughput.svg    (Pillar 3, grouped bars, M ops/sec)
  4. bench_memory_footprint.svg   (Pillar 4, stacked bars, bytes/member)

Win/loss badges are driven by the `winner` field in the JSON: green when
Expanse leads, amber when the SkipList+Dict reference leads (published, not
hidden).
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


def grouped_bar_chart(out_name, title, sub, unit, rows, fmt="{:.1f}"):
    """rows: list of (label, sublabel, expanse, skiplist, winner)."""
    max_val = max([max(r[2], r[3]) for r in rows] + [1e-9]) * 1.28
    bar_max = 300.0
    row_h = 52
    top = 96
    height = top + len(rows) * row_h + 24

    svg = svg_header(width=960, height=height, title=esc(title))
    svg += f"""
  <text x="30" y="34" class="t-title">{esc(title)}</text>
  <text x="30" y="50" class="t-sub">{esc(sub)} &#183; {esc(unit)} &#183; higher is better</text>
  <g transform="translate(600, 24)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">Expanse (single-trie ZSET)</text>
    <rect x="0" y="18" width="12" height="12" rx="2" class="b-skiplist"/>
    <text x="18" y="28" class="t-legend">SkipList + Dict (Redis)</text>
  </g>
  <line x1="30" y1="66" x2="930" y2="66" class="divider"/>
"""
    for i, (label, sublabel, exp, sl, winner) in enumerate(rows):
        y = top + i * row_h
        w_exp = max(2.0, (exp / max_val) * bar_max)
        w_sl = max(2.0, (sl / max_val) * bar_max)
        svg += f"""  <text x="30" y="{y + 8}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 22}" class="t-sub">{esc(sublabel)}</text>
  <rect x="300" y="{y - 4}" width="{w_exp:.1f}" height="13" rx="2" class="b-expanse"/>
  <text x="{308 + w_exp:.1f}" y="{y + 6}" class="t-val-accent">{fmt.format(exp)}</text>
  <rect x="300" y="{y + 12}" width="{w_sl:.1f}" height="13" rx="2" class="b-skiplist"/>
  <text x="{308 + w_sl:.1f}" y="{y + 22}" class="t-val-blue">{fmt.format(sl)}</text>
"""
        if winner == "expanse":
            ratio = exp / sl if sl > 0 else 0.0
            svg += f"""  <rect x="800" y="{y + 2}" width="130" height="18" rx="3" class="badge-win"/>
  <text x="865" y="{y + 15}" class="badge-win-text">Expanse {ratio:.2f}x</text>
"""
        else:
            ratio = sl / exp if exp > 0 else 0.0
            svg += f"""  <rect x="800" y="{y + 2}" width="130" height="18" rx="3" class="badge-loss"/>
  <text x="865" y="{y + 15}" class="badge-loss-text">SkipList {ratio:.2f}x</text>
"""
    svg += "</svg>\n"
    save_svg(RESULTS_DIR / out_name, svg)


ZADD_LABELS = {
    "fresh_insert": ("Fresh insert (cold build)", "N members, random scores"),
    "score_update": ("Score update", "delete+insert of composite key"),
    "zincrby": ("ZINCRBY", "read-modify-write increments"),
    "mixed_churn": ("Mixed churn", "add-new / update / remove"),
}
RANGE_LABELS = {
    "forward_small": ("ZRANGEBYSCORE small", "~64-member windows"),
    "forward_large": ("ZRANGEBYSCORE large", "~8192-member windows"),
    "reverse_small": ("ZREVRANGEBYSCORE small", "~64 members, no reverse iterator"),
    "reverse_large": ("ZREVRANGEBYSCORE large", "~8192 members, no reverse iterator"),
}
RANK_LABELS = {
    "zrank": ("ZRANK", "count_below vs span walk"),
    "zcount": ("ZCOUNT", "two rank primitives"),
    "zrank_by_rank": ("ZRANGE by rank (select)", "by_count vs span descent"),
}


def gen_from_mops(json_name, out_name, title, sub, unit, labels, exp_key, sl_key):
    path = RESULTS_DIR / json_name
    if not path.exists():
        return
    data = json.loads(path.read_text())
    pop = data.get("population", 0)
    scenarios = data["scenarios"]
    rows = []
    for key, (label, sublabel) in labels.items():
        if key not in scenarios:
            continue
        s = scenarios[key]
        rows.append((label, sublabel, s[exp_key], s[sl_key], s["winner"]))
    sub_full = sub.format(pop=f"{pop:,}")
    grouped_bar_chart(out_name, title, sub_full, unit, rows)


def generate_memory_chart():
    path = RESULTS_DIR / "baseline_memory.json"
    if not path.exists():
        return
    data = json.loads(path.read_text())
    entry = data[-1]  # largest population
    pop = entry["population"]

    svg = svg_header(width=960, height=340, title="ZSET Memory Footprint")
    svg += f"""
  <text x="30" y="34" class="t-title">MEMORY FOOTPRINT: BYTES PER MEMBER (LOWER IS BETTER)</text>
  <text x="30" y="50" class="t-sub">N = {pop:,} members &#183; deterministic GlobalAlloc accounting &#183; skip list arena-modeled (conservative baseline)</text>
  <line x1="30" y1="64" x2="930" y2="64" class="divider"/>
"""
    panels = [
        ("Random scores", "random_scores", 30),
        ("Sequential scores (score = member)", "sequential_scores", 490),
    ]
    max_scale = max(
        entry["random_scores"]["skiplist_bytes_per_member"],
        entry["sequential_scores"]["skiplist_bytes_per_member"],
        1.0,
    ) * 1.2
    bar_max = 300.0

    for title, key, x_off in panels:
        m = entry[key]
        exp_order = m["expanse_order_bytes_per_member"]
        exp_members = m["expanse_members_bytes_per_member"]
        sl_list = m["skiplist_list_bytes_per_member"]
        sl_dict = m["skiplist_dict_bytes_per_member"]
        exp_total = m["expanse_bytes_per_member"]
        sl_total = m["skiplist_bytes_per_member"]
        ratio = m["ratio_skiplist_over_expanse"]

        we_o = (exp_order / max_scale) * bar_max
        we_m = (exp_members / max_scale) * bar_max
        ws_l = (sl_list / max_scale) * bar_max
        ws_d = (sl_dict / max_scale) * bar_max

        svg += f"""
  <g transform="translate({x_off}, 86)">
    <text x="0" y="0" class="t-title">{esc(title)}</text>

    <text x="0" y="34" class="t-bar-label">Expanse</text>
    <rect x="0" y="42" width="{we_o:.1f}" height="16" rx="2" class="b-expanse"/>
    <rect x="{we_o:.1f}" y="42" width="{we_m:.1f}" height="16" class="b-expanse" fill-opacity="0.55"/>
    <text x="{we_o + we_m + 8:.1f}" y="55" class="t-val-accent">{exp_total:.1f}</text>
    <text x="0" y="74" class="t-note">order {exp_order:.1f} + members {exp_members:.1f} B/member</text>

    <text x="0" y="104" class="t-bar-label">SkipList + Dict</text>
    <rect x="0" y="112" width="{ws_l:.1f}" height="16" rx="2" class="b-skiplist"/>
    <rect x="{ws_l:.1f}" y="112" width="{ws_d:.1f}" height="16" class="b-skiplist" fill-opacity="0.55"/>
    <text x="{ws_l + ws_d + 8:.1f}" y="125" class="t-val-blue">{sl_total:.1f}</text>
    <text x="0" y="144" class="t-note">list {sl_list:.1f} + dict {sl_dict:.1f} B/member</text>

    <rect x="0" y="164" width="200" height="22" rx="3" class="badge-win"/>
    <text x="100" y="179" class="badge-win-text">&#10003; Expanse {ratio:.2f}x more compact</text>
  </g>
"""
    svg += """  <line x1="470" y1="80" x2="470" y2="300" class="divider"/>
</svg>
"""
    save_svg(RESULTS_DIR / "bench_memory_footprint.svg", svg)


def main():
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    gen_from_mops(
        "baseline_zadd.json", "bench_zadd_throughput.svg",
        "PILLAR 1 — ZADD CHURN THROUGHPUT",
        "{pop} members", "M ops/sec",
        ZADD_LABELS, "expanse_mops", "skiplist_mops",
    )
    gen_from_mops(
        "baseline_range.json", "bench_range_throughput.svg",
        "PILLAR 2 — RANGE ITERATION THROUGHPUT",
        "{pop} members", "M elements/sec",
        RANGE_LABELS, "expanse_melem_s", "skiplist_melem_s",
    )
    gen_from_mops(
        "baseline_rank.json", "bench_rank_throughput.svg",
        "PILLAR 3 — RANK & COUNT THROUGHPUT",
        "{pop} members", "M queries/sec",
        RANK_LABELS, "expanse_mops", "skiplist_mops",
    )
    generate_memory_chart()


if __name__ == "__main__":
    main()
