#!/usr/bin/env python3
"""
Dual-theme SVG chart generator for the LLM inference benchmark suite (#342).

Produces four charts:
  1. bench_draft_quality_alpha.svg   (Pillar A, macro reference-continuation acceptance alpha)
  2. bench_llm_datastore_scaling.svg (Pillar B, streaming ingestion throughput)
  3. bench_grammar_masks_memory.svg  (Pillar D, DFA mask cache memory)
  4. bench_prefix_lru_throughput.svg (Pillar E, KV-block table RAM)
"""

import json
import xml.etree.ElementTree as ET
from pathlib import Path

from theme import svg_header

BASE_DIR = Path(__file__).resolve().parent.parent
RESULTS_DIR = BASE_DIR / "results"


def save_svg(filepath: Path, content: str) -> None:
    try:
        ET.fromstring(content)
    except ET.ParseError as err:
        print(f"XML validation error in {filepath.name}: {err}")
        raise
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Generated & validated: {filepath}")


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def render_draft_quality_chart():
    json_path = RESULTS_DIR / "bench_draft_quality.json"
    if not json_path.exists():
        return
    with open(json_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    workloads = [
        ("humaneval_code", "HumanEval Code (N=40)", "OpenAI HumanEval/0..39 (MIT)"),
        ("summarization", "Document Summarization (N=9)", "Wikipedia Computer Science Corpus"),
        ("json_schemas", "JSON Schemas (N=5)", "SchemaStore JSON Schemas & Payloads"),
    ]

    max_val = 4.0
    bar_max = 340.0
    row_h = 56
    top = 96
    height = top + len(workloads) * row_h + 24

    svg = svg_header(width=960, height=height, title="PILLAR A — REFERENCE-CONTINUATION ACCEPTANCE ALPHA")
    svg += f"""
  <text x="30" y="34" class="t-title">PILLAR A — REFERENCE-CONTINUATION ACCEPTANCE ALPHA</text>
  <text x="30" y="50" class="t-sub">Macro mean acceptance length &#945; &#183; tokens/step &#183; higher is better (vs HuggingFace Adaptive Lookup)</text>
  <g transform="translate(620, 24)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">Expanse Variable LSM</text>
    <rect x="0" y="18" width="12" height="12" rx="2" class="b-baseline"/>
    <text x="18" y="28" class="t-legend">HF Adaptive Lookup</text>
  </g>
  <line x1="30" y1="66" x2="930" y2="66" class="divider"/>
"""

    for i, (key, label, sub) in enumerate(workloads):
        if key not in data:
            continue
        y = top + i * row_h
        base_val = data[key]["hf_adaptive_lookup"]["macro_acceptance_length_alpha"]
        exp_val = data[key]["expanse_longest_suffix"]["macro_acceptance_length_alpha"]
        ceil_info = data[key].get("_speculative_ceiling", {})
        gain_pct = ceil_info.get("tok_per_sec_ceiling_gain_pct", 0.0)
        delta_a = ceil_info.get("paired_delta_alpha", exp_val - base_val)

        w_exp = max(2.0, (exp_val / max_val) * bar_max)
        w_base = max(2.0, (base_val / max_val) * bar_max)

        if abs(gain_pct) < 0.5:
            badge = f"""  <rect x="760" y="{y + 2}" width="170" height="18" rx="3" class="badge-loss"/>
  <text x="845" y="{y + 15}" class="badge-loss-text">Dead heat (&#916;&#945; = {delta_a:+.2f})</text>"""
        elif gain_pct > 0:
            badge = f"""  <rect x="760" y="{y + 2}" width="170" height="18" rx="3" class="badge-win"/>
  <text x="845" y="{y + 15}" class="badge-win-text">+{gain_pct:.1f}% tok/s (&#916;&#945; = {delta_a:+.2f})</text>"""
        else:
            badge = f"""  <rect x="760" y="{y + 2}" width="170" height="18" rx="3" class="badge-loss"/>
  <text x="845" y="{y + 15}" class="badge-loss-text">HF Adaptive {abs(gain_pct):.1f}% higher</text>"""

        svg += f"""  <text x="30" y="{y + 8}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 22}" class="t-sub">{esc(sub)}</text>
  <rect x="250" y="{y - 4}" width="{w_exp:.1f}" height="13" rx="2" class="b-expanse"/>
  <text x="{258 + w_exp:.1f}" y="{y + 6}" class="t-val-accent">&#945; = {exp_val:.3f}</text>
  <rect x="250" y="{y + 12}" width="{w_base:.1f}" height="13" rx="2" class="b-baseline"/>
  <text x="{258 + w_base:.1f}" y="{y + 22}" class="t-val-gray">&#945; = {base_val:.3f}</text>
{badge}
"""

    svg += "</svg>\n"
    save_svg(RESULTS_DIR / "bench_draft_quality_alpha.svg", svg)


def render_datastore_chart():
    json_path = RESULTS_DIR / "bench_llm_datastore.json"
    if not json_path.exists():
        return
    with open(json_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    keys = [
        ("100000", "100k Tokens", "Continuous streaming vs per-token rebuild"),
        ("500000", "500k Tokens", "Continuous streaming vs per-token rebuild"),
        ("1000000", "1M Tokens", "Continuous streaming vs per-token rebuild"),
    ]

    # The RAM trade-off quoted in the subtitle is DERIVED from the artifact
    # (`memory_overhead_vs_static_index_x` per population), never stamped: a
    # hardcoded "6.9x" shipped here previously and understated the
    # pre-registered loss by ~3x (the artifact says 21.6x-24.7x).
    overheads = [
        data[p]["expanse_strmap"]["memory_overhead_vs_static_index_x"]
        for p, _, _ in keys
        if p in data and "memory_overhead_vs_static_index_x" in data[p]["expanse_strmap"]
    ]
    if overheads:
        lo, hi = min(overheads), max(overheads)
        ram_note = (
            f"Expanse trades {lo:.1f}x RAM for O(depth) updates"
            if abs(hi - lo) < 0.05
            else f"Expanse trades {lo:.1f}x-{hi:.1f}x RAM for O(depth) updates"
        )
    else:
        ram_note = "RAM overhead vs the static index: see suite README"

    # Bar geometry: the largest measured value sets the axis, so the plot is
    # not squeezed into a fraction of its width by a fixed ceiling.
    exp_vals = [data[p]["expanse_strmap"]["streaming_insert_tps"] / 1000.0 for p, _, _ in keys if p in data]
    max_val = max(exp_vals + [1.0]) * 1.15
    bar_max = 340.0
    row_h = 56
    top = 96
    height = top + len(keys) * row_h + 24

    svg = svg_header(width=960, height=height, title="PILLAR B — DYNAMIC DATASTORE INGESTION THROUGHPUT")
    svg += f"""
  <text x="30" y="34" class="t-title">PILLAR B — DYNAMIC DATASTORE INGESTION THROUGHPUT</text>
  <text x="30" y="50" class="t-sub">Streaming insertion throughput &#183; k tok/s &#183; higher is better</text>
  <text x="30" y="64" class="t-note">{esc(ram_note)} &#183; baseline bar clamped to a 2 px floor (not to scale)</text>
  <g transform="translate(580, 20)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">ExpanseStrMap (Streaming)</text>
    <rect x="0" y="18" width="12" height="12" rx="2" class="b-baseline"/>
    <text x="18" y="28" class="t-legend">Static Window Index (Rebuild/tok)</text>
  </g>
  <line x1="30" y1="76" x2="930" y2="76" class="divider"/>
"""

    for i, (pop, label, sub) in enumerate(keys):
        if pop not in data:
            continue
        y = top + i * row_h
        exp_tps = data[pop]["expanse_strmap"]["streaming_insert_tps"] / 1000.0
        static_info = data[pop].get("sorted_window_index", data[pop].get("suffix_array", {}))
        static_rebuild_ms = static_info.get("rebuild_time_ms", 100.0)
        static_single_tok_tps = (1000.0 / max(1e-6, static_rebuild_ms)) / 1000.0
        crossover = data[pop]["expanse_strmap"]["crossover_batch_size_tokens"]
        speedup = exp_tps / max(1e-6, static_single_tok_tps)

        w_exp = max(2.0, (exp_tps / max_val) * bar_max)
        w_static = max(2.0, (static_single_tok_tps / max_val) * bar_max)

        svg += f"""  <text x="30" y="{y + 8}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 22}" class="t-sub">{esc(sub)}</text>
  <rect x="250" y="{y - 4}" width="{w_exp:.1f}" height="13" rx="2" class="b-expanse"/>
  <text x="{258 + w_exp:.1f}" y="{y + 6}" class="t-val-accent">{exp_tps:.1f}k tok/s</text>
  <rect x="250" y="{y + 12}" width="{w_static:.1f}" height="13" rx="2" class="b-baseline"/>
  <text x="{258 + w_static:.1f}" y="{y + 22}" class="t-val-gray">{static_single_tok_tps:.2f}k tok/s</text>
  <rect x="760" y="{y + 2}" width="170" height="18" rx="3" class="badge-win"/>
  <text x="845" y="{y + 15}" class="badge-win-text">Expanse {speedup:.0f}x (B &lt; {crossover:,})</text>
"""

    svg += "</svg>\n"
    save_svg(RESULTS_DIR / "bench_llm_datastore_scaling.svg", svg)


def render_grammar_masks_chart():
    json_path = RESULTS_DIR / "bench_grammar_masks.json"
    if not json_path.exists():
        return
    with open(json_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    mem = data.get("memory_summary", {})
    dense_mb = mem.get("dense_bitmask_mb", 30.52)
    roaring_mb = mem.get("roaring_bitmap_mb", 10.18)
    expanse_mb = mem.get("expanse_set_mb", 21.23)

    items = [
        ("Dense Bitmask", "[u64] Array (Full vocab)", dense_mb, "baseline", 1.0),
        ("ExpanseSet", "Judy digital trie (Compressed)", expanse_mb, "expanse", dense_mb / max(0.01, expanse_mb)),
        ("Roaring Bitmap", "Container array (Compressed)", roaring_mb, "roaring", dense_mb / max(0.01, roaring_mb)),
    ]

    max_val = max(dense_mb, expanse_mb, roaring_mb, 1.0) * 1.15
    bar_max = 380.0
    row_h = 56
    top = 96
    height = top + len(items) * row_h + 24

    svg = svg_header(width=960, height=height, title="PILLAR D — GRAMMAR-CONSTRAINED DECODING MASKS")
    svg += f"""
  <text x="30" y="34" class="t-title">PILLAR D — GRAMMAR-CONSTRAINED DECODING MASKS</text>
  <text x="30" y="50" class="t-sub">Live resident heap across 2,000 DFA states (128k vocab) &#183; MB &#183; lower is better</text>
  <line x1="30" y1="66" x2="930" y2="66" class="divider"/>
"""

    for i, (label, sub, mb, kind, ratio) in enumerate(items):
        y = top + i * row_h
        w = max(2.0, (mb / max_val) * bar_max)
        if kind == "baseline":
            bar_cls = "b-baseline"
            val_cls = "t-val-gray"
            badge = f"""  <rect x="790" y="{y + 2}" width="140" height="18" rx="3" class="badge-loss"/>
  <text x="860" y="{y + 15}" class="badge-loss-text">Dense Baseline</text>"""
        elif kind == "expanse":
            bar_cls = "b-expanse"
            val_cls = "t-val-accent"
            if ratio >= 1.0:
                badge = f"""  <rect x="790" y="{y + 2}" width="140" height="18" rx="3" class="badge-win"/>
  <text x="860" y="{y + 15}" class="badge-win-text">Expanse {ratio:.1f}x lower</text>"""
            else:
                badge = f"""  <rect x="790" y="{y + 2}" width="140" height="18" rx="3" class="badge-loss"/>
  <text x="860" y="{y + 15}" class="badge-loss-text">Expanse {1.0 / max(0.01, ratio):.1f}x higher</text>"""
        else:
            bar_cls = "b-highlight"
            # Roaring: the competitor colour. This block and the Expanse
            # block above had each other's value colour, matching the bar
            # palette inversion corrected in theme.py.
            val_cls = "t-val-blue"
            badge = f"""  <rect x="790" y="{y + 2}" width="140" height="18" rx="3" class="badge-win"/>
  <text x="860" y="{y + 15}" class="badge-win-text">Roaring {ratio:.1f}x lower</text>"""

        svg += f"""  <text x="30" y="{y + 8}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 22}" class="t-sub">{esc(sub)}</text>
  <rect x="250" y="{y - 4}" width="{w:.1f}" height="20" rx="3" class="{bar_cls}"/>
  <text x="{258 + w:.1f}" y="{y + 11}" class="{val_cls}">{mb:.2f} MB</text>
{badge}
"""

    svg += "</svg>\n"
    save_svg(RESULTS_DIR / "bench_grammar_masks_memory.svg", svg)


def render_prefix_lru_chart():
    json_path = RESULTS_DIR / "bench_prefix_lru.json"
    if not json_path.exists():
        return
    with open(json_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    blocks = [
        ("100000", "100k Blocks", "Physical KV block table RAM"),
        ("500000", "500k Blocks", "Physical KV block table RAM"),
        ("1000000", "1M Blocks", "Physical KV block table RAM"),
    ]

    max_val = 260.0
    bar_max = 340.0
    row_h = 56
    top = 96
    height = top + len(blocks) * row_h + 24

    svg = svg_header(width=960, height=height, title="PILLAR E (APPENDIX) — PREFIX-CACHE KV-BLOCK TABLE")
    svg += f"""
  <text x="30" y="34" class="t-title">PILLAR E (APPENDIX) — PREFIX-CACHE KV-BLOCK TABLE</text>
  <text x="30" y="50" class="t-sub">Total index memory footprint &#183; MB &#183; lower is better</text>
  <g transform="translate(620, 24)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">ExpanseMap Table</text>
    <rect x="0" y="18" width="12" height="12" rx="2" class="b-baseline"/>
    <text x="18" y="28" class="t-legend">OrderedDict (vLLM)</text>
  </g>
  <line x1="30" y1="66" x2="930" y2="66" class="divider"/>
"""

    for i, (b_str, label, sub) in enumerate(blocks):
        if b_str not in data:
            continue
        y = top + i * row_h
        od_mem = data[b_str]["ordered_dict_lru"]["memory_mb"]
        exp_mem = data[b_str]["expanse_ordered_table"]["memory_mb"]
        reduction = od_mem / max(0.01, exp_mem)

        w_exp = max(2.0, (exp_mem / max_val) * bar_max)
        w_od = max(2.0, (od_mem / max_val) * bar_max)

        svg += f"""  <text x="30" y="{y + 8}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 22}" class="t-sub">{esc(sub)}</text>
  <rect x="250" y="{y - 4}" width="{w_exp:.1f}" height="13" rx="2" class="b-expanse"/>
  <text x="{258 + w_exp:.1f}" y="{y + 6}" class="t-val-accent">{exp_mem:.2f} MB</text>
  <rect x="250" y="{y + 12}" width="{w_od:.1f}" height="13" rx="2" class="b-baseline"/>
  <text x="{258 + w_od:.1f}" y="{y + 22}" class="t-val-gray">{od_mem:.2f} MB</text>
  <rect x="790" y="{y + 2}" width="140" height="18" rx="3" class="badge-win"/>
  <text x="860" y="{y + 15}" class="badge-win-text">Expanse {reduction:.1f}x lower</text>
"""

    svg += "</svg>\n"
    save_svg(RESULTS_DIR / "bench_prefix_lru_throughput.svg", svg)


def main():
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    render_draft_quality_chart()
    render_datastore_chart()
    render_grammar_masks_chart()
    render_prefix_lru_chart()


if __name__ == "__main__":
    main()
