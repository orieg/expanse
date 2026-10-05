#!/usr/bin/env python3
"""Driver for the allocator_overhead benchmark suite (Issue #1108, Gate G1).

Runs `example_allocator_overhead` at N = 10^7 keys on `main` (base ref) and
candidate `head` (two runs each, pin stated, with load snapshots).
Evaluates Gate G1:
  1. Census RSS after `shrink_to_fit()` drops by >= 75% of released `mem_held`
     on random u64 at 10^7 (Clause 1).
  2. No shape's pre-shrink RSS is worse than main by > 1% (Clause 2, across all
     five shapes: sequential, timestamp, random, prefix, uuid).

Emits structured JSON artifact (`baseline-allocator_overhead.json`), markdown
report (`head-to-head.md`), and exits 0 on PASS or non-zero on FAIL.

Usage:
    python3 scripts/allocator_overhead_bench.py --base-ref main --head-ref HEAD --runs 2
    python3 scripts/allocator_overhead_bench.py --self-test
"""

from __future__ import annotations

import argparse
import copy
import json
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

ALL_SHAPES = ("sequential", "timestamp", "random", "prefix", "uuid")
DEFAULT_POPULATION = 10_000_000
G1_RANDOM_RECOVERY_FLOOR = 0.75
G1_PRE_SHRINK_MAX_RATIO = 1.01


def parse_allocator_table(output_text: str) -> Dict[str, Dict[str, Optional[float]]]:
    """Parses the table emitted by `allocator_overhead.rs`.

    Expected row format:
    shape mem_used mem_held request usable +headers RSS trimmed allocs/key held/shr RSS/shr free/shr
    """
    results: Dict[str, Dict[str, Optional[float]]] = {}
    lines = output_text.splitlines()

    for line in lines:
        parts = line.strip().split()
        if not parts:
            continue
        shape = parts[0]
        if shape not in ALL_SHAPES:
            continue

        # Expected at least 11 columns
        if len(parts) < 11:
            continue

        def parse_val(v: str) -> Optional[float]:
            if v == "n/a":
                return None
            try:
                return float(v)
            except ValueError as e:
                raise ValueError(
                    f"Unparseable numeric value {v!r} in shape row {shape}"
                ) from e

        # Columns:
        # 0: shape
        # 1: mem_used
        # 2: mem_held
        # 3: request
        # 4: usable
        # 5: +headers
        # 6: RSS
        # 7: trimmed
        # 8: allocs/key
        # 9: held/shr
        # 10: RSS/shr
        # 11: free/shr (optional)
        # 12: ret_os/shr (optional: bytes returned to OS per key from mem_returned_to_os())
        mem_used = parse_val(parts[1])
        mem_held = parse_val(parts[2])
        pre_shrink_rss = parse_val(parts[7])
        if pre_shrink_rss is None:
            pre_shrink_rss = parse_val(parts[6])  # fallback to untrimmed RSS
        held_shr = parse_val(parts[9])
        post_shrink_rss = parse_val(parts[10])

        delta_held: Optional[float] = None
        if mem_held is not None and held_shr is not None:
            delta_held = mem_held - held_shr

        delta_rss: Optional[float] = None
        if pre_shrink_rss is not None and post_shrink_rss is not None:
            delta_rss = pre_shrink_rss - post_shrink_rss

        ret_os: Optional[float] = parse_val(parts[12]) if len(parts) > 12 else None

        # Option 1C amended denominator: bytes returned to the OS (madvised pages + deallocated regions)
        # from `mem_returned_to_os()`. On base ref / main where region carving is absent, fallback to delta_held.
        released_bytes: Optional[float] = None
        if ret_os is not None and ret_os > 0:
            released_bytes = ret_os
        elif delta_held is not None and delta_held > 0:
            released_bytes = delta_held

        recovery_ratio: Optional[float] = None
        if delta_rss is not None and released_bytes is not None and released_bytes > 0:
            recovery_ratio = delta_rss / released_bytes

        results[shape] = {
            "mem_used": mem_used,
            "mem_held": mem_held,
            "pre_shrink_rss": pre_shrink_rss,
            "held_shr": held_shr,
            "delta_held": delta_held,
            "ret_os": ret_os,
            "released_bytes": released_bytes,
            "post_shrink_rss": post_shrink_rss,
            "delta_rss": delta_rss,
            "recovery_ratio": recovery_ratio,
        }

    return results


