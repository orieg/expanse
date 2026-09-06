#!/usr/bin/env python3
"""Dual-theme SVGs for the Masstree comparison suite, derived entirely from results/.

Every axis bound, tick, point position and label offset is computed from the
data the chart renders; none is hardcoded (§8.15a). Charts:

  chart_memory_curve.svg        bytes/key against expanse occupancy, both arms,
                                allocator instrument, with Masstree's structural
                                line beside its slab-quantized allocator line.
  chart_latency_1m.svg          integer latency pillars at N = 1,000,000 as a ratio
                                per cell with its BCa interval.
  chart_string_latency.svg      the same for the string cells at the head population.
  chart_string_memory_sweep.svg allocator B/key across the string population sweep.
  chart_concurrent_writers.svg  C1 writer throughput per cell, both concurrent arms.
  chart_concurrent_readers.svg  C2 reader throughput per cell.
  chart_lookup_hit / lookup_miss / insert / scan / memory .svg — the house
                                per-pillar bar charts every comparative suite ships.
"""

import json
import math
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from theme import svg_header  # noqa: E402

BASE = Path(__file__).resolve().parent.parent
RESULTS = Path(os.environ["MASSTREE_RESULTS_DIR"]) if os.environ.get("MASSTREE_RESULTS_DIR") else BASE / "results"

W, H = 960, 420
PAD_L, PAD_R, PAD_T, PAD_B = 70, 230, 66, 54
GREEN, BLUE, AMBER = "#16a34a", "#2563eb", "#d97706"


def esc(s: str) -> str:
    return str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def nice_ceiling(v: float) -> float:
    if v <= 0:
        return 1.0
    exp = math.floor(math.log10(v))
    base = 10 ** exp
    for m in (1, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10):
        if m * base >= v:
            return m * base
    return 10 * base


def load(name: str):
    p = RESULTS / name
    return json.loads(p.read_text()) if p.exists() else None


def ratio_chart(cells: list, title: str, sub: str, ratio_key: str, lo_key: str, hi_key: str,
                label_fn, footer: str, higher_is_expanse: bool = True) -> str:
    """Ratio + BCa interval per row; colour from the interval, never the point."""
    row_h = 17
    height = PAD_T + len(cells) * row_h + 70
    labels = [label_fn(c) for c in cells]
    pad_l = max(PAD_L, int(max(len(s) for s in labels) * 5.6) + 20)
    plot_w = W - pad_l - PAD_R
    hi = nice_ceiling(max(c[hi_key] for c in cells) * 1.05)

    def sx(v: float) -> float:
        return pad_l + v / hi * plot_w

    out = [svg_header(W, height, title)]
    out.append(f'<rect class="bg" x="0" y="0" width="{W}" height="{height}"/>')
    out.append(f'<text class="t-title" x="{pad_l}" y="26">{esc(title.upper())}</text>')
    out.append(f'<text class="t-sub" x="{pad_l}" y="43">{esc(sub)}</text>')
    step = 1 if hi <= 6 else (2 if hi <= 12 else 5)
    i = 0
    while i <= hi:
        x = sx(i)
        out.append(f'<line class="grid" x1="{x:.1f}" y1="{PAD_T-6}" x2="{x:.1f}" y2="{PAD_T+len(cells)*row_h}"/>')
        out.append(f'<text class="t-axis-label" x="{x:.1f}" y="{PAD_T+len(cells)*row_h+16:.1f}" text-anchor="middle">{i:g}×</text>')
        i += step
    xp = sx(1.0)
    out.append(f'<line class="axis" x1="{xp:.1f}" y1="{PAD_T-6}" x2="{xp:.1f}" y2="{PAD_T+len(cells)*row_h}"/>')
    for i, c in enumerate(cells):
        y = PAD_T + i * row_h + row_h / 2
        exp_wins = c[lo_key] > 1.0
        mt_wins = c[hi_key] < 1.0
        colour = GREEN if exp_wins else (BLUE if mt_wins else AMBER)
        out.append(f'<line x1="{sx(c[lo_key]):.1f}" y1="{y:.1f}" x2="{sx(min(c[hi_key], hi)):.1f}" y2="{y:.1f}" '
                   f'stroke="{colour}" stroke-width="2" opacity="0.55"/>')
        out.append(f'<circle cx="{sx(c[ratio_key]):.1f}" cy="{y:.1f}" r="3.2" fill="{colour}"/>')
        out.append(f'<text class="t-axis-label" x="{pad_l-8}" y="{y+3:.1f}" text-anchor="end">{esc(labels[i])}</text>')
        out.append(f'<text class="t-legend" x="{pad_l+plot_w+14}" y="{y+4:.1f}" fill="{colour}">{c[ratio_key]:.3f}×</text>')
    out.append(f'<text class="t-note" x="{pad_l}" y="{height-24}">Amber = interval spans parity (BOUNDARY_RESULT); '
               f"blue = Masstree faster; green = Expanse faster.</text>")
    if footer:
        out.append(f'<text class="t-note" x="{pad_l}" y="{height-10}">{esc(footer)}</text>')
    out.append("</svg>")
    return "\n".join(out)


