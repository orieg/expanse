#!/usr/bin/env python3
"""
Dual-theme SVG charts for the Patricia / radix trie vs Expanse suite, derived
from results/baseline_*.json (AGENTS.md §8.2, §8.15a):

  1. chart_lookup.svg   u64 point lookup, 100% and 50% hit, at the largest n
  2. chart_insert.svg   u64 cold-build insert, both orders, at the largest n
  3. chart_scan.svg     u64 full traversal and string prefix scan, at the largest n
  4. chart_string.svg   string lookup ratio against shared-prefix length, per n
  5. chart_memory_u64.svg, chart_memory_paths.svg   requested bytes per key at the largest n

Timing charts plot the ratio Expanse ns / twin ns on a log axis centred on 1:
a bar to the left means Expanse is faster, to the right slower. The ratios span
several orders of magnitude, so a linear axis cannot show them; the decade lines
are labelled and the subtitle says bar length is logarithmic. Every axis bound
is computed from the data it draws. Whiskers are the BCa 95% interval.

    python3 docs/benchmarks/patricia_comparison/scripts/generate_charts.py
"""

from __future__ import annotations

import json
import math
import xml.etree.ElementTree as ET
from pathlib import Path

from theme import svg_footer, svg_header

BASE = Path(__file__).resolve().parent.parent
RESULTS = BASE / "results"

TWINS = (("patricia_tree", "patricia_tree", "b-patricia"),
         ("fast_radix_trie", "fast_radix_trie", "b-radix"),
         ("qp_trie", "qp-trie", "b-qp"))
DISTS = ("sequential", "clustered", "uniform_random", "sparse_stride", "zipfian")
W = 1000
LABEL_X, PLOT_X0, PLOT_X1, VALUE_X = 24, 200, 800, 990


def load(name: str) -> dict:
    return json.loads((RESULTS / f"baseline_{name}.json").read_text())


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def save(name: str, svg: str) -> None:
    ET.fromstring(svg)  # well-formed or raise
    (RESULTS / name).write_text(svg, encoding="utf-8")
    print(f"wrote {RESULTS / name}")


def draws(r: dict) -> int:
    if "raw_draws" in r:
        return r["raw_draws"] // 2 if r.get("hit_rate_pct") == 50 else r["raw_draws"]
    return r["population"]


def largest_n(rows: list[dict]) -> int:
    return max(draws(r) for r in rows)


def ratio_of(r: dict, key: str):
    if key not in r:
        return None
    lo, hi = r[f"{key}_ci"]
    return r[key], lo, hi


def invalid(r: dict, twin: str) -> bool:
    return any(k.startswith(twin) and k.endswith("_status") and v == "invalid" for k, v in r.items())


def fmt_ratio(v: float) -> str:
    return f"{1 / v:.3g}× faster" if v < 1 else f"{v:.3g}× slower"


def log_axis(values: list[float]) -> tuple[int, int]:
    lo = min(values + [1.0])
    hi = max(values + [1.0])
    return math.floor(math.log10(lo)), math.ceil(math.log10(hi))


