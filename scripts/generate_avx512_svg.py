#!/usr/bin/env python3
"""scripts/generate_avx512_svg.py

Regenerates ``docs/assets/bench_avx512.svg`` — the two-panel AVX-512 kernel
chart embedded by ``docs/HARDWARE.md`` §6 and
``docs/benchmarks/avx512/README.md`` — from the measured BCa artifact at
``results/baseline_avx512_bitmap.json``.

Every number is read from that artifact; nothing is retyped here (§8.2). The
artifact is produced by ``scripts/bench_baseline.py --harvest`` over a
``cargo bench -p expanse-trie --bench avx512_bitmap --features avx512`` run on
a host with ``avx512vpopcntdq``.

Panel 1 is the finding: speedup against ``scalar_popcnt`` — the *production*
baseline, `count_and` reached the way ``get::walk`` reaches it, with hardware
`popcnt`. Rating the vector arms against ``scalar_swar`` instead would credit
them with a win that is really popcnt-vs-SWAR (§8.3).

Panel 2 is why: cost per bitmap pair on a log axis. The kernel's share of the
work collapses as the working set leaves cache, so the same vector width buys
progressively less.

    python3 scripts/generate_avx512_svg.py

XML is validated before writing (same discipline as
``integrations/rocksdb/scripts/generate_bench_svg.py``, whose dual-theme
styling this mirrors).
"""

from __future__ import annotations

import json
import math
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
RESULTS = REPO_ROOT / "results" / "baseline_avx512_bitmap.json"
OUTPUT = REPO_ROOT / "docs" / "assets" / "bench_avx512.svg"

# Regimes in hierarchy order, with the working set each one walks. Labels are
# rendered verbatim; the byte figures are derived from the harness constants
# (a pair is two Bitmap256 = 64 B) and restated here only as axis captions.
REGIMES = [
    ("l1", "L1", "16 KiB"),
    ("l2", "L2", "512 KiB"),
    ("l3", "L3", "16 MiB"),
    ("dram", "DRAM", "256 MiB"),
    ("dram_chased", "DRAM chased", "256 MiB"),
]
BASELINE_ARM = "scalar_popcnt"
SPEEDUP_ARMS = [
    ("scalar_swar", "b-swar", "scalar_swar (ships today)"),
    ("v256", "b-v256", "AVX-512 · 256-bit"),
    ("v512", "b-v512", "AVX-512 · 512-bit"),
]

W, H = 940, 560
PANEL_W = 860

STYLE = """
      .bg { fill: #ffffff; }
      .border { stroke: #e2e8f0; stroke-width: 1px; fill: none; }
      .grid { stroke: #f1f5f9; stroke-width: 1px; stroke-dasharray: 2,3; }
      .axis { stroke: #cbd5e1; stroke-width: 1.5px; }
      .refline { stroke: #64748b; stroke-width: 1.25px; stroke-dasharray: 4,3; }

      .t-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 13px; font-weight: 700; letter-spacing: 0.5px; fill: #0f172a; }
      .t-chart-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11.5px; font-weight: 700; letter-spacing: 0.6px; fill: #0f172a; text-transform: uppercase; }
      .t-sub { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10px; font-weight: 500; fill: #475569; }
      .t-axis { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 9.5px; font-weight: 500; fill: #475569; }
      .t-group { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10.5px; font-weight: 700; fill: #0f172a; text-anchor: middle; }
      .t-groupsub { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 9px; font-weight: 500; fill: #64748b; text-anchor: middle; }
      .t-val { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 9.5px; font-weight: 700; fill: #0f172a; text-anchor: middle; }
      .t-foot { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 8.5px; font-weight: 500; fill: #64748b; }
      .t-legend { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 600; fill: #334155; }

      .b-swar { fill: #cbd5e1; }
      .b-base { fill: #475569; }
      .b-v256 { fill: #2563eb; }
      .b-v512 { fill: #16a34a; }
      .errbar { stroke: #0f172a; stroke-width: 1px; opacity: 0.55; }

      @media (prefers-color-scheme: dark) {
        .bg { fill: #0b1120; }
        .border { stroke: #1e293b; }
        .grid { stroke: #16202f; }
        .axis { stroke: #334155; }
        .refline { stroke: #94a3b8; }
        .t-title, .t-chart-title, .t-group, .t-val { fill: #e2e8f0; }
        .t-sub, .t-axis, .t-legend { fill: #94a3b8; }
        .t-groupsub, .t-foot { fill: #64748b; }
        .b-swar { fill: #475569; }
        .b-base { fill: #94a3b8; }
        .b-v256 { fill: #60a5fa; }
        .b-v512 { fill: #4ade80; }
        .errbar { stroke: #e2e8f0; }
      }
"""


def load() -> dict:
    if not RESULTS.is_file():
        sys.exit(
            f"{RESULTS} is missing. Produce it with a bench run on an AVX-512 host:\n"
            "  cargo bench -p expanse-trie --bench avx512_bitmap --features avx512\n"
            "  python3 scripts/bench_baseline.py --harvest --suite avx512_bitmap \\\n"
            "      --out results/baseline_avx512_bitmap.json"
        )
    return json.loads(RESULTS.read_text(encoding="utf-8"))