def memory_curve() -> str:
    data = load("baseline_memory.json")
    cells = sorted((c for c in data["cells"] if c["dist"] == "random"), key=lambda c: c["lambda"])
    xs = [c["lambda"] for c in cells]
    series = [
        ("masstree_alloc_bytes_per_key", BLUE, "Masstree, allocator (slab-quantized)", "4 0"),
        ("masstree_structural_bytes_per_key", BLUE, "Masstree, own node census", "5 4"),
        ("expanse_alloc_bytes_per_key", GREEN, "ExpanseMap, allocator", "4 0"),
        ("expanse_mem_used_bytes_per_key", GREEN, "ExpanseMap, own mem_used()", "5 4"),
    ]
    ys = [c[k] for c in cells for k, _, _, _ in series]
    y_hi = nice_ceiling(max(ys) * 1.08)
    plot_w, plot_h = W - PAD_L - PAD_R, H - PAD_T - PAD_B
    lo, hi = math.log10(min(xs)), math.log10(max(xs))

    def sx(lam):
        return PAD_L + (math.log10(lam) - lo) / max(hi - lo, 1e-9) * plot_w

    def sy(v):
        return PAD_T + plot_h - v / y_hi * plot_h

    out = [svg_header(W, H, "Memory across expanse occupancy — Expanse vs Masstree")]
    out.append(f'<rect class="bg" x="0" y="0" width="{W}" height="{H}"/>')
    out.append(f'<text class="t-title" x="{PAD_L}" y="26">MEMORY ACROSS EXPANSE OCCUPANCY · INTEGER MAP</text>')
    out.append(f'<text class="t-sub" x="{PAD_L}" y="43">bytes per key — solid: one allocator instrument for both arms; '
               f"dashed: each engine's own node census (never mixed, §3.3)</text>")
    for i in range(6):
        v = y_hi * i / 5
        y = sy(v)
        out.append(f'<line class="grid" x1="{PAD_L}" y1="{y:.1f}" x2="{PAD_L+plot_w}" y2="{y:.1f}"/>')
        out.append(f'<text class="t-axis-label" x="{PAD_L-8}" y="{y+3:.1f}" text-anchor="end">{v:.0f}</text>')
    for lam in sorted({round(c["lambda_target"]) for c in cells}):
        x = sx(lam)
        out.append(f'<line class="axis" x1="{x:.1f}" y1="{PAD_T+plot_h}" x2="{x:.1f}" y2="{PAD_T+plot_h+4}"/>')
        out.append(f'<text class="t-axis-label" x="{x:.1f}" y="{PAD_T+plot_h+17:.1f}" text-anchor="middle">{lam}</text>')
    out.append(f'<text class="t-axis-label" x="{PAD_L+plot_w/2:.1f}" y="{H-16}" text-anchor="middle">'
               f"expanse occupancy λ = N / populated 2-byte-prefix expanses (log scale)</text>")
    out.append(f'<text class="t-axis-label" x="16" y="{PAD_T+plot_h/2:.1f}" text-anchor="middle" '
               f'transform="rotate(-90 16 {PAD_T+plot_h/2:.1f})">B / key</text>')
    out.append(f'<line class="axis" x1="{PAD_L}" y1="{PAD_T+plot_h}" x2="{PAD_L+plot_w}" y2="{PAD_T+plot_h}"/>')
    legend_y = PAD_T + 4
    for key, colour, name, dash in series:
        d = " ".join(f"{'M' if i == 0 else 'L'}{sx(c['lambda']):.1f},{sy(c[key]):.1f}" for i, c in enumerate(cells))
        out.append(f'<path d="{d}" fill="none" stroke="{colour}" stroke-width="2" stroke-dasharray="{dash}" stroke-linejoin="round"/>')
        for c in cells:
            hollow = key == "masstree_alloc_bytes_per_key" and c.get("masstree_quantum_dominated")
            fill = "none" if hollow else colour
            out.append(f'<circle cx="{sx(c["lambda"]):.1f}" cy="{sy(c[key]):.1f}" r="{3.2 if hollow else 2.4}" '
                       f'fill="{fill}" stroke="{colour}" stroke-width="1.5"/>')
        lx = PAD_L + plot_w + 18
        out.append(f'<line x1="{lx}" y1="{legend_y}" x2="{lx+18}" y2="{legend_y}" stroke="{colour}" stroke-width="2" stroke-dasharray="{dash}"/>')
        out.append(f'<text class="t-legend" x="{lx+24}" y="{legend_y+4}">{esc(name)}</text>')
        legend_y += 18
    flagged = [c for c in cells if c.get("masstree_quantum_dominated")]
    note = (f"Hollow markers: {len(flagged)} Masstree allocator cell(s) flagged QUANTUM_DOMINATED (slab slack over a quarter of structural)."
            if flagged else "No Masstree allocator cell is quantum-dominated on this sweep.")
    out.append(f'<text class="t-note" x="{PAD_L}" y="{H-4}">{esc(note)} Winner per cell derived by comparison in the README table.</text>')
    out.append("</svg>")
    return "\n".join(out)


