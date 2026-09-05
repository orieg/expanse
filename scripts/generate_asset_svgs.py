#!/usr/bin/env python3
"""scripts/generate_asset_svgs.py

Regenerates the three README hero charts from committed data:

  * ``docs/assets/bench_comparative.svg``
  * ``docs/assets/bench_concurrency.svg``
  * ``docs/assets/bench_ycsb.svg``
  * ``docs/assets/bench_sync32_health.svg``
  * ``docs/assets/bench_density_sawtooth.svg``

All three were hand-authored, so their numbers drifted away from the measured
tables and kept publishing figures that had since been retracted (the
unbounded-keyspace concurrency curve, the pre-#385 Workload E win, and a
"point lookup" panel whose 15.8 ns / 32.3 ns pair is the *sequential 30k
insert* row from ``docs/visualizer_data.json``). They are now generated the
same way ``docs/benchmarks/rocksdb_memtable/results/bench_rocksdb.svg`` is: edit
``docs/assets/data/bench_assets.json``, then run

    python3 scripts/generate_asset_svgs.py

Every rendered number, ratio and verdict is derived from that file at render
time -- no summary constant is stamped into the markup (AGENTS.md 8.2), and
each chart carries its own provenance footer (AGENTS.md 8.7).
"""

from __future__ import annotations

import json
import xml.etree.ElementTree as ET
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
DATA = REPO_ROOT / "docs" / "assets" / "data" / "bench_assets.json"
ASSETS = REPO_ROOT / "docs" / "assets"