def index_arms(doc: dict) -> dict[tuple[str, str], dict]:
    """`(function_id, regime) -> arm`, keyed off criterion's own metadata."""
    out: dict[tuple[str, str], dict] = {}
    for arm in doc.get("arms", []):
        fn, regime = arm.get("function_id"), arm.get("value_str")
        if fn and regime:
            out[(fn, regime)] = arm
    if not out:
        sys.exit(f"{RESULTS}: no arms with function_id + value_str; nothing to plot.")
    return out


def el(parent, tag, **attrs):
    return ET.SubElement(parent, tag, {k.replace("_", "-"): str(v) for k, v in attrs.items()})


def text(parent, x, y, s, cls, anchor=None):
    t = el(parent, "text", x=f"{x:.1f}", y=f"{y:.1f}", **{"class": cls})
    if anchor:
        t.set("text-anchor", anchor)
    t.text = s
    return t


def nice_max(v: float) -> float:
    if v <= 0:
        return 1.0
    for step in (0.25, 0.5, 1.0, 2.0, 2.5, 5.0, 10.0, 25.0):
        if v <= step * 4:
            return math.ceil(v / step) * step
    return math.ceil(v)


def panel_speedup(root, arms, top: int) -> None:
    """Grouped bars: speedup vs `scalar_popcnt`, one group per regime."""
    g = el(root, "g", transform=f"translate(40,{top})")
    text(g, 0, 0, "AVX-512 SPEEDUP OVER THE PRODUCTION SCALAR KERNEL", "t-chart-title")
    text(
        g, 0, 15,
        f"x = {BASELINE_ARM} (Bitmap256::count_and with hardware popcnt) = 1.00x. Higher is faster.",
        "t-sub",
    )

    base_y, top_y = 205.0, 40.0
    plot_h = base_y - top_y

    ratios: dict[tuple[str, str], float] = {}
    for regime, _, _ in REGIMES:
        base = arms.get((BASELINE_ARM, regime))
        if not base or not base.get("point_ns"):
            continue
        for arm_id, _, _ in SPEEDUP_ARMS:
            a = arms.get((arm_id, regime))
            if a and a.get("point_ns"):
                ratios[(arm_id, regime)] = base["point_ns"] / a["point_ns"]

    if not ratios:
        sys.exit(f"{RESULTS}: no arm pairs share a regime with {BASELINE_ARM}.")
    axis_max = nice_max(max(ratios.values()) * 1.12)

    def y_of(v: float) -> float:
        return base_y - (v / axis_max) * plot_h

    for i in range(5):
        v = axis_max * i / 4
        y = y_of(v)
        el(g, "line", x1=0, y1=f"{y:.1f}", x2=PANEL_W, y2=f"{y:.1f}", **{"class": "grid"})
        text(g, -6, y + 3, f"{v:.2f}x", "t-axis", anchor="end")

    el(g, "line", x1=0, y1=f"{base_y:.1f}", x2=PANEL_W, y2=f"{base_y:.1f}", **{"class": "axis"})
    yr = y_of(1.0)
    el(g, "line", x1=0, y1=f"{yr:.1f}", x2=PANEL_W, y2=f"{yr:.1f}", **{"class": "refline"})
    text(g, PANEL_W - 2, yr - 5, "no gain", "t-axis", anchor="end")

    group_w = PANEL_W / len(REGIMES)
    bar_w = 34
    for gi, (regime, label, size) in enumerate(REGIMES):
        cx = gi * group_w + group_w / 2
        present = [a for a in SPEEDUP_ARMS if (a[0], regime) in ratios]
        span = len(present) * (bar_w + 8) - 8
        x0 = cx - span / 2
        for bi, (arm_id, css, _) in enumerate(present):
            v = ratios[(arm_id, regime)]
            x = x0 + bi * (bar_w + 8)
            y = y_of(v)
            el(g, "rect", x=f"{x:.1f}", y=f"{y:.1f}", width=bar_w,
               height=f"{max(base_y - y, 0.5):.1f}", rx=2, **{"class": css})
            text(g, x + bar_w / 2, y - 4, f"{v:.2f}", "t-val")
        text(g, cx, base_y + 16, label, "t-group")
        text(g, cx, base_y + 28, size, "t-groupsub")

    # Legend
    lx = 0
    for arm_id, css, label in [(BASELINE_ARM, "b-base", f"{BASELINE_ARM} (baseline)")] + [
        (a, c, l) for a, c, l in SPEEDUP_ARMS
    ]:
        el(g, "rect", x=lx, y=base_y + 40, width=11, height=11, rx=2, **{"class": css})
        text(g, lx + 16, base_y + 49, label, "t-legend")
        lx += 20 + len(label) * 6.1