def run_command(
    cmd: List[str], cwd: Optional[Path] = None
) -> Tuple[int, str, str]:
    res = subprocess.run(
        cmd,
        cwd=cwd or REPO_ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return res.returncode, res.stdout, res.stderr


def build_binary(ref: str, out_path: Path) -> None:
    """Builds the allocator_overhead example at `ref` and copies binary to `out_path`."""
    orig_branch = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, text=True
    ).strip()

    try:
        subprocess.check_call(["git", "checkout", ref], cwd=REPO_ROOT)
        subprocess.check_call(
            ["cargo", "build", "--release", "-p", "expanse-trie", "--example", "allocator_overhead"],
            cwd=REPO_ROOT,
        )
        built_bin = REPO_ROOT / "target" / "release" / "examples" / "allocator_overhead"
        if not built_bin.exists():
            built_bin = REPO_ROOT / "target" / "release" / "allocator_overhead"
        if not built_bin.exists():
            raise FileNotFoundError(f"Binary not found after build: {built_bin}")
        shutil.copy2(built_bin, out_path)
    finally:
        subprocess.check_call(["git", "checkout", orig_branch], cwd=REPO_ROOT)


def execute_run(
    bin_path: Path, population: int
) -> Tuple[Dict[str, Dict[str, Optional[float]]], str, Dict[str, Any]]:
    """Runs binary with population and captures load snapshot."""
    import bench_provenance as prov

    load_before = prov.snapshot_system_load()
    rc, stdout, stderr = run_command([str(bin_path), str(population)])
    load_after = prov.snapshot_system_load()

    if rc != 0:
        raise RuntimeError(f"Binary {bin_path} exited with {rc}:\n{stderr}\n{stdout}")

    data = parse_allocator_table(stdout)
    load_info = {
        "load_before": load_before,
        "load_after": load_after,
    }
    return data, stdout, load_info