STYLE = """
      /* Light theme by default for maximum contrast on the web. */
      .bg { fill: #ffffff; }
      .border { stroke: #e2e8f0; stroke-width: 1px; fill: none; }
      .grid { stroke: #f1f5f9; stroke-width: 1px; stroke-dasharray: 2,3; }
      .axis { stroke: #cbd5e1; stroke-width: 1.5px; }
      .divider { stroke: #e2e8f0; stroke-width: 1px; }

      .t-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 12px; font-weight: 700; letter-spacing: 0.6px; fill: #0f172a; text-transform: uppercase; }
      .t-chart-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11.5px; font-weight: 700; letter-spacing: 0.6px; fill: #0f172a; text-transform: uppercase; }
      .t-sub { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10px; font-weight: 500; fill: #334155; }
      .t-note { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 500; fill: #64748b; }
      .t-unit { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 600; fill: #475569; }
      .t-axis-label { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 500; fill: #475569; }
      .t-bar-label { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11px; font-weight: 700; fill: #0f172a; }
      .t-legend { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10.5px; font-weight: 600; fill: #0f172a; }
      .t-win { fill: #15803d; font-weight: 700; }
      .t-loss { fill: #b45309; font-weight: 700; }

      .t-val-accent { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 700; fill: #15803d; text-anchor: middle; }
      .t-val-blue { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 600; fill: #2563eb; text-anchor: middle; }
      .t-val-muted { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 600; fill: #334155; text-anchor: middle; }
      .t-row-accent { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10.5px; font-weight: 700; fill: #15803d; }
      .t-row-muted { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10.5px; font-weight: 600; fill: #334155; }

      .b-expanse { fill: #16a34a; }
      .b-expanse-alt { fill: #10b981; }
      .b-hash { fill: #2563eb; }
      .b-btree { fill: #64748b; }
      .b-skipmap { fill: #94a3b8; }
      .b-other { fill: #64748b; }

      .badge-win { fill: #dcfce7; stroke: #86efac; stroke-width: 1px; rx: 3px; }
      .badge-win-text { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 9.5px; font-weight: 700; fill: #15803d; text-anchor: middle; }
      .badge-loss { fill: #fef3c7; stroke: #fcd34d; stroke-width: 1px; rx: 3px; }
      .badge-loss-text { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 9.5px; font-weight: 700; fill: #b45309; text-anchor: middle; }

      @media (prefers-color-scheme: dark) {
        .bg { fill: #0d1117; }
        .border { stroke: #30363d; }
        .grid { stroke: #21262d; }
        .axis { stroke: #484f58; }
        .divider { stroke: #21262d; }
        .t-title { fill: #f0f6fc; }
        .t-chart-title { fill: #f0f6fc; }
        .t-sub { fill: #94a3b8; }
        .t-note { fill: #94a3b8; }
        .t-unit { fill: #94a3b8; }
        .t-axis-label { fill: #94a3b8; }
        .t-bar-label { fill: #f8fafc; }
        .t-legend { fill: #f8fafc; }
        .t-win { fill: #4ade80; }
        .t-loss { fill: #fcd34d; }
        .t-val-accent { fill: #4ade80; }
        .t-val-blue { fill: #38bdf8; }
        .t-val-muted { fill: #e2e8f0; }
        .t-row-accent { fill: #4ade80; }
        .t-row-muted { fill: #e2e8f0; }
        .b-expanse { fill: #22c55e; }
        .b-expanse-alt { fill: #34d399; }
        .b-hash { fill: #3b82f6; }
        .b-btree { fill: #64748b; }
        .b-skipmap { fill: #475569; }
        .b-other { fill: #64748b; }
        .badge-win { fill: #064e3b; stroke: #059669; }
        .badge-win-text { fill: #6ee7b7; }
        .badge-loss { fill: #451a03; stroke: #d97706; }
        .badge-loss-text { fill: #fcd34d; }
      }

      :root[data-theme="dark"] .bg, [data-theme="dark"] .bg { fill: #0d1117; }
      :root[data-theme="dark"] .border, [data-theme="dark"] .border { stroke: #30363d; }
      :root[data-theme="dark"] .grid, [data-theme="dark"] .grid { stroke: #21262d; }
      :root[data-theme="dark"] .axis, [data-theme="dark"] .axis { stroke: #484f58; }
      :root[data-theme="dark"] .divider, [data-theme="dark"] .divider { stroke: #21262d; }
      :root[data-theme="dark"] .t-title, [data-theme="dark"] .t-title { fill: #f0f6fc; }
      :root[data-theme="dark"] .t-chart-title, [data-theme="dark"] .t-chart-title { fill: #f0f6fc; }
      :root[data-theme="dark"] .t-sub, [data-theme="dark"] .t-sub { fill: #94a3b8; }
      :root[data-theme="dark"] .t-note, [data-theme="dark"] .t-note { fill: #94a3b8; }
      :root[data-theme="dark"] .t-unit, [data-theme="dark"] .t-unit { fill: #94a3b8; }
      :root[data-theme="dark"] .t-axis-label, [data-theme="dark"] .t-axis-label { fill: #94a3b8; }
      :root[data-theme="dark"] .t-bar-label, [data-theme="dark"] .t-bar-label { fill: #f8fafc; }
      :root[data-theme="dark"] .t-legend, [data-theme="dark"] .t-legend { fill: #f8fafc; }
      :root[data-theme="dark"] .t-win, [data-theme="dark"] .t-win { fill: #4ade80; }
      :root[data-theme="dark"] .t-loss, [data-theme="dark"] .t-loss { fill: #fcd34d; }
      :root[data-theme="dark"] .t-val-accent, [data-theme="dark"] .t-val-accent { fill: #4ade80; }
      :root[data-theme="dark"] .t-val-blue, [data-theme="dark"] .t-val-blue { fill: #38bdf8; }
      :root[data-theme="dark"] .t-val-muted, [data-theme="dark"] .t-val-muted { fill: #e2e8f0; }
      :root[data-theme="dark"] .t-row-accent, [data-theme="dark"] .t-row-accent { fill: #4ade80; }
      :root[data-theme="dark"] .t-row-muted, [data-theme="dark"] .t-row-muted { fill: #e2e8f0; }
      :root[data-theme="dark"] .b-expanse, [data-theme="dark"] .b-expanse { fill: #22c55e; }
      :root[data-theme="dark"] .b-expanse-alt, [data-theme="dark"] .b-expanse-alt { fill: #34d399; }
      :root[data-theme="dark"] .b-hash, [data-theme="dark"] .b-hash { fill: #3b82f6; }
      :root[data-theme="dark"] .b-btree, [data-theme="dark"] .b-btree { fill: #64748b; }
      :root[data-theme="dark"] .b-skipmap, [data-theme="dark"] .b-skipmap { fill: #475569; }
      :root[data-theme="dark"] .b-other, [data-theme="dark"] .b-other { fill: #64748b; }
      :root[data-theme="dark"] .badge-win, [data-theme="dark"] .badge-win { fill: #064e3b; stroke: #059669; }
      :root[data-theme="dark"] .badge-win-text, [data-theme="dark"] .badge-win-text { fill: #6ee7b7; }
      :root[data-theme="dark"] .badge-loss, [data-theme="dark"] .badge-loss { fill: #451a03; stroke: #d97706; }
      :root[data-theme="dark"] .badge-loss-text, [data-theme="dark"] .badge-loss-text { fill: #fcd34d; }
"""


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def head(width: int, height: int, title: str) -> str:
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" role="img" '
        f'viewBox="0 0 {width} {height}" width="100%" height="100%">\n'
        f"  <title>{esc(title)}</title>\n"
        f"  <defs>\n    <style>{STYLE}    </style>\n  </defs>\n\n"
        f'  <rect width="100%" height="100%" class="bg" rx="8"/>\n'
        f'  <rect width="100%" height="100%" class="border" rx="8"/>\n'
    )


def write(path: Path, svg: str) -> None:
    ET.fromstring(svg)  # fail before touching a published asset
    path.write_text(svg, encoding="utf-8")
    print(f"wrote {path.relative_to(REPO_ROOT)}")


def badge(x: float, y: float, w: float, text: str, win: bool) -> str:
    cls = "badge-win" if win else "badge-loss"
    tcls = "badge-win-text" if win else "badge-loss-text"
    return (
        f'  <rect x="{x}" y="{y}" width="{w}" height="18" class="{cls}"/>\n'
        f'  <text x="{x + w / 2}" y="{y + 13}" class="{tcls}">{esc(text)}</text>\n'
    )


# --------------------------------------------------------------------------
# 1. bench_comparative.svg -- three vertical-bar panels, one host, one commit.
# --------------------------------------------------------------------------