def ratio_chart(name: str, title: str, sub: str, groups: list[tuple[str, list]]) -> None:
    """groups: [(group label, [(row label, class, ratio-triple | "invalid" | None)])]."""
    dropped: dict[str, int] = {}
    total: dict[str, int] = {}
    kept = []
    for glabel, rows in groups:
        keep = []
        for row in rows:
            twin = next(lab for _, lab, c in TWINS if c == row[1])
            total[twin] = total.get(twin, 0) + 1
            if isinstance(row[2], tuple):
                keep.append(row)
            else:
                dropped[twin] = dropped.get(twin, 0) + 1
        if keep:
            kept.append((glabel, keep))
    groups = kept
    note2 = "; ".join(f"{t} not drawn on {k} of {total[t]} rows (failed validation, e.g. cannot hold the key set)"
                      for t, k in dropped.items())
    vals = [v for _, rows in groups for _, _, t in rows for v in t]
    d0, d1 = log_axis(vals)
    if d0 == d1:
        d1 += 1

    def x(v: float) -> float:
        return PLOT_X0 + (math.log10(v) - d0) / (d1 - d0) * (PLOT_X1 - PLOT_X0)

    row_h, gap, top = 15, 12, 96 + (14 if note2 else 0)
    n_rows = sum(len(rows) for _, rows in groups)
    height = top + n_rows * row_h + len(groups) * (gap + 16) + 40
    out = [svg_header(W, height, esc(title)),
           f'  <text x="{LABEL_X}" y="30" class="t-title">{esc(title)}</text>',
           f'  <text x="{LABEL_X}" y="46" class="t-sub">{esc(sub)}</text>',
           f'  <text x="{LABEL_X}" y="61" class="t-note">Log scale: each gridline is a factor of 10. '
           f'Left of 1 = Expanse faster, right = slower. Whiskers: BCa 95% interval.</text>']
    if note2:
        out.append(f'  <text x="{LABEL_X}" y="75" class="t-note">{esc(note2)}.</text>')
    ly = 84 if note2 else 70
    drawn = {c for _, rows in groups for _, c, _ in rows}
    lx = PLOT_X0
    for _, lab, cls in TWINS:
        if cls not in drawn:
            continue
        out.append(f'  <rect x="{lx}" y="{ly}" width="10" height="10" rx="2" class="{cls}"/>'
                   f'<text x="{lx + 14}" y="{ly + 9}" class="t-legend">vs {esc(lab)}</text>')
        lx += 150
    axis_bottom = height - 30
    for d in range(d0, d1 + 1):
        xv = x(10 ** d)
        cls = "axis" if d == 0 else "grid"
        out.append(f'  <line x1="{xv:.1f}" y1="{top - 6}" x2="{xv:.1f}" y2="{axis_bottom}" class="{cls}"/>')
        lab = "1" if d == 0 else (f"{10 ** -d:g}× faster" if d < 0 else f"{10 ** d:g}× slower")
        out.append(f'  <text x="{xv:.1f}" y="{axis_bottom + 14}" class="t-axis-label" '
                   f'text-anchor="middle">{lab}</text>')
    y = top
    one = x(1.0)
    for glabel, rows in groups:
        out.append(f'  <text x="{LABEL_X}" y="{y + 10}" class="t-bar-label">{esc(glabel)}</text>')
        y += 16
        for rlabel, cls, t in rows:
            out.append(f'  <text x="{LABEL_X + 10}" y="{y + 10}" class="t-axis-label">{esc(rlabel)}</text>')
            if True:
                v, lo, hi = t
                xa, xb = sorted((one, x(v)))
                out.append(f'  <rect x="{xa:.1f}" y="{y + 2}" width="{max(xb - xa, 0.5):.1f}" height="{row_h - 5}" '
                           f'rx="1.5" class="{cls}"/>')
                out.append(f'  <line x1="{x(lo):.1f}" y1="{y + row_h / 2 - 1:.1f}" x2="{x(hi):.1f}" '
                           f'y2="{y + row_h / 2 - 1:.1f}" class="axis"/>')
                tcls = "t-val-accent" if hi < 1 else ("t-val-loss" if lo > 1 else "t-unit")
                out.append(f'  <text x="{VALUE_X}" y="{y + 10}" class="{tcls}" text-anchor="end">'
                           f'{esc(fmt_ratio(v))}</text>')
            y += row_h
        y += gap
    out.append(svg_footer())
    save(name, "\n".join(out))


def twin_rows(r: dict, key_fmt: str, label: str) -> list:
    rows = []
    for twin, lab, cls in TWINS:
        t = ratio_of(r, key_fmt.format(twin=twin))
        rows.append((f"{label} · {lab}" if label else lab, cls,
                     t if t else ("invalid" if invalid(r, twin) else None)))
    return rows


def lookup_chart() -> None:
    hit, miss = load("lookup_hit")["results"], load("lookup_miss")["results"]
    n = largest_n(hit)
    groups = []
    for d in DISTS:
        rows = []
        for name, data in (("100% hit", hit), ("50% hit", miss)):
            r = next((r for r in data if r["distribution"] == d and r["order"] == "generator" and draws(r) == n), None)
            if r:
                rows += twin_rows(r, "ratio_expanse_over_{twin}", name)
        groups.append((d, rows))
    ratio_chart("chart_lookup.svg", "u64 point lookup · Expanse vs radix tries",
                f"n = {n:,} draws, generator build order · ratio = Expanse ns / twin ns · workload: "
                f"patricia_lookup_hit, patricia_lookup_miss", groups)


def insert_chart() -> None:
    ins = load("insert")["results"]
    n = largest_n(ins)
    groups = []
    for d in DISTS:
        r = next((r for r in ins if r["distribution"] == d and draws(r) == n), None)
        if r:
            groups.append((d, twin_rows(r, "ratio_expanse_over_{twin}_generator", "generator")
                           + twin_rows(r, "ratio_expanse_over_{twin}_shuffled", "shuffled")))
    ratio_chart("chart_insert.svg", "u64 cold-build insert · Expanse vs radix tries",
                f"n = {n:,} draws, both insertion orders · ratio = Expanse ns / twin ns · workload: patricia_insert",
                groups)