def evaluate_gate(
    base_runs: List[Dict[str, Dict[str, Optional[float]]]],
    head_runs: List[Dict[str, Dict[str, Optional[float]]]],
) -> Dict[str, Any]:
    """Evaluates Gate G1 against base runs with fail-closed and per-run checks.

    Enforces:
    - Fail-closed: Every shape in ALL_SHAPES must have valid pre- and post-shrink
      RSS in every run of both refs. Missing shapes or None RSS fail immediately.
    - Clause 1: Census RSS after shrink_to_fit() drops by >= 75% of released mem_held
      on random u64 at 10^7 in *both* head runs (mean is informational only).
    - Clause 2: No shape's pre-shrink RSS exceeds main by > 1% (ratio <= 1.0100)
      in *both* run pairs (mean is informational only).
    """
    failure_reasons: List[str] = []

    if not head_runs:
        failure_reasons.append("No head runs provided")
    if not base_runs:
        failure_reasons.append("No base runs provided; cannot evaluate Clause 2 against main")

    if head_runs and base_runs and len(head_runs) != len(base_runs):
        failure_reasons.append(
            f"Mismatched run counts: {len(head_runs)} head run(s) vs {len(base_runs)} base run(s)"
        )

    # Validate head runs: all shapes must be present and have valid RSS
    for r_idx, run in enumerate(head_runs):
        for shape in ALL_SHAPES:
            if shape not in run:
                failure_reasons.append(f"Head run {r_idx + 1}: missing shape '{shape}'")
                continue
            s_data = run[shape]
            pre_rss = s_data.get("pre_shrink_rss")
            post_rss = s_data.get("post_shrink_rss")
            if pre_rss is None or pre_rss <= 0:
                failure_reasons.append(
                    f"Head run {r_idx + 1}, shape '{shape}': invalid pre_shrink_rss ({pre_rss})"
                )
            if post_rss is None or post_rss <= 0:
                failure_reasons.append(
                    f"Head run {r_idx + 1}, shape '{shape}': invalid post_shrink_rss ({post_rss})"
                )
            if shape == "random":
                rel = s_data.get("released_bytes")
                if rel is None:
                    rel = s_data.get("delta_held")
                rec = s_data.get("recovery_ratio")
                if rel is None or rel <= 0:
                    failure_reasons.append(
                        f"Head run {r_idx + 1}, shape 'random': invalid released_bytes ({rel})"
                    )
                if rec is None:
                    failure_reasons.append(
                        f"Head run {r_idx + 1}, shape 'random': recovery_ratio is None"
                    )

    # Validate base runs: all shapes must be present and have valid RSS
    for r_idx, run in enumerate(base_runs):
        for shape in ALL_SHAPES:
            if shape not in run:
                failure_reasons.append(f"Base run {r_idx + 1}: missing shape '{shape}'")
                continue
            s_data = run[shape]
            pre_rss = s_data.get("pre_shrink_rss")
            post_rss = s_data.get("post_shrink_rss")
            if pre_rss is None or pre_rss <= 0:
                failure_reasons.append(
                    f"Base run {r_idx + 1}, shape '{shape}': invalid pre_shrink_rss ({pre_rss})"
                )
            if post_rss is None or post_rss <= 0:
                failure_reasons.append(
                    f"Base run {r_idx + 1}, shape '{shape}': invalid post_shrink_rss ({post_rss})"
                )

    # Evaluate Clause 1 per-run: random recovery >= 0.75 in every head run
    clause1_per_run: List[Optional[float]] = []
    clause1_pass_per_run: List[bool] = []
    for r in head_runs:
        rec = r.get("random", {}).get("recovery_ratio")
        passed = rec is not None and rec >= G1_RANDOM_RECOVERY_FLOOR
        clause1_per_run.append(rec)
        clause1_pass_per_run.append(passed)

    clause1_pass = len(clause1_pass_per_run) > 0 and all(clause1_pass_per_run)
    valid_rec_vals = [r for r in clause1_per_run if r is not None]
    clause1_mean = sum(valid_rec_vals) / len(valid_rec_vals) if valid_rec_vals else None

    # Evaluate Clause 2 per-run pair: ratio <= 1.01 for all shapes in all pairs
    ratio_per_run: Dict[str, List[Optional[float]]] = {s: [] for s in ALL_SHAPES}
    pass_per_run: Dict[str, List[bool]] = {s: [] for s in ALL_SHAPES}
    num_pairs = min(len(head_runs), len(base_runs))

    for i in range(num_pairs):
        for shape in ALL_SHAPES:
            h_pre = head_runs[i].get(shape, {}).get("pre_shrink_rss")
            b_pre = base_runs[i].get(shape, {}).get("pre_shrink_rss")
            if h_pre is not None and b_pre is not None and b_pre > 0:
                ratio = h_pre / b_pre
                passed = ratio <= G1_PRE_SHRINK_MAX_RATIO
                ratio_per_run[shape].append(ratio)
                pass_per_run[shape].append(passed)
            else:
                ratio_per_run[shape].append(None)
                pass_per_run[shape].append(False)

    clause2_pass = (
        num_pairs > 0
        and all(all(pass_per_run[s]) for s in ALL_SHAPES if pass_per_run[s])
        and all(len(pass_per_run[s]) == num_pairs for s in ALL_SHAPES)
    )

    all_ratios = [
        r for shape in ALL_SHAPES for r in ratio_per_run[shape] if r is not None
    ]
    max_pre_shrink_ratio = max(all_ratios) if all_ratios else None

    # Compute shape summaries
    shape_comparisons: Dict[str, Dict[str, Any]] = {}
    for shape in ALL_SHAPES:
        b_pre_runs = [
            r[shape]["pre_shrink_rss"]
            for r in base_runs
            if shape in r and r[shape].get("pre_shrink_rss") is not None
        ]
        h_pre_runs = [
            r[shape]["pre_shrink_rss"]
            for r in head_runs
            if shape in r and r[shape].get("pre_shrink_rss") is not None
        ]
        b_pre_mean = sum(b_pre_runs) / len(b_pre_runs) if b_pre_runs else None
        h_pre_mean = sum(h_pre_runs) / len(h_pre_runs) if h_pre_runs else None

        ratio_runs = ratio_per_run.get(shape, [])
        ratio_valid = [r for r in ratio_runs if r is not None]
        ratio_mean = sum(ratio_valid) / len(ratio_valid) if ratio_valid else None
        shape_clause2_pass = (
            all(pass_per_run.get(shape, [])) if pass_per_run.get(shape) else False
        )

        dh_runs = [
            r[shape]["delta_held"]
            for r in head_runs
            if shape in r and r[shape].get("delta_held") is not None
        ]
        dr_runs = [
            r[shape]["delta_rss"]
            for r in head_runs
            if shape in r and r[shape].get("delta_rss") is not None
        ]
        rel_runs = [
            r[shape]["released_bytes"]
            for r in head_runs
            if shape in r and r[shape].get("released_bytes") is not None
        ]
        rec_runs = [
            r[shape]["recovery_ratio"]
            for r in head_runs
            if shape in r and r[shape].get("recovery_ratio") is not None
        ]
        dh_mean = sum(dh_runs) / len(dh_runs) if dh_runs else None
        dr_mean = sum(dr_runs) / len(dr_runs) if dr_runs else None
        rel_mean = sum(rel_runs) / len(rel_runs) if rel_runs else None
        rec_mean = sum(rec_runs) / len(rec_runs) if rec_runs else None
        recovery_pass = clause1_pass if shape == "random" else None

        shape_comparisons[shape] = {
            "base_pre_shrink_rss_per_run": b_pre_runs,
            "head_pre_shrink_rss_per_run": h_pre_runs,
            "pre_shrink_ratio_per_run": ratio_runs,
            "pre_shrink_pass_per_run": pass_per_run.get(shape, []),
            "base_pre_shrink_rss": b_pre_mean,
            "head_pre_shrink_rss": h_pre_mean,
            "pre_shrink_ratio": ratio_mean,
            "pre_shrink_pass": shape_clause2_pass,
            "head_delta_held_per_run": dh_runs,
            "head_delta_rss_per_run": dr_runs,
            "head_released_bytes_per_run": rel_runs,
            "head_recovery_ratio_per_run": rec_runs,
            "head_delta_held": dh_mean,
            "head_delta_rss": dr_mean,
            "head_released_bytes": rel_mean,
            "head_recovery_ratio": rec_mean,
            "recovery_pass": recovery_pass,
        }

    overall_pass = clause1_pass and clause2_pass and len(failure_reasons) == 0

    return {
        "shapes": shape_comparisons,
        "clause1_recovery_per_run": clause1_per_run,
        "clause1_pass_per_run": clause1_pass_per_run,
        "clause1_mean": clause1_mean,
        "clause1_floor": G1_RANDOM_RECOVERY_FLOOR,
        "clause1_pass": clause1_pass,
        "clause2_ratio_per_run": ratio_per_run,
        "clause2_pass_per_run": pass_per_run,
        "clause2_max_ratio": max_pre_shrink_ratio,
        "clause2_ceiling": G1_PRE_SHRINK_MAX_RATIO,
        "clause2_pass": clause2_pass,
        "g1_random_recovery_ratio": clause1_mean,
        "g1_random_recovery_floor": G1_RANDOM_RECOVERY_FLOOR,
        "g1_random_recovery_pass": clause1_pass,
        "g1_max_pre_shrink_ratio": max_pre_shrink_ratio,
        "g1_max_pre_shrink_ceiling": G1_PRE_SHRINK_MAX_RATIO,
        "g1_pre_shrink_pass": clause2_pass,
        "failure_reasons": failure_reasons,
        "verdict": "PASS" if overall_pass else "FAIL",
    }


