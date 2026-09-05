#!/usr/bin/env python3
"""Dual-theme SVGs for the HOT comparison suite, derived entirely from results/.

Every axis bound, tick, point position and label offset below is computed from
the data the chart renders. None is hardcoded (§8.15a): every chart defect this
repo has found came from geometry fixed for the values that existed when the
chart was written, and this suite's headline figure is precisely one that moves —
a bar ceiling pinned for a 12 B/key world would clip the moment a re-measured arm
reaches 22.

Two charts:

  chart_memory_curve.svg — bytes/key against expanse occupancy, both arms. This
  is the suite's headline: Arm A's winner changes three times across the range,
  which a table states and a plot shows.

  chart_latency_1m.svg — the latency pillars at N = 1,000,000, as a ratio per
  cell with its BCa interval.
"""

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from theme import svg_header  # noqa: E402

BASE = Path(__file__).resolve().parent.parent
# HOT_RESULTS_DIR lets the string charts be exercised against a quick-mode
# directory without touching the committed results (§8.5).
RESULTS = Path(os.environ["HOT_RESULTS_DIR"]) if os.environ.get("HOT_RESULTS_DIR") else BASE / "results"

W, H = 960, 420
PAD_L, PAD_R, PAD_T, PAD_B = 70, 210, 66, 54


def esc(s: str) -> str:
    return (str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))


def nice_ceiling(v: float) -> float:
    """Smallest 'round' number at or above `v`, derived from its magnitude."""
    if v <= 0:
        return 1.0
    import math

    exp = math.floor(math.log10(v))
    base = 10**exp
    for m in (1, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10):
        if m * base >= v:
            return m * base
    return 10 * base


