#!/usr/bin/env python3
"""
Master runner for the search / inverted-index suite: ExpanseSet vs Roaring.

Executes the three wall-clock harnesses, saves their JSON to results/, and
regenerates the SVG charts:
  1. search_boolean  -> baseline_boolean.json  (Pillar 1: AND / OR / AND-NOT)
  2. search_wand     -> baseline_wand.json     (Pillar 2: WAND skip-scan)
  3. search_memory   -> baseline_memory.json   (Pillar 3: bits per docID)

The deterministic instruction-count arm (search_instructions, iai-callgrind)
is Linux-only and is run separately via the `instruction-counts` CI job or
`cargo bench -p expanse-trie --bench search_instructions` on a Linux host.
"""

import os
import json
import subprocess
import sys
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"
SCRIPTS_DIR = BASE_DIR / "scripts"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bench_provenance import (  # noqa: E402
    add_load, attach, estimators, git_sha, host_facts, rewrite,
)


BENCHES = [
    ("search_boolean", "baseline_boolean.json"),
    ("search_wand", "baseline_wand.json"),
    ("search_memory", "baseline_memory.json"),
]


def run_bench(bench_name: str, out_file: str, out_dir: Path, quick: bool, prov: dict):
    print(f"==> Running benchmark: {bench_name} (quick={quick})...")
    cmd = ["cargo", "bench", "-p", "expanse-trie", "--bench", bench_name, "--"]
    if quick:
        cmd.append("--quick")
    cmd.append("--json")

    res = subprocess.run(cmd, cwd=REPO_ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode != 0:
        print(f"Error running {bench_name}:", file=sys.stderr)
        print(res.stderr, file=sys.stderr)
        sys.exit(1)

    stdout = res.stdout.strip()
    start = stdout.find("[")
    if start == -1:
        print(f"No JSON payload in {bench_name} output:\n{stdout}", file=sys.stderr)
        sys.exit(1)
    parsed = json.loads(stdout[start:])

    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / out_file
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(attach(parsed, prov), f, indent=2)
    print(f"    Saved results to {out_path}")


def main():
    quick = "--quick" in sys.argv or "-q" in sys.argv
    print(f"Starting ExpanseSet vs Roaring inverted-index suite (quick={quick})...\n")

    # A --quick run produces reduced-sweep smoke data. Route it to the
    # gitignored results/quick/ scratch dir so it can never overwrite the
    # committed results/baseline_*.json — the corruption class fixed for the
    # llm_inference suite in #352.
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR

    prov = {
        "suite": "search_inverted_index",
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "estimators": estimators(
            "per-arm ns as the harness reports them; a quoted ratio is Expanse over Roaring on the same cell"
        ),
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "quick": quick,
        "loads": [],
    }
    add_load(prov, "start")

    for bench_name, out_file in BENCHES:
        run_bench(bench_name, out_file, out_dir, quick, prov)

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
        print("\n==> Generating SVG charts...")
        subprocess.run([sys.executable, str(SCRIPTS_DIR / "generate_charts.py")], check=True)
        print("\nAll benchmarks and charts generated successfully!")


if __name__ == "__main__":
    main()