def render_two_arm_chart(title: str, sub: str, unit: str, rows: list, lower_is_better: bool = True) -> str:
    """The house per-pillar chart. rows: (label, sublabel, expanse, masstree, verdict)."""
    if not rows:
        return ""
    max_val = max([max(r[2], r[3]) for r in rows] + [1e-9]) * 1.25
    bar_max, row_h, top = 280.0, 56, 96
    height = top + len(rows) * row_h + 24
    svg = svg_header(width=960, height=height, title=esc(title))
    better = "lower is better" if lower_is_better else "higher is better"
    svg += f"""
  <text x="30" y="34" class="t-title">{esc(title)}</text>
  <text x="30" y="50" class="t-sub">{esc(sub)} &#183; {esc(unit)} &#183; {better}</text>
  <g transform="translate(640, 20)">
    <rect x="0" y="0" width="10" height="10" rx="2" class="b-expanse"/>
    <text x="14" y="9" class="t-legend">Expanse</text>
    <rect x="90" y="0" width="10" height="10" rx="2" class="b-masstree"/>
    <text x="104" y="9" class="t-legend">Masstree</text>
  </g>
  <line x1="30" y1="66" x2="930" y2="66" class="divider"/>
"""
    for i, row in enumerate(rows):
        label, sublabel, exp, mt = row[0], row[1], row[2], row[3]
        verdict = row[4] if len(row) > 4 else None
        y = top + i * row_h
        w_exp = max(2.0, (exp / max_val) * bar_max)
        w_mt = max(2.0, (mt / max_val) * bar_max)
        svg += f"""  <text x="30" y="{y + 12}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 26}" class="t-sub">{esc(sublabel)}</text>
  <rect x="300" y="{y - 4}" width="{w_exp:.1f}" height="11" rx="2" class="b-expanse"/>
  <text x="{308 + w_exp:.1f}" y="{y + 5}" class="t-val-accent">{exp:.2f}</text>
  <rect x="300" y="{y + 13}" width="{w_mt:.1f}" height="11" rx="2" class="b-masstree"/>
  <text x="{308 + w_mt:.1f}" y="{y + 22}" class="t-val-blue">{mt:.2f}</text>
"""
        if lower_is_better:
            expanse_wins = exp <= mt
            ratio = (mt / exp) if expanse_wins and exp > 0 else (exp / mt if mt > 0 else 1.0)
        else:
            expanse_wins = exp >= mt
            ratio = (exp / mt) if expanse_wins and mt > 0 else (mt / exp if exp > 0 else 1.0)
        if verdict == "BOUNDARY_RESULT":
            cls, tcls, txt = "badge-loss", "badge-loss-text", "BOUNDARY"
        elif verdict == "QUANTUM_DOMINATED":
            cls, tcls, txt = "badge-loss", "badge-loss-text", "QUANTUM"
        elif expanse_wins:
            cls, tcls, txt = "badge-win", "badge-win-text", f"Expanse {ratio:.2f}x"
        else:
            cls, tcls, txt = "badge-loss", "badge-loss-text", f"Masstree {ratio:.2f}x"
        svg += f"""  <rect x="800" y="{y + 4}" width="130" height="20" rx="3" class="{cls}"/>
  <text x="865" y="{y + 18}" class="{tcls}">{esc(txt)}</text>
"""
    svg += "</svg>\n"
    return svg