def format_per_run(
    per_run: List[Optional[float]],
    mean_val: Optional[float],
    fmt: str = ".2f",
    suffix: str = "",
) -> str:
    clean_run = [v for v in per_run if v is not None]
    if not clean_run:
        return f"{mean_val:{fmt}}{suffix}" if mean_val is not None else "n/a"
    if len(clean_run) == 1:
        return f"{clean_run[0]:{fmt}}{suffix}"
    parts = [f"{v:{fmt}}{suffix}" for v in clean_run]
    mean_str = f" (mean {mean_val:{fmt}}{suffix})" if mean_val is not None else ""
    return f"{', '.join(parts)}{mean_str}"


def render_markdown(
    eval_result: Dict[str, Any],
    base_ref: Optional[str],
    head_ref: str,
    population: int,
    runs: int,
) -> str:
    c1_runs = [
        v * 100.0
        for v in eval_result.get("clause1_recovery_per_run", [])
        if v is not None
    ]
    c1_mean = eval_result.get("clause1_mean")
    c1_mean_pct = c1_mean * 100.0 if c1_mean is not None else None
    c1_formatted = format_per_run(c1_runs, c1_mean_pct, fmt=".1f", suffix="%")
    if eval_result.get("clause1_pass"):
        c1_summary = f"✅ PASS ({c1_formatted} >= {G1_RANDOM_RECOVERY_FLOOR * 100.0:.1f}%)"
    else:
        c1_summary = f"❌ FAIL ({c1_formatted} < {G1_RANDOM_RECOVERY_FLOOR * 100.0:.1f}%)"

    max_r = eval_result.get("clause2_max_ratio")
    max_r_str = f"{max_r:.4f}x" if max_r is not None else "n/a"
    if eval_result.get("clause2_pass"):
        c2_summary = f"✅ PASS (max ratio {max_r_str} <= {G1_PRE_SHRINK_MAX_RATIO:.4f}x)"
    else:
        c2_summary = f"❌ FAIL (max ratio {max_r_str} > {G1_PRE_SHRINK_MAX_RATIO:.4f}x)"

    lines = [
        "### Gate G1: Allocator Overhead & RSS Recovery Census (`example_allocator_overhead`)",
        "",
        f"- **Population**: {population:,} keys per shape (8-byte values)",
        f"- **Runs**: {runs} runs on Base ({base_ref or 'none'}) and Head ({head_ref})",
        f"- **Overall Verdict**: **{eval_result['verdict']}**",
        f"- **Clause 1 (random u64 RSS recovery >= 75.0% in all head runs)**: {c1_summary}",
        f"- **Clause 2 (pre-shrink RSS <= 1.0100x main for all shapes in all run pairs)**: {c2_summary}",
    ]

    reasons = eval_result.get("failure_reasons", [])
    if reasons:
        lines.append("- **Failure Reasons / Invariant Violations**:")
        for r in reasons:
            lines.append(f"  - ❌ {r}")

    lines.append("")
    lines.append(
        "| Shape | Base Pre-Shrink RSS | Head Pre-Shrink RSS | RSS Ratio (Head/Base) | Released OS/held (B/key) | Released RSS (B/key) | Recovery % (ΔRSS/ΔOS) | Clause 1 (>=75%) | Clause 2 (<=1.01x) |"
    )
    lines.append(
        "|:---|---:|---:|---:|---:|---:|---:|:---:|:---:|",
    )

    for shape in ALL_SHAPES:
        s = eval_result["shapes"].get(shape, {})
        b_rss_str = format_per_run(
            s.get("base_pre_shrink_rss_per_run", []), s.get("base_pre_shrink_rss")
        )
        h_rss_str = format_per_run(
            s.get("head_pre_shrink_rss_per_run", []), s.get("head_pre_shrink_rss")
        )
        ratio_str = format_per_run(
            s.get("pre_shrink_ratio_per_run", []),
            s.get("pre_shrink_ratio"),
            fmt=".4f",
            suffix="x",
        )
        rel_runs_to_show = s.get("head_released_bytes_per_run") or s.get("head_delta_held_per_run", [])
        rel_mean_to_show = (
            s.get("head_released_bytes")
            if s.get("head_released_bytes") is not None
            else s.get("head_delta_held")
        )
        d_held_str = format_per_run(
            rel_runs_to_show, rel_mean_to_show
        )
        d_rss_str = format_per_run(
            s.get("head_delta_rss_per_run", []), s.get("head_delta_rss")
        )

        rec_runs = [
            v * 100.0
            for v in s.get("head_recovery_ratio_per_run", [])
            if v is not None
        ]
        rec_mean = s.get("head_recovery_ratio")
        rec_mean_pct = rec_mean * 100.0 if rec_mean is not None else None
        rec_str = format_per_run(rec_runs, rec_mean_pct, fmt=".1f", suffix="%")

        c1_str = "—"
        if shape == "random":
            c1_str = "✅ PASS" if eval_result.get("clause1_pass") else "❌ FAIL"

        c2_str = "—"
        if s.get("pre_shrink_pass") is not None:
            c2_str = "✅ PASS" if s.get("pre_shrink_pass") else "❌ FAIL"

        lines.append(
            f"| `{shape}` | {b_rss_str} | {h_rss_str} | {ratio_str} | {d_held_str} | {d_rss_str} | {rec_str} | {c1_str} | {c2_str} |"
        )

    lines.append("")
    lines.append(
        "> **Gate Criteria**: Clause 1 requires random u64 RSS recovery >= 75.0% of released memory (OS bytes returned via madvise or region deallocation; fallback to released held memory) in **all head runs** (mean is informational only). "
        "Clause 2 requires no shape pre-shrink RSS to exceed main by > 1.0% (ratio <= 1.0100) in **all run pairs** (mean is informational only)."
    )
    return "\n".join(lines)


