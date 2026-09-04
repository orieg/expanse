#!/usr/bin/env python3
"""Catch charts that render unreadably: overflow, collision, clamped bars.

Every defect this checks for shipped to `main` in a generated chart, and every
one was found by a human looking at the picture rather than by a gate:

  * a 26px figure grew from "16.4 M" to "604.4 M" and ran underneath a label
    pinned at x=115 -- TEXT COLLISION;
  * a ~350px note was placed in a 285px card and overran into its neighbour --
    TEXT OVERFLOW;
  * a bar scale pinned at `max_ingest = 5.5` met re-measured arms at ~11.7, so
    every bar overflowed its card -- RECT OVERFLOW;
  * a log axis rendered a 1029x difference as ~20x of bar and pinned two values
    13% apart to the same 5px floor -- CLAMPED BARS.

The common cause is geometry hardcoded for the values that existed when the
chart was written, which silently breaks the moment the numbers are
re-measured. A chart is a published surface (8.7): a misleading bar is a
misleading figure.

Widths are ESTIMATED from font-size and glyph count, so the thresholds are
deliberately slack -- this catches gross breakage, not kerning. It is a
tripwire, not a layout engine.

    python3 scripts/check_chart_layout.py
    python3 scripts/check_chart_layout.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

# Mean glyph advance as a fraction of font-size. Monospace is wider per glyph.
MONO_ADV, SANS_ADV = 0.60, 0.52
# Bold runs wider, and the collision that shipped ("604.4 M" at 26px/800 under a
# label pinned at x=115) is only caught if weight is modelled.
BOLD_MULT = 1.12
# Slack before an overflow is reported, in px. Generous: estimates are crude.
OVERFLOW_SLACK = 8.0
# Two texts on the same baseline must be at least this far apart horizontally.
COLLISION_SLACK = 0.0
# A bar group where this fraction of bars sits at an identical clamped width
# has lost the distinctions it exists to show.
CLAMP_FRACTION = 0.5
# A container must be at least this big to be a card rather than a legend
# swatch or a tick mark.
MIN_CARD_W, MIN_CARD_H = 140.0, 50.0

FONT_SIZE_RE = re.compile(r"font-size:\s*([\d.]+)px")
CLASS_MONO = ("t-unit", "t-axis-label", "t-val", "badge")


def _adv(cls: str, style: str) -> float:
    base = MONO_ADV if any(c in cls for c in CLASS_MONO) else SANS_ADV
    weight = re.search(r"font-weight:\s*(\d+)", style or "")
    bold = (weight and int(weight.group(1)) >= 700) or "title" in (cls or "") or "bar-label" in (cls or "")
    return base * BOLD_MULT if bold else base


def _font_size(cls: str, style: str, css: dict[str, float]) -> float:
    m = FONT_SIZE_RE.search(style or "")
    if m:
        return float(m.group(1))
    for c in (cls or "").split():
        if c in css:
            return css[c]
    return 10.0


def parse_css_centered(svg: str) -> set[str]:
    """Classes that centre their text. `badge()` sets text-anchor in the CSS,
    not the attribute, so a centred label's x is its MIDPOINT -- measuring it
    as a left edge reported four correct badges as overflowing."""
    out: set[str] = set()
    for m in re.finditer(r"\.([A-Za-z0-9_-]+)\s*\{([^}]*)\}", svg):
        if "text-anchor: middle" in m.group(2):
            out.add(m.group(1))
    return out


def parse_css_sizes(svg: str) -> dict[str, float]:
    out: dict[str, float] = {}
    for m in re.finditer(r"\.([A-Za-z0-9_-]+)\s*\{([^}]*)\}", svg):
        fs = FONT_SIZE_RE.search(m.group(2))
        if fs:
            out.setdefault(m.group(1), float(fs.group(1)))
    return out


def text_len(s: str) -> int:
    return len(re.sub(r"&[a-zA-Z#0-9]+;", "X", re.sub(r"<[^>]+>", "", s)))


def check_svg(path: Path) -> list[str]:
    svg = path.read_text(encoding="utf-8", errors="replace")
    css = parse_css_sizes(svg)
    centered = parse_css_centered(svg)
    issues: list[str] = []

    # Walk <g transform="translate(x,y)"> groups, tracking absolute offset.
    # Cards are <rect> with an explicit width/height at the start of a group.
    for gm in re.finditer(r'<g transform="translate\(([-\d.]+),\s*([-\d.]+)\)">(.*?)(?=<g transform=|</g>)',
                          svg, re.S):
        body = gm.group(3)
        # A card is a LARGE rect. Legend swatches are 12x12 and axis ticks are
        # thinner still; treating one as the container reported every label in
        # the chart as overflowing it. Require a plausible panel size, and take
        # the largest such rect in the group as the container.
        cards = [(float(m.group(1)), float(m.group(2)), float(m.group(3)), float(m.group(4)))
                 for m in re.finditer(r'<rect x="([\d.]+)" y="([\d.]+)" width="([\d.]+)" height="([\d.]+)"', body)
                 if float(m.group(3)) >= MIN_CARD_W and float(m.group(4)) >= MIN_CARD_H]
        if not cards:
            continue
        cx, cy, cw, ch = max(cards, key=lambda c: c[2] * c[3])

        # --- rects (bars) escaping the card ---
        for rm in re.finditer(r'<rect x="([\d.]+)" y="([\d.]+)" width="([\d.]+)"', body):
            rx, ry, rw = float(rm.group(1)), float(rm.group(2)), float(rm.group(3))
            if rw == cw:
                continue  # the card itself
            if rx + rw > cx + cw + OVERFLOW_SLACK:
                issues.append(
                    f"bar overflows its card: x={rx:.0f}+w={rw:.0f} exceeds card right edge "
                    f"{cx + cw:.0f} — a bar scale is hardcoded and the data outgrew it")

        # --- texts escaping the card, and colliding with each other ---
        texts: list[tuple[float, float, float, float, str]] = []
        for tm in re.finditer(r'<text x="([\d.]+)" y="([\d.]+)"([^>]*)>(.*?)</text>', body, re.S):
            tx, ty, attrs, inner = float(tm.group(1)), float(tm.group(2)), tm.group(3), tm.group(4)
            cls = (re.search(r'class="([^"]*)"', attrs) or re.match("", "")).group(1) if 'class="' in attrs else ""
            style = (re.search(r'style="([^"]*)"', attrs).group(1) if 'style="' in attrs else "")
            if "text-anchor" in attrs or any(c in centered for c in (cls or "").split()):
                continue  # centred labels: x is a midpoint, not a left edge
            fs = _font_size(cls, style, css)
            w = text_len(inner) * fs * _adv(cls, style)
            if tx + w > cx + cw + OVERFLOW_SLACK:
                issues.append(
                    f"text overflows its card: '{re.sub(r'<[^>]+>', '', inner).strip()[:44]}' "
                    f"needs ~{w:.0f}px from x={tx:.0f}, card right edge {cx + cw:.0f}")
            texts.append((tx, ty, w, fs, re.sub(r"<[^>]+>", "", inner).strip()))

        # Pairwise, not binned: two labels whose baselines differ by less than
        # their own type size share a visual row. Bucketing by a fixed grid put
        # the shipped 78/72 collision into different bins and missed it.
        for i, (x1, y1, w1, f1, s1) in enumerate(texts):
            for x2, y2, w2, f2, s2 in texts[i + 1:]:
                if abs(y1 - y2) >= max(f1, f2) * 0.8:
                    continue
                lo, hi = ((x1, w1, s1), (x2, w2, s2)) if x1 <= x2 else ((x2, w2, s2), (x1, w1, s1))
                if lo[0] + lo[1] > hi[0] + COLLISION_SLACK:
                    issues.append(
                        f"text collision near y={min(y1, y2):.0f}: '{lo[2][:26]}' "
                        f"(~{lo[1]:.0f}px from x={lo[0]:.0f}) runs into '{hi[2][:26]}' at x={hi[0]:.0f}")

    # --- bars clamped to an identical size lose the comparison ---
    #
    # Check whichever dimension ENCODES THE VALUE. In a vertical chart every bar
    # shares a width by design (it is the bar thickness) and the height varies;
    # checking width there flagged five correct charts.
    pairs = [(float(a), float(b)) for a, b in
             re.findall(r'<rect [^>]*width="([\d.]+)"[^>]*height="([\d.]+)"[^>]*class="b-', svg)]
    pairs += [(float(a), float(b)) for a, b in
              re.findall(r'<rect [^>]*class="b-[^"]*"[^>]*width="([\d.]+)"[^>]*height="([\d.]+)"', svg)]
    # Legend swatches carry a b- class too and are small squares; counting them
    # split the width set in two and sent a correct vertical chart down the
    # horizontal branch.
    pairs = [(w, h) for w, h in pairs if not (abs(w - h) < 0.5 and w <= 20.0)]
    if len(pairs) >= 4:
        ws = {round(w, 1) for w, _ in pairs}
        hs = {round(h, 1) for _, h in pairs}
        # The value axis is simply the one that varies more. Requiring the other
        # to be perfectly constant was too strict: a vertical chart using two
        # bar thicknesses for two groups fell to the horizontal branch and
        # reported its thicknesses as clamped bars.
        if len(hs) > len(ws):
            vals, axis = [h for _, h in pairs], "height"
        else:
            vals, axis = [w for w, _ in pairs], "width"
        for cand in {round(v, 1) for v in vals}:
            n = sum(1 for v in vals if abs(v - cand) < 0.05)
            if n / len(vals) >= CLAMP_FRACTION and n >= 4:
                issues.append(
                    f"{n} of {len(vals)} bars share {axis} {cand:.1f}px — bars pinned at a scale "
                    f"floor or ceiling cannot show the differences the chart exists to show")
    return issues


def self_test() -> int:
    css = '<style>.t-sub { font-size: 10px; } .t-big { font-size: 26px; }</style>'

    def wrap(body: str) -> str:
        return f'<svg>{css}<g transform="translate(0, 0)">{body}</g></svg>'

    ok = wrap('<rect x="0" y="0" width="300" height="100"/>'
              '<text x="14" y="20" class="t-sub">short</text>')
    assert not check_svg_str(ok), check_svg_str(ok)

    # The 285px card that a ~350px note was placed into.
    over = wrap('<rect x="0" y="0" width="285" height="172"/>'
                '<text x="14" y="168" class="t-sub">N=100k: ratio 1.0003 [0.9964, 1.0045] '
                '&#8212; not resolved; base cost dominates.</text>')
    assert any("overflows its card" in i for i in check_svg_str(over)), check_svg_str(over)

    # The verbatim markup that shipped: "604.4 M" at 26px/800 running under a
    # "keys / sec" label pinned at x=115, which fitted the old "16.4 M".
    coll = wrap('<rect x="0" y="0" width="270" height="172"/>'
                '<text x="14" y="78" class="t-chart-title" style="font-size: 26px; '
                'font-weight: 800; fill: #15803d;">604.4 M</text>'
                '<text x="115" y="72" class="t-bar-label" style="font-size: 13px; '
                'font-weight: 700;">keys / sec</text>')
    assert any("collision" in i for i in check_svg_str(coll)), check_svg_str(coll)

    # A bar scaled against a hardcoded ceiling the data outgrew.
    bar = wrap('<rect x="0" y="0" width="315" height="172"/>'
               '<rect x="14" y="68" width="400" class="b-expanse"/>')
    assert any("bar overflows" in i for i in check_svg_str(bar)), check_svg_str(bar)

    # Bars pinned to a shared floor.
    clamp = ('<svg>' + css + ''.join(
        f'<rect x="10" y="{i*10}" width="5.0" height="8" class="b-x"/>' for i in range(5)) + '</svg>')
    assert any("share width" in i for i in check_svg_str(clamp)), check_svg_str(clamp)

    print("check_chart_layout.py --self-test: all checks passed")
    return 0


def check_svg_str(s: str) -> list[str]:
    import tempfile
    with tempfile.NamedTemporaryFile("w", suffix=".svg", delete=False) as f:
        f.write(s)
        p = Path(f.name)
    try:
        return check_svg(p)
    finally:
        p.unlink(missing_ok=True)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--paths", nargs="*", help="limit to these SVGs")
    args = ap.parse_args()
    if args.self_test:
        return self_test()

    root = Path(subprocess.run(["git", "rev-parse", "--show-toplevel"],
                               capture_output=True, text=True, check=True).stdout.strip())
    files = args.paths or subprocess.run(
        ["git", "ls-files", "--", "*.svg"], cwd=root,
        capture_output=True, text=True, check=True).stdout.split()

    total = 0
    for rel in files:
        p = root / rel
        if not p.is_file():
            continue
        for issue in check_svg(p):
            print(f"::error file={rel}::{issue}")
            total += 1
    print(f"check_chart_layout.py: {len(files)} chart(s) scanned, {total} layout issue(s).")
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())