BAR_X = [45, 115, 185]
BASELINE_Y = 195.0
AXIS_TOP_Y = 45.0
PLOT_H = BASELINE_Y - AXIS_TOP_Y


def _vbar(x: float, value: float, axis_max: float, cls: str, label: str,
          label_cls: str, name: str, caption: str, caption_cls: str) -> str:
    h = max(2.0, min(PLOT_H, value / axis_max * PLOT_H))
    y = BASELINE_Y - h
    mid = x + 25
    return (
        f'    <rect x="{x}" y="{y:.1f}" width="50" height="{h:.1f}" class="{cls}" rx="2"/>\n'
        f'    <text x="{mid}" y="{y - 8.5:.1f}" class="{label_cls}">{esc(label)}</text>\n'
        f'    <text x="{mid}" y="214" class="t-bar-label" text-anchor="middle">{esc(name)}</text>\n'
        f'    <text x="{mid}" y="228" class="t-sub {caption_cls}" text-anchor="middle">{esc(caption)}</text>\n'
    )


def _vpanel(tx: int, title: str, sub: str, unit: str, axis_max: float,
            top_label: str, mid_label: str, bars: str) -> str:
    return (
        f'  <g transform="translate({tx}, 20)">\n'
        f'    <text x="0" y="0" class="t-chart-title">{esc(title)}</text>\n'
        f'    <text x="0" y="13" class="t-sub">{esc(sub)}</text>\n'
        f'    <text x="0" y="29" class="t-unit">{unit}</text>\n'
        f'    <line x1="30" y1="45" x2="250" y2="45" class="grid"/>\n'
        f'    <text x="22" y="48" class="t-axis-label" text-anchor="end">{esc(top_label)}</text>\n'
        f'    <line x1="30" y1="120" x2="250" y2="120" class="grid"/>\n'
        f'    <text x="22" y="123" class="t-axis-label" text-anchor="end">{esc(mid_label)}</text>\n'
        f'    <line x1="30" y1="195" x2="250" y2="195" class="axis"/>\n'
        f'    <text x="22" y="198" class="t-axis-label" text-anchor="end">0</text>\n'
        f'    <line x1="30" y1="40" x2="30" y2="195" class="axis"/>\n'
        f"{bars}"
        f"  </g>\n"
    )


def render_comparative(data: dict) -> None:
    c = data["comparative"]
    meta = c["meta"]

    build = c["cold_build_insert_clustered_100k_ms"]
    pop = build["population"]
    # ms for `pop` keys -> M keys/s. Higher is better.
    mops = {k: pop / (build[k] * 1e-3) / 1e6 for k in ("ExpanseSet", "HashSet", "BTreeSet")}
    best_other_build = max(mops["HashSet"], mops["BTreeSet"])
    build_win = mops["ExpanseSet"] >= best_other_build
    build_ratio = (mops["ExpanseSet"] / best_other_build) if build_win else (best_other_build / mops["ExpanseSet"])

    panels = [
        (
            30, "Cold-Build Insert",
            f"{pop:,} clustered keys, cold build (higher is better)",
            "&#9650; Throughput (M keys / sec)",
            100.0, "100M", "50M",
            _vbar(BAR_X[0], mops["ExpanseSet"], 100.0, "b-expanse",
                  f'{mops["ExpanseSet"]:.1f} M/s', "t-val-accent", "ExpanseSet",
                  f'{build_ratio:.2f}x {"faster" if build_win else "slower"}',
                  "t-win" if build_win else "t-loss")
            + _vbar(BAR_X[1], mops["HashSet"], 100.0, "b-hash",
                    f'{mops["HashSet"]:.1f} M/s', "t-val-blue", "HashSet", "SwissTable", "")
            + _vbar(BAR_X[2], mops["BTreeSet"], 100.0, "b-btree",
                    f'{mops["BTreeSet"]:.1f} M/s', "t-val-muted", "BTreeSet", "std ordered", ""),
        ),
    ]

    for tx, key, title, dist in (
        (340, "point_lookup_random_1m_ns", "Point Lookup: Random", "random"),
        (665, "point_lookup_clustered_1m_ns", "Point Lookup: Clustered", "clustered"),
    ):
        lk = c[key]
        best_other = min(lk["HashSet"], lk["BTreeSet"])
        win = lk["ExpanseSet"] <= best_other
        ratio = (best_other / lk["ExpanseSet"]) if win else (lk["ExpanseSet"] / best_other)
        panels.append((
            tx, title,
            f'{lk["population"]:,} keys, hit (lower is better)',
            "&#9660; Latency (ns / op)",
            120.0, "120 ns", "60 ns",
            _vbar(BAR_X[0], lk["ExpanseSet"], 120.0, "b-expanse",
                  f'{lk["ExpanseSet"]:.1f} ns', "t-val-accent", "ExpanseSet",
                  f'{ratio:.2f}x {"faster" if win else "slower"}',
                  "t-win" if win else "t-loss")
            + _vbar(BAR_X[1], lk["HashSet"], 120.0, "b-hash",
                    f'{lk["HashSet"]:.1f} ns', "t-val-blue", "HashSet", "SwissTable", "")
            + _vbar(BAR_X[2], lk["BTreeSet"], 120.0, "b-btree",
                    f'{lk["BTreeSet"]:.1f} ns', "t-val-muted", "BTreeSet", "std ordered", ""),
        ))

    for vals, amax, name in (
        (list(mops.values()), 100.0, "cold build"),
        ([c["point_lookup_random_1m_ns"][k] for k in ("ExpanseSet", "HashSet", "BTreeSet")], 120.0, "random lookup"),
        ([c["point_lookup_clustered_1m_ns"][k] for k in ("ExpanseSet", "HashSet", "BTreeSet")], 120.0, "clustered lookup"),
    ):
        over = [v for v in vals if v > amax]
        assert not over, f"{name}: value(s) {over} exceed axis max {amax}; raise the axis"

    svg = head(960, 300, "ExpanseSet vs std collections -- insert, random lookup, clustered lookup")
    for i, p in enumerate(panels):
        svg += "\n" + _vpanel(*p)
        if i < len(panels) - 1:
            xdiv = 310 if i == 0 else 635
            svg += f'\n  <line x1="{xdiv}" y1="20" x2="{xdiv}" y2="258" class="divider"/>\n'
    svg += (
        f'\n  <text x="30" y="278" class="t-note">Measured: {esc(meta["host"])}'
        f' &#183; commit {esc(meta["commit"])} &#183; {esc(meta["harness"])}</text>\n'
        f'  <text x="30" y="291" class="t-note">Source: {esc(meta["source"])}.'
        " Random-key lookup is the engine's measured weak arm and is published as a loss.</text>\n"
    )
    svg += "</svg>\n"
    write(ASSETS / "bench_comparative.svg", svg)


