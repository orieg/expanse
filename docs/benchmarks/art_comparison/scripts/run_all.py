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

Each bench is built before its window opens and then runs as one process
inside one load window (`bench_windowed.run_bench_window`): the host's busy
CPU and the process's own CPU over its run, stored on the artifact it wrote
(`load`) and in `provenance.windows`. A closing snapshot follows the last
bench, and every artifact is re-stamped with the whole series. The written
artifacts are then judged by `check_bench_provenance.findings_for`, and a
finding fails the run (AGENTS.md section 8.17, #1214).

  run_all.py [--quick]
  run_all.py --self-test
"""

import copy
import json
import os
import platform
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
import bench_windowed  # noqa: E402

SUITE = "art_comparison"


BENCHES = [
    ("art_lookup_hit", "baseline_lookup_hit.json"),
    ("art_lookup_miss", "baseline_lookup_miss.json"),
    ("art_insert", "baseline_insert.json"),
    ("art_scan", "baseline_scan.json"),
    ("art_memory", "baseline_memory.json"),
    ("art_small_payload", "baseline_small_payload.json"),
]


def get_load_str() -> str:
    try:
        loads = os.getloadavg()
        return f"{loads[0]:.2f}"
    except Exception:
        return "N/A"


def get_git_sha() -> str:
    try:
        return subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT).decode().strip()
    except Exception:
        return "HEAD"


def get_kernel_str() -> str:
    try:
        return f"{platform.system()} {platform.release()}"
    except Exception:
        return "Linux"


def stamp(payload, meta: dict | None, prov: dict, window: dict) -> dict:
    """The artifact a bench's payload is written as: metadata, provenance, and its window."""
    if meta and isinstance(payload, dict):
        payload["metadata"] = dict(meta)
    out = attach(payload, prov)
    # The load window of the bench process that produced this file.
    out["load"] = window
    return out


def run_bench(bench_name: str, out_file: str, out_dir: Path, quick: bool, meta: dict | None,
              prov: dict) -> None:
    print(f"==> Running {bench_name} (quick={quick})...")
    args = ["--quick"] if quick else []
    args.append("--json")
    res, window = bench_windowed.run_bench_window(prov, "expanse-trie", bench_name, args)
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
        json.dump(stamp(payload, meta, prov, window), f, indent=2)
    print(f"    Saved {out_dir / out_file}  (window: {window})")


def new_prov(quick: bool) -> dict:
    prov = {
        "suite": SUITE,
        "issue": 387,
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "estimators": estimators(
            "mean(ART rounds) / mean(Expanse rounds) with a two-sample BCa 95% "
            "interval (scripts/bca_bootstrap.py); the per-arm columns beside it "
            "are medians of the same rounds"
        ),
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "quick": quick,
        "loads": [],
    }
    add_load(prov, "start")
    return prov


def judge_written(out_dir: Path) -> int:
    """Judges every written artifact by the provenance gate; the number of findings."""
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
    prov = new_prov(quick=True)
    window = {"since": "cell:art_lookup_hit", "wall_s": 3.0, "busy_cpus_since_prev": 1.0,
              "own_busy_cpus": 0.99, "foreign_busy_cpus": 0.01}
    prov["load_windows"] = True
    prov["windows"] = [{"id": "art_lookup_hit", "load": window}]
    # A snapshot as `add_load(prov, "end")` leaves it on a host with /proc.
    prov["loads"].append({"label": "end", "busy_cpus_since_prev": 1.0})
    payload = {"results": [{"population": 10000,
                            "rounds_raw": [{"round": 0, "expanse_ns": 1.0, "art_ns": 1.2}]}]}
    art = stamp(copy.deepcopy(payload), {"host": "x"}, prov, window)
    got = bench_windowed.judge(SUITE, [("baseline_lookup_hit.json", art)])
    assert got == [], f"a stamped artifact must pass the gate: {got}"
    assert art.get("load") == window, "the window must be stored on the artifact it timed"
    unattributed = copy.deepcopy(art)
    unattributed["provenance"]["windows"][0]["load"]["foreign_busy_cpus"] = None
    got = bench_windowed.judge(SUITE, [("baseline_lookup_hit.json", unattributed)])
    assert any("load windows" in f for f in got), \
        f"a window that could not attribute must be a finding: {got}"
    print("art_comparison run_all.py --self-test: all checks passed")
    return 0


def main() -> None:
    if "--self-test" in sys.argv:
        sys.exit(self_test())
    quick = "--quick" in sys.argv or "-q" in sys.argv
    print(f"ART comparison benchmark suite (quick={quick})\n")
    load_start = get_load_str()
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR

    meta = None
    if not quick:
        sha = get_git_sha()
        kernel_str = get_kernel_str()
        meta = {
            "host": "reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3, Ubuntu 22.04",
            "kernel": kernel_str,
            "load_start": load_start,
            "load_end": "N/A",  # updated after sweep
            "harness_sha": sha,
            "data_sha": sha,
        }

    prov = new_prov(quick)

    for bench_name, out_file in BENCHES:
        # One window per pillar, not one per sweep: a load average lags a
        # heavy process by about thirty seconds, so a single pair at the ends
        # cannot say which pillar ran beside one (AGENTS.md section 8.17).
        run_bench(bench_name, out_file, out_dir, quick, meta, prov)

    # Closes the last window in the series, and re-stamps: each artifact was
    # written before the windows after it existed.
    add_load(prov, "end")
    rewrite((out_dir / f for _, f in BENCHES), prov)

    load_end = get_load_str()

    if quick:
        print("\n==> Skipping chart regeneration (--quick).")
        print(f"    Quick smoke results were written to {out_dir} (gitignored);")
        print("    the committed results/baseline_*.json and SVG charts were")
        print("    not touched.")
    else:
        # Update load_end in all written JSON artifacts
        for _, out_file in BENCHES:
            p = out_dir / out_file
            with open(p, "r", encoding="utf-8") as f:
                d = json.load(f)
            if "metadata" in d:
                d["metadata"]["load_end"] = load_end
            with open(p, "w", encoding="utf-8") as f:
                json.dump(d, f, indent=2)

        print("\n==> Verifying BCa confidence intervals and statistics...")
        subprocess.run([sys.executable, str(SCRIPTS_DIR / "recompute_and_patch_json.py")], check=True)

        print("\n==> Generating SVG charts...")
        subprocess.run([sys.executable, str(SCRIPTS_DIR / "generate_charts.py")], check=True)

        print("\n==> Generating README.md tables from JSON artifacts...")
        from generate_readme import generate_readme
        readme_content = generate_readme()
        with open(BASE_DIR / "README.md", "w", encoding="utf-8") as f:
            f.write(readme_content)
        print(f"    Updated {BASE_DIR / 'README.md'}")
        print(f"Load average during sweep: start={load_start}, end={load_end}")
    if judge_written(out_dir):
        sys.exit(1)
    print("Done.")


if __name__ == "__main__":
    main()