def memory_curve() -> str:
    import math

    data = json.loads((RESULTS / "baseline_memory_curve.json").read_text())
    cells = data["cells"]
    arms = {}
    for c in cells:
        arms.setdefault(c["arm"], []).append(c)
    for v in arms.values():
        v.sort(key=lambda c: c["lambda"])

    xs = [c["lambda"] for c in cells]
    ys = [c["hot_alloc_bytes_per_key"] for c in cells] + [
        c["expanse_alloc_bytes_per_key"] for c in cells
    ]
    x_lo, x_hi = min(xs), max(xs)
    y_hi = nice_ceiling(max(ys) * 1.08)

    plot_w = W - PAD_L - PAD_R
    plot_h = H - PAD_T - PAD_B

    def sx(lam: float) -> float:
        # Occupancy spans ~1..61 and the interesting structure is at the low end,
        # so the axis is logarithmic. Stated on the axis label, not left implicit.
        lo, hi = math.log10(x_lo), math.log10(x_hi)
        return PAD_L + (math.log10(lam) - lo) / (hi - lo) * plot_w

    def sy(v: float) -> float:
        return PAD_T + plot_h - (v / y_hi) * plot_h

    out = [svg_header(W, H, "Memory across expanse occupancy — Expanse vs HOT")]
    out.append(f'<rect class="bg" x="0" y="0" width="{W}" height="{H}"/>')
    out.append(
        f'<text class="t-title" x="{PAD_L}" y="26">MEMORY ACROSS EXPANSE OCCUPANCY</text>'
    )
    out.append(
        f'<text class="t-sub" x="{PAD_L}" y="43">bytes per key, live heap held from the C allocator — '
        f"one instrument, both arms</text>"
    )

    # Y gridlines and labels, spaced from the derived ceiling.
    steps = 5
    for i in range(steps + 1):
        v = y_hi * i / steps
        y = sy(v)
        out.append(f'<line class="grid" x1="{PAD_L}" y1="{y:.1f}" x2="{PAD_L+plot_w}" y2="{y:.1f}"/>')
        out.append(f'<text class="t-axis-label" x="{PAD_L-8}" y="{y+3:.1f}" text-anchor="end">{v:.0f}</text>')

    # X ticks at the measured occupancies.
    for lam in sorted({round(c["lambda_target"]) for c in cells}):
        x = sx(lam)
        out.append(f'<line class="axis" x1="{x:.1f}" y1="{PAD_T+plot_h}" x2="{x:.1f}" y2="{PAD_T+plot_h+4}"/>')
        out.append(
            f'<text class="t-axis-label" x="{x:.1f}" y="{PAD_T+plot_h+17:.1f}" text-anchor="middle">{lam}</text>'
        )
    out.append(
        f'<text class="t-axis-label" x="{PAD_L+plot_w/2:.1f}" y="{H-16}" text-anchor="middle">'
        f"expanse occupancy λ = N / populated 2-byte-prefix expanses (log scale)</text>"
    )
    out.append(
        f'<text class="t-axis-label" x="16" y="{PAD_T+plot_h/2:.1f}" text-anchor="middle" '
        f'transform="rotate(-90 16 {PAD_T+plot_h/2:.1f})">B / key</text>'
    )
    out.append(f'<line class="axis" x1="{PAD_L}" y1="{PAD_T+plot_h}" x2="{PAD_L+plot_w}" y2="{PAD_T+plot_h}"/>')

    series = [
        ("set", "expanse_alloc_bytes_per_key", "#16a34a", "ExpanseSet (Arm A)", "4 0"),
        ("set", "hot_alloc_bytes_per_key", "#2563eb", "HOT set (Arm A)", "4 0"),
        ("map", "expanse_alloc_bytes_per_key", "#16a34a", "ExpanseMap (Arm B)", "5 4"),
        ("map", "hot_alloc_bytes_per_key", "#2563eb", "HOT map (Arm B)", "5 4"),
    ]
    legend_y = PAD_T + 4
    for arm, key, colour, label, dash in series:
        pts = arms.get(arm, [])
        if not pts:
            continue
        d = " ".join(
            f"{'M' if i == 0 else 'L'}{sx(c['lambda']):.1f},{sy(c[key]):.1f}"
            for i, c in enumerate(pts)
        )
        out.append(
            f'<path d="{d}" fill="none" stroke="{colour}" stroke-width="2" '
            f'stroke-dasharray="{dash}" stroke-linejoin="round"/>'
        )
        for c in pts:
            out.append(
                f'<circle cx="{sx(c["lambda"]):.1f}" cy="{sy(c[key]):.1f}" r="2.6" fill="{colour}"/>'
            )
        lx = PAD_L + plot_w + 18
        out.append(f'<line x1="{lx}" y1="{legend_y}" x2="{lx+18}" y2="{legend_y}" stroke="{colour}" '
                   f'stroke-width="2" stroke-dasharray="{dash}"/>')
        out.append(f'<text class="t-legend" x="{lx+24}" y="{legend_y+4}">{esc(label)}</text>')
        legend_y += 20

    # Annotate the band where Arm A's winner is Expanse — derived by comparison,
    # not by eye.
    set_pts = arms.get("set", [])
    win = [c for c in set_pts if c["expanse_alloc_bytes_per_key"] < c["hot_alloc_bytes_per_key"]]
    if win:
        x0, x1 = sx(win[0]["lambda"]), sx(win[-1]["lambda"])
        out.append(
            f'<rect x="{x0:.1f}" y="{PAD_T}" width="{max(1.0, x1-x0):.1f}" height="{plot_h}" '
            f'fill="#16a34a" opacity="0.07"/>'
        )
        out.append(
            f'<text class="t-note" x="{(x0+x1)/2:.1f}" y="{PAD_T-8}" text-anchor="middle">'
            f"Arm A: Expanse wins only here</text>"
        )

    out.append(
        f'<text class="t-note" x="{PAD_L}" y="{H-4}">HOT is flat across the range; Expanse crosses the '
        f"LEAF_CAP cascade. Winner changes three times.</text>"
    )
    out.append("</svg>")
    return "\n".join(out)


