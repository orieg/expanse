#!/usr/bin/env python3
"""
Master benchmark runner for Hashbrown / SwissTable vs BTreeMap vs ExpanseMap.

Executes all 5 benchmark harnesses:
1. hashbrown_native_suite (Criterion Native port)
2. hashbrown_ycsb (YCSB A-F workloads, through scripts/ycsb_bench.py)
3. hashbrown_tail_latency (HdrHistogram P50-P99.99)
4. hashbrown_container_dists (Ankerl/Tessil key distributions)
5. hashbrown_memory_alloc (GlobalAlloc live heap tracking)

Saves JSON outputs into results/ and regenerates SVG comparison charts.

Each single-process pillar is built before its window opens and runs as one
process inside one load window (`bench_windowed.run_bench_window`), stored on
the artifact it wrote (`load`) and in `provenance.windows`; a closing snapshot
follows, and every artifact is re-stamped with the whole series. The written
artifacts are then judged by `check_bench_provenance.findings_for`, and a
finding fails the run (AGENTS.md section 8.17, #1214).

  run_all.py [--quick]
  run_all.py --self-test
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
import bench_windowed  # noqa: E402

SUITE = "hashbrown_comparison"


BENCHES = [
    ("hashbrown_native_suite", "baseline_native.json"),
    ("hashbrown_tail_latency", "baseline_tail_latency.json"),
    ("hashbrown_container_dists", "baseline_distributions.json"),
    ("hashbrown_memory_alloc", "baseline_memory.json"),
]

def run_ycsb(quick: bool) -> None:
    """The YCSB pillar goes through `scripts/ycsb_bench.py` (#1005).

    One `cargo bench` pass per cell cannot carry an interval. The driver owns
    the rounds, the per-round load snapshots, the BCa intervals and the paired
    ratios, judges its artifact by `check_bench_provenance.py` before writing
    it, and confines `--quick` to `results/quick/` itself.
    """
    print(f"==> Running benchmark: hashbrown_ycsb through scripts/ycsb_bench.py (quick={quick})...")
    cmd = [sys.executable, str(REPO_ROOT / "scripts" / "ycsb_bench.py"), "--suite", "hashbrown"]
    if quick:
        cmd.append("--quick")
    res = subprocess.run(cmd, cwd=REPO_ROOT)
    if res.returncode != 0:
        print("Error running the hashbrown YCSB driver", file=sys.stderr)
        sys.exit(1)


def stamp(parsed, prov: dict, window: dict) -> dict:
    """The artifact a bench's payload is written as: provenance, and its window."""
    out = attach(parsed, prov)
    # The load window of the bench process that produced this file.
    out["load"] = window
    return out


def run_bench(bench_name: str, out_file: str, out_dir: Path, prov: dict, quick: bool = False):
    print(f"==> Running benchmark: {bench_name} (quick={quick})...")
    args = ["--quick"] if quick else []
    args.append("--json")
    res, window = bench_windowed.run_bench_window(prov, "expanse-trie", bench_name, args)
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
        json.dump(stamp(parsed, prov, window), f, indent=2)
    print(f"    Saved results to {out_path}  (window: {window})")


def judge_written(out_dir: Path) -> int:
    """Judges every written single-process artifact by the provenance gate.

    The YCSB driver judges its own artifact before writing it.
    """
    written = []
    for _, out_file in BENCHES:
        path = out_dir / out_file
        if path.is_file():
            written.append((out_file, json.loads(path.read_text(encoding="utf-8"))))
    findings = bench_windowed.judge(SUITE, written)
    for f in findings:
        print(f"::error::{f}", file=sys.stderr)
    print(f"==> check_bench_provenance: {len(written)} artifact(s), {len(findings)} finding(s)")
    return len(findings)


def self_test() -> int:
    """The runner's stamping path, judged by the gate without running a bench."""
    prov = {"suite": SUITE, "commit": "abc1234", "host": host_facts(),
            "estimators": estimators("synthetic"), "loads": []}
    add_load(prov, "start")
    window = {"since": "cell:hashbrown_tail_latency", "wall_s": 3.0,
              "busy_cpus_since_prev": 1.0, "own_busy_cpus": 0.99, "foreign_busy_cpus": 0.01}
    prov["load_windows"] = True
    prov["windows"] = [{"id": "hashbrown_tail_latency", "load": window}]
    prov["loads"].append({"label": "end", "busy_cpus_since_prev": 1.0})
    failures = []
    # A bare-array payload is wrapped, and the window rides on the wrapper.
    art = stamp([{"arm": "expanse", "rounds_raw": [{"round": 0, "ns": 1.0}]}], prov, window)
    got = bench_windowed.judge(SUITE, [("baseline_tail_latency.json", art)])
    if got:
        failures.append(f"a stamped artifact with rounds must pass the gate: {got}")
    if art.get("load") != window or "cells" not in art:
        failures.append("the window must be stored on the artifact it timed")
    # A payload without rounds is what the harnesses emit today: the gate
    # names it rather than the runner writing it quietly.
    got = bench_windowed.judge(SUITE, [("baseline_tail_latency.json",
                                        stamp([{"arm": "expanse", "p99_ns": 1.0}], prov, window))])
    if not any("rounds_raw" in g for g in got):
        failures.append(f"a payload with no rounds must be a finding: {got}")
    for f in failures:
        print(f"  FAIL {f}")
    print(f"hashbrown_comparison run_all.py --self-test: "
          f"{'all checks passed' if not failures else f'{len(failures)} failure(s)'}")
    return 1 if failures else 0

def main():
    if "--self-test" in sys.argv:
        sys.exit(self_test())
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

    # After the single-process pillars: the driver takes its own load snapshots
    # around each of its rounds and writes its own provenance block.
    run_ycsb(quick)

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
    if judge_written(out_dir):
        sys.exit(1)

if __name__ == "__main__":
    main()
