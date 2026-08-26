#!/usr/bin/env python3
"""
Generates dual-theme SVG comparison charts for the LLM Inference benchmark suite.

Reads:
- results/bench_draft_quality.json
- results/bench_datastore_scale.json
- results/bench_llama_lookup.json
- results/bench_prefix_lru.json

Emits:
- results/bench_draft_quality_alpha.svg
- results/bench_datastore_scale_memory.svg
- results/bench_llama_lookup_latency.svg
- results/bench_prefix_lru_throughput.svg
"""

import os
import json
from pathlib import Path
from theme import svg_header

RESULTS_DIR = Path(__file__).resolve().parent.parent / "results"

def generate_draft_quality_chart(data: dict, out_path: Path):
    """Generates Pillar 1 Mean Acceptance Length (alpha) grouped bar chart."""
    svg = [svg_header(width=960, height=320, title="Speculative Draft Quality: Mean Acceptance Length (alpha)")]
    
    svg.append('  <text x="24" y="28" class="t-title">PILLAR 1: SPECULATIVE DRAFT QUALITY (MEAN ACCEPTANCE LENGTH &alpha;)</text>')
    svg.append('  <text x="24" y="44" class="t-sub">Replay Verifier evaluation on deterministic token stream patterns (Higher &alpha; = fewer forward passes per generated token)</text>')
    
    # Legend
    svg.append('  <rect x="520" y="20" width="12" height="12" rx="2" class="b-hf"/>')
    svg.append('  <text x="536" y="30" class="t-legend">HF Fixed 3-Gram</text>')
    svg.append('  <rect x="660" y="20" width="12" height="12" rx="2" class="b-baseline"/>')
    svg.append('  <text x="676" y="30" class="t-legend">Dict Multimap Tree</text>')
    svg.append('  <rect x="800" y="20" width="12" height="12" rx="2" class="b-expanse"/>')
    svg.append('  <text x="816" y="30" class="t-legend">Expanse Longest-Suffix (LSM)</text>')
    
    workloads = [
        ("code_patterns", "Code Patterns"),
        ("summary_patterns", "Summary Patterns"),
        ("json_schemas", "JSON Schemas"),
    ]
    
    x_start = 60
    group_width = 280
    max_alpha = 5.5
    chart_bottom = 260
    chart_height = 180
    
    for a_val in [1.0, 2.0, 3.0, 4.0, 5.0]:
        y = chart_bottom - int((a_val / max_alpha) * chart_height)
        svg.append(f'  <line x1="50" y1="{y}" x2="920" y2="{y}" class="grid"/>')
        svg.append(f'  <text x="42" y="{y + 3}" class="t-axis-label" text-anchor="end">{a_val:.1f}&times;</text>')
        
    svg.append(f'  <line x1="50" y1="{chart_bottom}" x2="920" y2="{chart_bottom}" class="axis"/>')
    
    for gi, (w_key, w_label) in enumerate(workloads):
        gx = x_start + gi * group_width
        w_data = data.get(w_key, {})
        
        hf_a = w_data.get("hf_fixed_3gram", {}).get("mean_acceptance_length_alpha", 1.0)
        dict_a = w_data.get("dict_multimap_tree", {}).get("mean_acceptance_length_alpha", 1.0)
        exp_a = w_data.get("expanse_longest_suffix", {}).get("mean_acceptance_length_alpha", 1.0)
        
        bar_w = 40
        b1_x = gx + 20
        b2_x = b1_x + bar_w + 10
        b3_x = b2_x + bar_w + 10
        
        h1 = int((hf_a / max_alpha) * chart_height)
        h2 = int((dict_a / max_alpha) * chart_height)
        h3 = int((exp_a / max_alpha) * chart_height)
        
        y1 = chart_bottom - h1
        y2 = chart_bottom - h2
        y3 = chart_bottom - h3
        
        svg.append(f'  <rect x="{b1_x}" y="{y1}" width="{bar_w}" height="{h1}" rx="3" class="b-hf"/>')
        svg.append(f'  <text x="{b1_x + bar_w/2}" y="{y1 - 6}" class="t-val-blue" text-anchor="middle">{hf_a:.2f}&times;</text>')
        
        svg.append(f'  <rect x="{b2_x}" y="{y2}" width="{bar_w}" height="{h2}" rx="3" class="b-baseline"/>')
        svg.append(f'  <text x="{b2_x + bar_w/2}" y="{y2 - 6}" class="t-val-muted" text-anchor="middle">{dict_a:.2f}&times;</text>')
        
        svg.append(f'  <rect x="{b3_x}" y="{y3}" width="{bar_w}" height="{h3}" rx="3" class="b-expanse"/>')
        svg.append(f'  <text x="{b3_x + bar_w/2}" y="{y3 - 6}" class="t-val-accent" text-anchor="middle">{exp_a:.2f}&times;</text>')
        
        if exp_a > hf_a:
            win_pct = round(((exp_a - hf_a) / hf_a) * 100, 1)
            badge_x = b3_x + bar_w/2 - 28
            svg.append(f'  <rect x="{badge_x}" y="{y3 - 24}" width="56" height="15" class="badge-win"/>')
            svg.append(f'  <text x="{badge_x + 28}" y="{y3 - 13}" class="badge-win-text">+{win_pct}% &alpha;</text>')
            
        svg.append(f'  <text x="{gx + 90}" y="{chart_bottom + 20}" class="t-bar-label" text-anchor="middle">{w_label}</text>')
        
    svg.append('</svg>\n')
    with open(out_path, "w", encoding="utf-8") as f:
        f.write("".join(svg))
    print(f"Generated {out_path}")