def latency_1m() -> str:
    data = json.loads((RESULTS / "baseline_latency.json").read_text())
    cells = [
        c for c in data["cells"] if c["population"] == 1_000_000 and c["pillar"] != "scan"
    ]
    cells.sort(key=lambda c: (c["pillar"], c["arm"], c["dist"]))
    if not cells:
        raise SystemExit("no 1M non-scan cells in results")

    row_h = 17
    height = PAD_T + len(cells) * row_h + 56

    # The left margin is derived from the longest row label, not fixed. The
    # first version of this chart inherited the memory chart's PAD_L=70 and
    # clipped every label off the canvas — "insert · map · clustered" rendered
    # as "· clustered". `check_chart_layout.py` passed it, which is what §8.15a
    # means by verifying visually as well: the estimator is a gross-breakage
    # tripwire, not a layout engine.
    labels = [f'{c["pillar"]} · {c["arm"]} · {c["dist"]}' for c in cells]
    label_px = max(len(s) for s in labels) * 5.6  # 9.5px monospace, ~0.59em/char
    pad_l = max(PAD_L, int(label_px) + 20)

    plot_w = W - pad_l - PAD_R
    hi = nice_ceiling(max(c["ci_upper"] for c in cells) * 1.05)
    lo = 0.0

    def sx(v: float) -> float:
        return pad_l + (v - lo) / (hi - lo) * plot_w

    out = [svg_header(W, height, "Latency at N=1M — HOT / Expanse ratio with BCa 95% intervals")]
    out.append(f'<rect class="bg" x="0" y="0" width="{W}" height="{height}"/>')
    out.append(f'<text class="t-title" x="{pad_l}" y="26">LATENCY AT N = 1,000,000</text>')
    out.append(
        f'<text class="t-sub" x="{pad_l}" y="43">HOT ÷ Expanse, BCa 95% interval — right of the '
        f"parity line means Expanse is faster</text>"
    )

    for i in range(int(hi) + 1):
        x = sx(i)
        out.append(f'<line class="grid" x1="{x:.1f}" y1="{PAD_T-6}" x2="{x:.1f}" y2="{PAD_T+len(cells)*row_h}"/>')
        out.append(f'<text class="t-axis-label" x="{x:.1f}" y="{PAD_T+len(cells)*row_h+16:.1f}" '
                   f'text-anchor="middle">{i}×</text>')
    xp = sx(1.0)
    out.append(f'<line class="axis" x1="{xp:.1f}" y1="{PAD_T-6}" x2="{xp:.1f}" y2="{PAD_T+len(cells)*row_h}"/>')

    for i, c in enumerate(cells):
        y = PAD_T + i * row_h + row_h / 2
        expanse_wins = c["ci_lower"] > 1.0
        colour = "#16a34a" if expanse_wins else ("#2563eb" if c["ci_upper"] < 1.0 else "#d97706")
        out.append(f'<line x1="{sx(c["ci_lower"]):.1f}" y1="{y:.1f}" x2="{sx(c["ci_upper"]):.1f}" '
                   f'y2="{y:.1f}" stroke="{colour}" stroke-width="2" opacity="0.55"/>')
        out.append(f'<circle cx="{sx(c["hot_over_expanse"]):.1f}" cy="{y:.1f}" r="3.2" fill="{colour}"/>')
        label = f'{c["pillar"]} · {c["arm"]} · {c["dist"]}'
        out.append(f'<text class="t-axis-label" x="{pad_l-8}" y="{y+3:.1f}" text-anchor="end">{esc(label)}</text>')
        out.append(f'<text class="t-legend" x="{pad_l+plot_w+14}" y="{y+4:.1f}" fill="{colour}">'
                   f'{c["hot_over_expanse"]:.3f}×</text>')

    out.append(f'<text class="t-note" x="{pad_l}" y="{height-10}">'
               f"Amber = interval spans parity (BOUNDARY_RESULT); blue = HOT faster; green = Expanse faster.</text>")
    out.append("</svg>")
    return "\n".join(out)


