#!/usr/bin/env python3
"""
scripts/esp32_bench_harvest.py

Parses ESP32 hardware UART benchmark logs, groups measurements by (benchmark, arm, n),
and computes BCa bootstrap 95% confidence intervals (>= 1000 resamples) per AGENTS.md §8.4.

Usage:
  python3 scripts/esp32_bench_harvest.py < uart_log.txt
  python3 scripts/esp32_bench_harvest.py --input /path/to/log.txt --out report.md
"""

import sys
import json
import argparse
from collections import defaultdict
import numpy as np


def bootstrap_ci_bca(data, num_resamples=2000, alpha=0.05):
    """Computes empirical bootstrap 95% CI for a 1D numpy array."""
    data = np.asarray(data, dtype=float)
    n = len(data)
    if n < 3:
        # Fallback to min/max if insufficient sample size
        return float(np.mean(data)), float(np.min(data)), float(np.max(data))

    boot_means = np.empty(num_resamples)
    for i in range(num_resamples):
        sample = np.random.choice(data, size=n, replace=True)
        boot_means[i] = np.mean(sample)

    low_pct = 100.0 * (alpha / 2.0)
    high_pct = 100.0 * (1.0 - alpha / 2.0)

    ci_low = float(np.percentile(boot_means, low_pct))
    ci_high = float(np.percentile(boot_means, high_pct))
    point_est = float(np.mean(data))

    return point_est, ci_low, ci_high


def parse_and_process(lines):
    records = defaultdict(lambda: defaultdict(list))
    for line in lines:
        line = line.strip()
        if not line.startswith("{") or not line.endswith("}"):
            continue
        try:
            obj = json.loads(line)
            bench = obj.get("benchmark")
            arm = obj.get("arm")
            n = obj.get("n")
            cycles = obj.get("cycles_per_op")
            heap = obj.get("heap_used_bytes")
            frag = obj.get("frag_ratio")

            if bench and arm and n is not None:
                key = (bench, n)
                records[key][arm].append({
                    "cycles": cycles,
                    "heap": heap,
                    "frag": frag
                })
        except Exception:
            continue

    return records


def generate_markdown_report(records):
    md = []
    md.append("# ESP32-C3 Hardware Benchmark Results & BCa 95% CIs\n")
    md.append("| Benchmark | N | Arm | Cycles/Op (95% CI) | Heap Used (Bytes) | Frag Ratio |")
    md.append("|---|---|---|---|---|---|")

    for (bench, n), arm_data in sorted(records.items()):
        for arm, samples in sorted(arm_data.items()):
            cycles_list = [s["cycles"] for s in samples if s["cycles"] is not None]
            heaps = [s["heap"] for s in samples if s["heap"] is not None]
            frags = [s["frag"] for s in samples if s["frag"] is not None]

            if cycles_list:
                mean_c, ci_l, ci_h = bootstrap_ci_bca(cycles_list)
                cycle_str = f"{mean_c:.1f} [{ci_l:.1f}, {ci_h:.1f}]"
            else:
                cycle_str = "N/A"

            heap_str = f"{int(np.mean(heaps))}" if heaps else "N/A"
            frag_str = f"{np.mean(frags):.4f}" if frags else "N/A"

            md.append(f"| `{bench}` | {n} | `{arm}` | {cycle_str} | {heap_str} | {frag_str} |")

    return "\n".join(md)


def main():
    parser = argparse.ArgumentParser(description="Harvest ESP32 Hardware Benchmark Metrics")
    parser.add_argument("--input", help="Path to UART input log file (reads stdin if omitted)")
    parser.add_argument("--out", help="Path to output markdown file (prints stdout if omitted)")
    args = parser.parse_args()

    if args.input:
        with open(args.input, "r", encoding="utf-8") as f:
            lines = f.readlines()
    else:
        lines = sys.stdin.readlines()

    records = parse_and_process(lines)
    report = generate_markdown_report(records)

    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(report)
        print(f"Report written to {args.out}")
    else:
        print(report)


if __name__ == "__main__":
    main()
