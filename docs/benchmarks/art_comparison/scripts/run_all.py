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
    add_load, attach, estimators, git_sha, host_facts,
)


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


def run_bench(bench_name: str, out_file: str, out_dir: Path, quick: bool, meta: dict | None,
              prov: dict | None) -> None:
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

    if meta:
        payload["metadata"] = dict(meta)
    if prov is not None:
        payload = attach(payload, prov)

    out_dir.mkdir(parents=True, exist_ok=True)
    with open(out_dir / out_file, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2)
    print(f"    Saved {out_dir / out_file}")


def main() -> None:
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

    prov = {
        "suite": "art_comparison",
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

    for bench_name, out_file in BENCHES:
        # One snapshot per pillar, not one per sweep: a load average lags a
        # heavy process by about thirty seconds, so a single pair at the ends
        # cannot say which pillar ran beside one (AGENTS.md section 8.4).
        add_load(prov, f"before {bench_name}")
        run_bench(bench_name, out_file, out_dir, quick, meta, prov)

    add_load(prov, "end")

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
        print("Done.")


if __name__ == "__main__":
    main()