def render_two_arm_chart(
    out_name: str, title: str, sub: str, unit: str, rows: list, lower_is_better: bool = True
) -> str:
    """The house per-pillar chart, in the shape `art_comparison/` established.

    rows: list of (label, sublabel, expanse_value, hot_value, verdict). Bar
    lengths and the axis ceiling come from the rows (§8.15a); the badge ratio is
    computed, never written by hand.

    The badge is driven by the cell's **verdict**, not by which bar is shorter. A
    cell whose BCa interval spans parity gets a neutral `BOUNDARY` badge and
    claims no winner (§8.4). The first version of this chart compared the two
    point estimates and rendered a green "Expanse 1.00x" win for
    `lookup_hit · set · random`, whose interval is [0.982, 0.997] around 0.998 —
    a `BOUNDARY_RESULT` drawn as a victory.
    """
    if not rows:
        return ""
    max_val = max([max(r[2], r[3]) for r in rows] + [1e-9]) * 1.25
    bar_max = 280.0
    row_h = 56
    top = 96
    height = top + len(rows) * row_h + 24

    svg = svg_header(width=960, height=height, title=esc(title))
    better = "lower is better" if lower_is_better else "higher is better"
    svg += f"""
  <text x="30" y="34" class="t-title">{esc(title)}</text>
  <text x="30" y="50" class="t-sub">{esc(sub)} &#183; {esc(unit)} &#183; {better}</text>
  <g transform="translate(660, 20)">
    <rect x="0" y="0" width="10" height="10" rx="2" class="b-expanse"/>
    <text x="14" y="9" class="t-legend">Expanse</text>
    <rect x="90" y="0" width="10" height="10" rx="2" class="b-hot"/>
    <text x="104" y="9" class="t-legend">HOT</text>
  </g>
  <line x1="30" y1="66" x2="930" y2="66" class="divider"/>
"""
    for i, row in enumerate(rows):
        label, sublabel, exp, hot = row[0], row[1], row[2], row[3]
        verdict = row[4] if len(row) > 4 else None
        y = top + i * row_h
        w_exp = max(2.0, (exp / max_val) * bar_max)
        w_hot = max(2.0, (hot / max_val) * bar_max)
        svg += f"""  <text x="30" y="{y + 12}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 26}" class="t-sub">{esc(sublabel)}</text>
  <rect x="300" y="{y - 4}" width="{w_exp:.1f}" height="11" rx="2" class="b-expanse"/>
  <text x="{308 + w_exp:.1f}" y="{y + 5}" class="t-val-accent">{exp:.2f}</text>
  <rect x="300" y="{y + 13}" width="{w_hot:.1f}" height="11" rx="2" class="b-hot"/>
  <text x="{308 + w_hot:.1f}" y="{y + 22}" class="t-val-blue">{hot:.2f}</text>
"""
        if lower_is_better:
            expanse_wins = exp <= hot
            ratio = (hot / exp) if expanse_wins and exp > 0 else (exp / hot if hot > 0 else 1.0)
        else:
            expanse_wins = exp >= hot
            ratio = (exp / hot) if expanse_wins and hot > 0 else (hot / exp if exp > 0 else 1.0)

        if verdict == "BOUNDARY_RESULT":
            cls, tcls, txt = "badge-loss", "badge-loss-text", "BOUNDARY"
        elif expanse_wins:
            cls, tcls, txt = "badge-win", "badge-win-text", f"Expanse {ratio:.2f}x"
        else:
            cls, tcls, txt = "badge-loss", "badge-loss-text", f"HOT {ratio:.2f}x"
        svg += f"""  <rect x="800" y="{y + 4}" width="130" height="20" rx="3" class="{cls}"/>
  <text x="865" y="{y + 18}" class="{tcls}">{esc(txt)}</text>
"""
    svg += "</svg>\n"
    return svg


