#!/usr/bin/env python3
"""scripts/generate_stm32_svg.py

Renders the three STM32H747I-DISCO charts from the committed on-target
transcript summary ``docs/benchmarks/stm32h747/results.json`` (produced by
``integrations/stm32h747/harvest.py`` from one VCP transcript):

  * ``docs/assets/bench_stm32h747.svg`` — Expanse across clocks and cache
    states on the M7, plus the ISR-reader arm against the critical-section twin;
  * ``docs/assets/bench_stm32h747_alternatives.svg`` — Expanse against a
    sorted array, an open-addressing hash table and newlib's tsearch on the
    same fixtures, plus bytes per key;
  * ``docs/assets/bench_stm32h747_dualcore.svg`` — the cacheless Cortex-M4
    point and the M7-writer / M4-reader cells (optimistic, HSEM twin, and
    the unsupported cacheable-heap configuration).

Every bar, label, ratio and footer value is derived from that file at render
time; no summary constant is stamped into the markup (AGENTS.md §8.2) and each
footer carries the provenance (§8.7).

    python3 scripts/generate_stm32_svg.py
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from generate_asset_svgs import esc, head, write  # noqa: E402

DATA = REPO_ROOT / "docs" / "benchmarks" / "stm32h747" / "results.json"
OUT_MAIN = REPO_ROOT / "docs" / "assets" / "bench_stm32h747.svg"
OUT_ALT = REPO_ROOT / "docs" / "assets" / "bench_stm32h747_alternatives.svg"

FIXTURES = [
    ("ingest", "ingest 2,000 keys / insert"),
    ("can_dispatch", "CAN dispatch / get"),
    ("evict_bulk_loop", "evict 600 of 2,000 / per-key loop"),
    ("evict_bulk_range", "evict 600 of 2,000 / remove_range"),
    ("evict_steady_loop", "evict 25 of 2,000 / per-key loop"),
    ("evict_steady_range", "evict 25 of 2,000 / remove_range"),
]
CONFIGS = [  # (sysclk, dcache, label, fill)
    (64000000, 0, "64 MHz, D-cache off", "#cbd5e1"),
    (64000000, 1, "64 MHz, D-cache on", "#94a3b8"),
    (160000000, 0, "160 MHz, D-cache off", "#c7d2fe"),
    (160000000, 1, "160 MHz, D-cache on", "#6366f1"),
    (400000000, 0, "400 MHz, D-cache off", "#93c5fd"),
    (400000000, 1, "400 MHz, D-cache on", "#1d4ed8"),
]
IMPLS = [  # (impl key, label, fill)
    ("expanse", "Expanse (C ABI)", "#1d4ed8"),
    ("sorted_array", "sorted array, bsearch + memmove", "#f59e0b"),
    ("open_hash", "open-addressing hash, 50% load", "#10b981"),
    ("tsearch", "newlib tsearch (unbalanced BST)", "#94a3b8"),
]
# The alternatives chart shows each implementation's batched eviction: the
# ordered ones via remove_range, the hash table via its full scan.
ALT_ROWS = [
    ("ingest", {"*": "ingest"}, "ingest 2,000 sequential / insert"),
    ("can_dispatch", {"*": "can_dispatch"}, "CAN dispatch, 500 keys / get"),
    ("evict_bulk", {"*": "evict_bulk_range", "open_hash": "evict_bulk_scan"}, "evict 600 of 2,000, batched"),
    ("evict_steady", {"*": "evict_steady_range", "open_hash": "evict_steady_scan"}, "evict 25 of 2,000, batched"),
]


def mhz(hz: float) -> str:
    return f"{hz / 1e6:.1f} MHz"


def fixture(data: dict, impl: str, name: str, clk: int, dc: int, core: str = "m7") -> dict | None:
    for f in data["fixtures"]:
        if (f.get("core", "m7") == core and f.get("impl", "expanse") == impl and f["name"] == name
                and f["sysclk"] == clk and f["dcache"] == dc):
            return f
    return None


OUT_DUAL = REPO_ROOT / "docs" / "assets" / "bench_stm32h747_dualcore.svg"
M4_CLK = 200000000


def render_dualcore(data: dict) -> str:
    """Cortex-M4 (cacheless, 200 MHz) against the M7, and the two-core cells."""
    W, H = 1000, 560
    s = head(W, H, "Expanse on the STM32H747's Cortex-M4 and across both cores")
    s += '  <text x="20" y="26" class="t-title">The other core: cacheless Cortex-M4 point, and the M7-writer / M4-reader cells</text>\n'

    px, py = 20, 52
    s += f'  <g transform="translate({px}, {py})">\n'
    s += '    <text x="0" y="0" class="t-chart-title">Expanse on the M4 (200 MHz, no cache) vs the M7</text>\n'
    s += '    <text x="0" y="13" class="t-sub">core cycles per operation, min of 5 (lower is better); ns from host-verified clocks</text>\n'
    cols = [("m7", 400000000, 1, "M7 400 MHz, D-cache on", "#1d4ed8"),
            ("m7", 400000000, 0, "M7 400 MHz, D-cache off", "#93c5fd"),
            ("m4", M4_CLK, 0, "M4 200 MHz, no cache", "#f59e0b")]
    vals = {}
    for name, _ in FIXTURES:
        for core, clk, dc, _, _ in cols:
            f = fixture(data, "expanse", name, clk, dc, core)
            vals[(name, core, dc)] = f
    axis_max = 500 * (int(max(f["min"] for f in vals.values() if f) / 500) + 1)
    x0, bar_w, row_h = 215, 150, 52
    for i, (name, label) in enumerate(FIXTURES):
        y = 30 + i * row_h
        s += f'    <text x="{x0 - 8}" y="{y + 22}" class="t-legend" text-anchor="end">{esc(label)}</text>\n'
        for j, (core, _, dc, _, fill) in enumerate(cols):
            f = vals[(name, core, dc)]
            by = y + j * 13
            if not f:
                s += f'    <text x="{x0 + 4}" y="{by + 9}" class="t-note">n/a</text>\n'
                continue
            w = max(1.0, f["min"] / axis_max * bar_w)
            ns_txt = f' ({f["min_ns"]:,.0f} ns)' if f.get("min_ns") else ""
            s += (f'    <rect x="{x0}" y="{by}" width="{w:.1f}" height="11" rx="1" style="fill:{fill}"/>\n'
                  f'    <text x="{x0 + w + 4:.1f}" y="{by + 9}" class="t-axis-label">{f["min"]:,.0f}{esc(ns_txt)}</text>\n')
    y_axis = 30 + len(FIXTURES) * row_h
    for j, (_, _, _, lab, fill) in enumerate(cols):
        lx, ly = 0, y_axis + 8 + j * 15
        s += (f'    <rect x="{lx}" y="{ly}" width="10" height="10" rx="1" style="fill:{fill}"/>\n'
              f'    <text x="{lx + 14}" y="{ly + 9}" class="t-note">{esc(lab)}</text>\n')
    s += "  </g>\n"

    rx, ry = 500, 52
    dual = data.get("dual", [])
    s += f'  <g transform="translate({rx}, {ry})">\n'
    s += '    <text x="0" y="0" class="t-chart-title">Two cores on one sync32 map: M7 writes, M4 reads</text>\n'
    s += '    <text x="0" y="13" class="t-sub">map in the M7 heap (AXI SRAM); per writer duty: BUSY rate and M4 read cost incl. lock wait</text>\n'
    series = [
        ("noncacheable", "optimistic", "sync32 single-attempt reads, M7 heap non-cacheable (MPU)", "#2563eb"),
        ("noncacheable", "hsem", "hardware-semaphore twin: both sides lock HSEM 0 per access", "#94a3b8"),
        ("cacheable", "optimistic", "sync32 reads with the M7 heap cacheable — unsupported by the header", "#f59e0b"),
    ]
    y = 36
    for heap, mode, title, fill in series:
        cells = [d for d in dual if d.get("heap", d.get("arena")) == heap and d.get("mode", "optimistic") == mode]
        if not cells:
            continue
        s += f'    <rect x="0" y="{y - 9}" width="10" height="10" rx="1" style="fill:{fill}"/>\n'
        s += f'    <text x="14" y="{y}" class="t-legend">{esc(title)}</text>\n'
        s += (f'    <text x="0" y="{y + 17}" class="t-note">writer duty</text>'
              f'<text x="80" y="{y + 17}" class="t-note">BUSY</text>'
              f'<text x="200" y="{y + 17}" class="t-note">OK / not found</text>'
              f'<text x="315" y="{y + 17}" class="t-note">read cyc mean / max</text>'
              f'<text x="450" y="{y + 17}" class="t-note">bad</text>\n')
        for i, d in enumerate(cells):
            yy = y + 35 + i * 16
            duty = "full duty" if d["period"] == 0 else f'{d["mutations_per_s"] / 1000:g}k/s'
            bw = max(1.0, d["busy_rate"] * 60)
            bad = d["m4_bad"] + d["writer_bad"]
            s += (f'    <text x="0" y="{yy}" class="t-axis-label">{esc(duty)}</text>\n'
                  f'    <rect x="80" y="{yy - 9}" width="{bw:.1f}" height="10" rx="1" style="fill:{fill}"/>\n'
                  f'    <text x="{80 + bw + 4:.1f}" y="{yy}" class="t-axis-label">{100 * d["busy_rate"]:.1f}%</text>\n'
                  f'    <text x="200" y="{yy}" class="t-axis-label">{d["m4_ok"]:,} / {d["m4_nf"]:,}</text>\n'
                  f'    <text x="315" y="{yy}" class="t-axis-label">{d["m4_cyc_mean"]:,.0f} / {d["m4_cyc_max"]:,}</text>\n'
                  f'    <text x="450" y="{yy}" class="{"t-axis-label t-loss" if bad else "t-axis-label"}">{bad}</text>\n')
        y += 35 + len(cells) * 16 + 18
    if not dual:
        s += '    <text x="0" y="40" class="t-note">no dual-core cells in results.json</text>\n'
    hung = [d for d in dual if not d["m4_stopped"]]
    s += (f'    <text x="0" y="{y}" class="t-note">M4 reads AXI SRAM across the D2/D1 bridge; '
          f'{"every cell acknowledged stop" if not hung else f"{len(hung)} cell(s) left the M4 hung"}; '
          f'refusals {sum(d["refused"] for d in dual)}, arena-full {sum(d["arena_full"] for d in dual)}</text>\n')
    s += "  </g>\n"
    s += provenance(data, H)
    s += "</svg>\n"
    return s


def provenance(data: dict, H: int) -> str:
    info = data["info"]
    hz = info.get("measured_hz", {})
    calib = ", ".join(f"{int(k) / 1e6:g} nominal / {v / 1e6:.1f} measured MHz" for k, v in sorted(hz.items(), key=lambda kv: int(kv[0])))
    s = (f'  <text x="20" y="{H - 48}" class="t-note">measured: STM32H747I-DISCO, Cortex-M7 CPUID {info["cpuid"]} '
         f'(IDCODE {info.get("idcode", "?")}), D-cache {info["dcache_ways"]}-way x {info["dcache_sets"]} sets x '
         f'{info["dcache_line_bytes"]} B lines; direct SMPS supply, VOS3 to 160 MHz, VOS1 at 400 MHz</text>\n')
    s += f'  <text x="20" y="{H - 34}" class="t-note">clock check from the host side over 320M cycles: {esc(calib)}</text>\n'
    s += (f'  <text x="20" y="{H - 20}" class="t-note">DWT CYCCNT, single board; libexpanse staticlib commit '
          f'{info.get("commit", "see results.json")}; harness integrations/stm32h747; source docs/benchmarks/stm32h747/results.json</text>\n')
    return s


def render_main(data: dict) -> str:
    W, H = 1000, 600
    s = head(W, H, "Expanse on STM32H747I-DISCO (Cortex-M7): cycles per operation and the ISR-reader contract")
    s += '  <text x="20" y="26" class="t-title">Expanse on a Cortex-M7 (STM32H747I-DISCO), C ABI, on-target</text>\n'

    px, py = 20, 52
    s += f'  <g transform="translate({px}, {py})">\n'
    s += '    <text x="0" y="0" class="t-chart-title">Fixtures from benches/embedded_memtable.rs</text>\n'
    s += '    <text x="0" y="13" class="t-sub">core cycles per operation, min of 5 passes (lower is better)</text>\n'
    vals = {}
    for n, _ in FIXTURES:
        for c, d, _, _ in CONFIGS:
            f = fixture(data, "expanse", n, c, d)
            vals[(n, c, d)] = f["min"] if f else None
    present = [v for v in vals.values() if v is not None]
    axis_max = 500 * (int(max(present) / 500) + 1) if present else 1000
    x0, bar_w, gap, row_h = 215, 220, 1, 68
    for i, (name, label) in enumerate(FIXTURES):
        y = 30 + i * row_h
        s += f'    <text x="{x0 - 8}" y="{y + 34}" class="t-legend" text-anchor="end">{esc(label)}</text>\n'
        for j, (clk, dc, _, fill) in enumerate(CONFIGS):
            v = vals[(name, clk, dc)]
            if v is None:
                continue
            w = max(1.0, v / axis_max * bar_w)
            by = y + j * (9 + gap)
            s += (f'    <rect x="{x0}" y="{by}" width="{w:.1f}" height="9" rx="1" style="fill:{fill}"/>\n'
                  f'    <text x="{x0 + w + 4:.1f}" y="{by + 8}" class="t-axis-label">{v:,.0f}</text>\n')
    y_axis = 30 + len(FIXTURES) * row_h
    s += f'    <line x1="{x0}" y1="28" x2="{x0}" y2="{y_axis}" class="axis"/>\n'
    for j, (_, _, lab, fill) in enumerate(CONFIGS):
        lx, ly = (j % 3) * 150, y_axis + 10 + (j // 3) * 16
        s += (f'    <rect x="{lx}" y="{ly}" width="10" height="10" rx="1" style="fill:{fill}"/>\n'
              f'    <text x="{lx + 14}" y="{ly + 9}" class="t-note">{esc(lab)}</text>\n')
    s += "  </g>\n"

    rx, ry = 500, 52
    isr = [r for r in data["isr"] if r.get("core", "m7") == "m7"]
    sync = [r for r in isr if r["name"] == "isr_sync32"]
    cs = [r for r in isr if r["name"] == "isr_critical_section"]
    if not sync or not cs:
        s += provenance(data, H) + "</svg>\n"
        return s
    s += f'  <g transform="translate({rx}, {ry})">\n'
    s += '    <text x="0" y="0" class="t-chart-title">SysTick ISR reader while the main loop writes</text>\n'
    clk = sync[0]["sysclk"]
    s += (f'    <text x="0" y="13" class="t-sub">{clk / 1e6:g} MHz, D-cache on, {sync[0]["isr_n"]:,} interrupts per cell, '
          f'writer paced per row</text>\n')
    s += '    <text x="0" y="40" class="t-legend">writer duty</text>\n'
    s += '    <text x="112" y="40" class="t-legend">BUSY (sync32)</text>\n'
    s += '    <text x="240" y="40" class="t-legend">ISR entry latency, cycles (max / mean)</text>\n'
    s += ('    <rect x="240" y="45" width="10" height="10" rx="1" style="fill:#2563eb"/>'
          '<text x="254" y="54" class="t-note">sync32 reader</text>\n'
          '    <rect x="340" y="45" width="10" height="10" rx="1" style="fill:#94a3b8"/>'
          '<text x="354" y="54" class="t-note">cpsid/cpsie critical section</text>\n')
    lat_max = max(max(r["lat_max"] for r in sync), max(r["lat_max"] for r in cs))
    lat_w = 90
    for i, (a, b) in enumerate(zip(sync, cs)):
        y = 70 + i * 52
        duty = "full duty" if a["period"] == 0 else f'{a["mutations_per_s"] / 1000:g}k mut/s'
        s += f'    <text x="0" y="{y + 12}" class="t-bar-label">{esc(duty)}</text>\n'
        s += f'    <text x="0" y="{y + 26}" class="t-note">{a["writer_cycles_per_mutation"]:,.0f} cyc/mut</text>\n'
        bw = max(1.0, a["busy_rate"] * 80)
        s += (f'    <rect x="112" y="{y + 2}" width="{bw:.1f}" height="12" rx="1" style="fill:#f59e0b"/>\n'
              f'    <text x="{112 + bw + 4:.1f}" y="{y + 12}" class="t-axis-label">{100 * a["busy_rate"]:.1f}%</text>\n')
        s += f'    <text x="112" y="{y + 30}" class="t-note">{a["busy"]:,} of {a["isr_n"]:,} reads</text>\n'
        for k, (r, fill) in enumerate(((a, "#2563eb"), (b, "#94a3b8"))):
            w = max(1.0, r["lat_max"] / lat_max * lat_w)
            by = y + k * 16
            ns_txt = f' ({r["lat_max_ns"]:.0f} ns)' if r.get("lat_max_ns") else ""
            s += (f'    <rect x="240" y="{by}" width="{w:.1f}" height="12" rx="1" style="fill:{fill}"/>\n'
                  f'    <text x="{240 + w + 4:.1f}" y="{by + 10}" class="t-axis-label">'
                  f'{r["lat_max"]:,} / {r["lat_mean"]:.0f}{esc(ns_txt)}</text>\n')
    bad = sum(r["bad"] for r in isr)
    refused = sum(r["refused"] for r in sync)
    s += (f'    <text x="0" y="288" class="t-note">corrupted reads: {bad}; reclaim refusals: {refused}; '
          f'critical-section BUSY: {sum(r["busy"] for r in cs)} (it masks the interrupt instead)</text>\n')
    s += "  </g>\n"
    s += provenance(data, H)
    s += "</svg>\n"
    return s


def render_alternatives(data: dict) -> str:
    W, H = 1000, 540
    clk, dc = 400000000, 1
    s = head(W, H, "Expanse against a sorted array, an open-addressing hash table and newlib tsearch on the STM32H747I-DISCO")
    s += '  <text x="20" y="26" class="t-title">Expanse vs what firmware usually reaches for, same Cortex-M7, same fixtures</text>\n'

    px, py = 20, 52
    s += f'  <g transform="translate({px}, {py})">\n'
    s += f'    <text x="0" y="0" class="t-chart-title">cycles per operation at {clk / 1e6:g} MHz, D-cache on (log scale, lower is better)</text>\n'
    s += ('    <text x="0" y="13" class="t-sub">min of 5 passes, one fixture code path behind a vtable; batched eviction = remove_range, '
          'or a full scan for the hash table</text>\n')
    cells = {}
    for key, names, _ in ALT_ROWS:
        for impl, _, _ in IMPLS:
            f = fixture(data, impl, names.get(impl, names["*"]), clk, dc)
            cells[(key, impl)] = f["min"] if f else None
    lo, hi = 10.0, 10 ** math.ceil(math.log10(max(v for v in cells.values() if v)))
    x0, bar_w, row_h = 215, 290, 78
    def xpos(v: float) -> float:
        return x0 + (math.log10(v) - math.log10(lo)) / (math.log10(hi) - math.log10(lo)) * bar_w
    for i, (key, _, label) in enumerate(ALT_ROWS):
        y = 30 + i * row_h
        s += f'    <text x="{x0 - 8}" y="{y + 30}" class="t-legend" text-anchor="end">{esc(label)}</text>\n'
        base = cells[(key, "expanse")]
        for j, (impl, _, fill) in enumerate(IMPLS):
            v = cells[(key, impl)]
            by = y + j * 15
            if v is None:
                s += f'    <text x="{x0 + 4}" y="{by + 10}" class="t-note">n/a</text>\n'
                continue
            w = max(1.0, xpos(v) - x0)
            ratio = v / base if base else 0
            tag = "" if impl == "expanse" else (f"  {1 / ratio:.1f}x faster" if ratio < 1 else f"  {ratio:.1f}x slower")
            s += (f'    <rect x="{x0}" y="{by}" width="{w:.1f}" height="12" rx="1" style="fill:{fill}"/>\n'
                  f'    <text x="{x0 + w + 4:.1f}" y="{by + 10}" class="t-axis-label">{v:,.0f}{esc(tag)}</text>\n')
    y_axis = 30 + len(ALT_ROWS) * row_h
    for p in range(int(math.log10(lo)), int(math.log10(hi)) + 1):
        x = xpos(10 ** p)
        s += (f'    <line x1="{x:.1f}" y1="28" x2="{x:.1f}" y2="{y_axis}" class="grid"/>\n'
              f'    <text x="{x:.1f}" y="{y_axis + 12}" class="t-axis-label" text-anchor="middle">{10 ** p:,}</text>\n')
    for j, (_, lab, fill) in enumerate(IMPLS):
        lx, ly = (j % 2) * 240, y_axis + 26 + (j // 2) * 16
        s += (f'    <rect x="{lx}" y="{ly}" width="10" height="10" rx="1" style="fill:{fill}"/>\n'
              f'    <text x="{lx + 14}" y="{ly + 9}" class="t-note">{esc(lab)}</text>\n')
    s += "  </g>\n"

    rx, ry = 660, 52
    s += f'  <g transform="translate({rx}, {ry})">\n'
    s += '    <text x="0" y="0" class="t-chart-title">bytes per key, 2,000 keys</text>\n'
    s += '    <text x="0" y="13" class="t-sub">newlib heap in use (mallinfo), overhead included</text>\n'
    shapes = [("ingest", "sequential keys, one map"), ("ble_index", "hash keys, dual index (ordered) or one table (hash)")]
    bmax = max(b["heap_bytes_per_key"] for b in data["bytes"])
    for si, (shape, slabel) in enumerate(shapes):
        y = 34 + si * 120
        s += f'    <text x="0" y="{y}" class="t-legend">{esc(slabel)}</text>\n'
        for j, (impl, _, fill) in enumerate(IMPLS):
            b = next((b for b in data["bytes"] if b["impl"] == impl and b["shape"] == shape), None)
            by = y + 8 + j * 20
            if not b:
                continue
            w = max(1.0, b["heap_bytes_per_key"] / bmax * 200)
            s += (f'    <rect x="0" y="{by}" width="{w:.1f}" height="14" rx="1" style="fill:{fill}"/>\n'
                  f'    <text x="{w + 4:.1f}" y="{by + 11}" class="t-axis-label">{b["heap_bytes_per_key"]:.1f} B/key</text>\n')
    s += ('    <text x="0" y="292" class="t-note">the sorted array and the hash table are pre-sized to their capacity;</text>\n'
          '    <text x="0" y="305" class="t-note">Expanse and tsearch grow through malloc as keys arrive</text>\n')
    s += "  </g>\n"
    s += provenance(data, H)
    s += "</svg>\n"
    return s


def main() -> int:
    data = json.loads(DATA.read_text())
    write(OUT_MAIN, render_main(data))
    write(OUT_ALT, render_alternatives(data))
    write(OUT_DUAL, render_dualcore(data))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
