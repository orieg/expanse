#!/usr/bin/env python3
"""
Master runner for the Adaptive Radix Tree (ART) benchmark suite (#387).

Executes the five pillar benches (harness = false; each emits a JSON payload on
stdout under `--json`) and regenerates the dual-theme SVG charts:

  1. art_lookup_hit   -> baseline_lookup_hit.json   (Pillar 1: 100% Hit point lookup)
  2. art_lookup_miss  -> baseline_lookup_miss.json  (Pillar 2: 50/50 rejection miss)
  3. art_insert       -> baseline_insert.json       (Pillar 3: dynamic growth)
  4. art_scan         -> baseline_scan.json         (Pillar 4: range scan & iter)
  5. art_memory       -> baseline_memory.json       (Pillar 5: bytes/key census)
"""

import json
import subprocess
import sys
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"
SCRIPTS_DIR = BASE_DIR / "scripts"

BENCHES = [
    ("art_lookup_hit", "baseline_lookup_hit.json"),
    ("art_lookup_miss", "baseline_lookup_miss.json"),
    ("art_insert", "baseline_insert.json"),
    ("art_scan", "baseline_scan.json"),
    ("art_memory", "baseline_memory.json"),
]


def run_bench(bench_name: str, out_file: str, out_dir: Path, quick: bool) -> None:
    print(f"==> Running {bench_name} (quick={quick})...")
    cmd = ["cargo", "bench", "-p", "expanse-trie", "--bench", bench_name, "--"]
    if quick:
        cmd.append("--quick")
    cmd.append("--json")

    res = subprocess.run(cmd, cwd=REPO_ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode != 0:
        print(f"Error running {bench_name}:\n{res.stderr}", file=sys.stderr)
        sys.exit(1)

    stdout = res.stdout
    starts = [i for i in (stdout.find("{"), stdout.find("[")) if i != -1]
    if not starts:
        print(f"No JSON payload from {bench_name}:\n{stdout}", file=sys.stderr)
        sys.exit(1)
    payload = json.loads(stdout[min(starts):])

    out_dir.mkdir(parents=True, exist_ok=True)
    with open(out_dir / out_file, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2)
    print(f"    Saved {out_dir / out_file}")


def main() -> None:
    quick = "--quick" in sys.argv or "-q" in sys.argv
    print(f"ART comparison benchmark suite (quick={quick})\n")
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR
    for bench_name, out_file in BENCHES:
        run_bench(bench_name, out_file, out_dir, quick)
    if quick:
        print("\n==> Skipping chart regeneration (--quick).")
        print(f"    Quick smoke results were written to {out_dir} (gitignored);")
        print("    the committed results/baseline_*.json and SVG charts were")
        print("    not touched.")
    else:
        print("\n==> Generating SVG charts...")
        subprocess.run([sys.executable, str(SCRIPTS_DIR / "generate_charts.py")], check=True)
        print("Done.")


if __name__ == "__main__":
    main()