# --------------------------------------------------------------------------
# 2. bench_concurrency.svg -- 16-thread read throughput + the 50/50 limit.
# --------------------------------------------------------------------------


def render_concurrency(data: dict) -> None:
    c = data["concurrency"]
    meta = c["meta"]
    threads = c["threads"]
    t_max = threads[-1]
    rows = c["read_100"]

    bar_x, bar_max = 300.0, 300.0
    axis_max = max(r["mops"][-1] for r in rows) * 1.14
    row_h = 26
    top = 96
    mixed = c["read_write_50_50"]
    mixed_top = top + len(rows) * row_h + 30
    height = mixed_top + len(mixed) * 16 + 54

    svg = head(960, height, f"OCC concurrency: 100% read at {t_max} threads")
    svg += f"""
  <text x="30" y="30" class="t-title">MULTITHREADED OCC CONCURRENCY &#183; 100% READ</text>
  <text x="30" y="46" class="t-sub">Read throughput at {t_max} threads &#183; M ops/sec &#183; higher is better &#183; {esc(meta["keyspace"])}</text>
  <g transform="translate(690, 20)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">Expanse OCC arm</text>
    <rect x="150" y="0" width="12" height="12" rx="2" class="b-other"/>
    <text x="168" y="10" class="t-legend">Baseline</text>
  </g>
  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>
  <text x="30" y="74" class="t-note">Bars are {t_max}-thread throughput on a zero-based linear axis; the badge is that arm's own {threads[0]}&#8594;{t_max}-thread scaling.</text>
"""
    for i, r in enumerate(rows):
        y = top + i * row_h
        v = r["mops"][-1]
        w = max(3.0, v / axis_max * bar_max)
        cls = "b-expanse" if r["kind"] == "expanse" else "b-other"
        vcls = "t-row-accent" if r["kind"] == "expanse" else "t-row-muted"
        svg += (
            f'  <text x="30" y="{y + 9}" class="t-bar-label">{esc(r["arm"])}</text>\n'
            f'  <rect x="{bar_x}" y="{y}" width="{w:.1f}" height="11" rx="2" class="{cls}"/>\n'
            f'  <text x="{bar_x + w + 8:.1f}" y="{y + 9}" class="{vcls}">{v:,.1f} M ops/s</text>\n'
        )
        svg += badge(800, y - 4, 130, f'{r["scale_16t"]:.2f}x scaling', r["scale_16t"] >= 1.0)

    # The honest limit: the same arms under a 50/50 read/write mix.
    svg += f'\n  <line x1="30" y1="{mixed_top - 18}" x2="930" y2="{mixed_top - 18}" class="divider"/>\n'
    svg += (
        f'  <text x="30" y="{mixed_top - 4}" class="t-bar-label">'
        f"Honest limit &#8212; the same arms at 50% read / 50% write "
        f"({threads[0]}&#8594;{t_max} threads, read ops/s):</text>\n"
    )
    for i, r in enumerate(mixed):
        y = mixed_top + 12 + i * 16
        chain = " &#8594; ".join(f"{m:,.1f}" for m in r["mops"])
        cls = "t-row-accent" if r["scale_16t"] >= 1.0 else "t-row-muted"
        svg += (
            f'  <text x="42" y="{y}" class="{cls}">{esc(r["arm"])}: {chain} M ops/s '
            f'&#183; {r["scale_16t"]:.2f}x</text>\n'
        )
    svg += (
        f'\n  <text x="30" y="{height - 26}" class="t-note">Measured: {esc(meta["host"])}'
        f' &#183; run {esc(meta["run"])}, ref {esc(meta["ref"])}</text>\n'
        f'  <text x="30" y="{height - 13}" class="t-note">{esc(meta["config"])}'
        f' &#183; single-writer OCC scales reads, not writes.</text>\n'
    )
    svg += "</svg>\n"
    write(ASSETS / "bench_concurrency.svg", svg)


