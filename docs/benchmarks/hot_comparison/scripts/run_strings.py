#!/usr/bin/env python3
"""Runner for the string-key arms of the HOT comparison suite (#693).

Drives every string cell of METHODOLOGY §10, one process per cell (§9.2), and
harvests BCa 95% intervals for the wall-clock pillars.

What differs from ``run_all.py`` and why, all recorded in §10:

1. **The validation gate runs first and is fatal.** ``hot_string_validate``
   re-checks every silent-failure class the integer arms found (§10.8) on the
   string path, and measures HOT's 255-byte key window. Nothing is recorded if
   it fails (§8.1).

2. **The HOT column is a predicate, not a precondition** (§10.4). A cell whose
   population contains keys HOT cannot discriminate is still run — the Expanse
   side is never restricted — and its HOT column carries the finding
   (``NOT_REPRESENTABLE_HOT`` with the key count) instead of a number.

3. **Memory is a population sweep** with three columns per cell (§10.3): the
   index alone on both arms, the external key storage the harness owns, and
   ownership. String keys have no single density axis equivalent to λ; the
   candidate axis (occupancy of the discriminating chunk map) is a registered
   hypothesis the sweep is dense enough to test, not an assumption.
"""

import json
import os
import platform
import subprocess
import sys
import time
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"
CRATE = REPO_ROOT / "crates" / "expanse-hot-bench" / "Cargo.toml"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bca_bootstrap import bca_bootstrap_ratio_ci  # noqa: E402

# §10.5. The sweep is dense around N ≈ 1.23e5, where the registered chunk-map
# occupancy hypothesis places the LEAF_CAP cascade for the random shapes.
MEMORY_POPULATIONS = [1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000,
                      125_000, 150_000, 200_000, 500_000, 1_000_000]
LATENCY_POPULATIONS = [10_000, 100_000, 1_000_000]
SHAPES = ["short", "counter", "prefixed", "skewed", "beyond"]
SCAN_K = [10, 100, 1000]
# Arm C, Arm D, Arm E of §10.2. `bytes` (ExpanseBytesMap) is unordered and has
# no scan pillar.
ARMS = ["ptr", "map", "bytes"]
PILLARS = {
    "ptr": ["lookup_hit", "lookup_miss", "insert", "scan"],
    "map": ["lookup_hit", "lookup_miss", "insert", "scan"],
    "bytes": ["lookup_hit", "lookup_miss", "insert"],
}


def load_snapshot(label: str) -> dict:
    """Load average, before and between comparison runs (docs/BENCHMARKING.md rule 2)."""
    try:
        one, five, fifteen = os.getloadavg()
    except OSError:
        one = five = fifteen = float("nan")
    return {"label": label, "load1": round(one, 2), "load5": round(five, 2),
            "load15": round(fifteen, 2), "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}


def git_sha() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT
        ).decode().strip()
    except Exception:
        return "unknown"


def build(env: dict) -> None:
    """Builds the three string binaries once, at the ISA target §3.3 binds both arms to."""
    print("building hot_string_validate, hot_string_memory, hot_string_latency at -C target-cpu=haswell ...")
    subprocess.run(
        ["cargo", "build", "--release", "--manifest-path", str(CRATE),
         "--bin", "hot_string_validate", "--bin", "hot_string_memory", "--bin", "hot_string_latency"],
        check=True, env=env,
    )


def binary(name: str) -> Path:
    target = os.environ.get("CARGO_TARGET_DIR")
    root = Path(target) if target else (CRATE.parent / "target")
    return root / "release" / name


def run_cell(args: list, env: dict) -> list:
    """Runs one cell in its own process and returns its JSON lines."""
    proc = subprocess.run(args, capture_output=True, text=True, env=env)
    if proc.returncode != 0:
        raise RuntimeError(
            f"cell failed ({proc.returncode}): {' '.join(str(a) for a in args)}\n{proc.stderr.strip()}"
        )
    return [json.loads(line) for line in proc.stdout.splitlines() if line.startswith("{")]


def validate(env: dict) -> str:
    """The §10.8 gate. Fatal on failure; its transcript is kept with the results."""
    print("\n[0/3] hot_string_validate — the §10.8 gate")
    proc = subprocess.run([str(binary("hot_string_validate"))], capture_output=True, text=True, env=env)
    sys.stdout.write(proc.stdout)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit("hot_string_validate FAILED — no string cell is recorded (§8.1)")
    return proc.stdout


def sweep_memory(env: dict, quick: bool) -> dict:
    pops = MEMORY_POPULATIONS[:4] if quick else MEMORY_POPULATIONS
    shapes = ["short", "prefixed", "beyond"] if quick else SHAPES
    cells = []
    for arm in ARMS:
        for dist in shapes:
            for n in pops:
                rows = run_cell([str(binary("hot_string_memory")), arm, dist, str(n)], env)
                if len(rows) != 1:
                    raise RuntimeError(f"memory cell emitted {len(rows)} rows, expected 1")
                row = rows[0]
                cells.append(row)
                hot = row["hot_index_bytes_per_key"]
                hot_s = f"{hot:.2f}" if hot is not None else f"n/a ({row['hot_not_representable']} > window)"
                print(f"  memory {arm:>5} {dist:<9} N={n:<9} index: HOT {hot_s:>22}  "
                      f"Expanse {row['expanse_index_bytes_per_key']:.2f}  "
                      f"external {row['external_alloc_bytes_per_key']:.2f} B/key")
    return {"cells": cells}


