#!/usr/bin/env python3
"""
docs/benchmarks/search_inverted_index/scripts/generate_charts.py

Dual-theme SVG chart generator for the search / inverted-index suite. Every
win/loss badge is computed from the measured JSON — the charts self-label
which side won each cell, so a losing cell renders as a loss with no manual
editing.

Charts:
  1. bench_boolean_and.svg   — AND latency, ExpanseSet vs Roaring (log scale)
  2. bench_wand_skipscan.svg — WAND next-at-or-after ns/skip by distribution x regime
  3. bench_memory_bits.svg   — live-heap bits/docID by distribution
"""

import json
import math
import xml.etree.ElementTree as ET
from pathlib import Path
from theme import svg_header

BASE_DIR = Path(__file__).resolve().parent.parent
RESULTS_DIR = BASE_DIR / "results"


def save_and_validate_svg(filepath: Path, content: str):
    try:
        ET.fromstring(content)
    except ET.ParseError as err:
        print(f"XML Validation Error in {filepath.name}: {err}")
        raise
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Generated & Validated: {filepath}")


def load(name):
    p = RESULTS_DIR / name
    if not p.exists():
        return None
    with open(p) as f:
        return json.load(f)


def fmt_ns(ns):
    if ns >= 1_000_000:
        return f"{ns / 1_000_000:.2f} ms"
    if ns >= 1_000:
        return f"{ns / 1_000:.2f} µs"
    return f"{ns:.0f} ns"


def ratio_badge(expanse, roaring):
    """(text, css) — how Expanse compares to Roaring. Lower time is better."""
    if expanse <= 0 or roaring <= 0:
        return ("n/a", "badge-loss")
    if expanse <= roaring:
        return (f"Expanse {roaring / expanse:.1f}x faster", "badge-win")
    return (f"{expanse / roaring:.1f}x slower", "badge-loss")


#: Dynamic range (max/min) above which a log scale is the only readable
#: choice, because a zero-based linear axis would collapse the small arms to a
#: few pixels. Below it, linear from zero is used: a log axis with an offset
#: origin makes bar *lengths* non-proportional to their values (a 2.4x
#: difference rendered as a 3.6x-longer bar) for no legibility gain. At the
#: current data this keeps the two boolean-AND charts (67,000x and 62x spans)
#: on log and puts WAND (10x) and memory-bits (7x) on linear.
LOG_SCALE_THRESHOLD = 25.0


def bar_scale(vals):
    """Pick the bar scale for `vals` and return (kind, width_fn, scale_label).

    `kind` is "log" or "linear"; `width_fn(v)` maps a value to a bar length in
    px; `scale_label` is stamped into the chart title so the reader is never
    left to guess which scale the bars use.
    """
    bar_max = 300.0
    positive = [v for v in vals if v > 0]
    if not positive:
        return ("linear", lambda v: 3.0, "LINEAR SCALE FROM ZERO")
    lo, hi = min(positive), max(positive)

    if hi / lo >= LOG_SCALE_THRESHOLD:
        lo_log = math.log10(lo) - 0.15
        hi_log = math.log10(hi) + 0.05
        span = max(hi_log - lo_log, 0.5)

        def width(v):
            if v <= 0:
                return 3.0
            return max(3.0, (math.log10(v) - lo_log) / span * bar_max)

        return ("log", width, "LOG SCALE")

    axis_max = hi * 1.12

    def width(v):
        if v <= 0:
            return 3.0
        return max(3.0, (v / axis_max) * bar_max)

    return ("linear", width, "LINEAR SCALE FROM ZERO")


def log_bars_chart(filepath, base_title, sub, rows, unit_fmt):
    """Horizontal paired bars, log or zero-based linear per `bar_scale`.

    rows: list of (label, sublabel, expanse_val, roaring_val) — lower is better.
    `base_title` carries no scale wording; the chosen scale is appended.
    """
    vals = [v for _, _, e, r in rows for v in (e, r) if v > 0]
    if not vals:
        return
    _kind, width, scale_label = bar_scale(vals)
    title = f"{base_title} ({scale_label}, LOWER IS BETTER)"
    x0 = 360

    row_h = 46
    height = 96 + len(rows) * row_h + 20
    svg = svg_header(width=960, height=height, title=title)
    svg += f"""
  <text x="30" y="30" class="t-title">{title}</text>
  <text x="30" y="46" class="t-sub">{sub}</text>
  <g transform="translate(680, 20)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">ExpanseSet</text>
    <rect x="120" y="0" width="12" height="12" rx="2" class="b-roaring"/>
    <text x="138" y="10" class="t-legend">Roaring</text>
  </g>
  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>
"""
    y = 78
    for label, sub2, e_val, r_val in rows:
        w_e = width(e_val)
        w_r = width(r_val)
        text, css = ratio_badge(e_val, r_val)
        badge_text_css = "badge-win-text" if css == "badge-win" else "badge-loss-text"
        svg += f"""  <text x="30" y="{y + 13}" class="t-bar-label">{label}</text>
  <text x="30" y="{y + 27}" class="t-sub">{sub2}</text>
  <rect x="{x0}" y="{y}" width="{w_e:.1f}" height="11" rx="2" class="b-expanse"/>
  <text x="{x0 + w_e + 6:.1f}" y="{y + 9}" class="t-val-accent">{unit_fmt(e_val)}</text>
  <rect x="{x0}" y="{y + 15}" width="{w_r:.1f}" height="11" rx="2" class="b-roaring"/>
  <text x="{x0 + w_r + 6:.1f}" y="{y + 24}" class="t-val-blue">{unit_fmt(r_val)}</text>
  <rect x="775" y="{y + 4}" width="155" height="18" class="{css}"/>
  <text x="852" y="{y + 17}" class="{badge_text_css}">{text}</text>
"""
        y += row_h
    svg += "</svg>\n"
    save_and_validate_svg(filepath, svg)


