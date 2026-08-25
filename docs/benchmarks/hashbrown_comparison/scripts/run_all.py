#!/usr/bin/env python3
"""
Master benchmark runner for Hashbrown / SwissTable vs BTreeMap vs ExpanseMap.

Executes all 5 benchmark harnesses:
1. hashbrown_native_suite (Criterion Native port)
2. hashbrown_ycsb (YCSB A-F workloads)
3. hashbrown_tail_latency (HdrHistogram P50-P99.99)
4. hashbrown_container_dists (Ankerl/Tessil key distributions)
5. hashbrown_memory_alloc (GlobalAlloc live heap tracking)

Saves JSON outputs into results/ and regenerates SVG comparison charts.
"""

import os
import sys
import json
import subprocess
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent
RESULTS_DIR = BASE_DIR / "results"
SCRIPTS_DIR = BASE_DIR / "scripts"

BENCHES = [
    ("hashbrown_native_suite", "baseline_native.json"),
    ("hashbrown_ycsb", "baseline_ycsb.json"),
    ("hashbrown_tail_latency", "baseline_tail_latency.json"),
    ("hashbrown_container_dists", "baseline_distributions.json"),
    ("hashbrown_memory_alloc", "baseline_memory.json"),
]

def run_bench(bench_name: str, out_file: str, quick: bool = False):
    print(f"==> Running benchmark: {bench_name} (quick={quick})...")
    cmd = [
        "cargo", "bench", "-p", "expanse-trie",
        "--bench", bench_name,
        "--",
    ]
    if quick:
        cmd.append("--quick")
    cmd.append("--json")

    res = subprocess.run(cmd, cwd=REPO_ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode != 0:
        print(f"Error running {bench_name}:", file=sys.stderr)
        print(res.stderr, file=sys.stderr)
        sys.exit(1)

    # Extract JSON string from stdout (ignoring any cargo compilation banners)
    stdout = res.stdout.strip()
    json_start = stdout.find("[")
    if json_start == -1 or (stdout.find("{") != -1 and stdout.find("{") < json_start):
        json_start = stdout.find("{")

    if json_start == -1:
        print(f"Failed to find JSON payload in {bench_name} output:\n{stdout}", file=sys.stderr)
        sys.exit(1)

    json_str = stdout[json_start:]
    parsed = json.loads(json_str)

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out_path = RESULTS_DIR / out_file
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(parsed, f, indent=2)
    print(f"    Saved results to {out_path}")

def main():
    quick = "--quick" in sys.argv or "-q" in sys.argv
    print(f"Starting Hashbrown vs BTreeMap vs Expanse benchmark suite (quick={quick})...\n")

    for bench_name, out_file in BENCHES:
        run_bench(bench_name, out_file, quick=quick)

    print("\n==> Generating SVG comparison charts...")
    subprocess.run([sys.executable, str(SCRIPTS_DIR / "generate_charts.py")], check=True)
    print("\nAll benchmarks and charts generated successfully!")

if __name__ == "__main__":
    main()
