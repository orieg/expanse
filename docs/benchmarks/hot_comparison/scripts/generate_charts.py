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
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from theme import svg_header  # noqa: E402

BASE = Path(__file__).resolve().parent.parent
RESULTS = BASE / "results"

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


def main() -> int:
    RESULTS.mkdir(parents=True, exist_ok=True)
    (RESULTS / "chart_memory_curve.svg").write_text(memory_curve() + "\n")
    (RESULTS / "chart_latency_1m.svg").write_text(latency_1m() + "\n")
    print("wrote chart_memory_curve.svg and chart_latency_1m.svg")
    return 0


if __name__ == "__main__":
    sys.exit(main())
