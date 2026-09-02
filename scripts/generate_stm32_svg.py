#!/usr/bin/env python3
"""scripts/generate_stm32_svg.py

Renders ``docs/assets/bench_stm32h747.svg`` from the committed on-target
transcript summary ``docs/benchmarks/stm32h747/results.json`` (itself produced
by ``integrations/stm32h747/harvest.py`` from one VCP transcript). Every bar,
label, ratio and footer value is derived from that file at render time; no
summary constant is stamped into the markup (AGENTS.md §8.2) and the footer
carries the provenance (§8.7).

    python3 scripts/generate_stm32_svg.py
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from generate_asset_svgs import esc, head, write  # noqa: E402

DATA = REPO_ROOT / "docs" / "benchmarks" / "stm32h747" / "results.json"
OUT = REPO_ROOT / "docs" / "assets" / "bench_stm32h747.svg"

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
    (160000000, 0, "160 MHz, D-cache off", "#93c5fd"),
    (160000000, 1, "160 MHz, D-cache on", "#2563eb"),
]


def mhz(hz: int) -> str:
    return f"{hz / 1e6:g} MHz"


def fixture_min(data: dict, name: str, clk: int, dc: int) -> float:
    for f in data["fixtures"]:
        if f["name"] == name and f["sysclk"] == clk and f["dcache"] == dc:
            return f["min"]
    raise KeyError((name, clk, dc))


def render(data: dict) -> str:
    info = data["info"]
    W, H = 1000, 520
    s = head(W, H, "Expanse on STM32H747I-DISCO (Cortex-M7): cycles per operation and the ISR-reader contract")
    s += '  <text x="20" y="26" class="t-title">Expanse on a Cortex-M7 (STM32H747I-DISCO), C ABI, on-target</text>\n'

    # ---- left panel: cycles/op per fixture, four configurations ----------
    px, py = 20, 52
    s += f'  <g transform="translate({px}, {py})">\n'
    s += '    <text x="0" y="0" class="t-chart-title">Fixtures from benches/embedded_memtable.rs</text>\n'
    s += '    <text x="0" y="13" class="t-sub">core cycles per operation, min of 5 passes (lower is better)</text>\n'
    axis_max = max(fixture_min(data, n, c, d) for n, _ in FIXTURES for c, d, _, _ in CONFIGS)
    axis_max = 500 * (int(axis_max / 500) + 1)
    x0, bar_w, gap, row_h = 215, 220, 2, 56
    for i, (name, label) in enumerate(FIXTURES):
        y = 30 + i * row_h
        s += f'    <text x="{x0 - 8}" y="{y + 26}" class="t-legend" text-anchor="end">{esc(label)}</text>\n'
        for j, (clk, dc, _, fill) in enumerate(CONFIGS):
            v = fixture_min(data, name, clk, dc)
            w = max(1.0, v / axis_max * bar_w)
            by = y + j * (10 + gap)
            s += (f'    <rect x="{x0}" y="{by}" width="{w:.1f}" height="10" rx="1" style="fill:{fill}"/>\n'
                  f'    <text x="{x0 + w + 4:.1f}" y="{by + 8.5}" class="t-axis-label">{v:,.0f}</text>\n')
    y_axis = 30 + len(FIXTURES) * row_h
    s += f'    <line x1="{x0}" y1="28" x2="{x0}" y2="{y_axis}" class="axis"/>\n'
    for j, (_, _, lab, fill) in enumerate(CONFIGS):
        lx = 0 + j * 130
        s += (f'    <rect x="{lx}" y="{y_axis + 10}" width="10" height="10" rx="1" style="fill:{fill}"/>\n'
              f'    <text x="{lx + 14}" y="{y_axis + 19}" class="t-note">{esc(lab)}</text>\n')
    s += "  </g>\n"

    # ---- right panel: ISR-reader arm vs critical-section twin -------------
    rx, ry = 500, 52
    isr = data["isr"]
    sync = [r for r in isr if r["name"] == "isr_sync32"]
    cs = [r for r in isr if r["name"] == "isr_critical_section"]
    s += f'  <g transform="translate({rx}, {ry})">\n'
    s += '    <text x="0" y="0" class="t-chart-title">SysTick ISR reader while the main loop writes</text>\n'
    clk = sync[0]["sysclk"]
    s += (f'    <text x="0" y="13" class="t-sub">{esc(mhz(clk))}, D-cache on, {sync[0]["isr_n"]:,} interrupts per cell, '
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
            s += (f'    <rect x="240" y="{by}" width="{w:.1f}" height="12" rx="1" style="fill:{fill}"/>\n'
                  f'    <text x="{240 + w + 4:.1f}" y="{by + 10}" class="t-axis-label">'
                  f'{r["lat_max"]:,} / {r["lat_mean"]:.0f}</text>\n')
    bad = sum(r["bad"] for r in isr)
    refused = sum(r["refused"] for r in sync)
    s += (f'    <text x="0" y="288" class="t-note">corrupted reads: {bad}; reclaim refusals: {refused}; '
          f'critical-section BUSY: {sum(r["busy"] for r in cs)} (it masks the interrupt instead)</text>\n')
    s += "  </g>\n"

    # ---- footer / provenance ---------------------------------------------
    calib = info.get("calibration", [])
    calib_txt = ", ".join(f'{c["cycles"] / 1e6:g}M cycles in {c["host_seconds"]:.3f} s host time'
                          for c in calib) if calib else "no host calibration recorded"
    s += (f'  <text x="20" y="{H - 48}" class="t-note">measured: STM32H747I-DISCO, Cortex-M7 CPUID {info["cpuid"]}, '
          f'D-cache {info["dcache_ways"]}-way x {info["dcache_sets"]} sets x {info["dcache_line_bytes"]} B lines; '
          f'core 64 MHz HSI then 160 MHz PLL1 (HCLK 80 MHz); DWT CYCCNT, single board</text>\n')
    s += f'  <text x="20" y="{H - 34}" class="t-note">clock check from the host side: {esc(calib_txt)}</text>\n'
    s += (f'  <text x="20" y="{H - 20}" class="t-note">libexpanse staticlib commit {info.get("commit", "see results.json")}; '
          f'harness integrations/stm32h747; source docs/benchmarks/stm32h747/results.json</text>\n')
    s += "</svg>\n"
    return s


def main() -> int:
    data = json.loads(DATA.read_text())
    write(OUT, render(data))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
