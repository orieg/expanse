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


def _cyc(results: dict, bench: str, pop: int) -> float:
    """Mean cycles/op for one on-device arm, or a loud failure (§8.1)."""
    for entry in results["benchmarks"].values():
        if entry["benchmark"] == bench and entry["pop"] == pop:
            arm = next(iter(entry["arms"].values()))
            return float(arm["cycles_per_op"]["mean"])
    raise SystemExit(
        f"{ESP32_RESULTS}: no arm '{bench}' at population {pop}. Refusing to "
        f"render a panel with a missing arm (§8.1)."
    )


def render_on_device() -> int:
    """Renders the on-device chart from the ESP32 harvest artifact.

    Panels 1-2 are what was measured. Panel 3 is the same data in the unit a
    reader decides with: a cycles-per-op bar cannot answer "can this part
    keep up with my sensor?", and a rate can. It is labelled derived.

    Text is laid out to fit: bar captions stay short enough not to collide at
    70px bar spacing, and footer lines stay inside the canvas. An earlier
    revision overflowed both.
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
        raise SystemExit(f"{ESP32_RESULTS}: no cpu_hz; every derived cell needs it (§8.1).")
    mhz = hz / 1e6

    ingest = _cyc(results, "esp32_tsdb_ingest", 2000)
    ingest_500 = _cyc(results, "esp32_tsdb_ingest", 500)
    agg_500 = _cyc(results, "esp32_tsdb_aggregate_500", 500)
    agg_2k = _cyc(results, "esp32_tsdb_aggregate_500", 2000)
    record = _cyc(results, "esp32_ble_sighting_record", 2000)
    lookup = _cyc(results, "esp32_ble_point_lookup", 2000)
    evict = _cyc(results, "esp32_ble_ttl_eviction", 2000)

    churn = next((v for v in results.get("metrics", {}).values()
                  if v["metric"] == "churn_fragmentation"), None)
    if churn is None:
        raise SystemExit(f"{ESP32_RESULTS}: no churn_fragmentation metric (§8.1).")
    frag_delta = churn["fields"]["frag_after"]["mean"] - churn["fields"]["frag_before"]["mean"]
    retained = churn["fields"]["heap_retained_bytes"]["mean"]

    def us(c: float) -> str:
        return f"{c / mhz:.1f} &#181;s"

    def rate(c: float) -> float:
        return hz / c

    # Offered loads a reader would actually run these at.
    duty_ingest = 1000.0 * ingest / hz          # 1 kHz sensor
    duty_lookup = 1000.0 * lookup / hz          # 1 kHz dispatch
    duty_record = 10.0 * record / hz            # 10 BLE sightings/s

    mem_max = nice_axis_max([ingest, agg_500, agg_2k])
    ble_max = nice_axis_max([record, lookup, evict])
    rates = [rate(ingest), rate(lookup), rate(record)]
    rate_max = nice_axis_max(rates)

    def krate(v: float) -> str:
        return f"{v / 1000:.1f}k"

    p1 = panel(
        30, "Telemetry MemTable", "cycles/op &#183; lower is better",
        "&#9660; CPU cycles / op", mem_max, f"{mem_max:,.0f}", f"{mem_max / 2:,.0f}",
        bar(BAR_X[0], ingest, mem_max, "b-expanse", f"{ingest:,.0f}",
            "t-val-muted", "insert", us(ingest), False)
        + bar(BAR_X[1], agg_500, mem_max, "b-hashmap", f"{agg_500:,.0f}",
              "t-val-blue", "agg N=500", us(agg_500), False)
        + bar(BAR_X[2], agg_2k, mem_max, "b-hashmap", f"{agg_2k:,.0f}",
              "t-val-blue", "agg N=2k", us(agg_2k), False),
    )
    p2 = panel(
        350, "BLE Asset Tracker", "cycles/op &#183; N=2k &#183; lower is better",
        "&#9660; CPU cycles / op", ble_max, f"{ble_max:,.0f}", f"{ble_max / 2:,.0f}",
        bar(BAR_X[0], record, ble_max, "b-expanse", f"{record:,.0f}",
            "t-val-muted", "record", us(record), False)
        + bar(BAR_X[1], lookup, ble_max, "b-hashmap", f"{lookup:,.0f}",
              "t-val-blue", "lookup", us(lookup), False)
        + bar(BAR_X[2], evict, ble_max, "b-stdmap", f"{evict:,.0f}",
              "t-val-muted", "TTL evict", us(evict), False),
    )
    p3 = panel(
        670, "Headroom On One Core", "derived ops/s &#183; higher is better",
        "&#9650; operations / second", rate_max, f"{rate_max / 1000:,.0f}k",
        f"{rate_max / 2000:,.0f}k",
        bar(BAR_X[0], rates[0], rate_max, "b-expanse", krate(rates[0]),
            "t-val-muted", "insert", f"1kHz={duty_ingest * 100:.1f}%", False)
        + bar(BAR_X[1], rates[1], rate_max, "b-hashmap", krate(rates[1]),
              "t-val-blue", "lookup", f"1kHz={duty_lookup * 100:.1f}%", False)
        + bar(BAR_X[2], rates[2], rate_max, "b-stdmap", krate(rates[2]),
              "t-val-muted", "BLE rec", f"10/s={duty_record * 100:.2f}%", False),
    )

    footer = (
        f'  <text x="30" y="262" class="t-chart-sub">Measured on {prov["target"]} rev '
        f'v{prov.get("revision", "?")} &#183; {mhz:.0f} MHz &#183; ESP-IDF '
        f'{prov.get("idf", "?")} &#183; engine {prov.get("expanse", "?")} &#183; '
        f'10 reps/arm, BCa 95% CIs.</text>\n'
        f'  <text x="30" y="275" class="t-chart-sub">Single-arm: no unordered_map, std::map or '
        f'ring-buffer arm ran on the device. These size the part for a workload; they do not '
        f'rank Expanse against anything.</text>\n'
        f'  <text x="30" y="288" class="t-chart-sub">Panel 3 is derived from panels 1-2. Insert '
        f'is flat in population ({ingest_500:,.0f} cycles at N=500). Churn left {retained:,.0f} B '
        f'resident, fragmentation {frag_delta:+.4f}. FreeRTOS mutex inside every timed '
        f'window.</text>\n'
    )

    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 300" width="100%" height="100%">\n'
        "  <defs>\n    <style>" + STYLE + "    </style>\n  </defs>\n\n"
        '  <rect width="100%" height="100%" class="bg" rx="8"/>\n'
        '  <rect width="100%" height="100%" class="border" rx="8"/>\n\n'
        + p1
        + '\n  <line x1="330" y1="20" x2="330" y2="245" class="divider"/>\n\n'
        + p2
        + '\n  <line x1="650" y1="20" x2="650" y2="245" class="divider"/>\n\n'
        + p3
        + "\n"
        + footer
        + "</svg>\n"
    )
    ET.fromstring(svg)  # refuse to write malformed XML
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
              f'{ingest["hashmap"]:.1f}', "t-val-blue", "HashMap", "unordered", False)
        + bar(BAR_X[2], ingest["btreemap"], ing_max, "b-stdmap",
              f'{ingest["btreemap"]:.0f}', "t-val-muted", "BTreeMap", "ordered", False),
    )
    p3 = panel(
        670, "CAN ID Dispatch", f"29-bit IDs, N={CAN_N} &#183; measured &#183; lower is better",
        "&#9660; ns / lookup", lk_max, f"{lk_max:g}", f"{lk_max / 2:g}",
        bar(BAR_X[0], lookup["expanse"], lk_max, "b-expanse",
            f'{lookup["expanse"]:.1f}', "t-val-accent", "Expanse", lk_cap, lk_win)
        + bar(BAR_X[1], lookup["hashmap"], lk_max, "b-hashmap",
              f'{lookup["hashmap"]:.1f}', "t-val-blue", "HashMap", "unordered", False)
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