def standard_charts() -> list:
    written = []
    lat = load("baseline_latency.json")
    mem = load("baseline_memory.json")
    if lat:
        cells = lat["cells"]
        n = max(c["population"] for c in cells)
        for pillar, fname, title in (("lookup_hit", "chart_lookup_hit.svg", "POINT LOOKUP — 100% HIT"),
                                     ("lookup_miss", "chart_lookup_miss.svg", "POINT LOOKUP — 50% HIT / 50% MISS"),
                                     ("insert", "chart_insert.svg", "INSERTION INTO A COLD STRUCTURE")):
            rows = [(f'{c["dist"]} · integer map', f'N = {c["population"]:,} · 64-bit keys',
                     c["expanse_ns_per_op_median"], c["masstree_ns_per_op_median"], c["verdict"])
                    for c in sorted((x for x in cells if x["pillar"] == pillar and x["population"] == n), key=lambda x: x["dist"])]
            svg = render_two_arm_chart(title, f"N = {n:,}, reference host", "ns / op", rows)
            if svg:
                (RESULTS / fname).write_text(svg)
                written.append(fname)
        rows = [(f'{c["dist"]} · k={c["scan_k"]}', f'N = {c["population"]:,}',
                 c["expanse_ns_per_op_median"], c["masstree_ns_per_op_median"], c["verdict"])
                for c in sorted((x for x in cells if x["pillar"] == "scan" and x["population"] == n),
                                key=lambda x: (x["dist"], x["scan_k"]))]
        svg = render_two_arm_chart("ORDERED RANGE SCAN", f"N = {n:,}, reference host", "ns / element", rows)
        if svg:
            (RESULTS / "chart_scan.svg").write_text(svg)
            written.append("chart_scan.svg")
        (RESULTS / "chart_latency_1m.svg").write_text(ratio_chart(
            sorted((c for c in cells if c["population"] == n and c["pillar"] != "scan"), key=lambda c: (c["pillar"], c["dist"])),
            f"Integer keys · latency at N = {n:,}", "Masstree ÷ Expanse, BCa 95% interval — right of the parity line means Expanse is faster",
            "masstree_over_expanse", "ci_lower", "ci_upper", lambda c: f'{c["pillar"]} · {c["dist"]}', "") + "\n")
        written.append("chart_latency_1m.svg")
    if mem:
        rows = []
        for c in sorted((x for x in mem["cells"] if x["dist"] == "random"), key=lambda x: x["lambda"]):
            if round(c["lambda_target"] or 0) not in (8, 15, 23, 30, 46):
                continue
            rows.append((f'λ ≈ {c["lambda_target"]:.0f} · integer map', f'N = {c["population"]:,} · allocator instrument',
                         c["expanse_alloc_bytes_per_key"], c["masstree_alloc_bytes_per_key"],
                         "QUANTUM_DOMINATED" if c.get("masstree_quantum_dominated") else None))
        svg = render_two_arm_chart("LIVE HEAP MEMORY", "selected occupancies — see chart_memory_curve.svg for the sweep and both instruments",
                                   "bytes / key", rows)
        if svg:
            (RESULTS / "chart_memory.svg").write_text(svg)
            written.append("chart_memory.svg")
        (RESULTS / "chart_memory_curve.svg").write_text(memory_curve() + "\n")
        written.append("chart_memory_curve.svg")
    return written


SHAPE_COLOUR = {"short": "#16a34a", "counter": "#0891b2", "prefixed": "#d97706", "skewed": "#7c3aed", "beyond": "#dc2626"}


