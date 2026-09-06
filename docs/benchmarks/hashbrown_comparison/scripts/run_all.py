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
# `docs/benchmarks/<suite>` -> the repo root is three levels up, as in every
# other runner. This read `.parent.parent` and resolved to `docs/`; cargo walks
# up to find a manifest, so the sweep still ran and it went unnoticed.
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"
SCRIPTS_DIR = BASE_DIR / "scripts"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bench_provenance import (  # noqa: E402
    add_load, attach, estimators, git_sha, host_facts, rewrite,
)


BENCHES = [
    ("hashbrown_native_suite", "baseline_native.json"),
    ("hashbrown_ycsb", "baseline_ycsb.json"),
    ("hashbrown_tail_latency", "baseline_tail_latency.json"),
    ("hashbrown_container_dists", "baseline_distributions.json"),
    ("hashbrown_memory_alloc", "baseline_memory.json"),
]

def run_bench(bench_name: str, out_file: str, out_dir: Path, prov: dict, quick: bool = False):
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

    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / out_file
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(attach(parsed, prov), f, indent=2)
    print(f"    Saved results to {out_path}")

def main():
    quick = "--quick" in sys.argv or "-q" in sys.argv
    print(f"Starting Hashbrown vs BTreeMap vs Expanse benchmark suite (quick={quick})...\n")

    # A --quick run produces reduced-sweep smoke data. Route it to the
    # gitignored results/quick/ scratch dir so it can never overwrite the
    # committed results/baseline_*.json — the corruption class fixed for the
    # llm_inference suite in #352.
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR

    prov = {
        "suite": "hashbrown_comparison",
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "estimators": estimators(
            "per-arm ns/op and B/key as the Criterion harness reports them; where a ratio is quoted it is Expanse over the named competitor at the same population and hit rate"
        ),
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "quick": quick,
        "loads": [],
    }
    add_load(prov, "start")

    for bench_name, out_file in BENCHES:
        run_bench(bench_name, out_file, out_dir, prov, quick=quick)

    add_load(prov, "end")
    # The artifacts were written inside the loop above, before this
    # snapshot existed; re-stamp so each carries the whole load series.
    rewrite((out_dir / f for _, f in BENCHES), prov)

    if quick:
        print("\n==> Skipping chart regeneration (--quick).")
        print(f"    Quick smoke results were written to {out_dir} (gitignored);")
        print("    the committed results/baseline_*.json and SVG charts were")
        print("    not touched. Regenerating the committed charts from")
        print("    reduced-sweep data would ship blank/mislabeled SVGs.")
    else:
        print("\n==> Generating SVG comparison charts...")
        subprocess.run([sys.executable, str(SCRIPTS_DIR / "generate_charts.py")], check=True)
        print("\nAll benchmarks and charts generated successfully!")

if __name__ == "__main__":
    main()