def scan_chart() -> None:
    sc = load("scan")["results"]
    n = largest_n([r for r in sc if r["operation"] == "full_traversal"])
    groups = []
    for d in DISTS:
        r = next((r for r in sc if r["operation"] == "full_traversal" and r["distribution"] == d
                  and r["order"] == "generator" and draws(r) == n), None)
        if r:
            groups.append((f"full traversal · {d}", twin_rows(r, "ratio_expanse_over_{twin}", "")))
    pn = max(r["population"] for r in sc if r["operation"] == "prefix_scan")
    for order in ("generator", "sorted"):
        r = next((r for r in sc if r["operation"] == "prefix_scan" and r["order"] == order
                  and r["population"] == pn), None)
        if r:
            groups.append((f"prefix scan · paths, {order} build (n = {pn:,})",
                           twin_rows(r, "ratio_expanse_over_{twin}", "")))
    ratio_chart("chart_scan.svg", "Traversal and prefix scan · Expanse vs radix tries",
                f"full traversal: n = {n:,} u64 draws, generator order · prefix scan: 64 prefixes of "
                f"35-byte-prefixed paths · workload: patricia_scan", groups)


def string_chart() -> None:
    sl = load("string_lookup")["results"]
    pops = sorted({r["population"] for r in sl})
    plens = sorted({r["prefix_len"] for r in sl})
    vals = [v for r in sl if r["order"] == "generator" for t, _, _ in TWINS
            if f"ratio_expanse_over_{t}" in r for v in r[f"ratio_expanse_over_{t}_ci"]]
    d0, d1 = log_axis(vals)
    if d0 == d1:
        d1 += 1
    panel_w, panel_h, top = 300, 220, 110
    height = top + panel_h + 70
    out = [svg_header(W, height, "String lookup vs shared-prefix length"),
           f'  <text x="{LABEL_X}" y="30" class="t-title">String lookup vs shared-prefix length · Expanse vs radix tries</text>',
           f'  <text x="{LABEL_X}" y="46" class="t-sub">ratio = Expanse ns / twin ns, 50% hit, generator build order · '
           f'workload: patricia_string_lookup</text>',
           f'  <text x="{LABEL_X}" y="61" class="t-note">Log scale. Below 1 = Expanse faster. Points: geometric mean; '
           f'bars: BCa 95% interval.</text>']
    lx = LABEL_X
    for _, lab, cls in TWINS:
        out.append(f'  <rect x="{lx}" y="72" width="10" height="10" rx="2" class="{cls}"/>'
                   f'<text x="{lx + 14}" y="81" class="t-legend">vs {esc(lab)}</text>')
        lx += 150
    for pi, n in enumerate(pops):
        px0 = 60 + pi * (panel_w + 20)
        px1 = px0 + panel_w - 30
        py0, py1 = top, top + panel_h

        def xs(i: int) -> float:
            return px0 + i / max(len(plens) - 1, 1) * (px1 - px0)

        def ys(v: float) -> float:
            return py1 - (math.log10(v) - d0) / (d1 - d0) * (py1 - py0)

        out.append(f'  <text x="{px0}" y="{py0 - 10}" class="t-bar-label">n = {n:,}</text>')
        for d in range(d0, d1 + 1):
            yv = ys(10 ** d)
            out.append(f'  <line x1="{px0}" y1="{yv:.1f}" x2="{px1}" y2="{yv:.1f}" class="{"axis" if d == 0 else "grid"}"/>')
            out.append(f'  <text x="{px0 - 6}" y="{yv + 3:.1f}" class="t-axis-label" text-anchor="end">{10 ** d:g}</text>')
        for i, pl in enumerate(plens):
            out.append(f'  <text x="{xs(i):.1f}" y="{py1 + 16}" class="t-axis-label" text-anchor="middle">{pl} B</text>')
        for twin, _, cls in TWINS:
            pts = []
            for i, pl in enumerate(plens):
                r = next((r for r in sl if r["population"] == n and r["prefix_len"] == pl
                          and r["order"] == "generator"), None)
                t = ratio_of(r, f"ratio_expanse_over_{twin}") if r else None
                if t:
                    pts.append((xs(i), t))
            if not pts:
                continue
            stroke = cls.replace("b-", "")
            path = " ".join(f"{'M' if j == 0 else 'L'}{px:.1f},{ys(v):.1f}" for j, (px, (v, _, _)) in enumerate(pts))
            out.append(f'  <path d="{path}" class="ln-{stroke}" fill="none"/>')
            for px, (v, lo, hi) in pts:
                out.append(f'  <line x1="{px:.1f}" y1="{ys(lo):.1f}" x2="{px:.1f}" y2="{ys(hi):.1f}" class="axis"/>')
                out.append(f'  <circle cx="{px:.1f}" cy="{ys(v):.1f}" r="3.2" class="{cls}"/>')
    out.append(f'  <text x="{LABEL_X}" y="{height - 16}" class="t-note">x axis: shared-prefix length in bytes '
               f'(key = prefix + 12 hex digits).</text>')
    svg = "\n".join(out) + "\n" + svg_footer()
    # Line strokes reuse the twin palette.
    style = ("<style>.ln-patricia{stroke:#2563eb;stroke-width:1.6px}.ln-radix{stroke:#d97706;stroke-width:1.6px}"
             ".ln-qp{stroke:#7c3aed;stroke-width:1.6px}</style>")
    svg = svg.replace("</defs>", f"  {style}\n  </defs>", 1)
    save("chart_string.svg", svg)