def standard_charts() -> list:
    """The five per-pillar charts every comparative suite in this repo ships."""
    lat = json.loads((RESULTS / "baseline_latency.json").read_text())["cells"]
    mem = json.loads((RESULTS / "baseline_memory_curve.json").read_text())["cells"]
    written = []

    arm_name = {"set": "Arm A · set", "map": "Arm B · map"}
    for pillar, fname, label in (
        ("lookup_hit", "chart_lookup_hit.svg", "Point Lookup — 100% Hit"),
        ("lookup_miss", "chart_lookup_miss.svg", "Point Lookup — 50% Hit / 50% Miss"),
        ("insert", "chart_insert.svg", "Insertion Into A Cold Structure"),
    ):
        rows = []
        for c in sorted(
            (x for x in lat if x["pillar"] == pillar and x["population"] == 1_000_000),
            key=lambda x: (x["arm"], x["dist"]),
        ):
            rows.append(
                (
                    f'{c["dist"]} · {arm_name[c["arm"]]}',
                    f'N = {c["population"]:,} · {c["keyspace_bits"]}-bit keys',
                    c["expanse_ns_per_op_median"],
                    c["hot_ns_per_op_median"],
                    c["verdict"],
                )
            )
        svg = render_two_arm_chart(fname, label.upper(), "N = 1,000,000, reference host", "ns / op", rows)
        if svg:
            (RESULTS / fname).write_text(svg)
            written.append(fname)

    rows = []
    for c in sorted(
        (x for x in lat if x["pillar"] == "scan" and x["population"] == 1_000_000),
        key=lambda x: (x["arm"], x["dist"], x["scan_k"]),
    ):
        rows.append(
            (
                f'{c["dist"]} · k={c["scan_k"]} · {arm_name[c["arm"]]}',
                f'N = {c["population"]:,}',
                c["expanse_ns_per_op_median"],
                c["hot_ns_per_op_median"],
                c["verdict"],
            )
        )
    svg = render_two_arm_chart(
        "chart_scan.svg", "ORDERED RANGE SCAN", "N = 1,000,000, reference host", "ns / element", rows
    )
    if svg:
        (RESULTS / "chart_scan.svg").write_text(svg)
        written.append("chart_scan.svg")

    # Memory at the occupancies either side of the cascade, so the bar chart
    # cannot be read as "the" memory answer — the curve chart carries that.
    rows = []
    for c in sorted(mem, key=lambda x: (x["arm"], x["lambda_target"])):
        if c["lambda_target"] not in (8.0, 15.0, 23.0, 30.0, 46.0):
            continue
        rows.append(
            (
                f'λ ≈ {c["lambda_target"]:.0f} · {arm_name[c["arm"]]}',
                f'N = {c["population"]:,} · {c["keyspace_bits"]}-bit keys',
                c["expanse_alloc_bytes_per_key"],
                c["hot_alloc_bytes_per_key"],
            )
        )
    svg = render_two_arm_chart(
        "chart_memory.svg",
        "LIVE HEAP MEMORY",
        "selected occupancies either side of the LEAF_CAP cascade — see chart_memory_curve.svg for the full sweep",
        "bytes / key",
        rows,
    )
    if svg:
        (RESULTS / "chart_memory.svg").write_text(svg)
        written.append("chart_memory.svg")
    return written


# ---------------------------------------------------------------------------
# String-key arms (#693, METHODOLOGY §10).
# ---------------------------------------------------------------------------

SHAPE_COLOUR = {
    "short": "#16a34a",
    "counter": "#0891b2",
    "prefixed": "#d97706",
    "skewed": "#7c3aed",
    "beyond": "#dc2626",
}


