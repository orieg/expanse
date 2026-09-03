#!/usr/bin/env python3
"""scripts/generate_wasm_svg.py

Renders the WebAssembly suite chart from the committed fuel baseline
``results/baseline_wasm_fuel.json`` (written by ``scripts/wasm_fuel.py
--save-baseline``, one entry per target):

  * ``docs/assets/bench_wasm_fuel.svg`` — fuel per operation for every arm on
    wasm32 (32-bit engine) and wasm64 (64-bit engine), map arms and set arms
    side by side, and the engines' own ``mem_used`` bytes per key.

Every bar, label, ratio and footer value is derived from that file at render
time; no summary constant is stamped into the markup (AGENTS.md §8.2) and the
footer carries the provenance (§8.7). Regenerate after every re-baseline:

    python3 scripts/generate_wasm_svg.py
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from generate_asset_svgs import esc, head, write  # noqa: E402

DATA = REPO_ROOT / "results" / "baseline_wasm_fuel.json"
OUT = REPO_ROOT / "docs" / "assets" / "bench_wasm_fuel.svg"

TARGETS = [  # (target triple, legend label, fill)
    ("wasm32-unknown-unknown", "wasm32 · 32-bit engine (Edge32, 8 B)", "#1d4ed8"),
    ("wasm64-unknown-unknown", "wasm64 · 64-bit engine (Edge, 16 B)", "#f59e0b"),
]
OPS = [("insert", "insert"), ("get", "get / contains, 50% hit"), ("iterate", "iterate"), ("range", "range, 100 windows"), ("remove", "remove")]
DISTS = ["sequential", "clustered", "random"]

W, H = 1180, 560
PANEL_TOP = 118
ROW = 14  # px per bar row
GROUP_GAP = 8


def load() -> dict[str, dict]:
    if not DATA.exists():
        sys.exit(f"{DATA} missing — run scripts/wasm_fuel.py --save-baseline first")
    entries = json.loads(DATA.read_text(encoding="utf-8"))
    by = {e["target"]: e for e in entries}
    missing = [t for t, _, _ in TARGETS if t not in by]
    if missing:
        sys.exit(f"{DATA} lacks entries for {missing}")
    return by


def per_op(entry: dict) -> dict[str, float]:
    return {a["name"]: a["per_op"] for a in entry["arms"]}


def arm_name(structure: str, op: str, dist: str) -> str:
    op_key = {"get": "contains" if structure == "set" else "get"}.get(op, op)
    return f"{structure}_{op_key}/{dist}"


def fmt(v: float) -> str:
    return f"{v:,.0f}" if v >= 100 else f"{v:,.1f}"


def render_fuel_panel(by: dict[str, dict], structure: str, x0: int, width: int, title: str) -> str:
    """Paired horizontal bars, one row per (op, dist), bars scaled to the panel max."""
    vals = {t: per_op(by[t]) for t, _, _ in TARGETS}
    rows = [(op, label, d) for op, label in OPS for d in DISTS]
    vmax = max(vals[t][arm_name(structure, op, d)] for t, _, _ in TARGETS for op, _, d in rows)
    label_w = 118
    bar_x = x0 + label_w
    bar_w_max = width - label_w - 150
    s = f'  <text x="{x0}" y="{PANEL_TOP - 30}" class="t-chart-title">{esc(title)}</text>\n'
    s += f'  <text x="{x0}" y="{PANEL_TOP - 16}" class="t-note">fuel units per operation, exact; shorter is better; label is wasm64 ÷ wasm32</text>\n'
    y = PANEL_TOP
    for gi, (op, label) in enumerate(OPS):
        s += f'  <text x="{x0}" y="{y + 9}" class="t-legend">{esc(label)}</text>\n'
        y += 13
        for d in DISTS:
            name = arm_name(structure, op, d)
            s += f'  <text x="{bar_x - 6}" y="{y + 9}" class="t-axis-label" text-anchor="end">{esc(d)}</text>\n'
            v32 = vals[TARGETS[0][0]][name]
            v64 = vals[TARGETS[1][0]][name]
            for i, (t, _, fill) in enumerate(TARGETS):
                v = vals[t][name]
                w = max(1.0, bar_w_max * v / vmax)
                by_ = y + i * 6
                s += f'    <rect x="{bar_x}" y="{by_}" width="{w:.1f}" height="5" rx="1" style="fill:{fill}"/>\n'
            ratio = v64 / v32
            cls = "t-val-accent" if ratio < 1 else "t-val-blue"
            longest = bar_w_max * max(v32, v64) / vmax
            s += (f'  <text x="{bar_x + longest + 6}" y="{y + 9}" class="{cls}" style="text-anchor:start">'
                  f'{fmt(v32)} → {fmt(v64)}  {ratio:.2f}×</text>\n')
            y += ROW
        y += GROUP_GAP
    return s


def render_mem_panel(by: dict[str, dict], x0: int, width: int) -> str:
    s = f'  <text x="{x0}" y="{PANEL_TOP - 30}" class="t-chart-title">bytes per key, the engine\'s own mem_used</text>\n'
    s += f'  <text x="{x0}" y="{PANEL_TOP - 16}" class="t-note">N = 10,000 after the build; exact; label wasm64 ÷ wasm32</text>\n'
    label_w = 100
    bar_x = x0 + label_w
    bar_w_max = width - label_w - 150
    rows = [(st, d) for st in ("map", "set") for d in DISTS]
    vals = {t: by[t]["mem_used_bytes_per_key"] for t, _, _ in TARGETS}
    vmax = max(vals[t][f"{st}/{d}"] for t, _, _ in TARGETS for st, d in rows)
    y = PANEL_TOP
    for st, d in rows:
        key = f"{st}/{d}"
        s += f'  <text x="{bar_x - 6}" y="{y + 9}" class="t-axis-label" text-anchor="end">{esc(key)}</text>\n'
        for i, (t, _, fill) in enumerate(TARGETS):
            v = vals[t][key]
            w = max(1.0, bar_w_max * v / vmax)
            s += f'    <rect x="{bar_x}" y="{y + i * 6}" width="{w:.1f}" height="5" rx="1" style="fill:{fill}"/>\n'
        v32, v64 = vals[TARGETS[0][0]][key], vals[TARGETS[1][0]][key]
        ratio = v64 / v32
        cls = "t-val-accent" if ratio < 1 else "t-val-blue"
        longest = bar_w_max * max(v32, v64) / vmax
        s += (f'  <text x="{bar_x + longest + 6}" y="{y + 9}" class="{cls}" style="text-anchor:start">'
              f'{v32:.2f} → {v64:.2f}  {ratio:.2f}×</text>\n')
        y += ROW + 4
    return s


def render_reading(by: dict[str, dict], x0: int, y0: int) -> str:
    """The count of arms each engine wins, derived, not stamped."""
    v32, v64 = per_op(by[TARGETS[0][0]]), per_op(by[TARGETS[1][0]])
    names = sorted(v32)
    fewer64 = sum(1 for n in names if v64[n] < v32[n])
    m32, m64 = by[TARGETS[0][0]]["mem_used_bytes_per_key"], by[TARGETS[1][0]]["mem_used_bytes_per_key"]
    map_ratios = [m32[f"map/{d}"] / m64[f"map/{d}"] for d in DISTS]
    s = f'  <text x="{x0}" y="{y0}" class="t-legend">reading</text>\n'
    s += (f'  <text x="{x0}" y="{y0 + 15}" class="t-sub">the 64-bit engine consumes less fuel on {fewer64} of {len(names)} arms; '
          f'the 32-bit engine holds a map key in {min(map_ratios) * 100:.0f}–{max(map_ratios) * 100:.0f}% of the bytes</text>\n')
    s += (f'  <text x="{x0}" y="{y0 + 29}" class="t-note">fuel counts executed wasm instructions under wasmtime: not cycles, not wall clock, and not a mechanism — '
          f'why one engine spends fewer is not measured (AGENTS.md §8.9)</text>\n')
    return s


def provenance(by: dict[str, dict]) -> str:
    e32, e64 = by[TARGETS[0][0]], by[TARGETS[1][0]]
    s = (f'  <text x="20" y="{H - 44}" class="t-note">measured: {esc(e32["host"])}, wasmtime {esc(e32["wasmtime"])} (Python bindings, fuel metering, '
         f'two fresh-instance runs agreeing to the unit)</text>\n')
    s += f'  <text x="20" y="{H - 31}" class="t-note">wasm32 built by {esc(e32["rustc"])}; wasm64 built by {esc(e64["rustc"])} with -Z build-std</text>\n'
    s += (f'  <text x="20" y="{H - 18}" class="t-note">commit {esc(e32["commit"])}; module crates/expanse-wasm-fuel, driver scripts/wasm_fuel.py; '
          f'source results/baseline_wasm_fuel.json (N = {e32["pop"]:,}); the same source builds both targets</text>\n')
    return s


def render(by: dict[str, dict]) -> str:
    s = head(W, H, "Expanse on WebAssembly: exact fuel per operation on wasm32 (32-bit engine) and wasm64 (64-bit engine), and bytes per key")
    s += '  <text x="20" y="28" class="t-title">Expanse on WebAssembly — one source, two engines, exact fuel</text>\n'
    s += ('  <text x="20" y="44" class="t-sub">wasm32 selects the 32-bit engine and wasm64 the 64-bit engine; each row is one fixture on both, '
          'under one runtime. Fuel is the Callgrind analogue for the wasm targets and gates every wasm PR.</text>\n')
    lx = 20
    for t, label, fill in TARGETS:
        s += f'  <rect x="{lx}" y="58" width="10" height="10" rx="1" style="fill:{fill}"/>\n'
        s += f'  <text x="{lx + 14}" y="67" class="t-legend">{esc(label)}</text>\n'
        lx += 270
    s += render_fuel_panel(by, "map", 20, 400, "map arms, 32-bit vs 64-bit engine")
    s += f'  <line x1="430" y1="{PANEL_TOP - 40}" x2="430" y2="{H - 118}" class="divider"/>\n'
    s += render_fuel_panel(by, "set", 445, 400, "set arms, 32-bit vs 64-bit engine")
    s += f'  <line x1="855" y1="{PANEL_TOP - 40}" x2="855" y2="{H - 118}" class="divider"/>\n'
    s += render_mem_panel(by, 870, 300)
    s += f'  <line x1="20" y1="{H - 108}" x2="{W - 20}" y2="{H - 108}" class="divider"/>\n'
    s += render_reading(by, 20, H - 88)
    s += provenance(by)
    s += "</svg>\n"
    return s


def main() -> int:
    by = load()
    write(OUT, render(by))
    return 0


if __name__ == "__main__":
    sys.exit(main())
