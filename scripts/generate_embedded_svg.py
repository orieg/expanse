#!/usr/bin/env python3
"""scripts/generate_embedded_svg.py

Regenerates ``docs/assets/bench_embedded.svg`` (the three-panel embedded
storage-engine chart embedded by ``docs/DATABASE.md`` §5.4) from
``docs/benchmarks/embedded/results.json``.

Panels 1-2 (memory) are **derived**, not measured: every byte figure is
computed by importing ``scripts/embedded_envelope.py`` — the committed,
unit-tested derivation whose density constants come from the measured
``bytes_per_key_32`` example — so no constant is retyped here (§8.2).
Panel 3 (wall clock) is **measured**: it renders the BCa-harvested
``baseline-embedded_memtable.json`` a quiet-host bench run produces
(``scripts/bench_baseline.py --harvest``), and the chart footer carries
that run's provenance.

Refresh the committed record from a bench-host artifact, then the SVG:

    python3 scripts/generate_embedded_svg.py --from-baseline baseline-embedded_memtable.json
    python3 scripts/generate_embedded_svg.py           # re-render from committed results.json

XML is validated before writing (same discipline as
``integrations/rocksdb/scripts/generate_bench_svg.py``, whose geometry and
dual-theme styling this mirrors).
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Iterable
import xml.etree.ElementTree as ET

REPO_ROOT = Path(__file__).resolve().parents[1]
RESULTS = REPO_ROOT / "docs" / "benchmarks" / "embedded" / "results.json"
# The on-device harvest is a separate artifact and a separate chart: its arms
# are CPU cycles on a microcontroller, and the panels above are host
# wall-clock nanoseconds. Rendering them on one canvas would juxtapose two
# metric domains with no shared workload, which is the complection §8.12
# exists to stop.
ESP32_RESULTS = REPO_ROOT / "docs" / "benchmarks" / "embedded" / "esp32.json"
ESP32_OUTPUT = REPO_ROOT / "docs" / "assets" / "bench_esp32_ondevice.svg"
OUTPUT = REPO_ROOT / "docs" / "assets" / "bench_embedded.svg"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
import embedded_envelope as env  # noqa: E402  (single source of the memory model)

N = 2000  # the mid population every #556 table reports

# Criterion full_ids for the measured panels (see benches/embedded_memtable.rs).
# The measured loop shapes, needed to state per-operation units honestly:
# ingest cycles run N inserts + a range flush; CAN dispatch runs 500 lookups
# per iteration; eviction runs one full eviction pass per iteration.
CAN_N = 500
MEASURED_ARMS = {
    "ingest_expanse": f"embedded_tsdb_ingest_and_flush/expanse_map32_ingest/{N}",
    "ingest_hashmap": f"embedded_tsdb_ingest_and_flush/hashmap_ingest/{N}",
    "ingest_btreemap": f"embedded_tsdb_ingest_and_flush/btreemap_ingest/{N}",
    "lookup_expanse": f"embedded_can_dispatch_lookup/expanse_map32/{CAN_N}",
    "lookup_hashmap": f"embedded_can_dispatch_lookup/hashmap/{CAN_N}",
    "lookup_btreemap": f"embedded_can_dispatch_lookup/btreemap/{CAN_N}",
    "evict_bulk_expanse": f"embedded_ble_ttl_eviction/expanse_dual_trie_eviction/{N}",
    "evict_bulk_hashmap": f"embedded_ble_ttl_eviction/hashmap_full_scan_eviction/{N}",
    "evict_steady_expanse": f"embedded_ble_ttl_eviction/expanse_dual_trie_eviction_steady/{N}",
    "evict_steady_hashmap": f"embedded_ble_ttl_eviction/hashmap_full_scan_eviction_steady/{N}",
    # Batched eviction through `remove_range` (#578), next to the per-key loop.
    "evict_bulk_range_expanse": f"embedded_ble_ttl_eviction/expanse_dual_trie_eviction_range/{N}",
    "evict_steady_range_expanse": f"embedded_ble_ttl_eviction/expanse_dual_trie_eviction_range_steady/{N}",
}

# Plot geometry shared by all three panels (rocksdb chart's layout).
BASELINE_Y = 195.0
AXIS_TOP_Y = 45.0
PLOT_H = BASELINE_Y - AXIS_TOP_Y
BAR_X = [45, 115, 185]
BAR_X2 = [60, 170]
# Four-arm layout for the on-device comparison panels.
BAR_X4 = [32, 90, 148, 206]
BAR_W4 = 46
BAR_W = 50

STYLE = """
      .bg { fill: #ffffff; }
      .border { stroke: #e2e8f0; stroke-width: 1px; fill: none; }
      .grid { stroke: #f1f5f9; stroke-width: 1px; stroke-dasharray: 2,3; }
      .axis { stroke: #cbd5e1; stroke-width: 1.5px; }
      .divider { stroke: #e2e8f0; stroke-width: 1px; }

      .t-chart-title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11.5px; font-weight: 700; letter-spacing: 0.6px; fill: #0f172a; text-transform: uppercase; }
      .t-chart-sub { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10px; font-weight: 500; fill: #334155; }
      .t-unit-header { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 600; fill: #475569; }
      .t-axis-label { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 500; fill: #475569; }
      .t-win { fill: #16a34a; font-weight: 600; }
      .t-bar-label { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11px; font-weight: 700; fill: #0f172a; }

      .t-val-accent { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 700; fill: #16a34a; text-anchor: middle; }
      .t-val-muted { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 600; fill: #334155; text-anchor: middle; }
      .t-val-blue { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 600; fill: #2563eb; text-anchor: middle; }

      .b-expanse { fill: #16a34a; }
      .b-hashmap { fill: #2563eb; }
      .b-stdmap { fill: #475569; }

      @media (prefers-color-scheme: dark) {
        :root:not([data-theme="light"]) .bg { fill: #0b1220; }
        :root:not([data-theme="light"]) .border { stroke: #1e293b; }
        :root:not([data-theme="light"]) .grid { stroke: #16233b; }
        :root:not([data-theme="light"]) .axis { stroke: #334155; }
        :root:not([data-theme="light"]) .divider { stroke: #1e293b; }
        :root:not([data-theme="light"]) .t-chart-title { fill: #e2e8f0; }
        :root:not([data-theme="light"]) .t-chart-sub { fill: #94a3b8; }
        :root:not([data-theme="light"]) .t-unit-header, :root:not([data-theme="light"]) .t-axis-label { fill: #64748b; }
        :root:not([data-theme="light"]) .t-bar-label { fill: #e2e8f0; }
        :root:not([data-theme="light"]) .t-val-muted { fill: #94a3b8; }
        :root:not([data-theme="light"]) .t-win { fill: #4ade80; }
        :root:not([data-theme="light"]) .t-val-accent { fill: #4ade80; }
        :root:not([data-theme="light"]) .t-val-blue { fill: #60a5fa; }
        :root:not([data-theme="light"]) .b-expanse { fill: #22c55e; }
        :root:not([data-theme="light"]) .b-hashmap { fill: #3b82f6; }
        :root:not([data-theme="light"]) .b-stdmap { fill: #475569; }
      }
      [data-theme="dark"] .bg, :root[data-theme="dark"] .bg { fill: #0b1220; }
      [data-theme="dark"] .border, :root[data-theme="dark"] .border { stroke: #1e293b; }
      [data-theme="dark"] .grid, :root[data-theme="dark"] .grid { stroke: #16233b; }
      [data-theme="dark"] .axis, :root[data-theme="dark"] .axis { stroke: #334155; }
      [data-theme="dark"] .divider, :root[data-theme="dark"] .divider { stroke: #1e293b; }
      [data-theme="dark"] .t-chart-title, :root[data-theme="dark"] .t-chart-title { fill: #e2e8f0; }
      [data-theme="dark"] .t-chart-sub, :root[data-theme="dark"] .t-chart-sub { fill: #94a3b8; }
      [data-theme="dark"] .t-unit-header, :root[data-theme="dark"] .t-unit-header { fill: #64748b; }
      [data-theme="dark"] .t-axis-label, :root[data-theme="dark"] .t-axis-label { fill: #64748b; }
      [data-theme="dark"] .t-bar-label, :root[data-theme="dark"] .t-bar-label { fill: #e2e8f0; }
      [data-theme="dark"] .t-val-muted, :root[data-theme="dark"] .t-val-muted { fill: #94a3b8; }
      [data-theme="dark"] .t-win, :root[data-theme="dark"] .t-win { fill: #4ade80; }
      [data-theme="dark"] .t-val-accent, :root[data-theme="dark"] .t-val-accent { fill: #4ade80; }
      [data-theme="dark"] .t-val-blue, :root[data-theme="dark"] .t-val-blue { fill: #60a5fa; }
      [data-theme="dark"] .b-expanse, :root[data-theme="dark"] .b-expanse { fill: #22c55e; }
      [data-theme="dark"] .b-hashmap, :root[data-theme="dark"] .b-hashmap { fill: #3b82f6; }
      [data-theme="dark"] .b-stdmap, :root[data-theme="dark"] .b-stdmap { fill: #475569; }
"""


def nice_axis_max(values: "Iterable[float]") -> float:
    """1/1.2/1.5/2/2.5/3/4/5/6/7/8/10 ladder ceiling (see the rocksdb
    generator for why the ladder is finer than 1/2/5)."""
    peak = max(values)
    if peak <= 0:
        raise ValueError("axis needs at least one positive value")
    target = peak / 0.9
    exp = math.floor(math.log10(target))
    for mult in (1.0, 1.2, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0):
        candidate = mult * (10.0**exp)
        if candidate >= target:
            return candidate
    return 10.0 ** (exp + 1)


def bar(x: int, value: float, axis_max: float, css: str, val_text: str, val_css: str,
        label: str, caption: str, caption_win: bool) -> str:
    height = round(value / axis_max * PLOT_H, 1)
    y = round(BASELINE_Y - height, 1)
    cx = x + BAR_W // 2
    caption_cls = "t-chart-sub t-win" if caption_win else "t-chart-sub"
    return (
        f'    <rect x="{x}" y="{y}" width="{BAR_W}" height="{height}" class="{css}" rx="2"/>\n'
        f'    <text x="{cx}" y="{y - 8.5:.1f}" class="{val_css}">{val_text}</text>\n'
        f'    <text x="{cx}" y="214" class="t-bar-label" text-anchor="middle">{label}</text>\n'
        f'    <text x="{cx}" y="228" class="{caption_cls}" text-anchor="middle">{caption}</text>\n'
    )


def bar_w(x: int, width: int, value: float, axis_max: float, css: str, val_text: str,
          val_css: str, label: str, caption: str, caption_win: bool) -> str:
    """`bar` with an explicit width, for the four-arm comparison panels."""
    height = round(value / axis_max * PLOT_H, 1)
    y = round(BASELINE_Y - height, 1)
    cx = x + width // 2
    caption_cls = "t-chart-sub t-win" if caption_win else "t-chart-sub"
    return (
        f'    <rect x="{x}" y="{y}" width="{width}" height="{height}" class="{css}" rx="2"/>\n'
        f'    <text x="{cx}" y="{y - 8.5:.1f}" class="{val_css}">{val_text}</text>\n'
        f'    <text x="{cx}" y="214" class="t-bar-label" text-anchor="middle">{label}</text>\n'
        f'    <text x="{cx}" y="228" class="{caption_cls}" text-anchor="middle">{caption}</text>\n'
    )


def panel(tx: int, title: str, sub: str, unit: str, axis_max: float,
          top_label: str, mid_label: str, bars: str) -> str:
    return (
        f'  <g transform="translate({tx}, 20)">\n'
        f'    <text x="0" y="0" class="t-chart-title">{title}</text>\n'
        f'    <text x="0" y="13" class="t-chart-sub">{sub}</text>\n'
        f'    <text x="0" y="29" class="t-unit-header">{unit}</text>\n'
        f'    <line x1="30" y1="45" x2="250" y2="45" class="grid"/>\n'
        f'    <text x="22" y="48" class="t-axis-label" text-anchor="end">{top_label}</text>\n'
        f'    <line x1="30" y1="120" x2="250" y2="120" class="grid"/>\n'
        f'    <text x="22" y="123" class="t-axis-label" text-anchor="end">{mid_label}</text>\n'
        f'    <line x1="30" y1="195" x2="250" y2="195" class="axis"/>\n'
        f'    <text x="22" y="198" class="t-axis-label" text-anchor="end">0</text>\n'
        f'    <line x1="30" y1="40" x2="30" y2="195" class="axis"/>\n'
        f"{bars}"
        f"  </g>\n"
    )


def derive_memory() -> dict:
    """All memory cells straight from the envelope module — never retyped."""
    return {
        "tsdb_bytes_per_key": {
            "expanse": env.mem_expanse_tsdb(N) / N,
            "hashmap": env.mem_unordered_map(N) / N,
            "stdmap": env.mem_std_map(N) / N,
        },
    }


def rebuild_results(baseline_path: Path) -> dict:
    doc = json.loads(baseline_path.read_text(encoding="utf-8"))
    by_id = {}
    for a in doc.get("arms", []):
        arm_id = a.get("id") or a.get("arm") or a.get("full_id")
        if arm_id:
            by_id[arm_id] = a
    wall = {}
    for name, full_id in MEASURED_ARMS.items():
        a = by_id.get(full_id)
        if a is None:
            raise SystemExit(
                f"error: arm '{full_id}' absent from {baseline_path} — refusing to "
                f"render a panel with a missing arm (§8.1)."
            )
        wall[name] = {
            "point_ns": a["point_ns"],
            "ci_lower_ns": a.get("ci_lower_ns"),
            "ci_upper_ns": a.get("ci_upper_ns"),
            "status": a.get("status"),
        }
    return {
        "meta": {
            "population": N,
            "can_lookups_per_iter": CAN_N,
            "memory_model": "scripts/embedded_envelope.py (density constants from the "
                            "measured bytes_per_key_32 example; auxiliary slab arrays "
                            "modeled exactly)",
            "eviction_shapes": {
                "bulk": "600 of 2,000 expired per pass (symmetric < 6,000 ms cutoff)",
                "steady": "25 of 2,000 expired per pass (the O(expired) claim's regime)",
            },
            "wallclock_provenance": doc.get("provenance", {}),
            "wallclock_statistics": doc.get("statistics", {}),
        },
        "wallclock_ns": wall,
    }


def loss_caption(ours: float, best_other: float, win_text: str) -> tuple[str, bool]:
    """A win says how much faster; a loss says N x slower — never a sub-1.0
    ratio a reader has to invert (§8.7 honest reporting)."""
    if ours <= best_other:
        return (win_text.format(best_other / ours), True)
    return (f"{ours / best_other:.1f}&#215; slower", False)


def _arm(results: dict, bench: str, pop: int, arm: str) -> dict:
    """One arm of one on-device benchmark, or a loud failure (§8.1)."""
    for entry in results["benchmarks"].values():
        if entry["benchmark"] == bench and entry["pop"] == pop:
            if arm in entry["arms"]:
                return entry["arms"][arm]
            raise SystemExit(
                f"{ESP32_RESULTS}: benchmark '{bench}' at population {pop} has no "
                f"arm '{arm}' (has {sorted(entry['arms'])}). Refusing to render a "
                f"comparison with a missing arm (§8.1)."
            )
    raise SystemExit(f"{ESP32_RESULTS}: no benchmark '{bench}' at population {pop} (§8.1).")


def _cyc(results: dict, bench: str, pop: int, arm: str = "expanse_memtable") -> float:
    """The arm's median cycles/op — the robust figure, not the mean.

    One repetition with a FreeRTOS tick or a flash-cache miss storm inside
    its timed window moves the mean by more than any code change this chart
    shows; the harvester records both and flags the contaminated arms.
    """
    c = _arm(results, bench, pop, arm)["cycles_per_op"]
    return float(c.get("median", c["mean"]))


def render_on_device() -> int:
    """Renders the on-device comparison chart from the ESP32 harvest.

    Four panels, chosen to state the trade rather than to flatter it: two
    where Expanse loses and two where it wins. The arms it loses to are real
    containers under the same lock, holding the same payload, retiring the
    same set -- see components/expanse/test/twin_containers.h.
    """
    if not ESP32_RESULTS.exists():
        raise SystemExit(
            f"{ESP32_RESULTS} does not exist. Harvest a run first:\n"
            f"  python3 scripts/esp32_bench_harvest.py --input <log> "
            f"--emit-json docs/benchmarks/embedded/esp32.json"
        )
    results = json.loads(ESP32_RESULTS.read_text(encoding="utf-8"))
    prov = results.get("provenance") or {}
    if not prov.get("target"):
        raise SystemExit(f"{ESP32_RESULTS}: no provenance; refusing to render an "
                         f"unidentified target (§8.7).")
    hz = float(prov.get("cpu_hz", 0))
    if not hz:
        raise SystemExit(f"{ESP32_RESULTS}: no cpu_hz (§8.1).")
    mhz = hz / 1e6
    POP = 2000

    seq = {a: _cyc(results, "esp32_tsdb_ingest", POP, a)
           for a in ("expanse_memtable", "hash_open_addressing", "sorted_array", "ring_buffer")}
    shuf = {a: _cyc(results, "esp32_tsdb_ingest_shuffled", POP, a)
            for a in ("expanse_memtable", "hash_open_addressing", "sorted_array")}
    agg = {a: _cyc(results, "esp32_tsdb_aggregate_500", POP, a)
           for a in ("expanse_memtable", "hash_open_addressing", "sorted_array", "ring_buffer")}
    mem = {a: float(_arm(results, "esp32_tsdb_ingest", POP, a)["heap_used_bytes"]) / POP
           for a in ("expanse_memtable", "sorted_array", "hash_open_addressing")}
    lookup = {a: _cyc(results, "esp32_ble_point_lookup", POP, a)
              for a in ("expanse_slab", "hash_open_addressing", "linear_scan")}

    def ratio(ours: float, theirs: float) -> tuple[str, bool]:
        """Ours against one named peer, and whether we won it."""
        won = theirs > ours
        return (f"{theirs / ours:.1f}&#215; faster" if won
                else f"{ours / theirs:.1f}&#215; slower"), won

    seq_max = nice_axis_max(seq.values())
    shuf_max = nice_axis_max(shuf.values())
    agg_max = nice_axis_max(agg.values())
    mem_max = nice_axis_max(mem.values())

    # The ingest and scan captions compare Expanse against the SORTED ARRAY,
    # not against whichever twin happens to be fastest. The hash is unordered:
    # it cannot answer a range query at all, so beating or losing to it says
    # nothing about the ordered-structure choice these panels are about. Its
    # bar stays for absolute context, captioned "no order".
    seq_peer = seq["sorted_array"]
    shuf_peer = shuf["sorted_array"]
    agg_peer = agg["sorted_array"]
    best_mem = min(v for k, v in mem.items() if k != "expanse_memtable")

    seq_cap, seq_win = ratio(seq["expanse_memtable"], seq_peer)
    shuf_cap, shuf_win = ratio(shuf["expanse_memtable"], shuf_peer)
    agg_cap, agg_win = ratio(agg["expanse_memtable"], agg_peer)

    p1 = panel(
        30, "Ingest: Keys In Order", f"N={POP:,} &#183; ratio vs sorted array",
        "&#9660; CPU cycles / op", seq_max, f"{seq_max:,.0f}", f"{seq_max / 2:,.0f}",
        bar_w(BAR_X4[0], BAR_W4, seq["expanse_memtable"], seq_max, "b-expanse",
              f'{seq["expanse_memtable"]:,.0f}',
              "t-val-accent" if seq_win else "t-val-muted", "Expanse",
              seq_cap, seq_win)
        + bar_w(BAR_X4[1], BAR_W4, seq["hash_open_addressing"], seq_max, "b-hashmap",
                f'{seq["hash_open_addressing"]:,.0f}', "t-val-blue", "hash", "no order", False)
        + bar_w(BAR_X4[2], BAR_W4, seq["sorted_array"], seq_max, "b-stdmap",
                f'{seq["sorted_array"]:,.0f}', "t-val-muted", "sorted", "appends", False)
        + bar_w(BAR_X4[3], BAR_W4, seq["ring_buffer"], seq_max, "b-stdmap",
                f'{seq["ring_buffer"]:,.0f}', "t-val-muted", "ring", "no index", False),
    )
    p2 = panel(
        350, "Ingest: Keys Shuffled", f"N={POP:,} &#183; ratio vs sorted array",
        "&#9660; CPU cycles / op", shuf_max, f"{shuf_max:,.0f}", f"{shuf_max / 2:,.0f}",
        bar_w(BAR_X4[0], BAR_W4, shuf["expanse_memtable"], shuf_max, "b-expanse",
              f'{shuf["expanse_memtable"]:,.0f}',
              "t-val-accent" if shuf_win else "t-val-muted", "Expanse",
              shuf_cap, shuf_win)
        + bar_w(BAR_X4[1], BAR_W4, shuf["hash_open_addressing"], shuf_max, "b-hashmap",
                f'{shuf["hash_open_addressing"]:,.0f}', "t-val-blue", "hash", "no order", False)
        + bar_w(BAR_X4[2], BAR_W4, shuf["sorted_array"], shuf_max, "b-stdmap",
                f'{shuf["sorted_array"]:,.0f}', "t-val-muted", "sorted", "memmove", False),
    )
    p3 = panel(
        670, "Range Scan", f"per key walked &#183; ratio vs sorted array",
        "&#9660; CPU cycles / key", agg_max, f"{agg_max:,.0f}", f"{agg_max / 2:,.0f}",
        bar_w(BAR_X4[0], BAR_W4, agg["expanse_memtable"], agg_max, "b-expanse",
              f'{agg["expanse_memtable"]:,.0f}',
              "t-val-accent" if agg_win else "t-val-muted", "Expanse",
              agg_cap, agg_win)
        + bar_w(BAR_X4[1], BAR_W4, agg["hash_open_addressing"], agg_max, "b-hashmap",
                f'{agg["hash_open_addressing"]:,.0f}', "t-val-blue", "hash", "scan all", False)
        + bar_w(BAR_X4[2], BAR_W4, agg["sorted_array"], agg_max, "b-stdmap",
                f'{agg["sorted_array"]:,.0f}', "t-val-muted", "sorted", "contig.", False)
        + bar_w(BAR_X4[3], BAR_W4, agg["ring_buffer"], agg_max, "b-stdmap",
                f'{agg["ring_buffer"]:,.0f}', "t-val-muted", "ring", "scan all", False),
    )
    p4 = panel(
        990, "Memory Per Key", f"live heap delta &#183; N={POP:,}",
        "&#9660; bytes / key", mem_max, f"{mem_max:.0f} B", f"{mem_max / 2:.0f} B",
        bar_w(BAR_X4[0], BAR_W4, mem["expanse_memtable"], mem_max, "b-expanse",
              f'{mem["expanse_memtable"]:.2f}', "t-val-accent", "Expanse",
              f"{best_mem / mem['expanse_memtable']:.1f}&#215; denser", True)
        + bar_w(BAR_X4[1], BAR_W4, mem["sorted_array"], mem_max, "b-stdmap",
                f'{mem["sorted_array"]:.2f}', "t-val-muted", "sorted", "flat 8B", False)
        + bar_w(BAR_X4[2], BAR_W4, mem["hash_open_addressing"], mem_max, "b-hashmap",
                f'{mem["hash_open_addressing"]:.2f}', "t-val-blue", "hash", "0.7 load", False),
    )

    ble_ratio, _ = ratio(lookup["expanse_slab"], lookup["hash_open_addressing"])
    footer = (
        f'  <text x="30" y="262" class="t-chart-sub">Measured on {prov["target"]} rev '
        f'v{prov.get("revision", "?")} &#183; {mhz:.0f} MHz &#183; ESP-IDF '
        f'{prov.get("idf", "?")} &#183; engine {prov.get("expanse", "?")} &#183; '
        f'10 reps/arm, BCa 95% CIs.</text>\n'
        f'  <text x="30" y="275" class="t-chart-sub">Every arm runs under the same FreeRTOS '
        f'recursive mutex, stores the same payload and retires the same set. The sorted array '
        f'is the same container in both ingest panels: order of arrival is the only difference.</text>\n'
        f'  <text x="30" y="288" class="t-chart-sub">Not shown: BLE point lookup, where Expanse is '
        f'{ble_ratio.replace("&#215;", "x")} than the hash twin, and TTL eviction, where it loses at '
        f'both expiry regimes measured. Full table in docs/DATABASE.md &#167;5.4.</text>\n'
    )

    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1300 300" width="100%" height="100%">\n'
        "  <defs>\n    <style>" + STYLE + "    </style>\n  </defs>\n\n"
        '  <rect width="100%" height="100%" class="bg" rx="8"/>\n'
        '  <rect width="100%" height="100%" class="border" rx="8"/>\n\n'
        + p1 + '\n  <line x1="330" y1="20" x2="330" y2="245" class="divider"/>\n\n'
        + p2 + '\n  <line x1="650" y1="20" x2="650" y2="245" class="divider"/>\n\n'
        + p3 + '\n  <line x1="970" y1="20" x2="970" y2="245" class="divider"/>\n\n'
        + p4 + "\n" + footer + "</svg>\n"
    )
    ET.fromstring(svg)
    ESP32_OUTPUT.write_text(svg, encoding="utf-8")
    print(f"wrote {ESP32_OUTPUT}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--from-baseline", type=Path,
                    help="baseline-embedded_memtable.json from a bench-host run; "
                         "rebuilds results.json before rendering")
    ap.add_argument("--on-device", action="store_true",
                    help="render the on-device ESP32 chart from "
                         "docs/benchmarks/embedded/esp32.json instead")
    args = ap.parse_args()

    if args.on_device:
        return render_on_device()

    if args.from_baseline:
        results = rebuild_results(args.from_baseline)
        RESULTS.parent.mkdir(parents=True, exist_ok=True)
        RESULTS.write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {RESULTS}")
    else:
        results = json.loads(RESULTS.read_text(encoding="utf-8"))

    mem = derive_memory()["tsdb_bytes_per_key"]  # derived live, never persisted
    wall = {k: v["point_ns"] for k, v in results["wallclock_ns"].items()}
    prov = results["meta"]["wallclock_provenance"]

    # Integration units: ns per event / per lookup; eviction stays per pass.
    ingest = {k.removeprefix("ingest_"): wall[k] / N for k in
              ("ingest_expanse", "ingest_hashmap", "ingest_btreemap")}
    lookup = {k.removeprefix("lookup_"): wall[k] / CAN_N for k in
              ("lookup_expanse", "lookup_hashmap", "lookup_btreemap")}
    evict_us = {
        "steady_expanse": wall["evict_steady_expanse"] / 1000.0,
        "steady_hashmap": wall["evict_steady_hashmap"] / 1000.0,
        "bulk_expanse": wall["evict_bulk_expanse"] / 1000.0,
        "bulk_hashmap": wall["evict_bulk_hashmap"] / 1000.0,
        "steady_range": wall["evict_steady_range_expanse"] / 1000.0,
        "bulk_range": wall["evict_bulk_range_expanse"] / 1000.0,
    }

    mem_max = nice_axis_max(mem.values())
    ing_max = nice_axis_max(ingest.values())
    lk_max = nice_axis_max(lookup.values())
    ev_max = nice_axis_max(
        [evict_us["steady_expanse"], evict_us["steady_range"], evict_us["steady_hashmap"]])

    mem_ratio = mem["stdmap"] / mem["expanse"]
    ing_cap, ing_win = loss_caption(
        ingest["expanse"], min(ingest["hashmap"], ingest["btreemap"]), "{:.1f}&#215; faster")
    lk_cap, lk_win = loss_caption(
        lookup["expanse"], min(lookup["hashmap"], lookup["btreemap"]), "{:.1f}&#215; faster")
    ev_cap, ev_win = loss_caption(
        evict_us["steady_range"], evict_us["steady_hashmap"], "{:.1f}&#215; faster")
    ev_loop_cap, ev_loop_win = loss_caption(
        evict_us["steady_expanse"], evict_us["steady_hashmap"], "{:.1f}&#215; faster")

    p1 = panel(
        30, "Sensor TSDB Density", f"1 kHz timestamps, N={N:,} &#183; derived &#183; lower is better",
        "&#9660; SRAM (Bytes / key)", mem_max, f"{mem_max:g} B", f"{mem_max / 2:g} B",
        bar(BAR_X[0], mem["expanse"], mem_max, "b-expanse",
            f'{mem["expanse"]:.2f}', "t-val-accent", "Expanse",
            f"{mem_ratio:.1f}&#215; denser", True)
        + bar(BAR_X[1], mem["hashmap"], mem_max, "b-hashmap",
              f'{mem["hashmap"]:.0f}', "t-val-blue", "HashMap", "reserved", False)
        + bar(BAR_X[2], mem["stdmap"], mem_max, "b-stdmap",
              f'{mem["stdmap"]:.0f}', "t-val-muted", "std::map", "rb-tree", False),
    )
    p2 = panel(
        350, "Telemetry Ingest", f"per event, incl. flush share &#183; measured &#183; lower is better",
        "&#9660; ns / event", ing_max, f"{ing_max:g}", f"{ing_max / 2:g}",
        bar(BAR_X[0], ingest["expanse"], ing_max, "b-expanse",
            f'{ingest["expanse"]:.0f}', "t-val-accent", "Expanse", ing_cap, ing_win)
        + bar(BAR_X[1], ingest["hashmap"], ing_max, "b-hashmap",
              f'{ingest["hashmap"]:.1f}', "t-val-blue", "HashMap", "no order", False)
        + bar(BAR_X[2], ingest["btreemap"], ing_max, "b-stdmap",
              f'{ingest["btreemap"]:.0f}', "t-val-muted", "BTreeMap", "ordered", False),
    )
    p3 = panel(
        670, "CAN ID Dispatch", f"29-bit IDs, N={CAN_N} &#183; measured &#183; lower is better",
        "&#9660; ns / lookup", lk_max, f"{lk_max:g}", f"{lk_max / 2:g}",
        bar(BAR_X[0], lookup["expanse"], lk_max, "b-expanse",
            f'{lookup["expanse"]:.1f}', "t-val-accent", "Expanse", lk_cap, lk_win)
        + bar(BAR_X[1], lookup["hashmap"], lk_max, "b-hashmap",
              f'{lookup["hashmap"]:.1f}', "t-val-blue", "HashMap", "no order", False)
        + bar(BAR_X[2], lookup["btreemap"], lk_max, "b-stdmap",
              f'{lookup["btreemap"]:.1f}', "t-val-muted", "BTreeMap", "ordered", False),
    )
    p4 = panel(
        990, "Stale-Device Expiry", f"evict 25 of {N:,} &#183; batched / per-key &#183; measured",
        "&#9660; &#181;s / housekeeping pass", ev_max, f"{ev_max:g}", f"{ev_max / 2:g}",
        bar(BAR_X[0], evict_us["steady_range"], ev_max, "b-expanse",
            f'{evict_us["steady_range"]:.2f}', "t-val-accent", "batched", ev_cap, ev_win)
        + bar(BAR_X[1], evict_us["steady_expanse"], ev_max, "b-expanse",
              f'{evict_us["steady_expanse"]:.1f}', "t-val-accent", "per-key",
              ev_loop_cap, ev_loop_win)
        + bar(BAR_X[2], evict_us["steady_hashmap"], ev_max, "b-hashmap",
              f'{evict_us["steady_hashmap"]:.1f}', "t-val-blue", "HashMap",
              f"scans all {N // 1000}k", False),
    )

    run_num = str(prov.get("run_id", "unrecorded")).rstrip("/").split("/")[-1]
    host = prov.get("host_description", "unrecorded host")
    commit = str(prov.get("commit", "unrecorded"))[:9]
    bulk_line = (
        f'bulk shape (600 of {N:,} expired): remove_range {evict_us["bulk_range"]:.1f} &#181;s, '
        f'per-key loop {evict_us["bulk_expanse"]:.0f} &#181;s vs sweep {evict_us["bulk_hashmap"]:.1f} &#181;s'
        + (' — the linear sweep still wins bulk expiry'
           if evict_us["bulk_range"] > evict_us["bulk_hashmap"] else '')
    )
    scaling_line = (
        "Expiry scaling: Expanse&#8217;s pass cost follows the stale count; the HashMap sweep "
        f"follows the tracked count (~68 KiB flat table, L2-resident on this host)"
    )
    footer = (
        f'  <text x="30" y="262" class="t-chart-sub">Panel 1 derived by scripts/embedded_envelope.py; '
        f'panels 2-4 measured: {host} &#183; commit {commit}, run {run_num}.</text>\n'
        f'  <text x="30" y="275" class="t-chart-sub">{scaling_line}.</text>\n'
        f'  <text x="30" y="288" class="t-chart-sub">{bulk_line}.</text>\n'
        f'  <text x="30" y="301" class="t-chart-sub">Host caveat: a 30 MiB L3 flatters flat scans; '
        f'the on-device chart is rendered separately (--on-device, ESP32 only; C3/C6 unharvested) &#183; '
        f'BCa 95% CIs in docs/benchmarks/embedded/results.json.</text>\n'
    )

    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1300 313" width="100%" height="100%">\n'
        "  <defs>\n    <style>" + STYLE + "    </style>\n  </defs>\n\n"
        '  <rect width="100%" height="100%" class="bg" rx="8"/>\n'
        '  <rect width="100%" height="100%" class="border" rx="8"/>\n\n'
        + p1
        + '\n  <line x1="330" y1="20" x2="330" y2="245" class="divider"/>\n\n'
        + p2
        + '\n  <line x1="650" y1="20" x2="650" y2="245" class="divider"/>\n\n'
        + p3
        + '\n  <line x1="970" y1="20" x2="970" y2="245" class="divider"/>\n\n'
        + p4
        + "\n"
        + footer
        + "</svg>\n"
    )

    ET.fromstring(svg)  # refuse to write malformed XML
    OUTPUT.write_text(svg, encoding="utf-8")
    print(f"wrote {OUTPUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
