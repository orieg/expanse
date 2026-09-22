#!/usr/bin/env python3
"""
Runner for the Patricia trie vs Expanse suite (docs/benchmarks/patricia_comparison/).

Runs each harness (harness = false; `--json` prints one payload on stdout), attaches
host facts, the core pin and a load snapshot before every harness
(scripts/bench_provenance.py, AGENTS.md §8.17), and writes
results/baseline_<pillar>.json. `--quick` writes to the gitignored results/quick/
and never touches the committed baselines (§8.5).

Invoke through ../run.sh, which takes the host-wide benchmark lock and applies the
P-core pin; running this file directly skips both.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bench_provenance import add_load, attach, estimators, git_sha, host_facts  # noqa: E402

BENCHES = [
    ("patricia_memory", "baseline_memory.json"),
    ("patricia_lookup_hit", "baseline_lookup_hit.json"),
    ("patricia_lookup_miss", "baseline_lookup_miss.json"),
    ("patricia_insert", "baseline_insert.json"),
    ("patricia_string", "baseline_string_lookup.json"),
]


def run_bench(name: str, out_file: str, out_dir: Path, quick: bool, prov: dict) -> None:
    print(f"==> {name} (quick={quick})")
    cmd = ["cargo", "bench", "-p", "expanse-trie", "--bench", name, "--"]
    if quick:
        cmd.append("--quick")
    cmd.append("--json")
    res = subprocess.run(cmd, cwd=REPO_ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode != 0:
        print(f"{name} failed:\n{res.stderr}", file=sys.stderr)
        sys.exit(1)
    start = res.stdout.find("{")
    if start == -1:
        print(f"{name} printed no JSON payload:\n{res.stdout}", file=sys.stderr)
        sys.exit(1)
    payload = attach(json.loads(res.stdout[start:]), prov)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / out_file).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"    wrote {out_dir / out_file}")


def main() -> None:
    quick = "--quick" in sys.argv
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR
    prov = {
        "suite": "patricia_comparison",
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "estimators": estimators(
            "mean of per-round paired ratios Expanse ns / Patricia ns, BCa 95% "
            "interval from art_common::bca_ci (2000 resamples, label in "
            "ratio_ci_method); per-arm columns are medians of the same rounds; "
            "memory rows are exact byte counts with no interval"
        ),
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "quick": quick,
        "loads": [],
    }
    add_load(prov, "start")
    for name, out_file in BENCHES:
        add_load(prov, f"before {name}")
        run_bench(name, out_file, out_dir, quick, prov)
    add_load(prov, "end")
    print("Done. README result tables are pending the first committed full run.")


if __name__ == "__main__":
    main()
