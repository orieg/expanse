#!/usr/bin/env python3
"""
Runner for the Patricia trie vs Expanse suite (docs/benchmarks/patricia_comparison/).

Runs each harness (harness = false; `--json` prints one payload on stdout), attaches
host facts, the core pin and a load snapshot before every harness
(scripts/bench_provenance.py, AGENTS.md §8.17), and writes
results/baseline_<pillar>.json. `--quick` writes to the gitignored results/quick/
and never touches the committed baselines (§8.5).

Two entry points hold the host-wide benchmark lock and the P-core pin before
calling this file: `/benchmark patricia_comparison` on a pull request (the
bare-metal workflow passes --host-desc and --run-id, and uploads the artifacts)
and ../run.sh on the host. Running this file directly skips both.
"""

import argparse
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


def summarize(name: str, payload: dict) -> None:
    """One console line per row: what the PR comment shows. The artifact is the record."""
    for r in payload.get("results", []):
        label = r.get("distribution", "?") + (f"/{r['order']}" if "order" in r else "")
        if "ratio_ci" in r:
            lo, hi = r["ratio_ci"]
            print(f"  {name:22} n={r['population']:>8} {label:26} Expanse {r['expanse_ns_op']:9.2f} ns"
                  f"  Patricia {r['patricia_ns_op']:9.2f} ns  Exp/Pat {r['ratio_expanse_over_patricia']:.3f}"
                  f" [{lo:.3f}, {hi:.3f}] ({r['ratio_ci_method']})")
        else:
            print(f"  {name:22} n={r['population']:>8} {label:26} Expanse {r['expanse_bytes_per_key']:7.2f} B/key"
                  f"  Patricia {r['patricia_bytes_per_key']:7.2f} B/key"
                  f"  order-invariant {r.get('patricia_order_invariant', 'n/a')}")


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
    raw = json.loads(res.stdout[start:])
    summarize(name, raw)
    payload = attach(raw, prov)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / out_file).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"    wrote {out_dir / out_file}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.strip().splitlines()[0])
    ap.add_argument("--quick", action="store_true", help="smoke run into results/quick/ (gitignored)")
    ap.add_argument("--host-desc", default="", help="anonymized host description (never a hostname)")
    ap.add_argument("--run-id", default="", help="CI run URL this artifact came from")
    args = ap.parse_args()
    quick = args.quick
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
        "host_description": args.host_desc or None,
        "run_id": args.run_id or None,
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