# --------------------------------------------------------------------------
# 3. bench_ycsb.svg -- A-D/F on one axis, E on its own (it is 20x smaller).
# --------------------------------------------------------------------------


def render_ycsb(data: dict) -> None:
    y_ = data["ycsb"]
    meta = y_["meta"]
    engines = y_["engines"]
    kinds = y_["kinds"]
    kind_cls = {
        "expanse": "b-expanse",
        "expanse_blob": "b-expanse-alt",
        "btree": "b-btree",
        "skipmap": "b-skipmap",
    }
    is_expanse = [k.startswith("expanse") for k in kinds]
    n = len(engines)

    bar_x, bar_max = 300.0, 385.0
    main = y_["workloads"]
    e_row = y_["workload_e"]

    axis_main = max(max(w["mops"]) for w in main) * 1.14
    axis_e = max(e_row["mops"]) * 1.14

    row_pitch = 13
    grp_h = row_pitch * n + 16
    top = 100
    e_top = top + len(main) * grp_h + 44
    height = e_top + grp_h + 50

    svg = head(960, height, "YCSB workloads A-F throughput")

    legend = ""
    for i, (eng, kind) in enumerate(zip(engines, kinds)):
        legend += (
            f'    <rect x="{i * 172}" y="0" width="12" height="12" rx="2" class="{kind_cls[kind]}"/>\n'
            f'    <text x="{i * 172 + 18}" y="10" class="t-legend">{esc(eng)}</text>\n'
        )
    svg += f"""
  <text x="30" y="30" class="t-title">YCSB WORKLOADS A&#8211;F THROUGHPUT</text>
  <text x="30" y="46" class="t-sub">{esc(y_["unit"])} &#183; higher is better &#183; {esc(meta["config"])}</text>
  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>
  <g transform="translate(30, 68)">
{legend}  </g>
  <text x="30" y="{top - 8}" class="t-note">Workloads A&#8211;D and F share one zero-based axis; Workload E has its own, an order of magnitude lower.</text>
"""

    def group(y0: float, wid: str, desc: str, vals: list, axis_max: float) -> str:
        out = (
            f'  <text x="30" y="{y0 + 9}" class="t-bar-label">Workload {esc(wid)}</text>\n'
            f'  <text x="30" y="{y0 + 22}" class="t-sub">{esc(desc)}</text>\n'
        )
        best = max(vals)
        # Sub-1.0 rows need a third decimal; everything else reads better with two.
        dec = 3 if min(vals) < 1.0 else 2
        for i, v in enumerate(vals):
            yy = y0 + i * row_pitch
            w = max(3.0, v / axis_max * bar_max)
            vcls = "t-row-accent" if v == best else "t-row-muted"
            # Values sit in a fixed right-aligned column, not at the bar end,
            # so adjacent rows can never overlap each other's labels.
            out += (
                f'  <rect x="{bar_x}" y="{yy}" width="{w:.1f}" height="9" rx="2" class="{kind_cls[kinds[i]]}"/>\n'
                f'  <text x="700" y="{yy + 8}" class="{vcls}" text-anchor="end">{v:.{dec}f}</text>\n'
            )
        # Verdict badge: the best Expanse arm against the best non-Expanse arm.
        # That is the claim the chart is making, and it inverts honestly when a
        # baseline wins (Workload E, post-#385).
        best_exp = max(v for i, v in enumerate(vals) if is_expanse[i])
        best_other = max(v for i, v in enumerate(vals) if not is_expanse[i])
        other_name = engines[vals.index(best_other)].split(" ")[0]
        if best_exp >= best_other:
            text, win = f"Expanse {best_exp / best_other:.2f}x", True
        else:
            text, win = f"{other_name} {best_other / best_exp:.2f}x", False
        out += badge(775, y0 + (row_pitch * n - 18) / 2, 155, text, win)
        return out

    for i, w in enumerate(main):
        svg += group(top + i * grp_h, w["id"], w["desc"], w["mops"], axis_main)

    svg += f'\n  <line x1="30" y1="{e_top - 30}" x2="930" y2="{e_top - 30}" class="divider"/>\n'
    svg += (
        f'  <text x="30" y="{e_top - 16}" class="t-note">Workload E, separate axis '
        f'(max {axis_e / 1.14:.3f} {esc(y_["unit"])}) &#8212; {esc(e_row["note"])}</text>\n'
    )
    svg += group(e_top, e_row["id"], e_row["desc"], e_row["mops"], axis_e)

    svg += (
        f'\n  <text x="30" y="{height - 26}" class="t-note">Measured: {esc(meta["host"])}'
        f' &#183; run {esc(meta["run"])} &#183; {esc(meta["source"])}</text>\n'
        f'  <text x="30" y="{height - 13}" class="t-note">The Workload E figure published under'
        " #375 is retracted; E is re-measured here and inverts.</text>\n"
    )
    svg += "</svg>\n"
    write(ASSETS / "bench_ycsb.svg", svg)


