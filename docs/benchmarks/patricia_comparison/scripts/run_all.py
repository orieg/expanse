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
from bench_provenance import add_load, attach, estimators, git_sha, host_facts, rewrite  # noqa: E402

# (bench, artifact, full-run populations). Each population runs in its own
# harness process with a load snapshot before it, so contamination during a
# long harness is attributable to one population (AGENTS.md §8.17).
FULL = [10_000, 100_000, 1_000_000]
BENCHES = [
    ("patricia_memory", "baseline_memory.json", [1_000] + FULL),
    ("patricia_lookup_hit", "baseline_lookup_hit.json", FULL),
    ("patricia_lookup_miss", "baseline_lookup_miss.json", FULL),
    ("patricia_insert", "baseline_insert.json", FULL),
    ("patricia_string", "baseline_string_lookup.json", FULL),
    ("patricia_scan", "baseline_scan.json", FULL),
]
QUICK = [10_000, 50_000]


def summarize(name: str, payload: dict) -> None:
    """One block per row: what the PR comment shows. The artifact is the record."""
    for r in payload.get("results", []):
        head = " ".join(f"{k}={r[k]}" for k in ("distribution", "prefix_len", "order", "population") if k in r)
        print(f"  {name} {head}")
        for k, v in r.items():
            if k.endswith("_invalid_reason"):
                print(f"      {k}: {v}")
            elif k.endswith("_ns_op") or k.endswith("bytes_per_key"):
                print(f"      {k:56} {v:.3f}")
            elif k.startswith("ratio_") and k.endswith("_ci"):
                base = k[: -len("_ci")]
                print(f"      {base:56} {r[base]:.3f} [{v[0]:.3f}, {v[1]:.3f}] ({r[base + '_ci_method']})")


def run_pop(name: str, pop: int, quick: bool) -> dict:
    cmd = ["cargo", "bench", "-q", "-p", "expanse-trie", "--bench", name, "--"]
    if quick:
        cmd.append("--quick")
    cmd += ["--pop", str(pop), "--json"]
    res = subprocess.run(cmd, cwd=REPO_ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode != 0:
        print(f"{name} --pop {pop} failed:\n{res.stderr}", file=sys.stderr)
        sys.exit(1)
    start = res.stdout.find("{")
    if start == -1:
        print(f"{name} printed no JSON payload:\n{res.stdout}", file=sys.stderr)
        sys.exit(1)
    return json.loads(res.stdout[start:])


def run_bench(name: str, out_file: str, pops: list[int], out_dir: Path, quick: bool, prov: dict) -> None:
    print(f"==> {name} (quick={quick}, populations={pops})")
    merged = None
    for pop in pops:
        add_load(prov, f"before {name} n={pop}")
        payload = run_pop(name, pop, quick)
        summarize(name, payload)
        if merged is None:
            merged = payload
        else:
            merged["results"].extend(payload["results"])
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / out_file).write_text(json.dumps(attach(merged, prov), indent=2) + "\n", encoding="utf-8")
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
            "geometric mean of per-round paired ratios (Expanse ns / twin ns, "
            "and each arm's generator / shuffled insert), BCa 95% interval on the "
            "per-round log ratios from art_common::bca_ci_labeled (2000 "
            "resamples; construction label in <ratio>_ci_method); per-arm "
            "columns are medians of the same rounds; memory rows are exact "
            "requested and usable byte counts with no interval"
        ),
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "host_description": args.host_desc or None,
        "run_id": args.run_id or None,
        "quick": quick,
        "loads": [],
    }
    # Build every harness before the first snapshot, so compilation is not
    # running beside, or immediately before, a timed pass.
    build = subprocess.run(
        ["cargo", "bench", "-q", "-p", "expanse-trie", "--no-run"]
        + [a for name, _, _ in BENCHES for a in ("--bench", name)],
        cwd=REPO_ROOT,
    )
    if build.returncode != 0:
        sys.exit(build.returncode)
    add_load(prov, "start")
    for name, out_file, pops in BENCHES:
        run_bench(name, out_file, QUICK if quick else pops, out_dir, quick, prov)
    add_load(prov, "end")
    # Each artifact was written inside the loop, before this snapshot; re-stamp
    # so the last population's interval has a snapshot after it too (§8.17).
    stamped = rewrite([out_dir / f for _, f, _ in BENCHES], prov)
    print(f"Re-stamped provenance with the end snapshot into {stamped} artifacts.")
    if not quick:
        subprocess.run([sys.executable, str(BASE_DIR / "scripts" / "generate_readme.py")], check=True)


if __name__ == "__main__":
    main()