def generate_datastore_memory_chart(data: dict, out_path: Path):
    """Generates Pillar 2 Memory Footprint (MB) comparison chart."""
    svg = [svg_header(width=960, height=320, title="Datastore Scale: Memory Footprint (MB)")]
    
    svg.append('  <text x="24" y="28" class="t-title">PILLAR 2: DATASTORE SCALE &mdash; LIVE MEMORY FOOTPRINT (MB)</text>')
    svg.append('  <text x="24" y="44" class="t-sub">Index memory across token populations (Lower = More tokens fit in RAM; Expanse vs CPython dict vs Sorted NumPy)</text>')
    
    svg.append('  <rect x="520" y="20" width="12" height="12" rx="2" class="b-hf"/>')
    svg.append('  <text x="536" y="30" class="t-legend">CPython dict (Heap)</text>')
    svg.append('  <rect x="680" y="20" width="12" height="12" rx="2" class="b-baseline"/>')
    svg.append('  <text x="696" y="30" class="t-legend">Sorted NumPy (Static)</text>')
    svg.append('  <rect x="830" y="20" width="12" height="12" rx="2" class="b-expanse"/>')
    svg.append('  <text x="846" y="30" class="t-legend">ExpanseMap</text>')
    
    pops = sorted(data.keys(), key=lambda x: int(x))
    x_start = 60
    group_width = int(840 / max(1, len(pops)))
    max_mb = max([data[k]["cpython_dict"]["memory_mb"] for k in pops] + [1.0]) * 1.15
    chart_bottom = 260
    chart_height = 180
    
    for step in range(5):
        mb_val = (max_mb / 4) * step
        y = chart_bottom - int((mb_val / max_mb) * chart_height)
        svg.append(f'  <line x1="50" y1="{y}" x2="920" y2="{y}" class="grid"/>')
        svg.append(f'  <text x="42" y="{y + 3}" class="t-axis-label" text-anchor="end">{mb_val:.1f} MB</text>')
        
    svg.append(f'  <line x1="50" y1="{chart_bottom}" x2="920" y2="{chart_bottom}" class="axis"/>')
    
    for gi, pop_k in enumerate(pops):
        gx = x_start + gi * group_width
        pop_d = data[pop_k]
        
        dict_mb = pop_d["cpython_dict"]["memory_mb"]
        np_mb = pop_d["sorted_numpy"]["memory_mb"]
        exp_mb = pop_d["expanse_map"]["memory_mb"]
        
        bar_w = min(45, int(group_width / 4))
        b1_x = gx + 15
        b2_x = b1_x + bar_w + 6
        b3_x = b2_x + bar_w + 6
        
        h1 = int((dict_mb / max_mb) * chart_height)
        h2 = int((np_mb / max_mb) * chart_height)
        h3 = int((exp_mb / max_mb) * chart_height)
        
        y1 = chart_bottom - h1
        y2 = chart_bottom - h2
        y3 = chart_bottom - h3
        
        svg.append(f'  <rect x="{b1_x}" y="{y1}" width="{bar_w}" height="{h1}" rx="3" class="b-hf"/>')
        svg.append(f'  <text x="{b1_x + bar_w/2}" y="{y1 - 5}" class="t-val-blue" text-anchor="middle">{dict_mb:.1f}M</text>')
        
        svg.append(f'  <rect x="{b2_x}" y="{y2}" width="{bar_w}" height="{h2}" rx="3" class="b-baseline"/>')
        svg.append(f'  <text x="{b2_x + bar_w/2}" y="{y2 - 5}" class="t-val-muted" text-anchor="middle">{np_mb:.1f}M</text>')
        
        svg.append(f'  <rect x="{b3_x}" y="{y3}" width="{bar_w}" height="{h3}" rx="3" class="b-expanse"/>')
        svg.append(f'  <text x="{b3_x + bar_w/2}" y="{y3 - 5}" class="t-val-accent" text-anchor="middle">{exp_mb:.1f}M</text>')
        
        red_x = pop_d["expanse_map"].get("memory_reduction_vs_dict_x", 1.0)
        badge_x = b3_x + bar_w/2 - 24
        svg.append(f'  <rect x="{badge_x}" y="{y3 - 22}" width="48" height="14" class="badge-win"/>')
        svg.append(f'  <text x="{badge_x + 24}" y="{y3 - 12}" class="badge-win-text">{red_x:.1f}&times; less</text>')
        
        pop_num = int(pop_k)
        pop_label = f"{pop_num//1000}k tokens" if pop_num < 1000000 else f"{pop_num//1000000}M tokens"
        svg.append(f'  <text x="{gx + group_width//2}" y="{chart_bottom + 20}" class="t-bar-label" text-anchor="middle">{pop_label}</text>')
        
    svg.append('</svg>\n')
    with open(out_path, "w", encoding="utf-8") as f:
        f.write("".join(svg))
    print(f"Generated {out_path}")

