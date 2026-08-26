#!/usr/bin/env python3
"""
Dual-theme SVG chart generator for the LLM inference benchmark suite (#342).

Produces four charts:
  1. bench_draft_quality_alpha.svg   (Pillar A, mean acceptance length alpha)
  2. bench_llm_datastore_scaling.svg (Pillar B, memory & crossover batch size)
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
        ("humaneval_code", "HumanEval Code", "Open coding model (greedy)"),
        ("summarization", "Document Summarization", "Open summarization task"),
        ("json_schemas", "JSON Schemas", "Structured extraction"),
    ]

    max_val = 5.2
    bar_max = 340.0
    row_h = 56
    top = 96
    height = top + len(workloads) * row_h + 24

    svg = svg_header(width=960, height=height, title="PILLAR A — SPECULATIVE DRAFT QUALITY (ALPHA)")
    svg += f"""
  <text x="30" y="34" class="t-title">PILLAR A — SPECULATIVE DRAFT QUALITY</text>
  <text x="30" y="50" class="t-sub">Mean acceptance length &#945; &#183; tokens/step &#183; higher is better</text>
  <g transform="translate(620, 24)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">Expanse Variable LSM</text>
    <rect x="0" y="18" width="12" height="12" rx="2" class="b-baseline"/>
    <text x="18" y="28" class="t-legend">HF Fixed 3-gram</text>
  </g>
  <line x1="30" y1="66" x2="930" y2="66" class="divider"/>
"""

    for i, (key, label, sub) in enumerate(workloads):
        if key not in data:
            continue
        y = top + i * row_h
        hf_val = data[key]["hf_fixed_3gram"]["mean_acceptance_length_alpha"]
        exp_val = data[key]["expanse_longest_suffix"]["mean_acceptance_length_alpha"]

        w_exp = max(2.0, (exp_val / max_val) * bar_max)
        w_hf = max(2.0, (hf_val / max_val) * bar_max)
        diff_pct = ((exp_val - hf_val) / hf_val) * 100.0

        svg += f"""  <text x="30" y="{y + 8}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 22}" class="t-sub">{esc(sub)}</text>
  <rect x="250" y="{y - 4}" width="{w_exp:.1f}" height="13" rx="2" class="b-expanse"/>
  <text x="{258 + w_exp:.1f}" y="{y + 6}" class="t-val-blue">&#945; = {exp_val:.3f}</text>
  <rect x="250" y="{y + 12}" width="{w_hf:.1f}" height="13" rx="2" class="b-baseline"/>
  <text x="{258 + w_hf:.1f}" y="{y + 22}" class="t-val-gray">&#945; = {hf_val:.3f}</text>
  <rect x="790" y="{y + 2}" width="140" height="18" rx="3" class="badge-win"/>
  <text x="860" y="{y + 15}" class="badge-win-text">Expanse +{diff_pct:.1f}% &#945;</text>
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
        ("100000", "100k Tokens", "Incremental update vs periodic rebuild"),
        ("500000", "500k Tokens", "Incremental update vs periodic rebuild"),
        ("1000000", "1M Tokens", "Incremental update vs periodic rebuild"),
    ]

    max_val = 110.0
    bar_max = 340.0
    row_h = 56
    top = 96
    height = top + len(keys) * row_h + 24

    svg = svg_header(width=960, height=height, title="PILLAR B — DYNAMIC DATASTORE VS SUFFIX ARRAY")
    svg += f"""
  <text x="30" y="34" class="t-title">PILLAR B — DYNAMIC DATASTORE VS SUFFIX ARRAY</text>
  <text x="30" y="50" class="t-sub">Memory footprint &#183; Bytes/token &#183; lower is better (Expanse wins dynamic updates)</text>
  <g transform="translate(600, 24)">
    <rect x="0" y="0" width="12" height="12" rx="2" class="b-expanse"/>
    <text x="18" y="10" class="t-legend">ExpanseStrMap (Dynamic)</text>
    <rect x="0" y="18" width="12" height="12" rx="2" class="b-baseline"/>
    <text x="18" y="28" class="t-legend">Suffix Array (Static Native)</text>
  </g>
  <line x1="30" y1="66" x2="930" y2="66" class="divider"/>
"""

    for i, (pop, label, sub) in enumerate(keys):
        if pop not in data:
            continue
        y = top + i * row_h
        sa_b = data[pop]["suffix_array"]["bytes_per_token"]
        exp_b = data[pop]["expanse_strmap"]["bytes_per_token"]
        crossover = data[pop]["expanse_strmap"]["crossover_batch_size_tokens"]

        w_exp = max(2.0, (exp_b / max_val) * bar_max)
        w_sa = max(2.0, (sa_b / max_val) * bar_max)

        svg += f"""  <text x="30" y="{y + 8}" class="t-bar-label">{esc(label)}</text>
  <text x="30" y="{y + 22}" class="t-sub">{esc(sub)}</text>
  <rect x="250" y="{y - 4}" width="{w_exp:.1f}" height="13" rx="2" class="b-expanse"/>
  <text x="{258 + w_exp:.1f}" y="{y + 6}" class="t-val-blue">{exp_b:.1f} B/tok</text>
  <rect x="250" y="{y + 12}" width="{w_sa:.1f}" height="13" rx="2" class="b-baseline"/>
  <text x="{258 + w_sa:.1f}" y="{y + 22}" class="t-val-gray">{sa_b:.1f} B/tok</text>
  <rect x="760" y="{y + 2}" width="170" height="18" rx="3" class="badge-win"/>
  <text x="845" y="{y + 15}" class="badge-win-text">Expanse Win: B &lt; {crossover:,}</text>
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

    max_val = max(dense_mb, 1.0) * 1.25
    bar_max = 380.0
    row_h = 56
    top = 96
    height = top + len(items) * row_h + 24

    svg = svg_header(width=960, height=height, title="PILLAR D — GRAMMAR-CONSTRAINED DECODING MASKS")
    svg += f"""
  <text x="30" y="34" class="t-title">PILLAR D — GRAMMAR-CONSTRAINED DECODING MASKS</text>
  <text x="30" y="50" class="t-sub">Total RAM across 2,000 DFA states (128k vocab) &#183; MB &#183; lower is better</text>
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
            val_cls = "t-val-blue"
            badge = f"""  <rect x="790" y="{y + 2}" width="140" height="18" rx="3" class="badge-win"/>
  <text x="860" y="{y + 15}" class="badge-win-text">Expanse {ratio:.1f}x lower</text>"""
        else:
            bar_cls = "b-highlight"
            val_cls = "t-val-accent"
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
  <text x="{258 + w_exp:.1f}" y="{y + 6}" class="t-val-blue">{exp_mem:.2f} MB</text>
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