def string_charts() -> list:
    written = []
    slat = load("baseline_string_latency.json")
    if slat:
        n_head = max(c["population"] for c in slat["cells"])
        all_cells = [c for c in slat["cells"] if abs(c["population"] - n_head) <= n_head * 0.01 and c["pillar"] != "scan"]
        cells = sorted((c for c in all_cells if c["masstree_over_expanse"] is not None), key=lambda c: (c["pillar"], c["dist"]))
        withheld = sorted({c["dist"] for c in all_cells if c["masstree_over_expanse"] is None})
        if cells:
            footer = (f"Masstree column withheld on {', '.join(withheld)}: keys exceed MASSTREE_MAXKEYLEN (§3.4); Expanse measured alone."
                      if withheld else "")
            (RESULTS / "chart_string_latency.svg").write_text(ratio_chart(
                cells, f"String keys · latency at N = {n_head:,}",
                "Masstree ÷ Expanse, BCa 95% interval — right of the parity line means Expanse is faster",
                "masstree_over_expanse", "ci_lower", "ci_upper", lambda c: f'{c["pillar"]} · {c["dist"]}', footer) + "\n")
            written.append("chart_string_latency.svg")
    smem = load("baseline_string_memory.json")
    if smem:
        cells = smem["cells"]
        shapes = [s for s in SHAPE_COLOUR if any(c["dist"] == s for c in cells)]
        xs = sorted({c["population"] for c in cells})
        # Flagged Masstree allocator points (QUANTUM_DOMINATED, §3.3) are the 2 MiB
        # slab divided by a small N, not the index; they are left off the line and
        # out of the axis ceiling, and Masstree's own node census is drawn instead.
        def mt_alloc_ok(c):
            return c["masstree_alloc_bytes_per_key"] is not None and not c.get("masstree_quantum_dominated")

        ys = ([c["expanse_alloc_bytes_per_key"] for c in cells]
              + [c["masstree_alloc_bytes_per_key"] for c in cells if mt_alloc_ok(c)]
              + [c["masstree_structural_bytes_per_key"] for c in cells if c["masstree_structural_bytes_per_key"] is not None])
        y_hi = nice_ceiling(max(ys) * 1.08)
        plot_w, plot_h = W - PAD_L - PAD_R, H - PAD_T - PAD_B
        lo, hi = math.log10(xs[0]), math.log10(xs[-1])

        def sx(n):
            return PAD_L + (math.log10(n) - lo) / max(hi - lo, 1e-9) * plot_w

        def sy(v):
            return PAD_T + plot_h - min(v, y_hi) / y_hi * plot_h

        out = [svg_header(W, H, "String keys — allocator bytes per key across population")]
        out.append(f'<rect class="bg" x="0" y="0" width="{W}" height="{H}"/>')
        out.append(f'<text class="t-title" x="{PAD_L}" y="26">STRING KEYS · ALLOCATOR BYTES PER KEY ACROSS POPULATION</text>')
        out.append(f'<text class="t-sub" x="{PAD_L}" y="43">solid = ExpanseStrMap allocator; dashed = Masstree allocator where not quantum-dominated; '
                   f"dotted = Masstree's own node census (§3.3)</text>")
        for i in range(6):
            v = y_hi * i / 5
            y = sy(v)
            out.append(f'<line class="grid" x1="{PAD_L}" y1="{y:.1f}" x2="{PAD_L+plot_w}" y2="{y:.1f}"/>')
            out.append(f'<text class="t-axis-label" x="{PAD_L-8}" y="{y+3:.1f}" text-anchor="end">{v:.0f}</text>')
        for n in [n for n in xs if n in (1_000, 10_000, 100_000, 1_000_000)] or xs:
            x = sx(n)
            lab = "1M" if n >= 1_000_000 else f"{n // 1000}k"
            out.append(f'<line class="axis" x1="{x:.1f}" y1="{PAD_T+plot_h}" x2="{x:.1f}" y2="{PAD_T+plot_h+4}"/>')
            out.append(f'<text class="t-axis-label" x="{x:.1f}" y="{PAD_T+plot_h+17:.1f}" text-anchor="middle">{lab}</text>')
        out.append(f'<text class="t-axis-label" x="{PAD_L+plot_w/2:.1f}" y="{H-16}" text-anchor="middle">population N (log scale)</text>')
        out.append(f'<text class="t-axis-label" x="16" y="{PAD_T+plot_h/2:.1f}" text-anchor="middle" transform="rotate(-90 16 {PAD_T+plot_h/2:.1f})">B / key</text>')
        out.append(f'<line class="axis" x1="{PAD_L}" y1="{PAD_T+plot_h}" x2="{PAD_L+plot_w}" y2="{PAD_T+plot_h}"/>')
        legend_y = PAD_T + 4
        for shape in shapes:
            pts = sorted((c for c in cells if c["dist"] == shape), key=lambda c: c["population"])
            colour = SHAPE_COLOUR[shape]
            for key, dash, name in (("expanse_alloc_bytes_per_key", "4 0", f"{shape} · Expanse"),
                                    ("masstree_alloc_bytes_per_key", "5 4", f"{shape} · Masstree allocator"),
                                    ("masstree_structural_bytes_per_key", "2 3", f"{shape} · Masstree nodes")):
                series = [c for c in pts if c[key] is not None
                          and (key != "masstree_alloc_bytes_per_key" or mt_alloc_ok(c))]
                if not series:
                    continue
                d = " ".join(f"{'M' if i == 0 else 'L'}{sx(c['population']):.1f},{sy(c[key]):.1f}" for i, c in enumerate(series))
                out.append(f'<path d="{d}" fill="none" stroke="{colour}" stroke-width="2" stroke-dasharray="{dash}" stroke-linejoin="round"/>')
                for c in series:
                    out.append(f'<circle cx="{sx(c["population"]):.1f}" cy="{sy(c[key]):.1f}" r="2.2" fill="{colour}"/>')
                lx = PAD_L + plot_w + 18
                out.append(f'<line x1="{lx}" y1="{legend_y}" x2="{lx+18}" y2="{legend_y}" stroke="{colour}" stroke-width="2" stroke-dasharray="{dash}"/>')
                out.append(f'<text class="t-legend" x="{lx+24}" y="{legend_y+4}">{esc(name)}</text>')
                legend_y += 17
        n_flag = sum(1 for c in cells if c["masstree_alloc_bytes_per_key"] is not None and not mt_alloc_ok(c))
        out.append(f'<text class="t-note" x="{PAD_L}" y="{H-4}">{n_flag} QUANTUM_DOMINATED Masstree allocator cell(s) left off the dashed lines (table carries them). '
                   f"Masstree has no line for a shape whose keys exceed 255 bytes (§3.4).</text>")
        out.append("</svg>")
        (RESULTS / "chart_string_memory_sweep.svg").write_text("\n".join(out) + "\n")
        written.append("chart_string_memory_sweep.svg")
    return written


