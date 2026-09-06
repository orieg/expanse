#!/usr/bin/env python3
"""
Master runner for the Redis ZSET (sorted set) engine benchmark suite (#330).

Executes the four pillar benches (harness = false; each emits a JSON payload on
stdout under `--json`) and regenerates the dual-theme SVG charts:

  1. zset_zadd    -> baseline_zadd.json     (Pillar 1: ZADD churn)
  2. zset_range   -> baseline_range.json    (Pillar 2: ZRANGEBYSCORE/ZREVRANGE)
  3. zset_rank    -> baseline_rank.json     (Pillar 3: ZRANK/ZCOUNT)
  4. zset_memory  -> baseline_memory.json   (Pillar 4: bytes/member)
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
    ("zset_zadd", "baseline_zadd.json"),
    ("zset_range", "baseline_range.json"),
    ("zset_rank", "baseline_rank.json"),
    ("zset_memory", "baseline_memory.json"),
]


def run_bench(bench_name: str, out_file: str, out_dir: Path, quick: bool, prov: dict) -> None:
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
    # The JSON payload starts at the first '{' or '[' (cargo banners precede it).
    starts = [i for i in (stdout.find("{"), stdout.find("[")) if i != -1]
    if not starts:
        print(f"No JSON payload from {bench_name}:\n{stdout}", file=sys.stderr)
        sys.exit(1)
    payload = json.loads(stdout[min(starts):])

    out_dir.mkdir(parents=True, exist_ok=True)
    with open(out_dir / out_file, "w", encoding="utf-8") as f:
        json.dump(attach(payload, prov), f, indent=2)
    print(f"    Saved {out_dir / out_file}")


def main() -> None:
    quick = "--quick" in sys.argv or "-q" in sys.argv
    print(f"Redis ZSET engine benchmark suite (quick={quick})\n")
    # A --quick run produces reduced-sweep smoke data. Route it to the
    # gitignored results/quick/ scratch dir so it can never overwrite the
    # committed results/baseline_*.json — the corruption class fixed for the
    # llm_inference suite in #352.
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR
    prov = {
        "suite": "redis_zset_engine",
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "estimators": estimators(
            "per-arm ns/op and B/member as the harness reports them; a quoted ratio is Expanse over the named competitor on the same scenario"
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
        print("Done.")


if __name__ == "__main__":
    main()