def log_bars_chart3(filepath, base_title, sub, rows, unit_fmt):
    """Horizontal triple bars: stateless / cursor / Roaring.

    rows: list of (label, sublabel, stateless_val, cursor_val, roaring_val) —
    lower is better. The win/loss badge compares the #340 cursor against
    Roaring (the parity target); the stateless re-descent is the baseline
    being improved. Scale is log or zero-based linear per `bar_scale`, and the
    chosen scale is appended to `base_title`.
    """
    vals = [v for _, _, s, c, r in rows for v in (s, c, r) if v > 0]
    if not vals:
        return
    _kind, width, scale_label = bar_scale(vals)
    title = f"{base_title} ({scale_label}, LOWER IS BETTER)"
    x0 = 360

    row_h = 58
    height = 100 + len(rows) * row_h + 20
    svg = svg_header(width=960, height=height, title=title)
    svg += f"""
  <text x="30" y="30" class="t-title">{title}</text>
  <text x="30" y="46" class="t-sub">{sub}</text>
  <g transform="translate(560, 20)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">Stateless</text>
    <rect x="110" y="0" width="12" height="12" rx="2" class="b-cursor"/>
    <text x="128" y="10" class="t-legend">Cursor #340</text>
    <rect x="250" y="0" width="12" height="12" rx="2" class="b-roaring"/>
    <text x="268" y="10" class="t-legend">Roaring</text>
  </g>
  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>
"""
    y = 80
    for label, sub2, s_val, c_val, r_val in rows:
        w_s = width(s_val)
        w_c = width(c_val)
        w_r = width(r_val)
        text, css = ratio_badge(c_val, r_val)
        badge_text_css = "badge-win-text" if css == "badge-win" else "badge-loss-text"
        svg += f"""  <text x="30" y="{y + 16}" class="t-bar-label">{label}</text>
  <text x="30" y="{y + 30}" class="t-sub">{sub2}</text>
  <rect x="{x0}" y="{y}" width="{w_s:.1f}" height="11" rx="2" class="b-expanse"/>
  <text x="{x0 + w_s + 6:.1f}" y="{y + 9}" class="t-val-accent">{unit_fmt(s_val)}</text>
  <rect x="{x0}" y="{y + 15}" width="{w_c:.1f}" height="11" rx="2" class="b-cursor"/>
  <text x="{x0 + w_c + 6:.1f}" y="{y + 24}" class="t-val-amber">{unit_fmt(c_val)}</text>
  <rect x="{x0}" y="{y + 30}" width="{w_r:.1f}" height="11" rx="2" class="b-roaring"/>
  <text x="{x0 + w_r + 6:.1f}" y="{y + 39}" class="t-val-blue">{unit_fmt(r_val)}</text>
  <rect x="775" y="{y + 12}" width="155" height="18" class="{css}"/>
  <text x="852" y="{y + 25}" class="{badge_text_css}">{text}</text>
"""
        y += row_h
    svg += "</svg>\n"
    save_and_validate_svg(filepath, svg)


def generate_boolean_chart():
    data = load("baseline_boolean.json")
    if not data:
        return
    sizes = sorted({d["size"] for d in data if d["cell"] == "symmetric"})
    top = sizes[-1]
    dist_order = ["dense", "clustered", "sparse", "zipfian"]
    rows = []
    for dist in dist_order:
        rec = next(
            (
                d
                for d in data
                if d["cell"] == "symmetric" and d["op"] == "and" and d["size"] == top and d["distribution"] == dist
            ),
            None,
        )
        if rec:
            rows.append(
                (
                    dist.capitalize(),
                    f"symmetric |A|=|B|={rec['na']:,}",
                    # Native structural kernel (#339) is now the Expanse arm;
                    # the composed path is in the README table's second column.
                    rec.get("expanse_native_ns", rec["expanse_ns"]),
                    rec["roaring_ns"],
                )
            )
    # Skewed-size AND rows (tiny B into huge A).
    for rec in [d for d in data if d["cell"] == "skewed" and d["op"] == "and"]:
        rows.append(
            (
                f"Skewed {rec['distribution']}",
                f"|A|={rec['na']:,}  |B|={rec['nb']:,}",
                rec.get("expanse_native_ns", rec["expanse_ns"]),
                rec["roaring_ns"],
            )
        )
    log_bars_chart(
        RESULTS_DIR / "bench_boolean_and.svg",
        "BOOLEAN AND: INTERSECTION LATENCY",
        f"Cardinality of A ∩ B • top size N={top:,} per list • ExpanseSet native kernel #339 (intersection_len) vs Roaring (intersection_len)",
        rows,
        fmt_ns,
    )