def generate_prefix_lru_chart(data: dict, out_path: Path):
    """Generates Pillar 4 Prefix LRU Memory Reduction chart."""
    svg = [svg_header(width=960, height=320, title="Prefix-Cache Indexing & LRU Memory")]
    
    svg.append('  <text x="24" y="28" class="t-title">PILLAR 4: PREFIX-CACHE KV BLOCK MEMORY FOOTPRINT (MB)</text>')
    svg.append('  <text x="24" y="44" class="t-sub">KV-cache block allocation memory overhead across active block populations (Expanse vs collections.OrderedDict)</text>')
    
    svg.append('  <rect x="620" y="20" width="12" height="12" rx="2" class="b-hf"/>')
    svg.append('  <text x="636" y="30" class="t-legend">OrderedDict (Heap Table)</text>')
    svg.append('  <rect x="800" y="20" width="12" height="12" rx="2" class="b-expanse"/>')
    svg.append('  <text x="816" y="30" class="t-legend">ExpanseMap Ordered Table</text>')
    
    pops = sorted(data.keys(), key=lambda x: int(x))
    x_start = 80
    group_width = int(800 / max(1, len(pops)))
    max_mb = max([data[k]["ordered_dict_lru"]["memory_mb"] for k in pops] + [1.0]) * 1.15
    chart_bottom = 260
    chart_height = 180
    
    for step in range(5):
        mb_val = (max_mb / 4) * step
        y = chart_bottom - int((mb_val / max_mb) * chart_height)
        svg.append(f'  <line x1="50" y1="{y}" x2="920" y2="{y}" class="grid"/>')
        svg.append(f'  <text x="42" y="{y + 3}" class="t-axis-label" text-anchor="end">{mb_val:.1f} MB</text>')
        
    svg.append(f'  <line x1="50" y1="{chart_bottom}" x2="920" y2="{chart_bottom}" class="axis"/>')
    
    for gi, pop_k in enumerate(pops):
        gx = x_start + gi * group_width
        pop_d = data[pop_k]
        
        od_mb = pop_d["ordered_dict_lru"]["memory_mb"]
        exp_mb = pop_d["expanse_ordered_table"]["memory_mb"]
        
        bar_w = 60
        b1_x = gx + 30
        b2_x = b1_x + bar_w + 20
        
        h1 = int((od_mb / max_mb) * chart_height)
        h2 = int((exp_mb / max_mb) * chart_height)
        
        y1 = chart_bottom - h1
        y2 = chart_bottom - h2
        
        svg.append(f'  <rect x="{b1_x}" y="{y1}" width="{bar_w}" height="{h1}" rx="3" class="b-hf"/>')
        svg.append(f'  <text x="{b1_x + bar_w/2}" y="{y1 - 5}" class="t-val-blue" text-anchor="middle">{od_mb:.1f} MB</text>')
        
        svg.append(f'  <rect x="{b2_x}" y="{y2}" width="{bar_w}" height="{h2}" rx="3" class="b-expanse"/>')
        svg.append(f'  <text x="{b2_x + bar_w/2}" y="{y2 - 5}" class="t-val-accent" text-anchor="middle">{exp_mb:.1f} MB</text>')
        
        red_x = pop_d["expanse_ordered_table"].get("memory_reduction_vs_od_x", 1.0)
        badge_x = b2_x + bar_w/2 - 28
        svg.append(f'  <rect x="{badge_x}" y="{y2 - 22}" width="56" height="14" class="badge-win"/>')
        svg.append(f'  <text x="{badge_x + 28}" y="{y2 - 12}" class="badge-win-text">{red_x:.1f}&times; less</text>')
        
        pop_num = int(pop_k)
        pop_label = f"{pop_num//1000}k KV Blocks" if pop_num < 1000000 else f"{pop_num//1000000}M KV Blocks"
        svg.append(f'  <text x="{gx + group_width//2}" y="{chart_bottom + 20}" class="t-bar-label" text-anchor="middle">{pop_label}</text>')
        
    svg.append('</svg>\n')
    with open(out_path, "w", encoding="utf-8") as f:
        f.write("".join(svg))
    print(f"Generated {out_path}")