def string_latency() -> str:
    """Ratio + BCa interval per cell at the largest measured population, Arms C/D/E.

    Cells whose HOT column is withheld (§10.4, `beyond`) are not plotted as a
    ratio — there is none — and are listed in the footer instead, so the chart
    cannot be read as HOT having been measured on them.
    """
    data = json.loads((RESULTS / "baseline_string_latency.json").read_text())
    # Populations are post-deduplication counts (`skewed` at N = 1M holds
    # 998,150 keys), so the head population is selected by proximity, not
    # equality: every cell within 1% of the largest count belongs to it.
    n_head = max(c["population"] for c in data["cells"])
    all_cells = [c for c in data["cells"]
                 if abs(c["population"] - n_head) <= n_head * 0.01 and c["pillar"] != "scan"]
    cells = [c for c in all_cells if c["hot_over_expanse"] is not None]
    withheld = sorted({c["dist"] for c in all_cells if c["hot_over_expanse"] is None})
    cells.sort(key=lambda c: (c["pillar"], c["arm"], c["dist"]))
    if not cells:
        raise SystemExit("no string latency cells with a HOT column")

    row_h = 17
    height = PAD_T + len(cells) * row_h + 70
    labels = [f'{c["pillar"]} · {c["arm"]} · {c["dist"]}' for c in cells]
    pad_l = max(PAD_L, int(max(len(s) for s in labels) * 5.6) + 20)
    plot_w = W - pad_l - PAD_R
    hi = nice_ceiling(max(c["ci_upper"] for c in cells) * 1.05)

    def sx(v: float) -> float:
        return pad_l + v / hi * plot_w

    out = [svg_header(W, height, f"String keys, latency at N={n_head:,} — HOT / Expanse ratio with BCa 95% intervals")]
    out.append(f'<rect class="bg" x="0" y="0" width="{W}" height="{height}"/>')
    out.append(f'<text class="t-title" x="{pad_l}" y="26">STRING KEYS · LATENCY AT N = {n_head:,}</text>')
    out.append(f'<text class="t-sub" x="{pad_l}" y="43">HOT ÷ Expanse, BCa 95% interval — right of the '
               f"parity line means Expanse is faster</text>")
    step = 1 if hi <= 6 else (2 if hi <= 12 else 5)
    for i in range(0, int(hi) + 1, step):
        x = sx(i)
        out.append(f'<line class="grid" x1="{x:.1f}" y1="{PAD_T-6}" x2="{x:.1f}" y2="{PAD_T+len(cells)*row_h}"/>')
        out.append(f'<text class="t-axis-label" x="{x:.1f}" y="{PAD_T+len(cells)*row_h+16:.1f}" '
                   f'text-anchor="middle">{i}×</text>')
    xp = sx(1.0)
    out.append(f'<line class="axis" x1="{xp:.1f}" y1="{PAD_T-6}" x2="{xp:.1f}" y2="{PAD_T+len(cells)*row_h}"/>')
    for i, c in enumerate(cells):
        y = PAD_T + i * row_h + row_h / 2
        colour = "#16a34a" if c["ci_lower"] > 1.0 else ("#2563eb" if c["ci_upper"] < 1.0 else "#d97706")
        out.append(f'<line x1="{sx(c["ci_lower"]):.1f}" y1="{y:.1f}" x2="{sx(c["ci_upper"]):.1f}" '
                   f'y2="{y:.1f}" stroke="{colour}" stroke-width="2" opacity="0.55"/>')
        out.append(f'<circle cx="{sx(c["hot_over_expanse"]):.1f}" cy="{y:.1f}" r="3.2" fill="{colour}"/>')
        out.append(f'<text class="t-axis-label" x="{pad_l-8}" y="{y+3:.1f}" text-anchor="end">{esc(labels[i])}</text>')
        out.append(f'<text class="t-legend" x="{pad_l+plot_w+14}" y="{y+4:.1f}" fill="{colour}">'
                   f'{c["hot_over_expanse"]:.3f}×</text>')
    out.append(f'<text class="t-note" x="{pad_l}" y="{height-24}">'
               f"Amber = interval spans parity (BOUNDARY_RESULT); blue = HOT faster; green = Expanse faster.</text>")
    if withheld:
        out.append(f'<text class="t-note" x="{pad_l}" y="{height-10}">HOT column withheld on '
                   f"{', '.join(withheld)}: every key exceeds HOT's 255-byte window (§10.4); Expanse measured alone.</text>")
    out.append("</svg>")
    return "\n".join(out)


def _n_label(n: int) -> str:
    return "1M" if n >= 1_000_000 else f"{n // 1000}k"