def concurrent_charts() -> list:
    conc = load("baseline_concurrent.json")
    if not conc:
        return []
    cells = conc["throughput"]

    def rows_for(role, pillar):
        return [(f"{c['arm']} · W={c['writers']} R={c['readers']}", f"{c['workload_id']} · {c['rounds']} rounds",
                 c[f"expanse_{role}_mops_median"], c[f"masstree_{role}_mops_median"], c[f"{role}_verdict"])
                for c in cells if c["pillar"] == pillar and f"{role}_verdict" in c]

    written = []
    svg = render_two_arm_chart("Writer throughput as writer count scales — SyncExpanse vs Masstree",
                               "C1: W writers insert 2^20 fresh keys into a 2^20 prefill, fixed work, arms interleaved",
                               "M inserts/s (median of rounds)", rows_for("writer", "C1"), lower_is_better=False)
    if svg:
        (RESULTS / "chart_concurrent_writers.svg").write_text(svg)
        written.append("chart_concurrent_writers.svg")
    svg = render_two_arm_chart("Reader throughput alongside writers — SyncExpanse vs Masstree",
                               "C2: 8 readers probe 50/50 while W writers insert; W=0 is the reader-only reference",
                               "M lookups/s (median of rounds)", rows_for("reader", "C2"), lower_is_better=False)
    if svg:
        (RESULTS / "chart_concurrent_readers.svg").write_text(svg)
        written.append("chart_concurrent_readers.svg")
    return written


def main() -> int:
    RESULTS.mkdir(parents=True, exist_ok=True)
    written = standard_charts() + string_charts() + concurrent_charts()
    print("wrote " + (", ".join(written) if written else "nothing (no results present)"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