def memory_chart(key_type: str, name: str, title: str) -> None:
    mem = [r for r in load("memory")["results"] if r["key_type"] == key_type]
    n = max(r["population"] for r in mem)
    arms = (("expanse", "Expanse", "b-expanse"),) + TWINS
    groups, dropped = [], {}
    for r in mem:
        if r["population"] != n:
            continue
        if key_type == "u64":
            label, orders = r["distribution"], ("generator", "shuffled")
        else:
            label, orders = f"prefix {r['prefix_len']} B", ("generator", "sorted")
        rows = []
        for a, lab, cls in arms:
            for o in orders:
                v = r.get(f"{a}_{o}_requested_bytes_per_key")
                if v is None:
                    dropped[lab] = dropped.get(lab, 0) + 1
                else:
                    rows.append((f"{lab} · {o}", cls, v))
        groups.append((label, rows))
    vmax = max(v for _, rows in groups for _, _, v in rows)
    scale = (PLOT_X1 - PLOT_X0) / vmax
    note2 = "; ".join(f"{t} not drawn on {k} rows (failed validation)" for t, k in dropped.items())
    row_h, gap, top = 13, 10, 80 + (12 if note2 else 0)
    height = top + sum(len(r) for _, r in groups) * row_h + len(groups) * (gap + 16) + 20
    out = [svg_header(W, height, esc(title)),
           f'  <text x="{LABEL_X}" y="30" class="t-title">{esc(title)}</text>',
           f'  <text x="{LABEL_X}" y="46" class="t-sub">n = {n:,}, requested bytes per key, exact allocator counts, '
           f'both build orders · lower is better · workload: patricia_memory</text>',
           f'  <text x="{LABEL_X}" y="61" class="t-note">Linear scale from 0 to the largest cell. Usable bytes '
           f'are in the tables; neither includes the allocator\'s per-chunk header.</text>']
    if note2:
        out.append(f'  <text x="{LABEL_X}" y="74" class="t-note">{esc(note2)}.</text>')
    y = top
    for glabel, rows in groups:
        out.append(f'  <text x="{LABEL_X}" y="{y + 10}" class="t-bar-label">{esc(glabel)}</text>')
        y += 16
        for rlabel, cls, v in rows:
            out.append(f'  <text x="{LABEL_X + 10}" y="{y + 9}" class="t-axis-label">{esc(rlabel)}</text>')
            out.append(f'  <rect x="{PLOT_X0}" y="{y + 1}" width="{max(v * scale, 0.5):.1f}" height="{row_h - 3}" '
                       f'rx="1.5" class="{cls}"/>')
            out.append(f'  <text x="{VALUE_X}" y="{y + 9}" class="t-unit" text-anchor="end">{v:.1f} B/key</text>')
            y += row_h
        y += gap
    out.append(svg_footer())
    save(name, "\n".join(out))


CHARTS = ("chart_lookup.svg", "chart_insert.svg", "chart_scan.svg", "chart_string.svg",
          "chart_memory_u64.svg", "chart_memory_paths.svg")


def main() -> None:
    lookup_chart()
    insert_chart()
    scan_chart()
    string_chart()
    memory_chart("u64", "chart_memory_u64.svg", "Live heap per key · u64 keys")
    memory_chart("prefixed_path", "chart_memory_paths.svg", "Live heap per key · shared-prefix string keys")


if __name__ == "__main__":
    main()
