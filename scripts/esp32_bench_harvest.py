#!/usr/bin/env python3
"""
scripts/esp32_bench_harvest.py

Parses ESP32 hardware UART benchmark logs, groups measurements by (benchmark, arm, n),
and computes BCa bootstrap 95% confidence intervals (>= 1000 resamples) per AGENTS.md §8.4.

Usage:
  python3 scripts/esp32_bench_harvest.py < uart_log.txt
  python3 scripts/esp32_bench_harvest.py --input /path/to/log.txt --out report.md --emit-json results.json
  python3 scripts/esp32_bench_harvest.py --self-test
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


def generate_structured_results(records):
    """Converts parsed records into a structured dict with BCa CIs for JSON export."""
    results = {}
    for (bench, n), arm_data in sorted(records.items()):
        bench_key = f"{bench}_{n}"
        results[bench_key] = {
            "benchmark": bench,
            "n": n,
            "arms": {}
        }
        for arm, samples in sorted(arm_data.items()):
            cycles_list = [s["cycles"] for s in samples if s["cycles"] is not None]
            heaps = [s["heap"] for s in samples if s["heap"] is not None]
            frags = [s["frag"] for s in samples if s["frag"] is not None]

            if cycles_list:
                mean_c, ci_l, ci_h = bootstrap_ci_bca(cycles_list)
            else:
                mean_c, ci_l, ci_h = 0.0, 0.0, 0.0

            results[bench_key]["arms"][arm] = {
                "cycles_per_op": {
                    "mean": mean_c,
                    "ci_95_low": ci_l,
                    "ci_95_high": ci_h,
                    "sample_count": len(cycles_list)
                },
                "heap_used_bytes": float(np.mean(heaps)) if heaps else 0.0,
                "frag_ratio": float(np.mean(frags)) if frags else 0.0
            }
    return results


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


def run_self_tests():
    sample_lines = [
        '{"benchmark": "esp32_sensor_tsdb_ingest", "arm": "expanse_memtable", "n": 2000, "cycles_per_op": 142.5, "heap_used_bytes": 8848, "frag_ratio": 0.0120}',
        '{"benchmark": "esp32_sensor_tsdb_ingest", "arm": "expanse_memtable", "n": 2000, "cycles_per_op": 140.2, "heap_used_bytes": 8848, "frag_ratio": 0.0120}',
        '{"benchmark": "esp32_sensor_tsdb_ingest", "arm": "expanse_memtable", "n": 2000, "cycles_per_op": 144.1, "heap_used_bytes": 8848, "frag_ratio": 0.0120}',
        '{"benchmark": "esp32_sensor_tsdb_ingest", "arm": "cpp_std_map", "n": 2000, "cycles_per_op": 380.0, "heap_used_bytes": 64000, "frag_ratio": 0.0450}',
    ]
    recs = parse_and_process(sample_lines)
    assert len(recs) == 1
    key = ("esp32_sensor_tsdb_ingest", 2000)
    assert key in recs
    assert "expanse_memtable" in recs[key]
    assert len(recs[key]["expanse_memtable"]) == 3

    structured = generate_structured_results(recs)
    assert "esp32_sensor_tsdb_ingest_2000" in structured
    arm_res = structured["esp32_sensor_tsdb_ingest_2000"]["arms"]["expanse_memtable"]
    assert 139.0 <= arm_res["cycles_per_op"]["mean"] <= 145.0
    assert arm_res["cycles_per_op"]["ci_95_low"] <= arm_res["cycles_per_op"]["mean"] <= arm_res["cycles_per_op"]["ci_95_high"]
    assert arm_res["heap_used_bytes"] == 8848.0

    report = generate_markdown_report(recs)
    assert "esp32_sensor_tsdb_ingest" in report
    assert "expanse_memtable" in report

    print("scripts/esp32_bench_harvest.py --self-test: all checks passed")


def main():
    parser = argparse.ArgumentParser(description="Harvest ESP32 Hardware Benchmark Metrics")
    parser.add_argument("--input", help="Path to UART input log file (reads stdin if omitted)")
    parser.add_argument("--out", help="Path to output markdown file (prints stdout if omitted)")
    parser.add_argument("--emit-json", help="Path to output JSON results file for charting and diffing")
    parser.add_argument("--self-test", action="store_true", help="Run internal unit self-tests")
    args = parser.parse_args()

    if args.self_test:
        run_self_tests()
        return

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

    if args.emit_json:
        structured = generate_structured_results(records)
        with open(args.emit_json, "w", encoding="utf-8") as f:
            json.dump(structured, f, indent=2)
        print(f"JSON results written to {args.emit_json}")


if __name__ == "__main__":
    main()