def generate_llama_lookup_chart(data: dict, out_path: Path):
    """Generates Pillar 3 C++ llama.cpp lookup benchmark latency chart."""
    svg = [svg_header(width=960, height=320, title="Native C++ llama.cpp Lookup Cache Latency")]
    
    svg.append('  <text x="24" y="28" class="t-title">PILLAR 3: NATIVE C++ LLAMA.CPP LOOKUP CACHE DRAFT LATENCY (&mu;s)</text>')
    svg.append('  <text x="24" y="44" class="t-sub">Candidate draft query latency across context lengths (expanse::str_map vs stock std::unordered_map)</text>')
    
    svg.append('  <rect x="620" y="20" width="12" height="12" rx="2" class="b-hf"/>')
    svg.append('  <text x="636" y="30" class="t-legend">Stock std::unordered_map</text>')
    svg.append('  <rect x="800" y="20" width="12" height="12" rx="2" class="b-expanse"/>')
    svg.append('  <text x="816" y="30" class="t-legend">expanse::str_map (7-Bit Trie)</text>')
    
    contexts = sorted(data.keys(), key=lambda x: int(x))
    x_start = 80
    group_width = int(800 / max(1, len(contexts)))
    max_us = max([data[k]["expanse_llama_cache"]["draft_latency_us"] for k in contexts] + [1.0]) * 1.2
    chart_bottom = 260
    chart_height = 180
    
    for step in range(5):
        us_val = (max_us / 4) * step
        y = chart_bottom - int((us_val / max_us) * chart_height)
        svg.append(f'  <line x1="50" y1="{y}" x2="920" y2="{y}" class="grid"/>')
        svg.append(f'  <text x="42" y="{y + 3}" class="t-axis-label" text-anchor="end">{us_val:.1f} &mu;s</text>')
        
    svg.append(f'  <line x1="50" y1="{chart_bottom}" x2="920" y2="{chart_bottom}" class="axis"/>')
    
    for gi, ctx_k in enumerate(contexts):
        gx = x_start + gi * group_width
        ctx_d = data[ctx_k]
        
        stock_us = ctx_d["stock_llama_cache"]["draft_latency_us"]
        exp_us = ctx_d["expanse_llama_cache"]["draft_latency_us"]
        
        bar_w = 60
        b1_x = gx + 30
        b2_x = b1_x + bar_w + 20
        
        h1 = int((stock_us / max_us) * chart_height)
        h2 = int((exp_us / max_us) * chart_height)
        
        y1 = chart_bottom - h1
        y2 = chart_bottom - h2
        
        svg.append(f'  <rect x="{b1_x}" y="{y1}" width="{bar_w}" height="{h1}" rx="3" class="b-hf"/>')
        svg.append(f'  <text x="{b1_x + bar_w/2}" y="{y1 - 5}" class="t-val-blue" text-anchor="middle">{stock_us:.2f} &mu;s</text>')
        
        svg.append(f'  <rect x="{b2_x}" y="{y2}" width="{bar_w}" height="{h2}" rx="3" class="b-expanse"/>')
        svg.append(f'  <text x="{b2_x + bar_w/2}" y="{y2 - 5}" class="t-val-accent" text-anchor="middle">{exp_us:.2f} &mu;s</text>')
        
        ctx_num = int(ctx_k)
        ctx_label = f"{ctx_num//1000}k Context"
        svg.append(f'  <text x="{gx + group_width//2}" y="{chart_bottom + 20}" class="t-bar-label" text-anchor="middle">{ctx_label}</text>')
        
    svg.append('</svg>\n')
    with open(out_path, "w", encoding="utf-8") as f:
        f.write("".join(svg))
    print(f"Generated {out_path}")