def string_memory_sweep() -> str:
    """Arm C ownership bytes/key across the population sweep, per shape.

    Solid = Expanse (its index, which is also its ownership); dashed = HOT
    ownership (index plus the string table its leaves point at). Where HOT's
    column is withheld the shape has an Expanse line only. Axis bounds derive
    from the data (§8.15a).
    """
    import math

    data = json.loads((RESULTS / "baseline_string_memory.json").read_text())
    cells = [c for c in data["cells"] if c["arm"] == "ptr"]
    if not cells:
        raise SystemExit("no Arm C memory cells")
    shapes = [s for s in SHAPE_COLOUR if any(c["dist"] == s for c in cells)]
    xs = sorted({c["population"] for c in cells})
    ys = [c["expanse_ownership_bytes_per_key"] for c in cells] + [
        c["hot_ownership_bytes_per_key"] for c in cells if c["hot_ownership_bytes_per_key"] is not None
    ]
    y_hi = nice_ceiling(max(ys) * 1.08)
    plot_w = W - PAD_L - PAD_R
    plot_h = H - PAD_T - PAD_B
    lo, hi = math.log10(xs[0]), math.log10(xs[-1])

    def sx(n: int) -> float:
        return PAD_L + (math.log10(n) - lo) / max(hi - lo, 1e-9) * plot_w

    def sy(v: float) -> float:
        return PAD_T + plot_h - v / y_hi * plot_h

    out = [svg_header(W, H, "String keys, Arm C — ownership bytes per key across population")]
    out.append(f'<rect class="bg" x="0" y="0" width="{W}" height="{H}"/>')
    out.append(f'<text class="t-title" x="{PAD_L}" y="26">STRING KEYS · ARM C · OWNERSHIP BYTES PER KEY</text>')
    out.append(f'<text class="t-sub" x="{PAD_L}" y="43">solid = ExpanseStrMap (copies its keys); '
               f"dashed = HOT index + the harness-owned strings its leaves point at (§10.3)</text>")
    steps = 5
    for i in range(steps + 1):
        v = y_hi * i / steps
        y = sy(v)
        out.append(f'<line class="grid" x1="{PAD_L}" y1="{y:.1f}" x2="{PAD_L+plot_w}" y2="{y:.1f}"/>')
        out.append(f'<text class="t-axis-label" x="{PAD_L-8}" y="{y+3:.1f}" text-anchor="end">{v:.0f}</text>')
    ticks = [n for n in xs if n in (1_000, 10_000, 100_000, 1_000_000)] or xs
    for n in ticks:
        x = sx(n)
        out.append(f'<line class="axis" x1="{x:.1f}" y1="{PAD_T+plot_h}" x2="{x:.1f}" y2="{PAD_T+plot_h+4}"/>')
        out.append(f'<text class="t-axis-label" x="{x:.1f}" y="{PAD_T+plot_h+17:.1f}" text-anchor="middle">'
                   f"{_n_label(n)}</text>")
    out.append(f'<text class="t-axis-label" x="{PAD_L+plot_w/2:.1f}" y="{H-16}" text-anchor="middle">'
               f"population N (log scale)</text>")
    out.append(f'<text class="t-axis-label" x="16" y="{PAD_T+plot_h/2:.1f}" text-anchor="middle" '
               f'transform="rotate(-90 16 {PAD_T+plot_h/2:.1f})">B / key</text>')
    out.append(f'<line class="axis" x1="{PAD_L}" y1="{PAD_T+plot_h}" x2="{PAD_L+plot_w}" y2="{PAD_T+plot_h}"/>')

    legend_y = PAD_T + 4
    for shape in shapes:
        pts = sorted((c for c in cells if c["dist"] == shape), key=lambda c: c["population"])
        colour = SHAPE_COLOUR[shape]
        for key, dash, name in (("expanse_ownership_bytes_per_key", "4 0", f"{shape} · Expanse"),
                                ("hot_ownership_bytes_per_key", "5 4", f"{shape} · HOT")):
            series = [c for c in pts if c[key] is not None]
            if not series:
                continue
            d = " ".join(f"{'M' if i == 0 else 'L'}{sx(c['population']):.1f},{sy(c[key]):.1f}"
                         for i, c in enumerate(series))
            out.append(f'<path d="{d}" fill="none" stroke="{colour}" stroke-width="2" '
                       f'stroke-dasharray="{dash}" stroke-linejoin="round"/>')
            for c in series:
                out.append(f'<circle cx="{sx(c["population"]):.1f}" cy="{sy(c[key]):.1f}" r="2.2" fill="{colour}"/>')
            lx = PAD_L + plot_w + 18
            out.append(f'<line x1="{lx}" y1="{legend_y}" x2="{lx+18}" y2="{legend_y}" stroke="{colour}" '
                       f'stroke-width="2" stroke-dasharray="{dash}"/>')
            out.append(f'<text class="t-legend" x="{lx+24}" y="{legend_y+4}">{esc(name)}</text>')
            legend_y += 17
    out.append(f'<text class="t-note" x="{PAD_L}" y="{H-4}">HOT has no line for a shape whose keys exceed its '
               f"255-byte window; the Expanse line stands alone there (§10.4).</text>")
    out.append("</svg>")
    return "\n".join(out)


