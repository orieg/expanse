#!/usr/bin/env python3
"""The #789 ablations on the single-writer cells (#568 PR 0, item 5).

Builds `masstree_concurrent` once per engine feature variant — `default`,
`lock-padded`, `advance-every-4096`, `advance-never` — each into its own
target directory, and runs the same two throughput cells on every build:
C1 W=1 R=0 (the writer alone) and C2 W=1 R=8 (the writer with eight
readers). Each cell is the harness's own interleaved Expanse-vs-Masstree
round loop, so the competitor arm rides along as a same-process control; the
figure this script publishes is Expanse's absolute insert and lookup rate per
variant, with a BCa 95% interval over rounds, never a ratio.

What each variant removes or moves (AGENTS.md section 6, one increment per
measurement):

- `lock-padded`: the writer mutex and the tree version word on their own
  cache lines (`sync::Line`), so a writer's version stores and a reader's
  version loads stop sharing a line with the lock and the collector handle.
- `advance-every-4096`: the reader-slot scan inside the critical section
  runs every 4096 writes instead of every 32.
- `advance-never`: no epoch advance on the write path at all. Sound only
  because these cells retire nothing that must be reclaimed within the run
  (fresh-key inserts; the bins grow, nothing is freed).

A variant that moves a cell by more than the two runs of the baseline moved
between themselves (BENCHMARKING rule 18) is evidence about that mechanism;
a variant that does not is a negative result and is published as one.

Usage (reference host, under the suite lock and pin — the campaign script
holds both):

    python3 docs/benchmarks/concurrency/scripts/ablations.py --out docs/benchmarks/concurrency/results/ablations.json
    python3 docs/benchmarks/concurrency/scripts/ablations.py --self-test
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci  # noqa: E402
from bench_provenance import add_load, host_facts, load_snapshot, git_sha  # noqa: E402

CRATE = REPO_ROOT / "crates" / "expanse-hot-bench" / "Cargo.toml"
VARIANTS = ("default", "lock-padded", "advance-every-4096", "advance-never")
CELLS = ((1, 0), (1, 8))
WORKLOAD_ID = "masstree_conc_map_64bit"


class Preflight(Exception):
    """A named infrastructure cause, with the fix. Always fatal."""


def target_dir(variant: str) -> Path:
    return CRATE.parent / f"target-abl-{variant}"


def build(variant: str, env: dict) -> Path:
    """Builds the throughput binary for `variant`; returns its path."""
    features = "masstree" if variant == "default" else f"masstree,{variant}"
    tdir = target_dir(variant)
    benv = dict(env)
    benv["CARGO_TARGET_DIR"] = str(tdir)
    args = ["cargo", "build", "--release", "--manifest-path", str(CRATE),
            "--features", features, "--bin", "masstree_concurrent"]
    print(f"building masstree_concurrent [{features}] into {tdir.name} ...")
    subprocess.run(args, check=True, env=benv)
    exe = tdir / "release" / "masstree_concurrent"
    if not exe.is_file():
        raise Preflight(f"{exe} was not produced")
    return exe


def run_cell(exe: Path, writers: int, readers: int, env: dict) -> list[dict]:
    proc = subprocess.run([str(exe), "map", str(writers), str(readers)],
                          capture_output=True, text=True, env=env, check=False)
    if proc.returncode != 0:
        raise Preflight(f"{exe.name} map {writers} {readers} exited {proc.returncode}: "
                        f"{proc.stderr.strip()[:600]}")
    rows = [json.loads(line) for line in proc.stdout.splitlines() if line.startswith("{")]
    rows = [r for r in rows if r.get("role") == "throughput"]
    if not rows:
        raise Preflight(f"{exe.name} map {writers} {readers} emitted no throughput rows")
    if any(r.get("workload_id") != WORKLOAD_ID for r in rows):
        raise Preflight(f"unexpected workload_id in rows (want {WORKLOAD_ID})")
    return rows


def summarise(variant: str, writers: int, readers: int, rows: list[dict], seed: int) -> dict:
    def interval(key: str) -> dict | None:
        vals = [r[key] for r in rows if r.get(key) is not None]
        if len(vals) < 3:
            return None
        mean, lo, hi = bca_bootstrap_ci(vals, seed=seed)
        return {"mean": mean, "ci_lower": lo, "ci_upper": hi, "n": len(vals)}

    return {
        "workload_id": WORKLOAD_ID,
        "variant": variant,
        "writers": writers,
        "readers": readers,
        "rounds": len(rows),
        "expanse_writer_mops": interval("expanse_writer_mops"),
        "expanse_reader_mops": interval("expanse_reader_mops"),
        "masstree_writer_mops": interval("masstree_writer_mops"),
        "masstree_reader_mops": interval("masstree_reader_mops"),
        "rounds_raw": rows,
    }


def run_all(env: dict, seed: int, quick: bool) -> dict:
    prov = {
        "suite": "concurrency",
        "issue": 568,
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "variants": list(VARIANTS),
        "estimators": {
            "point": "mean over rounds of the harness's per-round M ops/s",
            "interval": "BCa 95% over rounds, bca_bootstrap.py",
        },
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "loads": [load_snapshot("start")],
        "quick": quick,
    }
    variants = VARIANTS[:2] if quick else VARIANTS
    cells: list[dict] = []
    exes = {v: build(v, env) for v in variants}
    add_load(prov, "after-build")
    for v in variants:
        for w, r in CELLS:
            add_load(prov, f"cell:{v}:W{w}:R{r}")
            rows = run_cell(exes[v], w, r, env)
            cells.append(summarise(v, w, r, rows, seed))
            c = cells[-1]
            wi, ri = c["expanse_writer_mops"], c["expanse_reader_mops"]
            print(f"  {v:>18} W={w} R={r}: writer {wi['mean']:.2f} [{wi['ci_lower']:.2f}, {wi['ci_upper']:.2f}] M/s"
                  + (f"  readers {ri['mean']:.2f} [{ri['ci_lower']:.2f}, {ri['ci_upper']:.2f}] M/s" if ri else ""))
    add_load(prov, "end")
    return {"provenance": prov, "cells": cells}


def render(art: dict) -> str:
    out = ["| variant | W | R | Expanse inserts M/s [BCa 95%] | Expanse lookups M/s [BCa 95%] |",
           "|---|--:|--:|---|---|"]
    for c in art["cells"]:
        wi, ri = c["expanse_writer_mops"], c["expanse_reader_mops"]
        ws = f"{wi['mean']:.2f} [{wi['ci_lower']:.2f}, {wi['ci_upper']:.2f}]" if wi else "n/a"
        rs = f"{ri['mean']:.2f} [{ri['ci_lower']:.2f}, {ri['ci_upper']:.2f}]" if ri else "no readers"
        out.append(f"| `{c['variant']}` | {c['writers']} | {c['readers']} | {ws} | {rs} |")
    return "\n".join(out)


def self_test() -> int:
    rows = [{"workload_id": WORKLOAD_ID, "role": "throughput", "expanse_writer_mops": 5.0 + 0.1 * i,
             "expanse_reader_mops": None, "masstree_writer_mops": 4.0, "masstree_reader_mops": None}
            for i in range(5)]
    c = summarise("default", 1, 0, rows, seed=1)
    assert c["rounds"] == 5 and c["expanse_reader_mops"] is None
    w = c["expanse_writer_mops"]
    assert 5.19 < w["mean"] < 5.21 and w["ci_lower"] <= w["mean"] <= w["ci_upper"], w
    text = render({"cells": [c]})
    assert "| `default` | 1 | 0 |" in text and "no readers" in text, text
    # A row from another workload is refused before it can be summarised.
    try:
        bad = [dict(rows[0], workload_id="other")]
        if any(r.get("workload_id") != WORKLOAD_ID for r in bad):
            raise Preflight("unexpected workload_id")
        raise AssertionError("workload id check did not fire")
    except Preflight:
        pass
    print("ablations.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--quick", action="store_true", help="two variants, scratch output only")
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if platform.system() != "Linux":
        raise Preflight("the Masstree arm builds on x86-64 Linux with AVX2/BMI2 only; run this on the reference host")
    if args.quick and args.out and "results/quick" not in str(args.out) and "scratch" not in str(args.out):
        raise Preflight("--quick output must go to a gitignored scratch path (AGENTS.md section 8.5)")
    # The P-core pin, applied here and recorded in the artifact (AGENTS.md
    # §6.5: a directly-invoked wall-clock harness takes the pin itself).
    pin = bench_pin.apply("ablations")
    env = dict(os.environ)
    env["EXPANSE_BENCH_PIN_APPLIED"] = pin
    # The suites build the Masstree arm at -C target-cpu=haswell; match it so
    # the competitor control is the same code as the published cells.
    env["RUSTFLAGS"] = (env.get("RUSTFLAGS", "") + " -C target-cpu=haswell").strip()
    art = run_all(env, args.seed, args.quick)
    text = json.dumps(art, indent=2) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text)
        print(f"wrote {args.out} ({len(art['cells'])} cells)")
    else:
        sys.stdout.write(text)
    print(render(art))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Preflight as e:
        print(f"::error::ablations.py: {e}", file=sys.stderr)
        sys.exit(1)