def sweep_latency(env: dict, quick: bool) -> dict:
    pops = [10_000] if quick else LATENCY_POPULATIONS
    shapes = ["short", "prefixed", "beyond"] if quick else SHAPES
    cells = []
    for arm in ARMS:
        for pillar in PILLARS[arm]:
            for dist in shapes:
                for n in pops:
                    ks = SCAN_K if pillar == "scan" else [0]
                    for k in ks:
                        args = [str(binary("hot_string_latency")), arm, pillar, dist, str(n)]
                        if pillar == "scan":
                            args.append(str(k))
                        rows = run_cell(args, env)
                        head = rows[0]
                        exp = [r["expanse_ns_per_op"] for r in rows]
                        exp_med = round(sorted(exp)[len(exp) // 2], 4)
                        cell = {
                            "workload_id": head["workload_id"],
                            "pillar": pillar, "arm": arm, "dist": dist,
                            "population": head["population"],
                            "mean_key_len": head["mean_key_len"],
                            "hot_not_representable": head["hot_not_representable"],
                            "scan_k": k, "rounds": len(rows),
                            "expanse_ns_per_op_median": exp_med,
                        }
                        if head["hot_not_representable"] == 0:
                            hot = [r["hot_ns_per_op"] for r in rows]
                            # Ratio of two independently sampled means, gated on
                            # the CI lower bound (§8.4). Above 1.0 means Expanse
                            # is faster, since HOT is the numerator.
                            ratio, lo, hi = bca_bootstrap_ratio_ci(hot, exp, num_resamples=2000, seed=42)
                            cell.update({
                                "hot_ns_per_op_median": round(sorted(hot)[len(hot) // 2], 4),
                                "hot_over_expanse": round(ratio, 4),
                                "ci_lower": round(lo, 4), "ci_upper": round(hi, 4),
                                "verdict": ("BOUNDARY_RESULT" if lo <= 1.0 <= hi
                                            else ("expanse" if lo > 1.0 else "hot")),
                            })
                            print(f"  {pillar:<12} {arm:>5} {dist:<9} N={n:<8} k={k:<5} "
                                  f"ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}]")
                        else:
                            # §10.4: the finding goes in HOT's column, not a
                            # number over a smaller population.
                            cell.update({
                                "hot_ns_per_op_median": None,
                                "hot_over_expanse": None, "ci_lower": None, "ci_upper": None,
                                "verdict": "NOT_REPRESENTABLE_HOT",
                            })
                            print(f"  {pillar:<12} {arm:>5} {dist:<9} N={n:<8} k={k:<5} "
                                  f"Expanse {exp_med:.2f} ns/op; HOT: {head['hot_not_representable']} keys "
                                  f"exceed its window — column withheld")
                        cells.append(cell)
    return {"cells": cells}


def main() -> int:
    quick = "--quick" in sys.argv
    env = dict(os.environ)
    env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-cpu=haswell"

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR
    out_dir.mkdir(parents=True, exist_ok=True)
    if quick:
        print("QUICK MODE — reduced sweep, writing to gitignored results/quick/ (§8.5)")

    build(env)

    provenance = {
        "suite": "hot_comparison",
        "issue": 693,
        "commit": git_sha(),
        "hot_commit": "96bf6fb",
        "cpu": platform.processor() or platform.machine(),
        "platform": platform.platform(),
        "rustflags": env["RUSTFLAGS"].strip(),
        "cxx_flags": "-march=haswell -O3 -std=c++17 -DNDEBUG",
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "loads": [load_snapshot("start")],
        "quick": quick,
    }

    validate_log = validate(env)
    provenance["loads"].append(load_snapshot("after-validate"))

    print("\n[1/3] memory pillar — population sweep, three columns per cell (§10.3)")
    memory = sweep_memory(env, quick)
    provenance["loads"].append(load_snapshot("after-memory"))

    print("\n[2/3] latency pillars")
    latency = sweep_latency(env, quick)
    provenance["loads"].append(load_snapshot("end"))

    (out_dir / "baseline_string_memory.json").write_text(
        json.dumps({"provenance": provenance, **memory}, indent=2) + "\n")
    (out_dir / "baseline_string_latency.json").write_text(
        json.dumps({"provenance": provenance, **latency}, indent=2) + "\n")
    (out_dir / "string_validate.log").write_text(validate_log)

    loads = [snap["load1"] for snap in provenance["loads"]]
    print(f"\nload average across the run: {loads}")
    if max(loads) - min(loads) > 2.0:
        print("WARNING: load shifted by more than 2 during the run — "
              "the comparison is contaminated (docs/BENCHMARKING.md rule 2)")
    print(f"wrote {out_dir}/baseline_string_memory.json, baseline_string_latency.json and string_validate.log")
    return 0


if __name__ == "__main__":
    sys.exit(main())