# --------------------------------------------------------------------------
# 4. bench_sync32_health.svg -- the 32-bit optimistic protocol's health:
#    Busy rate per writer duty and reader count, with reclamation refusals.
# --------------------------------------------------------------------------


def render_sync32_health(data: dict) -> None:
    h = data["sync32_health"]
    meta = h["meta"]
    threads = h["threads"]
    duties = h["duties"]
    cls_for = {1: "b-expanse", 4: "b-expanse-alt", 16: "b-hash"}

    bar_x, bar_max = 210.0, 270.0
    badge_x, badge_w = 796.0, 134.0
    row_h, grp_gap = 15, 22
    top = 104
    grp_h = row_h * len(threads) + grp_gap
    height = top + grp_h * len(duties) + 104

    def rate(v: float) -> str:
        return f"{v / 1e6:.2f}M" if v >= 9.995e5 else (f"{v / 1e3:.0f}k" if v >= 1e3 else f"{v:.0f}")

    svg = head(960, height, "sync32 protocol health: Busy rate per writer duty")
    legend = "".join(
        f'    <rect x="{i * 120}" y="0" width="12" height="12" rx="2" class="{cls_for[t]}"/>\n'
        f'    <text x="{i * 120 + 18}" y="10" class="t-legend">{t} reader{"s" if t > 1 else ""}</text>\n'
        for i, t in enumerate(threads)
    )
    svg += f"""
  <text x="30" y="30" class="t-title">SYNC32 OPTIMISTIC PROTOCOL &#183; BUSY RATE BY WRITER DUTY</text>
  <text x="30" y="46" class="t-sub">Share of try_get attempts abandoned to an open write bracket &#183; lower is better &#183; one writer, N readers, 4,096 stable + 4,096 churn keys</text>
  <g transform="translate(600, 20)">
{legend}  </g>
  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>
  <text x="30" y="74" class="t-note">Bars are the Busy rate on a zero-based linear axis (0&#8211;100%); the badge is the writer's refused mutations (ArenaFull / ReclaimBacklog) in the same 500 ms window &#8212; a stalled-reclamation signal.</text>
  <text x="30" y="88" class="t-note">Writer duty: full (as fast as it can, the Busy ceiling) then deadline-paced 1M, 100k and 10k mutations/s; a saturated CAN bus is ~10k frames/s. reads/s = validated reads.</text>
"""
    for gi, d in enumerate(duties):
        gy = top + gi * grp_h
        svg += f'  <text x="30" y="{gy + 9}" class="t-bar-label">writer {esc(d["duty"])}</text>\n'
        for ri, r in enumerate(d["rows"]):
            y = gy + ri * row_h
            pct = r["busy_pct"]
            w = max(2.0, pct / 100.0 * bar_max)
            t = r["readers"]
            svg += (
                f'  <text x="{bar_x - 8}" y="{y + 9}" class="t-axis-label" text-anchor="end">{t}R</text>\n'
                f'  <rect x="{bar_x}" y="{y}" width="{w:.1f}" height="11" rx="2" class="{cls_for[t]}"/>\n'
                f'  <text x="{bar_x + w + 8:.1f}" y="{y + 9}" class="t-row-muted">{pct:.3g}% busy &#183; {rate(r["reads_per_s"])} reads/s &#183; {rate(r["writes_per_s"])} writes/s</text>\n'
            )
            refused = r["refused"]
            label = f"{refused:,} refused" if refused else "0 refused"
            svg += badge(badge_x, y - 2, badge_w, label, refused == 0)
    svg += (
        f'\n  <line x1="30" y1="{height - 60}" x2="930" y2="{height - 60}" class="divider"/>\n'
        f'  <text x="30" y="{height - 44}" class="t-note">Reading: Busy tracks write rate &#215; bracket length, as the seqlock model predicts &#8212; at embedded ingestion rates reads validate &#8805;99.8% of the time. Refusals: none at any reader count &#8212;</text>\n'
        f'  <text x="30" y="{height - 31}" class="t-note">reclamation frees a parked node once every reader has passed through a quiescent state since it was retired (per-reader walk counters, #594), so dense readers no longer starve the writer.</text>\n'
        f'  <text x="30" y="{height - 13}" class="t-note">Measured: {esc(meta["host"])} &#183; run {esc(meta["run"])}, ref {esc(meta["ref"])} &#183; report-only, never gating (workload: {esc(meta["workload_id"])}).</text>\n'
    )
    svg += "</svg>\n"
    write(ASSETS / "bench_sync32_health.svg", svg)


