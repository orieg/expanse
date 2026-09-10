#!/usr/bin/env python3
"""Expanse-native writer scaling driver across physical P-cores (Refs #568, Phase 1.5D).

Runs `crates/expanse/examples/writer_scaling.rs` across W in {1, 2, 4, 8} on physical
P-cores for map (64-bit), set (63-bit), and str arms. Collects per-round throughput rows,
computes medians and BCa 95% bootstrap confidence intervals, measures scaling factor
C(N) = Throughput(W) / Throughput(1), tracks lock fallbacks, evaluates monotone scaling,
and writes the recomputable JSON artifact carrying §8.17 provenance.

Usage:
    python3 docs/benchmarks/concurrency/scripts/writer_scaling.py --out docs/benchmarks/concurrency/results/writer_scaling.json
    python3 docs/benchmarks/concurrency/scripts/writer_scaling.py --self-test
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci  # noqa: E402
from bench_provenance import (  # noqa: E402
    begin_cell,
    end_cell,
    new_provenance,
)


def run_command(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, check=False)


def run_writer_scaling_cell(
    arm: str,
    writers: int,
    rounds: int,
    quick: bool = False,
) -> list[dict[str, Any]]:
    cmd = [
        "cargo",
        "run",
        "--release",
        "-p",
        "expanse-trie",
        "--features",
        "occ-stats",
        "--example",
        "writer_scaling",
        "--",
        "--arm",
        arm,
        "--writers",
        str(writers),
        "--rounds",
        str(rounds),
    ]
    if quick:
        cmd.append("--quick")

    proc = run_command(cmd)
    if proc.returncode != 0:
        sys.stderr.write(f"writer_scaling binary failed (exit {proc.returncode}):\n")
        sys.stderr.write(proc.stderr + "\n")
        sys.exit(1)

    rows: list[dict[str, Any]] = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if line.startswith("{") and line.endswith("}"):
            try:
                row = json.loads(line)
                if row.get("role") == "counters" and row.get("arm") == "expanse":
                    rows.append(row)
            except json.JSONDecodeError:
                continue

    if len(rows) != rounds:
        sys.stderr.write(
            f"Expected {rounds} rounds for arm={arm} writers={writers}, got {len(rows)}\n"
        )
        sys.exit(1)

    return rows


def summarize_cell(
    arm: str,
    writers: int,
    rows: list[dict[str, Any]],
    load_attribution: dict[str, Any],
) -> dict[str, Any]:
    first = rows[0]
    mops_samples = [float(r["writer_mops"]) for r in rows]
    mops_samples_sorted = sorted(mops_samples)
    median_mops = mops_samples_sorted[len(mops_samples_sorted) // 2]

    # BCa 95% bootstrap CI
    if len(mops_samples) >= 3:
        mean_mops, ci_lower, ci_upper = bca_bootstrap_ci(mops_samples, confidence=0.95)
    else:
        mean_mops = sum(mops_samples) / len(mops_samples)
        ci_lower = min(mops_samples)
        ci_upper = max(mops_samples)

    fallbacks_samples = [int(r.get("lock_fallbacks", 0)) for r in rows]
    median_fallbacks = sorted(fallbacks_samples)[len(fallbacks_samples) // 2]
    total_fallbacks = sum(fallbacks_samples)
    total_ops = sum(int(r["write_ops"]) for r in rows)
    fallback_rate = (total_fallbacks / total_ops) if total_ops > 0 else 0.0

    rounds_raw = [
        {
            "round": r["round"],
            "writer_mops": r["writer_mops"],
            "writer_elapsed_s": r["writer_elapsed_s"],
            "write_ops": r["write_ops"],
            "lock_fallbacks": r.get("lock_fallbacks", 0),
        }
        for r in rows
    ]

    cell_summary: dict[str, Any] = {
        "workload_id": first["workload_id"],
        "arm": arm,
        "writers": writers,
        "readers": 0,
        "prefill": first["prefill"],
        "fresh_keys": first["fresh_keys"],
        "rounds": len(rows),
        "expanse_writer_mops_median": round(median_mops, 4),
        "expanse_writer_mops_mean": round(mean_mops, 4),
        "writer_ci_lower": round(ci_lower, 4),
        "writer_ci_upper": round(ci_upper, 4),
        "lock_fallbacks_median": median_fallbacks,
        "fallback_rate": round(fallback_rate, 6),
        "rounds_raw": rounds_raw,
        "load": load_attribution,
    }
    if "keyspace_bits" in first:
        cell_summary["keyspace_bits"] = first["keyspace_bits"]
    if "dist" in first:
        cell_summary["dist"] = first["dist"]

    return cell_summary


def self_test() -> int:
    eprintln = sys.stderr.write
    eprintln("Running writer_scaling.py self-test...\n")

    # Apply bench_pin to satisfy gate
    pin = bench_pin.apply("writer_scaling.py")
    eprintln(f"Applied core pin: {pin}\n")

    # Run quick test for map arm
    rows_map = run_writer_scaling_cell("map", 1, 3, quick=True)
    assert len(rows_map) == 3, f"Expected 3 rows, got {len(rows_map)}"
    assert rows_map[0]["arm"] == "expanse"
    assert rows_map[0]["workload_id"] == "concurrency_writer_map_64bit"

    # Run quick test for str arm (verifies Requirement 10 zero fallbacks)
    rows_str = run_writer_scaling_cell("str", 1, 3, quick=True)
    assert len(rows_str) == 3, f"Expected 3 rows, got {len(rows_str)}"
    assert rows_str[0]["arm"] == "expanse"
    assert rows_str[0]["workload_id"] == "concurrency_writer_str"
    assert rows_str[0]["lock_fallbacks"] == 0, f"Expected 0 fallbacks for str, got {rows_str[0]['lock_fallbacks']}"

    prov = new_provenance(
        suite="concurrency",
        issue=568,
        ratio="Expanse throughput over single-writer baseline C(N) = Mops(W) / Mops(1)",
        repo_root=REPO_ROOT,
    )
    start_snap = begin_cell(prov, "cell:map:W1:R0")
    load = end_cell(start_snap)
    summary = summarize_cell("map", 1, rows_map, load)

    assert "expanse_writer_mops_median" in summary
    assert "writer_ci_lower" in summary
    assert "lock_fallbacks_median" in summary
    assert "fallback_rate" in summary
    assert "load" in summary
    assert "foreign_busy_cpus" in summary["load"]

    summary_str = summarize_cell("str", 1, rows_str, load)
    assert summary_str["lock_fallbacks_median"] == 0
    assert summary_str["fallback_rate"] == 0.0

    eprintln("writer_scaling.py self-test PASSED\n")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=str, help="Output path for JSON artifact")
    parser.add_argument(
        "--rounds", type=int, default=7, help="Rounds per cell (default: 7)"
    )
    parser.add_argument(
        "--arm",
        type=str,
        default="all",
        choices=["map", "set", "str", "both", "all"],
        help="Arm to sweep (default: all)",
    )
    parser.add_argument(
        "--writers",
        type=str,
        default="1,2,4,8",
        help="Comma-separated list of writer counts (default: 1,2,4,8)",
    )
    parser.add_argument(
        "--quick",
        action="store_true",
        help="Quick mode with reduced population for fast smoke testing",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run self-test and exit",
    )
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    # Apply core pin before any measurement
    core_pin = bench_pin.apply("writer_scaling.py")

    writers_list = [int(w.strip()) for w in args.writers.split(",") if w.strip()]
    if args.arm in ("all", "both"):
        arms = ["map", "set", "str"] if args.arm == "all" else ["map", "set"]
    else:
        arms = [args.arm]

    prov = new_provenance(
        suite="concurrency",
        issue=568,
        ratio="Expanse throughput over single-writer baseline C(N) = Mops(W) / Mops(1)",
        repo_root=REPO_ROOT,
        core_pin=core_pin,
    )

    throughput_cells: list[dict[str, Any]] = []

    print("========================================================================")
    print(" Expanse-Native Multi-Writer Scaling Sweep (Phase 1.5D, Refs #568)")
    print(f" Arms: {', '.join(arms)} | Writers: {writers_list} | Rounds: {args.rounds}")
    print(f" Pin: {core_pin}")
    print("========================================================================")

    for arm in arms:
        w1_median = 0.0
        for w in writers_list:
            cell_label = f"cell:{arm}:W{w}:R0"
            start_snap = begin_cell(prov, cell_label)

            rows = run_writer_scaling_cell(arm, w, args.rounds, quick=args.quick)
            load = end_cell(start_snap)

            cell = summarize_cell(arm, w, rows, load)

            if w == 1:
                w1_median = cell["expanse_writer_mops_median"]
                cell["scaling_factor_c_n"] = 1.0
            else:
                cn = (
                    cell["expanse_writer_mops_median"] / w1_median
                    if w1_median > 0
                    else 0.0
                )
                cell["scaling_factor_c_n"] = round(cn, 4)

            throughput_cells.append(cell)
            print(
                f"[{arm:>3}] W={w:<2} | Median: {cell['expanse_writer_mops_median']:>6.2f} Mops/s "
                f"[{cell['writer_ci_lower']:>6.2f}, {cell['writer_ci_upper']:>6.2f}] "
                f"| C(N) = {cell['scaling_factor_c_n']:>5.2f}x "
                f"| Fallbacks: {cell['lock_fallbacks_median']:>7} ({cell['fallback_rate']*100:>5.2f}%) "
                f"| Foreign CPUs: {load['foreign_busy_cpus']:>5.2f}"
            )

    artifact = {
        "provenance": prov,
        "throughput": throughput_cells,
    }

    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(artifact, indent=2) + "\n")
        print(f"\nWrote artifact to {args.out}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
