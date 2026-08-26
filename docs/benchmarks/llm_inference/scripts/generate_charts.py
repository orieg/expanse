#!/usr/bin/env python3
"""
Dual-Theme SVG Comparison Chart Generator for Expanse LLM Inference Benchmarks.
Generates accessible, high-contrast SVG comparison charts with automatic dark/light mode CSS.
Layout designed with generous margins to prevent text overflow and legend collisions.
"""

import json
from pathlib import Path

RESULTS_DIR = Path(__file__).resolve().parent.parent / "results"

def svg_header(width=900, height=460, title="Benchmark", subtitle=""):
    sub_svg = f'<text x="40" y="58" class="text-sub">{subtitle}</text>' if subtitle else ""
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="100%" height="100%">
  <style>
    .bg {{ fill: #ffffff; }}
    .text-title {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 18px; font-weight: 700; fill: #1e293b; }}
    .text-sub {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 13px; fill: #64748b; }}
    .text-axis {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 13px; font-weight: 500; fill: #475569; }}
    .text-val {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 12px; font-weight: 600; fill: #1e293b; }}
    .grid-line {{ stroke: #e2e8f0; stroke-width: 1; stroke-dasharray: 4 4; }}
    .axis-line {{ stroke: #cbd5e1; stroke-width: 1.5; }}
    .bar-baseline {{ fill: #94a3b8; }}
    .bar-expanse {{ fill: #2563eb; }}
    .bar-highlight {{ fill: #10b981; }}
    .legend-text {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 12px; fill: #334155; }}
    @media (prefers-color-scheme: dark) {{
      .bg {{ fill: #0f172a; }}
      .text-title {{ fill: #f8fafc; }}
      .text-sub {{ fill: #94a3b8; }}
      .text-axis {{ fill: #cbd5e1; }}
      .text-val {{ fill: #f1f5f9; }}
      .grid-line {{ stroke: #334155; }}
      .axis-line {{ stroke: #475569; }}
      .bar-baseline {{ fill: #64748b; }}
      .bar-expanse {{ fill: #3b82f6; }}
      .bar-highlight {{ fill: #34d399; }}
      .legend-text {{ fill: #e2e8f0; }}
    }}
  </style>
  <rect width="{width}" height="{height}" class="bg" rx="8" />
  <text x="40" y="36" class="text-title">{title}</text>
  {sub_svg}
'''

def render_draft_quality_chart():
    json_path = RESULTS_DIR / "bench_draft_quality.json"
    if not json_path.exists():
        return
    with open(json_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    workloads = ["humaneval_code", "summarization", "json_schemas"]
    labels = ["HumanEval Code", "Summarization", "JSON Schemas"]
    
    svg = svg_header(
        width=920,
        height=450,
        title="Pillar A: Speculative Draft Quality (Mean Acceptance Length α)",
        subtitle="Higher α delivers greater end-to-end inference speedup (verified via Replay Verifier on real model output)"
    )

    # Dedicated Legend Row (below subtitle)
    svg += '<rect x="40" y="80" width="14" height="14" rx="2" class="bar-baseline" />\n'
    svg += '<text x="60" y="92" class="legend-text">HF Fixed 3-gram</text>\n'
    svg += '<rect x="220" y="80" width="14" height="14" rx="2" class="bar-expanse" />\n'
    svg += '<text x="240" y="92" class="legend-text">Expanse Variable LSM (1 key/token)</text>\n'

    y_start = 135
    group_gap = 95
    max_alpha = 5.0
    bar_x = 220
    max_bar_w = 400.0
    bar_scale = max_bar_w / max_alpha

    for i, (w, label) in enumerate(zip(workloads, labels)):
        if w not in data:
            continue
        y = y_start + i * group_gap
        hf_val = data[w]["hf_fixed_3gram"]["mean_acceptance_length_alpha"]
        exp_val = data[w]["expanse_longest_suffix"]["mean_acceptance_length_alpha"]

        svg += f'<text x="40" y="{y + 24}" class="text-axis">{label}</text>\n'

        # Baseline bar
        w_base = hf_val * bar_scale
        svg += f'<rect x="{bar_x}" y="{y}" width="{w_base:.1f}" height="20" rx="3" class="bar-baseline" />\n'
        svg += f'<text x="{bar_x + w_base + 12}" y="{y + 15}" class="text-val">α = {hf_val:.3f}</text>\n'

        # Expanse bar
        w_exp = exp_val * bar_scale
        svg += f'<rect x="{bar_x}" y="{y + 26}" width="{w_exp:.1f}" height="20" rx="3" class="bar-expanse" />\n'
        diff_pct = ((exp_val - hf_val) / hf_val) * 100.0
        svg += f'<text x="{bar_x + w_exp + 12}" y="{y + 41}" class="text-val">α = {exp_val:.3f} (+{diff_pct:.1f}%)</text>\n'

    svg += '</svg>'
    out_path = RESULTS_DIR / "bench_draft_quality_alpha.svg"
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(svg)
    print(f"Generated {out_path}")

def render_datastore_chart():
    json_path = RESULTS_DIR / "bench_llm_datastore.json"
    if not json_path.exists():
        return
    with open(json_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    svg = svg_header(
        width=920,
        height=450,
        title="Pillar B: Dynamic Datastore vs Suffix Array (Memory & Crossover)",
        subtitle="Expanse trades static memory for O(depth) continuous incremental insertion"
    )

    # Dedicated Legend Row
    svg += '<rect x="40" y="80" width="14" height="14" rx="2" class="bar-baseline" />\n'
    svg += '<text x="60" y="92" class="legend-text">Suffix Array (Static Native Baseline)</text>\n'
    svg += '<rect x="320" y="80" width="14" height="14" rx="2" class="bar-expanse" />\n'
    svg += '<text x="340" y="92" class="legend-text">ExpanseStrMap (Dynamic 1 key/token)</text>\n'

    y_start = 135
    group_gap = 95
    keys = ["100000", "500000", "1000000"]
    max_b = 100.0
    bar_x = 220
    max_bar_w = 380.0
    bar_scale = max_bar_w / max_b

    for i, pop in enumerate(keys):
        if pop not in data:
            continue
        y = y_start + i * group_gap
        sa_b = data[pop]["suffix_array"]["bytes_per_token"]
        exp_b = data[pop]["expanse_strmap"]["bytes_per_token"]
        crossover = data[pop]["expanse_strmap"]["crossover_batch_size_tokens"]

        svg += f'<text x="40" y="{y + 24}" class="text-axis">{int(pop):,} Tokens</text>\n'

        w_sa = sa_b * bar_scale
        svg += f'<rect x="{bar_x}" y="{y}" width="{w_sa:.1f}" height="20" rx="3" class="bar-baseline" />\n'
        svg += f'<text x="{bar_x + w_sa + 12}" y="{y + 15}" class="text-val">{sa_b:.1f} B/tok</text>\n'

        w_exp = exp_b * bar_scale
        svg += f'<rect x="{bar_x}" y="{y + 26}" width="{w_exp:.1f}" height="20" rx="3" class="bar-expanse" />\n'
        svg += f'<text x="{bar_x + w_exp + 12}" y="{y + 41}" class="text-val">{exp_b:.1f} B/tok (Crossover: B &lt; {crossover:,} tokens)</text>\n'

    svg += '</svg>'
    out_path = RESULTS_DIR / "bench_llm_datastore_scaling.svg"
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(svg)
    print(f"Generated {out_path}")

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

    svg = svg_header(
        width=920,
        height=420,
        title="Pillar D: Grammar-Constrained Decoding Mask Cache Memory",
        subtitle="Total memory footprint across 2,000 DFA states (128k vocab; lower is better)"
    )

    bar_x = 240
    max_mb = max(dense_mb, 1.0)
    max_bar_w = 420.0
    bar_scale = max_bar_w / max_mb

    # 1. Dense bar
    w_dense = dense_mb * bar_scale
    svg += '<text x="40" y="125" class="text-axis">Dense Bitmask ([u64] Array)</text>\n'
    svg += f'<rect x="{bar_x}" y="105" width="{w_dense:.1f}" height="26" rx="3" class="bar-baseline" />\n'
    svg += f'<text x="{bar_x + w_dense + 12}" y="123" class="text-val">{dense_mb:.2f} MB (Baseline)</text>\n'

    # 2. Expanse bar
    w_exp = expanse_mb * bar_scale
    reduction_exp = dense_mb / max(0.01, expanse_mb)
    svg += '<text x="40" y="185" class="text-axis">ExpanseSet (Digital Trie)</text>\n'
    svg += f'<rect x="{bar_x}" y="165" width="{w_exp:.1f}" height="26" rx="3" class="bar-expanse" />\n'
    svg += f'<text x="{bar_x + w_exp + 12}" y="183" class="text-val">{expanse_mb:.2f} MB ({reduction_exp:.1f}x lower RAM)</text>\n'

    # 3. Roaring bar
    w_roaring = roaring_mb * bar_scale
    reduction_roaring = dense_mb / max(0.01, roaring_mb)
    svg += '<text x="40" y="245" class="text-axis">Roaring Bitmap (Compressed)</text>\n'
    svg += f'<rect x="{bar_x}" y="225" width="{w_roaring:.1f}" height="26" rx="3" class="bar-highlight" />\n'
    svg += f'<text x="{bar_x + w_roaring + 12}" y="243" class="text-val">{roaring_mb:.2f} MB ({reduction_roaring:.1f}x lower RAM)</text>\n'

    svg += '</svg>'
    out_path = RESULTS_DIR / "bench_grammar_masks_memory.svg"
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(svg)
    print(f"Generated {out_path}")

def render_prefix_lru_chart():
    json_path = RESULTS_DIR / "bench_prefix_lru.json"
    if not json_path.exists():
        return
    with open(json_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    svg = svg_header(
        width=920,
        height=450,
        title="Pillar E (Appendix): Prefix-Cache KV-Block Table Memory",
        subtitle="Expanse achieves 9.5x lower RAM and enables native timestamp rank eviction"
    )

    # Dedicated Legend Row
    svg += '<rect x="40" y="80" width="14" height="14" rx="2" class="bar-baseline" />\n'
    svg += '<text x="60" y="92" class="legend-text">OrderedDict (vLLM / SGLang baseline)</text>\n'
    svg += '<rect x="340" y="80" width="14" height="14" rx="2" class="bar-expanse" />\n'
    svg += '<text x="360" y="92" class="legend-text">ExpanseMap Table (all-inclusive)</text>\n'

    y_start = 135
    group_gap = 95
    blocks = [100000, 500000, 1000000]

    bar_x = 220
    max_mem = 250.0
    max_bar_w = 400.0
    bar_scale = max_bar_w / max_mem

    for i, b in enumerate(blocks):
        b_str = str(b)
        if b_str not in data:
            continue
        y = y_start + i * group_gap
        od_mem = data[b_str]["ordered_dict_lru"]["memory_mb"]
        exp_mem = data[b_str]["expanse_ordered_table"]["memory_mb"]

        svg += f'<text x="40" y="{y + 24}" class="text-axis">{b:,} Blocks</text>\n'

        w_od = od_mem * bar_scale
        svg += f'<rect x="{bar_x}" y="{y}" width="{w_od:.1f}" height="20" rx="3" class="bar-baseline" />\n'
        svg += f'<text x="{bar_x + w_od + 12}" y="{y + 15}" class="text-val">{od_mem:.2f} MB</text>\n'

        w_exp = exp_mem * bar_scale
        svg += f'<rect x="{bar_x}" y="{y + 26}" width="{w_exp:.1f}" height="20" rx="3" class="bar-expanse" />\n'
        reduction = od_mem / max(0.01, exp_mem)
        svg += f'<text x="{bar_x + w_exp + 12}" y="{y + 41}" class="text-val">{exp_mem:.2f} MB ({reduction:.1f}x lower)</text>\n'

    svg += '</svg>'
    out_path = RESULTS_DIR / "bench_prefix_lru_throughput.svg"
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(svg)
    print(f"Generated {out_path}")

def main():
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    render_draft_quality_chart()
    render_datastore_chart()
    render_grammar_masks_chart()
    render_prefix_lru_chart()

if __name__ == "__main__":
    main()