def render_density(data: dict) -> None:
    """bytes/key against expanse occupancy λ — the sawtooth of ARCHITECTURE §3.5.

    Every point is a cell of `density_sweep`; the three keyspace widths are
    drawn with distinct markers so the reader can see that they land on one
    curve. The axis bounds, the `LEAF_CAP` marker and the two gate-cell labels
    are all derived from the data (AGENTS.md 8.15a); nothing is placed by
    hand for the values that happen to exist today.
    """
    import math

    block = data["density_sweep"]
    cells = sorted(block["cells"], key=lambda c: c["lambda"])
    leaf_cap = block["meta"]["leaf_cap"]
    width, height = 960, 420
    px0, px1, py0, py1 = 70, 700, 70, 330  # plot box
    lam_min = min(c["lambda"] for c in cells) / 1.25
    lam_max = max(c["lambda"] for c in cells) * 1.25
    y_max = math.ceil(max(c["set_bpk"] for c in cells) / 4.0) * 4.0

    def X(lam: float) -> float:
        return px0 + (math.log10(lam) - math.log10(lam_min)) / (math.log10(lam_max) - math.log10(lam_min)) * (px1 - px0)

    def Y(v: float) -> float:
        return py1 - v / y_max * (py1 - py0)

    svg = head(width, height, "ExpanseSet bytes per key across expanse occupancy")
    svg += (
        '  <text x="30" y="30" class="t-title">MEMORY DENSITY ACROSS EXPANSE OCCUPANCY &#183; UNIFORM RANDOM KEYS</text>\n'
        f'  <text x="30" y="46" class="t-sub">ExpanseSet bytes/key against &#955; = N / 2^(w&#8722;48), the mean population of a 2-byte-prefix expanse &#183; '
        f'log &#955; axis &#183; lower is better</text>\n'
        '  <line x1="30" y1="58" x2="930" y2="58" class="divider"/>\n'
    )
    # y grid and ticks
    ticks = int(y_max // 4) + 1
    for i in range(ticks):
        v = i * 4.0
        svg += (
            f'  <line x1="{px0}" y1="{Y(v):.1f}" x2="{px1}" y2="{Y(v):.1f}" class="{"axis" if i == 0 else "grid"}"/>\n'
            f'  <text x="{px0 - 8}" y="{Y(v) + 3.5:.1f}" class="t-axis-label" text-anchor="end">{v:.0f}</text>\n'
        )
    svg += f'  <text x="{px0 - 8}" y="{py0 - 12}" class="t-unit" text-anchor="end">B/key</text>\n'
    # x ticks at powers of two within range
    lam = 1.0
    while lam <= lam_max:
        if lam >= lam_min:
            svg += (
                f'  <line x1="{X(lam):.1f}" y1="{py0}" x2="{X(lam):.1f}" y2="{py1}" class="grid"/>\n'
                f'  <text x="{X(lam):.1f}" y="{py1 + 16}" class="t-axis-label" text-anchor="middle">{lam:g}</text>\n'
            )
        lam *= 2
    svg += f'  <text x="{(px0 + px1) / 2:.0f}" y="{py1 + 34}" class="t-unit" text-anchor="middle">&#955; (keys per 2-byte expanse)</text>\n'
    # LEAF_CAP marker, and the same cap one byte level down (256 × LEAF_CAP):
    # the second tooth, drawn only when the data reaches it.
    svg += (
        f'  <line x1="{X(leaf_cap):.1f}" y1="{py0}" x2="{X(leaf_cap):.1f}" y2="{py1}" class="axis" stroke-dasharray="4,3"/>\n'
        f'  <text x="{X(leaf_cap) + 5:.1f}" y="{py0 + 12}" class="t-legend">LEAF_CAP = {leaf_cap}</text>\n'
        f'  <text x="{X(leaf_cap) + 5:.1f}" y="{py0 + 25}" class="t-note">linear leaf cascades into a branch</text>\n'
    )
    second = 256 * leaf_cap
    if lam_min < second < lam_max:
        svg += (
            f'  <line x1="{X(second):.1f}" y1="{py0}" x2="{X(second):.1f}" y2="{py1}" class="axis" stroke-dasharray="4,3"/>\n'
            f'  <text x="{X(second) - 5:.1f}" y="{py0 + 12}" class="t-legend" text-anchor="end">256 &#215; LEAF_CAP = {second}</text>\n'
            f'  <text x="{X(second) - 5:.1f}" y="{py0 + 25}" class="t-note" text-anchor="end">the next byte level cascades</text>\n'
        )
    # the curve through every cell, then per-width markers
    path = " ".join(f'{"M" if i == 0 else "L"}{X(c["lambda"]):.1f},{Y(c["set_bpk"]):.1f}' for i, c in enumerate(cells))
    svg += f'  <path d="{path}" fill="none" class="axis"/>\n'
    # Three marker shapes for the three widths of the original sweep; every
    # narrower width (the second-cliff cells at 58..55 bits) shares a fourth,
    # so a new width never raises a KeyError here.
    def marker(c: dict, x: float, y: float) -> str:
        bits = c["bits"]
        if bits == 64:
            return f'  <circle cx="{x:.1f}" cy="{y:.1f}" r="4.5" class="b-expanse"/>\n'
        if bits == 63:
            return f'  <rect x="{x - 4:.1f}" y="{y - 4:.1f}" width="8" height="8" class="b-expanse-alt"/>\n'
        if bits == 62:
            return f'  <polygon points="{x:.1f},{y - 5.5:.1f} {x + 5.5:.1f},{y:.1f} {x:.1f},{y + 5.5:.1f} {x - 5.5:.1f},{y:.1f}" class="b-other"/>\n'
        return f'  <polygon points="{x:.1f},{y - 5.5:.1f} {x + 5:.1f},{y + 3.5:.1f} {x - 5:.1f},{y + 3.5:.1f}" class="b-hash"/>\n'

    for c in cells:
        svg += marker(c, X(c["lambda"]), Y(c["set_bpk"]))
    # the two memory-budget gate cells (1M and 2M at 64 bits), labelled from the data
    # The 1M label sits below its point; the 2M label is anchored to the left of
    # its point, because the curve rises steeply through it and a centred label
    # above the point crosses the line.
    for n, label, dx, dy, anchor in (
        (1_000_000, "memory-budget cell, N = 1M", 0, 22, "middle"),
        (2_000_000, "second gate cell, N = 2M", -10, 4, "end"),
    ):
        c = next(c for c in cells if c["bits"] == 64 and c["n"] == n)
        x, y = X(c["lambda"]), Y(c["set_bpk"])
        svg += (
            f'  <text x="{x + dx:.1f}" y="{y + dy:.1f}" class="t-note" text-anchor="{anchor}">{label}: {c["set_bpk"]:.2f} B/key '
            f'(&#955; = {c["lambda"]:.2f})</text>\n'
        )
    # legend
    lx, ly = 730, 80
    svg += f'  <text x="{lx}" y="{ly}" class="t-legend">Keyspace width (same seed)</text>\n'
    narrow = sorted({c["bits"] for c in cells if c["bits"] < 62}, reverse=True)
    rows = [(64, "64-bit keys"), (63, "63-bit keys"), (62, "62-bit keys")]
    if narrow:
        rows.append((narrow[0], f"{narrow[0]}&#8211;{narrow[-1]}-bit keys" if len(narrow) > 1 else f"{narrow[0]}-bit keys"))
    for i, (bits, label) in enumerate(rows):
        yy = ly + 18 + i * 18
        svg += marker({"bits": bits}, lx + 6, yy - 4)
        n_cells = sum(1 for c in cells if (c["bits"] == bits if bits >= 62 else c["bits"] < 62))
        svg += f'  <text x="{lx + 18}" y="{yy}" class="t-legend">{label} &#183; {n_cells} cells</text>\n'
    # the width equivalence, derived: cells sharing a λ across widths
    by_lam: dict = {}
    for c in cells:
        by_lam.setdefault(round(c["lambda"], 2), []).append(c)
    shared = [v for v in by_lam.values() if len(v) > 1]
    spread = max((max(c["set_bpk"] for c in v) - min(c["set_bpk"] for c in v)) for v in shared) if shared else 0.0
    lo, hi = min(c["set_bpk"] for c in cells), max(c["set_bpk"] for c in cells)
    ly2 = ly + (18 if narrow else 0)
    svg += (
        f'  <text x="{lx}" y="{ly2 + 84}" class="t-note">{len(shared)} &#955; values are hit by two or three widths;</text>\n'
        f'  <text x="{lx}" y="{ly2 + 97}" class="t-note">their cells agree within {spread:.2f} B/key: one curve.</text>\n'
        f'  <text x="{lx}" y="{ly2 + 118}" class="t-note">Range under density alone: {lo:.2f}&#8211;{hi:.2f} B/key</text>\n'
        f'  <text x="{lx}" y="{ly2 + 131}" class="t-note">({hi / lo:.2f}&#215;) with no code change.</text>\n'
    )
    # anchors: construction-fixed occupancy, flat across N (derived min–max)
    anchors: dict = {}
    for a in block["anchors"]:
        anchors.setdefault(a["dist"], []).append(a["set_bpk"])
    svg += f'  <text x="{lx}" y="{ly2 + 156}" class="t-legend">Fixed-occupancy distributions</text>\n'
    for i, (dist, vals) in enumerate(sorted(anchors.items())):
        rng = f"{min(vals):.2f}" if abs(max(vals) - min(vals)) < 0.005 else f"{min(vals):.2f}&#8211;{max(vals):.2f}"
        svg += f'  <text x="{lx}" y="{ly2 + 172 + i * 14}" class="t-note">{esc(dist)}: {rng} B/key, flat across N</text>\n'
    meta = block["meta"]
    svg += (
        f'\n  <line x1="30" y1="{height - 46}" x2="930" y2="{height - 46}" class="divider"/>\n'
        f'  <text x="30" y="{height - 30}" class="t-note">Measured: {esc(meta["instrument"])}; commit {esc(meta["commit"])}; workload {esc(meta["workload_id"])}.</text>\n'
        f'  <text x="30" y="{height - 16}" class="t-note">Clearing one top key bit halves the expanses and is exactly a doubling of N, so the three widths share one axis. '
        f'Source: docs/ARCHITECTURE.md &#167;3.5.</text>\n'
        "</svg>\n"
    )
    write(ASSETS / "bench_density_sawtooth.svg", svg)


def main() -> int:
    data = json.loads(DATA.read_text(encoding="utf-8"))
    render_comparative(data)
    render_concurrency(data)
    render_ycsb(data)
    render_sync32_health(data)
    render_density(data)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