def self_test() -> int:
    """Validates table parsing, calculation accuracy, fail-closed handling, and per-run checks."""
    mock_base_output = """
allocator census, N = 10000000 keys per shape, 8-byte values (bytes/key)
shape        mem_used  mem_held   request    usable  +headers       RSS   trimmed allocs/key  held/shr   RSS/shr  free/shr
sequential       8.56      8.59      8.59      8.84      9.09      9.10      9.12     0.0314      8.58      9.12      0.01
timestamp       20.92     21.47     21.47     21.52     21.56     21.82     21.82     0.0055     21.47     21.83      0.25
random          22.69     32.70     32.70     33.05     33.41     33.70     33.72     0.0443     28.39     32.24      4.61
prefix          38.76     40.40     40.36     42.25     47.21     47.24     47.24     0.6193     39.14     47.24      1.27
uuid            60.86     66.81     66.24     78.50     86.93     87.12     87.13     1.0536     65.71     87.13      1.28
"""
    # Mock candidate output passing Gate G1:
    # random: trimmed = 33.72, held = 32.70, held/shr = 28.39 (delta held = 4.31).
    # RSS/shr = 29.80 -> delta RSS = 33.72 - 29.80 = 3.92 -> recovery = 3.92 / 4.31 = 90.95% >= 75%.
    # All shapes pre-shrink RSS equal to base (ratio = 1.000 <= 1.01).
    mock_head_output_pass = """
allocator census, N = 10000000 keys per shape, 8-byte values (bytes/key)
shape        mem_used  mem_held   request    usable  +headers       RSS   trimmed allocs/key  held/shr   RSS/shr  free/shr
sequential       8.56      8.59      8.59      8.84      9.09      9.10      9.12     0.0314      8.58      9.12      0.01
timestamp       20.92     21.47     21.47     21.52     21.56     21.82     21.82     0.0055     21.47     21.83      0.25
random          22.69     32.70     32.70     33.05     33.41     33.70     33.72     0.0443     28.39     29.80      4.61
prefix          38.76     40.40     40.36     42.25     47.21     47.24     47.24     0.6193     39.14     47.24      1.27
uuid            60.86     66.81     66.24     78.50     86.93     87.12     87.13     1.0536     65.71     87.13      1.28
"""
    base_data = parse_allocator_table(mock_base_output)
    head_data_pass = parse_allocator_table(mock_head_output_pass)

    assert "random" in base_data, "random shape must be parsed"
    assert base_data["random"]["pre_shrink_rss"] == 33.72
    assert base_data["random"]["held_shr"] == 28.39
    assert abs(base_data["random"]["delta_held"] - 4.31) < 1e-4

    # 1. Clean PASS fixture (2 runs each)
    eval_pass = evaluate_gate([base_data, base_data], [head_data_pass, head_data_pass])
    assert eval_pass["verdict"] == "PASS", f"expected PASS, got {eval_pass['verdict']}"
    assert eval_pass["clause1_pass"] is True
    assert eval_pass["clause2_pass"] is True
    assert eval_pass["g1_random_recovery_pass"] is True
    assert eval_pass["g1_pre_shrink_pass"] is True
    assert eval_pass["clause1_mean"] is not None and eval_pass["clause1_mean"] > 0.90
    assert len(eval_pass["failure_reasons"]) == 0

    # 2. Fail-closed: missing shape in head run
    head_missing = copy.deepcopy(head_data_pass)
    del head_missing["uuid"]
    eval_missing_head = evaluate_gate([base_data, base_data], [head_data_pass, head_missing])
    assert eval_missing_head["verdict"] == "FAIL", "Missing shape in head must produce FAIL"
    assert any("missing shape 'uuid'" in r for r in eval_missing_head["failure_reasons"])

    # 2b. Fail-closed: no base runs at all. This is the state a base ref that
    # failed to build leaves behind (the build failure is only a warning), so
    # the gate must not pass on head runs alone.
    eval_no_base = evaluate_gate([], [head_data_pass, head_data_pass])
    assert eval_no_base["verdict"] == "FAIL", "No base runs must produce FAIL"
    assert any("No base runs provided" in r for r in eval_no_base["failure_reasons"])

    # 3. Fail-closed: missing shape in base run
    base_missing = copy.deepcopy(base_data)
    del base_missing["prefix"]
    eval_missing_base = evaluate_gate([base_missing, base_data], [head_data_pass, head_data_pass])
    assert eval_missing_base["verdict"] == "FAIL", "Missing shape in base must produce FAIL"
    assert any("missing shape 'prefix'" in r for r in eval_missing_base["failure_reasons"])

    # 4. Fail-closed: None RSS in head run (e.g. non-Linux /proc/self/statm unavailable)
    head_none_rss = copy.deepcopy(head_data_pass)
    head_none_rss["random"]["pre_shrink_rss"] = None
    eval_none_head = evaluate_gate([base_data, base_data], [head_data_pass, head_none_rss])
    assert eval_none_head["verdict"] == "FAIL", "None RSS in head must produce FAIL"
    assert any("invalid pre_shrink_rss" in r for r in eval_none_head["failure_reasons"])

    # 5. Fail-closed: None RSS in base run
    base_none_rss = copy.deepcopy(base_data)
    base_none_rss["sequential"]["post_shrink_rss"] = None
    eval_none_base = evaluate_gate([base_data, base_none_rss], [head_data_pass, head_data_pass])
    assert eval_none_base["verdict"] == "FAIL", "None post-shrink RSS in base must produce FAIL"
    assert any("invalid post_shrink_rss" in r for r in eval_none_base["failure_reasons"])

    # 6. Averaging hides failing run: Clause 1 mean passes (0.775) but run 2 fails (0.70)
    head_c1_run1 = copy.deepcopy(head_data_pass)
    head_c1_run1["random"]["recovery_ratio"] = 0.85
    head_c1_run2 = copy.deepcopy(head_data_pass)
    head_c1_run2["random"]["recovery_ratio"] = 0.70
    eval_c1_split = evaluate_gate([base_data, base_data], [head_c1_run1, head_c1_run2])
    assert eval_c1_split["clause1_mean"] is not None and eval_c1_split["clause1_mean"] > 0.75, (
        f"mean {eval_c1_split['clause1_mean']} must exceed floor 0.75"
    )
    assert eval_c1_split["clause1_pass"] is False, "Clause 1 must FAIL when run 2 (0.70) < 0.75"
    assert eval_c1_split["verdict"] == "FAIL", "Overall verdict must be FAIL"

    # 7. Averaging hides failing run: Clause 2 mean passes (1.0100) but run 2 fails (1.0150)
    head_c2_run1 = copy.deepcopy(head_data_pass)
    head_c2_run1["sequential"]["pre_shrink_rss"] = 9.12 * 1.005
    head_c2_run2 = copy.deepcopy(head_data_pass)
    head_c2_run2["sequential"]["pre_shrink_rss"] = 9.12 * 1.015
    eval_c2_split = evaluate_gate([base_data, base_data], [head_c2_run1, head_c2_run2])
    assert eval_c2_split["shapes"]["sequential"]["pre_shrink_ratio"] is not None
    assert eval_c2_split["shapes"]["sequential"]["pre_shrink_ratio"] <= 1.01001, (
        f"mean ratio {eval_c2_split['shapes']['sequential']['pre_shrink_ratio']} must be <= 1.01"
    )
    assert eval_c2_split["clause2_pass"] is False, "Clause 2 must FAIL when run 2 ratio (1.015) > 1.01"
    assert eval_c2_split["shapes"]["sequential"]["pre_shrink_pass"] is False
    assert eval_c2_split["verdict"] == "FAIL", "Overall verdict must be FAIL"

    # 8. Clause 1 fails in both runs (recovery < 75%)
    head_data_fail_c1 = copy.deepcopy(head_data_pass)
    head_data_fail_c1["random"]["post_shrink_rss"] = 32.24
    head_data_fail_c1["random"]["delta_rss"] = 33.72 - 32.24
    head_data_fail_c1["random"]["recovery_ratio"] = 1.48 / 4.31
    eval_fail_c1 = evaluate_gate([base_data, base_data], [head_data_fail_c1, head_data_fail_c1])
    assert eval_fail_c1["verdict"] == "FAIL"
    assert eval_fail_c1["clause1_pass"] is False

    # 9. Clause 2 fails in both runs (sequential ratio > 1.01)
    head_data_fail_c2 = copy.deepcopy(head_data_pass)
    head_data_fail_c2["sequential"]["pre_shrink_rss"] = 9.30
    eval_fail_c2 = evaluate_gate([base_data, base_data], [head_data_fail_c2, head_data_fail_c2])
    assert eval_fail_c2["verdict"] == "FAIL"
    assert eval_fail_c2["clause2_pass"] is False

    # 10. Option 1C: Region carving where delta_held == 0 but ret_os > 0
    mock_head_output_ret_os = """
allocator census, N = 10000000 keys per shape, 8-byte values (bytes/key)
shape        mem_used  mem_held   request    usable  +headers       RSS   trimmed allocs/key  held/shr   RSS/shr  free/shr ret_os/shr
sequential       8.56      8.59      8.59      8.84      9.09      9.10      9.12     0.0314      8.59      9.12      0.01       0.00
timestamp       20.92     21.47     21.47     21.52     21.56     21.82     21.82     0.0055     21.47     21.83      0.25       0.00
random          22.69     32.70     32.70     33.05     33.41     33.70     33.72     0.0443     32.70     29.80      0.00       4.31
prefix          38.76     40.40     40.36     42.25     47.21     47.24     47.24     0.6193     40.40     47.24      0.00       0.00
uuid            60.86     66.81     66.24     78.50     86.93     87.12     87.13     1.0536     66.81     87.13      0.00       0.00
"""
    head_data_ret_os = parse_allocator_table(mock_head_output_ret_os)
    assert head_data_ret_os["random"]["delta_held"] == 0.0
    assert head_data_ret_os["random"]["ret_os"] == 4.31
    assert head_data_ret_os["random"]["released_bytes"] == 4.31
    assert head_data_ret_os["random"]["recovery_ratio"] is not None
    assert abs(head_data_ret_os["random"]["recovery_ratio"] - (3.92 / 4.31)) < 1e-4

    # 10a. Option 1C clean PASS under region carving
    eval_ret_os_pass = evaluate_gate([base_data, base_data], [head_data_ret_os, head_data_ret_os])
    assert eval_ret_os_pass["verdict"] == "PASS", f"Option 1C expected PASS, got {eval_ret_os_pass['verdict']}"
    assert eval_ret_os_pass["clause1_pass"] is True
    assert eval_ret_os_pass["clause2_pass"] is True

    # 10b. Discrimination: zero delta_held WITHOUT ret_os must FAIL Clause 1 (old denominator would fail)
    head_data_no_ret_os = copy.deepcopy(head_data_ret_os)
    head_data_no_ret_os["random"]["ret_os"] = None
    head_data_no_ret_os["random"]["released_bytes"] = None
    head_data_no_ret_os["random"]["recovery_ratio"] = None
    eval_no_ret_os = evaluate_gate([base_data, base_data], [head_data_no_ret_os, head_data_no_ret_os])
    assert eval_no_ret_os["verdict"] == "FAIL", "Zero delta_held without ret_os must FAIL"
    assert eval_no_ret_os["clause1_pass"] is False
    assert any("invalid released_bytes" in r or "recovery_ratio is None" in r for r in eval_no_ret_os["failure_reasons"])

    # 10c. Discrimination: ret_os present but recovery < 75% must FAIL Clause 1
    head_data_ret_os_low = copy.deepcopy(head_data_ret_os)
    head_data_ret_os_low["random"]["post_shrink_rss"] = 32.00
    head_data_ret_os_low["random"]["delta_rss"] = 33.72 - 32.00  # 1.72
    head_data_ret_os_low["random"]["recovery_ratio"] = 1.72 / 4.31  # ~39.9% < 75%
    eval_ret_os_low = evaluate_gate([base_data, base_data], [head_data_ret_os_low, head_data_ret_os_low])
    assert eval_ret_os_low["verdict"] == "FAIL", "ret_os recovery < 75% must FAIL"
    assert eval_ret_os_low["clause1_pass"] is False

    md = render_markdown(eval_pass, "main", "candidate", 10_000_000, 2)
    assert "Gate G1: Allocator Overhead & RSS Recovery Census" in md
    assert "✅ PASS" in md

    print("allocator_overhead_bench.py self-test: all checks passed.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-ref", default=None, help="Base Git ref to compare against")
    parser.add_argument("--head-ref", default="HEAD", help="Head Git ref (candidate)")
    parser.add_argument("--population", type=int, default=DEFAULT_POPULATION, help="Keys per shape")
    parser.add_argument("--runs", type=int, default=2, help="Rounds per ref")
    parser.add_argument("--out", default="baseline-allocator_overhead.json", help="Output JSON artifact path")
    parser.add_argument("--md-out", default="head-to-head.md", help="Output markdown table path")
    parser.add_argument("--host-desc", default=None, help="Hardware description")
    parser.add_argument("--run-id", default=None, help="Run URL / ID")
    parser.add_argument("--base-log", default=None, help="Pre-captured log file for base")
    parser.add_argument("--head-log", default=None, help="Pre-captured log file for head")
    parser.add_argument("--self-test", action="store_true", help="Run unit tests and exit")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    import bench_pin

    pin = bench_pin.apply("allocator_overhead_bench.py")

    tmp_dir = Path(tempfile.mkdtemp(prefix="alloc_overhead_bench_"))
    base_runs: List[Dict[str, Dict[str, Optional[float]]]] = []
    head_runs: List[Dict[str, Dict[str, Optional[float]]]] = []
    raw_logs: List[str] = []

    try:
        # Pre-captured logs mode
        if args.base_log and Path(args.base_log).exists():
            with open(args.base_log, "r") as f:
                base_runs.append(parse_allocator_table(f.read()))

        if args.head_log and Path(args.head_log).exists():
            with open(args.head_log, "r") as f:
                head_runs.append(parse_allocator_table(f.read()))

        # Live execution mode if logs not provided
        if not head_runs:
            head_bin = tmp_dir / "allocator_overhead_head"
            print(f"Building candidate head ref: {args.head_ref}...", flush=True)
            build_binary(args.head_ref, head_bin)

            base_bin: Optional[Path] = None
            if args.base_ref and not base_runs:
                base_bin = tmp_dir / "allocator_overhead_base"
                print(f"Building base ref: {args.base_ref}...", flush=True)
                try:
                    build_binary(args.base_ref, base_bin)
                except Exception as e:
                    print(f"Warning: failed to build base ref {args.base_ref}: {e}", file=sys.stderr)
                    base_bin = None

            # Execute runs
            for r in range(1, args.runs + 1):
                if base_bin is not None:
                    print(f"Executing Base run {r}/{args.runs}...", flush=True)
                    b_data, b_stdout, _ = execute_run(base_bin, args.population)
                    base_runs.append(b_data)
                    raw_logs.append(f"=== Base Run {r} ===\n" + b_stdout)

                print(f"Executing Head run {r}/{args.runs}...", flush=True)
                h_data, h_stdout, _ = execute_run(head_bin, args.population)
                head_runs.append(h_data)
                raw_logs.append(f"=== Head Run {r} ===\n" + h_stdout)

        eval_result = evaluate_gate(base_runs, head_runs)
        eval_result["metadata"] = {
            "suite": "allocator_overhead",
            "host_desc": args.host_desc,
            "run_id": args.run_id,
            "pin": pin,
            "base_ref": args.base_ref,
            "head_ref": args.head_ref,
            "population": args.population,
            "runs": args.runs,
            "timestamp": time.time(),
        }

        # Write JSON
        with open(args.out, "w") as f:
            json.dump(eval_result, f, indent=2)
        print(f"Wrote JSON artifact to {args.out}", flush=True)

        # Write Markdown
        md_text = render_markdown(
            eval_result,
            args.base_ref,
            args.head_ref,
            args.population,
            args.runs,
        )
        with open(args.md_out, "w") as f:
            f.write(md_text + "\n")
        print(f"Wrote Markdown summary to {args.md_out}", flush=True)
        print("\n" + md_text + "\n")

        return 0 if eval_result["verdict"] == "PASS" else 1

    finally:
        shutil.rmtree(tmp_dir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