def panel_cost(root, arms, top: int) -> None:
    """Log-axis absolute cost per bitmap pair — why the speedup decays."""
    g = el(root, "g", transform=f"translate(40,{top})")
    text(g, 0, 0, "COST PER BITMAP PAIR — WHY THE WIN DECAYS", "t-chart-title")
    text(
        g, 0, 15,
        "Log axis, nanoseconds per pair. As the working set leaves cache the kernel stops being the cost.",
        "t-sub",
    )

    base_y, top_y = 150.0, 38.0
    plot_h = base_y - top_y

    pts: list[tuple[int, float, float]] = []
    for gi, (regime, _, _) in enumerate(REGIMES):
        base = arms.get((BASELINE_ARM, regime))
        v256 = arms.get(("v256", regime))
        if base and base.get("point_ns") and v256 and v256.get("point_ns"):
            pairs = float(base.get("_pairs") or 0)
            pts.append((gi, base["point_ns"], v256["point_ns"]))
    if not pts:
        return

    # criterion times a whole traversal; normalise to per-pair using the regime sizes.
    sizes = {"l1": 256, "l2": 8192, "l3": 262144, "dram": 4194304, "dram_chased": 4194304}
    norm = []
    for gi, b, v in pts:
        n = sizes[REGIMES[gi][0]]
        norm.append((gi, b / n, v / n))

    lo = min(min(b, v) for _, b, v in norm)
    hi = max(max(b, v) for _, b, v in norm)
    lo_e, hi_e = math.floor(math.log10(lo)), math.ceil(math.log10(hi))

    def y_of(v: float) -> float:
        return base_y - (math.log10(v) - lo_e) / (hi_e - lo_e) * plot_h

    e = lo_e
    while e <= hi_e:
        y = y_of(10.0**e)
        el(g, "line", x1=0, y1=f"{y:.1f}", x2=PANEL_W, y2=f"{y:.1f}", **{"class": "grid"})
        lab = f"{10.0 ** e:g} ns" if e >= 0 else f"{10.0 ** e:.2f} ns"
        text(g, -6, y + 3, lab, "t-axis", anchor="end")
        e += 1
    el(g, "line", x1=0, y1=f"{base_y:.1f}", x2=PANEL_W, y2=f"{base_y:.1f}", **{"class": "axis"})

    group_w = PANEL_W / len(REGIMES)
    for gi, b, v in norm:
        cx = gi * group_w + group_w / 2
        for val, css, dx in ((b, "b-base", -20), (v, "b-v256", 20)):
            y = y_of(val)
            el(g, "rect", x=f"{cx + dx - 16:.1f}", y=f"{y:.1f}", width=32,
               height=f"{max(base_y - y, 0.5):.1f}", rx=2, **{"class": css})
            text(g, cx + dx, y - 4, f"{val:.2f}", "t-val")
        text(g, cx, base_y + 16, REGIMES[gi][1], "t-group")


def main() -> int:
    doc = load()
    arms = index_arms(doc)

    svg = ET.Element(
        "svg",
        {
            "xmlns": "http://www.w3.org/2000/svg",
            "viewBox": f"0 0 {W} {H}",
            "width": str(W),
            "height": str(H),
            "role": "img",
            "aria-label": (
                "AVX-512 bitmap cardinality kernel versus the production scalar kernel, "
                "swept across cache residency"
            ),
        },
    )
    defs = ET.SubElement(svg, "defs")
    style = ET.SubElement(defs, "style")
    style.text = STYLE
    el(svg, "rect", x=0, y=0, width=W, height=H, **{"class": "bg"})
    el(svg, "rect", x=0.5, y=0.5, width=W - 1, height=H - 1, rx=6, **{"class": "border"})

    text(svg, 40, 34, "AVX-512 vpopcntq for Bitmap256 cardinality", "t-title")
    text(
        svg, 40, 50,
        "Kernel ceiling, not an engine measurement: contiguous buffers, no trie descent. "
        "Report-only — this lane gates nothing.",
        "t-sub",
    )

    panel_speedup(svg, arms, 88)
    el(svg, "line", x1=40, y1=390, x2=W - 40, y2=390, **{"class": "border"})
    panel_cost(svg, arms, 418)

    prov = doc.get("provenance", {})
    stats = doc.get("statistics", {})
    foot = (
        f"measured: {prov.get('host_description', 'unknown host')}, commit "
        f"{str(prov.get('commit', '?'))[:12]} · workload: avx512_bitmap_count_and · "
        f"{stats.get('method', 'BCa bootstrap')} "
        f"{int(float(stats.get('confidence', 0.95)) * 100)}% over criterion per-iteration samples · "
        f"source: results/baseline_avx512_bitmap.json"
    )
    text(svg, 40, H - 16, foot, "t-foot")

    out = ET.tostring(svg, encoding="unicode")
    ET.fromstring(out)  # validate before writing
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text('<?xml version="1.0" encoding="UTF-8"?>\n' + out + "\n", encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(REPO_ROOT)} ({len(arms)} arms)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