def generate_boolean_materialize_chart():
    """AND materialization latency (#348): build the result set, not just its
    cardinality. Direct-emission ExpanseSet vs Roaring bitmap `&`. The v1
    ordered-merge + insert path is reported in the README table."""
    data = load("baseline_boolean.json")
    if not data:
        return
    # Only present once the materialization arm has been measured.
    if not any("expanse_materialize_ns" in d for d in data):
        return
    sizes = sorted({d["size"] for d in data if d["cell"] == "symmetric"})
    if not sizes:
        return
    top = sizes[-1]
    dist_order = ["dense", "clustered", "sparse", "zipfian"]
    rows = []
    for dist in dist_order:
        rec = next(
            (
                d
                for d in data
                if d["cell"] == "symmetric"
                and d["op"] == "and"
                and d["size"] == top
                and d["distribution"] == dist
                and "expanse_materialize_ns" in d
            ),
            None,
        )
        if rec:
            rows.append(
                (
                    dist.capitalize(),
                    f"symmetric |A|=|B|={rec['na']:,}",
                    rec["expanse_materialize_ns"],
                    rec["roaring_materialize_ns"],
                )
            )
    if not rows:
        return
    log_bars_chart(
        RESULTS_DIR / "bench_boolean_and_materialize.svg",
        "BOOLEAN AND: MATERIALIZATION LATENCY",
        f"Result set A ∩ B built • top size N={top:,} per list • ExpanseSet direct emission #348 (intersection) vs Roaring bitmap AND",
        rows,
        fmt_ns,
    )


def generate_wand_chart():
    data = load("baseline_wand.json")
    if not data:
        return
    sizes = sorted({d["size"] for d in data})
    top = sizes[-1]
    rows = []
    dist_order = ["dense", "clustered", "sparse"]
    regimes = ["shallow", "medium", "deep"]
    for dist in dist_order:
        for regime in regimes:
            rec = next(
                (d for d in data if d["size"] == top and d["list_dist"] == dist and d["regime"] == regime),
                None,
            )
            if rec:
                rows.append(
                    (
                        f"{dist.capitalize()} / {regime}",
                        f"{rec['skips']:,} skips • stride~{rec['avg_stride']:,}",
                        rec["expanse_ns_per_skip"],
                        # Cursor arm (#340). Older JSON without it falls back to
                        # the stateless value so the chart still renders.
                        rec.get("expanse_cursor_ns_per_skip", rec["expanse_ns_per_skip"]),
                        rec["roaring_ns_per_skip"],
                    )
                )
    log_bars_chart3(
        RESULTS_DIR / "bench_wand_skipscan.svg",
        "WAND SKIP-SCAN: NANOSECONDS PER ADVANCE",
        f"Monotonic target advance • top size N={top:,} • stateless next_at_or_after vs #340 cursor advance_to vs Roaring advance_to",
        rows,
        lambda v: f"{v:.1f} ns",
    )


def generate_memory_chart():
    data = load("baseline_memory.json")
    if not data:
        return
    sizes = sorted({d["size"] for d in data})
    top = sizes[-1]
    rows = []
    for dist in ["dense", "clustered", "sparse", "shard"]:
        rec = next((d for d in data if d["size"] == top and d["distribution"] == dist), None)
        if rec:
            rows.append(
                (
                    dist.capitalize(),
                    f"N={rec['actual']:,} docIDs",
                    rec["expanse_heap_bits_per_docid"],
                    rec["roaring_heap_bits_per_docid"],
                )
            )
    log_bars_chart(
        RESULTS_DIR / "bench_memory_bits.svg",
        "MEMORY: LIVE-HEAP BITS PER DOCID",
        f"Resident heap via GlobalAlloc tracker • top size N={top:,} • ExpanseSet vs Roaring (roaring-rs 0.10, no run containers)",
        rows,
        lambda v: f"{v:.2f} b",
    )


def main():
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    generate_boolean_chart()
    generate_boolean_materialize_chart()
    generate_wand_chart()
    generate_memory_chart()


if __name__ == "__main__":
    main()