def string_charts() -> list:
    written = []
    if (RESULTS / "baseline_string_latency.json").exists():
        (RESULTS / "chart_string_latency.svg").write_text(string_latency() + "\n")
        written.append("chart_string_latency.svg")
    if (RESULTS / "baseline_string_memory.json").exists():
        (RESULTS / "chart_string_memory_sweep.svg").write_text(string_memory_sweep() + "\n")
        written.append("chart_string_memory_sweep.svg")
    return written
def concurrent_charts() -> None:
    """The HOT-ROWEX arm (#692, §11): writer and reader throughput per cell.

    Two house per-pillar charts, higher is better. Rows come straight from the
    committed cells and the badge from each cell's BCa verdict, so a cell whose
    interval spans parity is `BOUNDARY` and claims no winner (§8.4). Nothing is
    rendered when the arm has not been measured.
    """
    path = RESULTS / "baseline_concurrent.json"
    if not path.exists():
        return
    cells = json.loads(path.read_text())["throughput"]

    def rows_for(role: str, pillar: str) -> list:
        out = []
        for c in cells:
            if c["pillar"] != pillar or f"{role}_verdict" not in c:
                continue
            out.append((
                f"{c['arm']} · W={c['writers']} R={c['readers']}",
                f"{c['workload_id']} · {c['rounds']} rounds",
                c[f"expanse_{role}_mops_median"],
                c[f"rowex_{role}_mops_median"],
                c[f"{role}_verdict"],
            ))
        return out

    svg = render_two_arm_chart(
        "chart_concurrent_writers.svg",
        "Writer throughput as writer count scales — SyncExpanse vs HOT-ROWEX",
        "C1: W writers insert 2^20 fresh keys into a 2^20 prefill, fixed work, arms interleaved",
        "M inserts/s (median of rounds)", rows_for("writer", "C1"), lower_is_better=False,
    )
    if svg:
        (RESULTS / "chart_concurrent_writers.svg").write_text(svg)
    svg = render_two_arm_chart(
        "chart_concurrent_readers.svg",
        "Reader throughput alongside writers — SyncExpanse vs HOT-ROWEX",
        "C2: 8 readers probe 50/50 while W writers insert; W=0 is the reader-only reference",
        "M lookups/s (median of rounds)", rows_for("reader", "C2"), lower_is_better=False,
    )
    if svg:
        (RESULTS / "chart_concurrent_readers.svg").write_text(svg)


def main() -> int:
    RESULTS.mkdir(parents=True, exist_ok=True)
    written = []
    if (RESULTS / "baseline_memory_curve.json").exists():
        (RESULTS / "chart_memory_curve.svg").write_text(memory_curve() + "\n")
        (RESULTS / "chart_latency_1m.svg").write_text(latency_1m() + "\n")
        written += ["chart_memory_curve.svg", "chart_latency_1m.svg"] + standard_charts()
    written += string_charts()
    print("wrote " + ", ".join(written))
    concurrent_charts()
    (RESULTS / "chart_memory_curve.svg").write_text(memory_curve() + "\n")
    (RESULTS / "chart_latency_1m.svg").write_text(latency_1m() + "\n")
    extra = standard_charts()
    print("wrote chart_memory_curve.svg, chart_latency_1m.svg, " + ", ".join(extra))
    return 0


if __name__ == "__main__":
    sys.exit(main())