def main():
    # Clean up old SVGs if present
    old_tps_svg = RESULTS_DIR / "bench_llama_lookup_tps.svg"
    if old_tps_svg.exists():
        old_tps_svg.unlink()

    p1_path = RESULTS_DIR / "bench_draft_quality.json"
    if p1_path.exists():
        with open(p1_path, "r", encoding="utf-8") as f:
            generate_draft_quality_chart(json.load(f), RESULTS_DIR / "bench_draft_quality_alpha.svg")

    p2_path = RESULTS_DIR / "bench_datastore_scale.json"
    if p2_path.exists():
        with open(p2_path, "r", encoding="utf-8") as f:
            generate_datastore_memory_chart(json.load(f), RESULTS_DIR / "bench_datastore_scale_memory.svg")

    p3_path = RESULTS_DIR / "bench_llama_lookup.json"
    if p3_path.exists():
        with open(p3_path, "r", encoding="utf-8") as f:
            generate_llama_lookup_chart(json.load(f), RESULTS_DIR / "bench_llama_lookup_latency.svg")

    p4_path = RESULTS_DIR / "bench_prefix_lru.json"
    if p4_path.exists():
        with open(p4_path, "r", encoding="utf-8") as f:
            generate_prefix_lru_chart(json.load(f), RESULTS_DIR / "bench_prefix_lru_throughput.svg")

if __name__ == "__main__":
    main()
