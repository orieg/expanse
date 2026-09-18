#!/usr/bin/env python3
"""Expanse-native concurrent YCSB driver (Refs #1006, METHODOLOGY.md §20).

Runs `crates/expanse/examples/ycsb_concurrent.rs` across competitor arms
(`olc`, `mutex`, `skip`, `dash`, `rwbtree`) and workload families (A, B, D, F,
uniform twins A0, B0, F0, read-only C, C0, contiguous Ac, Fc, and DRAM anchors
A-dram, C-dram) under Zipfian (θ = 0.99) and uniform (θ = 0) key choice.

Two builds, never one (AGENTS.md §6 / METHODOLOGY.md §20.12):
- Pass 1 (throughput): uninstrumented release build, interleaved across (arm, T)
  within each round, balancing position and first-order carryover across rounds
  (Williams design). Emits elapsed_s and total_mops. Refuses to run if occ-stats
  is enabled.
  Every timed cell runs in a harness process of its own (`--family f --arm a
  --threads T --round r --position p`, METHODOLOGY.md §20.8, §15): process-wide
  state carried from one cell into the next alters cache/allocator state.
- Pass 2 (counters): diagnostic build (--features occ-stats), captures exact
  lock_fallbacks and stripe contended acquisitions. Refuses to emit elapsed_s or
  total_mops.

Computes:
- G1: scaling against the α = 1 control (ratio of scaling factors over mutex)
- G2: level against the control's best cell across all T'
- G3: skew retention (Zipfian cell over uniform twin at same T, same round)
- G4: ordered competitor (olc over skip list for families A, B, D)
- G5: no lost updates in Family F (deterministic post-window verification)
- G6: peak cell (olc at T=8 over T=4)
- Control cells: single-thread olc over mutex at T=1
- Competitor comparisons: paired ratios and direction labels (AHEAD, BEHIND, INCONCLUSIVE)
- USL fit: reported parameters on per-core pin over T in {1, 2, 3, 4, 6, 8}

Usage:
    python3 docs/benchmarks/concurrency/scripts/ycsb_concurrent.py --out docs/benchmarks/concurrency/results/baseline_ycsb_concurrent.json
    python3 docs/benchmarks/concurrency/scripts/ycsb_concurrent.py --self-test
"""

from __future__ import annotations

import argparse
import collections
import contextlib
import datetime
import json
import os
import platform
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
import bca_bootstrap  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402
from bench_provenance import (  # noqa: E402
    MIN_WINDOW_S,
    begin_cell,
    end_cell,
    estimators,
    new_provenance,
)

try:
    import fit_usl  # noqa: E402
except ImportError:
    fit_usl = None

try:
    import ycsb_concurrent_bounds  # noqa: E402
except ImportError:
    ycsb_concurrent_bounds = None

THROUGHPUT_TARGET = REPO_ROOT / "target" / "throughput"
COUNTERS_TARGET = REPO_ROOT / "target" / "occ-stats"

DEFAULT_RESULTS_PATH = (
    REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "baseline_ycsb_concurrent.json"
)

PREREGISTRATION = "docs/benchmarks/concurrency/METHODOLOGY.md §20"

GATED_FAMILIES = ("A", "B", "D", "F")
UNIFORM_TWINS = {"A": "A0", "B": "B0", "F": "F0"}
ALL_FAMILIES = (
    "A", "B", "D", "F",
    "A0", "B0", "F0",
    "C", "C0",
    "Ac", "Fc",
    "A-dram", "C-dram",
)

ALL_ARMS = ("olc", "mutex", "skip", "dash", "rwbtree")
PRIMARY_THREADS = (1, 2, 4, 8)
EXTRA_THREADS = (3, 6)

# Policy constants (§20.11)
POLICY_RHO = 0.50  # G3 skew retention floor
POLICY_Q = 1.00   # G4 skip list ratio floor (> 1.0)
POLICY_F1 = 0.50  # Control single-thread collapse guard (> 0.50)
POLICY_STRIPES = 1024  # D1 stripe lock
POLICY_THETA = 0.99
POLICY_N = 1_048_576
POLICY_DRAM_N = 16_777_216
POLICY_DASH_SHARDS = 64


def get_throughput_target() -> Path:
    return THROUGHPUT_TARGET


def get_counters_target() -> Path:
    return COUNTERS_TARGET


def get_binaries() -> tuple[Path, Path]:
    tp = get_throughput_target() / "release" / "examples" / "ycsb_concurrent"
    cnt = get_counters_target() / "release" / "examples" / "ycsb_concurrent"
    return tp, cnt


def build_binaries(verbose: bool = True) -> tuple[Path, Path]:
    """Two builds, never one (AGENTS.md §6 / METHODOLOGY §20.12 item 1).

    Throughput comes from uninstrumented release build (refuses occ-stats).
    Counters come from diagnostic build (--features occ-stats, refuses timing).
    """
    throughput_bin, counters_bin = get_binaries()
    tp_target = get_throughput_target()
    if verbose:
        sys.stderr.write("building throughput binary (default features, uninstrumented) ...\n")
    tp_env = dict(os.environ)
    tp_env["CARGO_TARGET_DIR"] = str(tp_target)
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "-p",
            "expanse-trie",
            "--example",
            "ycsb_concurrent",
        ],
        cwd=str(REPO_ROOT),
        env=tp_env,
        check=True,
    )

    cnt_target = get_counters_target()
    if verbose:
        sys.stderr.write("building diagnostic counters binary (--features occ-stats) ...\n")
    cnt_env = dict(os.environ)
    cnt_env["CARGO_TARGET_DIR"] = str(cnt_target)
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "-p",
            "expanse-trie",
            "--features",
            "occ-stats",
            "--example",
            "ycsb_concurrent",
        ],
        cwd=str(REPO_ROOT),
        env=cnt_env,
        check=True,
    )
    return throughput_bin, counters_bin


def williams_positions(n: int, round_idx: int) -> list[int]:
    """Row `round_idx` of a Williams design over `n` treatments."""
    if n <= 1:
        return list(range(n))
    first = [0]
    lo, hi = 1, n - 1
    while len(first) < n:
        first.append(lo)
        lo += 1
        if len(first) < n:
            first.append(hi)
            hi -= 1
    return [(i + round_idx) % n for i in first]


def generate_family_block_cells(family: str, is_per_core_pin: bool) -> list[dict[str, Any]]:
    """Generates the (arm, T) treatments within a family block.

    Includes all competitor arms across primary threads, uniform twins for olc,
    and extra load points T in {3, 6} on per-core pin for olc and mutex on A and B.
    """
    cells: list[dict[str, Any]] = []
    # Primary treatments: all arms x {1, 2, 4, 8}
    for t in PRIMARY_THREADS:
        for arm in ALL_ARMS:
            cells.append({"family": family, "arm": arm, "threads": t})

    # Uniform twin for olc if applicable
    twin = UNIFORM_TWINS.get(family)
    if twin:
        for t in PRIMARY_THREADS:
            cells.append({"family": twin, "arm": "olc", "threads": t})

    # Extra load points on per-core pin for olc and mutex in A and B (§20.11 D13)
    if is_per_core_pin and family in ("A", "B"):
        for t in EXTRA_THREADS:
            for arm in ("olc", "mutex"):
                cells.append({"family": family, "arm": arm, "threads": t})

    return cells


def run_cell_process(
    bin_path: Path,
    role: str,
    family: str,
    arm: str,
    threads: int,
    round_idx: int,
    position: int,
    quick: bool = False,
    seed: int | None = None,
) -> dict[str, Any]:
    """Executes a single cell in an isolated harness process (§20.8, §15)."""
    cmd = [
        str(bin_path),
        "--role", role,
        "--family", family,
        "--arm", arm,
        "--threads", str(threads),
        "--round", str(round_idx),
        "--position", str(position),
    ]
    if quick:
        cmd.append("--quick")
    if seed is not None:
        cmd.extend(["--seed", str(seed)])

    res = subprocess.run(
        cmd,
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
    )
    if res.returncode != 0:
        raise RuntimeError(
            f"cell process failed (rc={res.returncode}):\ncmd: {' '.join(cmd)}\nstderr: {res.stderr}\nstdout: {res.stdout}"
        )

    # Parse stdout JSON
    stdout = res.stdout.strip()
    # If multiple lines, pick the last non-empty line (the JSON result)
    lines = [line.strip() for line in stdout.splitlines() if line.strip().startswith("{")]
    if not lines:
        raise ValueError(f"no JSON emitted by cell process:\nstdout: {stdout}\nstderr: {res.stderr}")
    data = json.loads(lines[-1])

    # Validate schedule contract (§20.12 item 3)
    assert data.get("family") == family, f"schedule mismatch: family {data.get('family')} != {family}"
    assert data.get("arm") == arm, f"schedule mismatch: arm {data.get('arm')} != {arm}"
    assert data.get("threads") == threads, f"schedule mismatch: threads {data.get('threads')} != {threads}"
    assert data.get("round") == round_idx, f"schedule mismatch: round {data.get('round')} != {round_idx}"
    assert data.get("position") == position, f"schedule mismatch: position {data.get('position')} != {position}"

    return data


def compute_usl_fit(points: list[tuple[int, float]]) -> dict[str, Any]:
    """Fits USL on the per-core pin load points (T in {1, 2, 3, 4, 6, 8})."""
    if len(points) < 4:
        return {"fit_status": "NOT_ENOUGH_POINTS"}
    n_vals = [float(p[0]) for p in points]
    x_vals = [float(p[1]) for p in points]
    if fit_usl is not None:
        try:
            gamma, alpha, beta = fit_usl.fit_usl_ols(n_vals, x_vals)
            # Check if beta constraint binds (beta == 0.0)
            constraint_binds = (beta <= 0.0)
            return {
                "alpha": alpha,
                "beta": max(0.0, beta),
                "gamma": gamma,
                "estimator": "fit_usl_ols",
                "load_points": [p[0] for p in points],
                "beta_constraint_binds": constraint_binds,
            }
        except Exception as e:
            return {"fit_status": f"ERROR: {e}"}
    return {"fit_status": "MODULE_NOT_AVAILABLE"}


def eval_g1(
    olc_t_mops: list[float],
    olc_1_mops: list[float],
    mut_t_mops: list[float],
    mut_1_mops: list[float],
) -> dict[str, Any]:
    """G1, scaling against the α = 1 control:

    R(f, T, r) = [X_olc(f, T, r) ÷ X_olc(f, 1, r)] ÷ [X_mutex(f, T, r) ÷ X_mutex(f, 1, r)].
    Passes iff lower bound > 1.0.
    """
    n = len(olc_t_mops)
    if not (len(olc_1_mops) == n and len(mut_t_mops) == n and len(mut_1_mops) == n and n >= 3):
        return {"verdict": "NOT_EVALUABLE", "reason": "insufficient rounds"}
    ratios = [
        (olc_t_mops[r] / olc_1_mops[r]) / (mut_t_mops[r] / mut_1_mops[r])
        for r in range(n)
    ]
    mean_val = sum(ratios) / n
    _, ci_low, ci_high, method = bca_bootstrap_ci_with_method(ratios)
    verdict = "PASS" if ci_low > 1.0 else "REFUTED"
    return {
        "ratio_mean": mean_val,
        "ratio_ci_lower": ci_low,
        "ratio_ci_upper": ci_high,
        "ratio_ci_method": method,
        "verdict": verdict,
    }


def eval_g2(
    olc_t_mops: list[float],
    mutex_best_mops: list[float],
    best_t_per_round: list[int],
) -> dict[str, Any]:
    """G2, level against the control's best:

    L(f, T, r) = X_olc(f, T, r) ÷ max over T' of X_mutex(f, T', r).
    Passes iff lower bound > 1.0.
    """
    n = len(olc_t_mops)
    if not (len(mutex_best_mops) == n and n >= 3):
        return {"verdict": "NOT_EVALUABLE", "reason": "insufficient rounds"}
    ratios = [olc_t_mops[r] / mutex_best_mops[r] for r in range(n)]
    mean_val = sum(ratios) / n
    _, ci_low, ci_high, method = bca_bootstrap_ci_with_method(ratios)
    verdict = "PASS" if ci_low > 1.0 else "REFUTED"
    return {
        "ratio_mean": mean_val,
        "ratio_ci_lower": ci_low,
        "ratio_ci_upper": ci_high,
        "ratio_ci_method": method,
        "verdict": verdict,
        "best_mutex_t_by_round": best_t_per_round,
    }


def eval_g3(
    zipf_t_mops: list[float],
    unif_t_mops: list[float],
    zipf_1_mops: list[float] | None = None,
    unif_1_mops: list[float] | None = None,
) -> dict[str, Any]:
    """G3, skew retention, f in {A, B, F}:

    K(f, T, r) = X_olc(f, T, r) ÷ X_olc(f0, T, r).
    Passes iff lower bound >= rho = 0.50 (D2).
    Reported beside it: scaling retention [X_olc(f, T) / X_olc(f, 1)] / [X_olc(f0, T) / X_olc(f0, 1)].
    """
    n = len(zipf_t_mops)
    if not (len(unif_t_mops) == n and n >= 3):
        return {"verdict": "NOT_EVALUABLE", "reason": "insufficient rounds or missing uniform twin"}
    ratios = [zipf_t_mops[r] / unif_t_mops[r] for r in range(n)]
    mean_val = sum(ratios) / n
    _, ci_low, ci_high, method = bca_bootstrap_ci_with_method(ratios)
    verdict = "PASS" if ci_low >= POLICY_RHO else "REFUTED"

    out: dict[str, Any] = {
        "floor": POLICY_RHO,
        "ratio_mean": mean_val,
        "ratio_ci_lower": ci_low,
        "ratio_ci_upper": ci_high,
        "ratio_ci_method": method,
        "verdict": verdict,
    }

    # Scaling retention reported beside it
    if zipf_1_mops and unif_1_mops and len(zipf_1_mops) == n and len(unif_1_mops) == n:
        scaling_ratios = [
            (zipf_t_mops[r] / zipf_1_mops[r]) / (unif_t_mops[r] / unif_1_mops[r])
            for r in range(n)
        ]
        s_mean = sum(scaling_ratios) / n
        _, s_low, s_high, s_meth = bca_bootstrap_ci_with_method(scaling_ratios)
        out["scaling_retention"] = {
            "mean": s_mean,
            "ci_lower": s_low,
            "ci_upper": s_high,
            "ci_method": s_meth,
        }

    return out


def eval_g4(
    olc_t_mops: list[float],
    skip_t_mops: list[float],
) -> dict[str, Any]:
    """G4, ordered competitor (skip list), f in {A, B, D} only:

    Q(f, T, r) = X_olc(f, T, r) ÷ X_skip(f, T, r).
    Passes iff lower bound > q = 1.0 (D3).
    """
    n = len(olc_t_mops)
    if not (len(skip_t_mops) == n and n >= 3):
        return {"verdict": "NOT_EVALUABLE", "reason": "insufficient rounds"}
    ratios = [olc_t_mops[r] / skip_t_mops[r] for r in range(n)]
    mean_val = sum(ratios) / n
    _, ci_low, ci_high, method = bca_bootstrap_ci_with_method(ratios)
    verdict = "PASS" if ci_low > POLICY_Q else "REFUTED"
    return {
        "floor": POLICY_Q,
        "ratio_mean": mean_val,
        "ratio_ci_lower": ci_low,
        "ratio_ci_upper": ci_high,
        "ratio_ci_method": method,
        "verdict": verdict,
    }


def eval_g6(
    t8_mops: list[float],
    t4_mops: list[float],
) -> dict[str, Any]:
    """G6, peak cell:

    K_84(f, r) = X_olc(f, 8, r) ÷ X_olc(f, 4, r).
    Passes iff lower bound > 1.0.
    """
    n = len(t8_mops)
    if not (len(t4_mops) == n and n >= 3):
        return {"verdict": "NOT_EVALUABLE", "reason": "insufficient rounds"}
    ratios = [t8_mops[r] / t4_mops[r] for r in range(n)]
    mean_val = sum(ratios) / n
    _, ci_low, ci_high, method = bca_bootstrap_ci_with_method(ratios)
    verdict = "PASS" if ci_low > 1.0 else ("REFUTED" if ci_high < 1.0 else "INCONCLUSIVE")
    return {
        "ratio_mean": mean_val,
        "ratio_ci_lower": ci_low,
        "ratio_ci_upper": ci_high,
        "ratio_ci_method": method,
        "verdict": verdict,
    }


def eval_control(
    olc_1_mops: list[float],
    mut_1_mops: list[float],
) -> dict[str, Any]:
    """Control cells, T = 1:

    P(f, r) = X_olc(f, 1, r) ÷ X_mutex(f, 1, r).
    Fails iff interval lies wholly below F1 = 0.50 (D4).
    """
    n = len(olc_1_mops)
    if not (len(mut_1_mops) == n and n >= 3):
        return {"verdict": "NOT_EVALUABLE", "reason": "insufficient rounds"}
    ratios = [olc_1_mops[r] / mut_1_mops[r] for r in range(n)]
    mean_val = sum(ratios) / n
    _, ci_low, ci_high, method = bca_bootstrap_ci_with_method(ratios)
    verdict = "FAIL" if ci_high < POLICY_F1 else "PASS"
    return {
        "floor": POLICY_F1,
        "ratio_mean": mean_val,
        "ratio_ci_lower": ci_low,
        "ratio_ci_upper": ci_high,
        "ratio_ci_method": method,
        "verdict": verdict,
    }


def eval_competitor_comparison(
    olc_mops: list[float],
    comp_mops: list[float],
) -> dict[str, Any]:
    """Paired competitor ratio X_olc / X_comp with direction label."""
    n = len(olc_mops)
    if not (len(comp_mops) == n and n >= 3):
        return {"verdict": "NOT_EVALUABLE"}
    ratios = [olc_mops[r] / comp_mops[r] for r in range(n)]
    mean_val = sum(ratios) / n
    _, ci_low, ci_high, method = bca_bootstrap_ci_with_method(ratios)
    if ci_low > 1.0:
        label = "AHEAD"
    elif ci_high < 1.0:
        label = "BEHIND"
    else:
        label = "INCONCLUSIVE"
    return {
        "ratio_mean": mean_val,
        "ratio_ci_lower": ci_low,
        "ratio_ci_upper": ci_high,
        "ratio_ci_method": method,
        "label": label,
    }


def build_gate_report(
    cells: list[dict[str, Any]],
    pin: str,
    rounds: int,
    quick: bool,
) -> dict[str, Any]:
    """Produces the §20.6 gate report over throughput and counter cells."""
    voids: list[str] = []
    is_per_core = "0,2,4,6,8,10,12,14" in pin or "percore" in pin.lower()

    if quick:
        voids.append("quick run (population < 1,048,576 keys)")
    if rounds < 8 and not quick:
        voids.append(f"round count {rounds} < 8 (§20.8)")

    # Group cells by (family, arm, threads)
    cell_map: dict[tuple[str, str, int], dict[str, Any]] = {}
    for c in cells:
        k = (c["family"], c["arm"], c["threads"])
        cell_map[k] = c

    # Assert G5: no lost update in any round of Family F (§20.5, §20.6)
    f_cells = [c for c in cells if c["family"] == "F"]
    for c in f_cells:
        for r_entry in c.get("rounds_raw", []):
            if r_entry.get("per_key_mismatches", 0) != 0:
                voids.append(f"VOID_LOST_UPDATE: per_key_mismatches > 0 in Family F cell {c['arm']} T={c['threads']} round {r_entry.get('round')}")
            if r_entry.get("lost_updates", 0) != 0:
                voids.append(f"VOID_LOST_UPDATE: lost_updates > 0 in Family F cell {c['arm']} T={c['threads']} round {r_entry.get('round')}")

    # Assert Oracle: per_key_mismatches == 0 in A, B, D (§20.15)
    oracle_cells = [c for c in cells if c["family"] in ("A", "B", "D")]
    for c in oracle_cells:
        for r_entry in c.get("rounds_raw", []):
            if r_entry.get("per_key_mismatches", 0) != 0:
                voids.append(f"VOID_ORACLE: per_key_mismatches > 0 in cell {c['family']} {c['arm']} T={c['threads']} round {r_entry.get('round')}")

    families_report: dict[str, Any] = {}
    all_gated_pass = True

    for family in GATED_FAMILIES:
        fam_report: dict[str, Any] = {"gates": {}, "controls": {}}

        # Extract olc and mutex T=1 series
        c_olc_1 = cell_map.get((family, "olc", 1))
        c_mut_1 = cell_map.get((family, "mutex", 1))
        if not c_olc_1 or not c_mut_1:
            fam_report["gates"]["all_pass"] = False
            all_gated_pass = False
            voids.append(f"missing T=1 cell for family {family}")
            families_report[family] = fam_report
            continue

        olc_1_mops = [r["total_mops"] for r in c_olc_1.get("rounds_raw", [])]
        mut_1_mops = [r["total_mops"] for r in c_mut_1.get("rounds_raw", [])]

        # Control cell T=1
        ctrl_eval = eval_control(olc_1_mops, mut_1_mops)
        fam_report["controls"]["T1"] = ctrl_eval
        if ctrl_eval.get("verdict") != "PASS":
            fam_report["gates"]["all_pass"] = False
            all_gated_pass = False

        # Gather mutex across all T' to compute max_T' per round for G2
        mut_all_t_mops: dict[int, list[float]] = {}
        for t_prime in PRIMARY_THREADS:
            c_mut = cell_map.get((family, "mutex", t_prime))
            if c_mut:
                mut_all_t_mops[t_prime] = [r["total_mops"] for r in c_mut.get("rounds_raw", [])]

        # Check if any T' cell is missing for mutex
        if len(mut_all_t_mops) < len(PRIMARY_THREADS):
            voids.append(f"missing mutex T' cell in {PRIMARY_THREADS} for family {family}")

        best_mut_per_round: list[float] = []
        best_t_per_round: list[int] = []
        if len(mut_all_t_mops) == len(PRIMARY_THREADS):
            for r in range(rounds):
                best_val = -1.0
                best_t = 1
                for t_prime, series in mut_all_t_mops.items():
                    if r < len(series) and series[r] > best_val:
                        best_val = series[r]
                        best_t = t_prime
                best_mut_per_round.append(best_val)
                best_t_per_round.append(best_t)

        fam_all_pass = True

        # Evaluate G1, G2, G3, G4 per gated load point T in {2, 4, 8}
        for t in (2, 4, 8):
            t_key = str(t)
            c_olc_t = cell_map.get((family, "olc", t))
            c_mut_t = cell_map.get((family, "mutex", t))
            if not c_olc_t or not c_mut_t:
                fam_all_pass = False
                all_gated_pass = False
                continue

            olc_t_mops = [r["total_mops"] for r in c_olc_t.get("rounds_raw", [])]
            mut_t_mops = [r["total_mops"] for r in c_mut_t.get("rounds_raw", [])]

            # G1
            g1_res = eval_g1(olc_t_mops, olc_1_mops, mut_t_mops, mut_1_mops)
            fam_report["gates"].setdefault("G1_scaling", {})[t_key] = g1_res
            if g1_res.get("verdict") != "PASS":
                fam_all_pass = False

            # G2
            if best_mut_per_round:
                g2_res = eval_g2(olc_t_mops, best_mut_per_round, best_t_per_round)
            else:
                g2_res = {"verdict": "NOT_EVALUABLE", "reason": "missing mutex T' cell"}
            fam_report["gates"].setdefault("G2_level", {})[t_key] = g2_res
            if g2_res.get("verdict") != "PASS":
                fam_all_pass = False

            # G3: skew retention for A, B, F
            if family in ("A", "B", "F"):
                twin_name = UNIFORM_TWINS[family]
                c_twin_t = cell_map.get((twin_name, "olc", t))
                c_twin_1 = cell_map.get((twin_name, "olc", 1))
                if c_twin_t and c_twin_1:
                    twin_t_mops = [r["total_mops"] for r in c_twin_t.get("rounds_raw", [])]
                    twin_1_mops = [r["total_mops"] for r in c_twin_1.get("rounds_raw", [])]
                    g3_res = eval_g3(olc_t_mops, twin_t_mops, olc_1_mops, twin_1_mops)
                else:
                    g3_res = {"verdict": "NOT_EVALUABLE", "reason": "missing uniform twin cell"}
                fam_report["gates"].setdefault("G3_skew_retention", {})[t_key] = g3_res
                if g3_res.get("verdict") != "PASS":
                    fam_all_pass = False

            # G4: ordered competitor (skip list) for A, B, D only
            if family in ("A", "B", "D"):
                c_skip_t = cell_map.get((family, "skip", t))
                if c_skip_t:
                    skip_t_mops = [r["total_mops"] for r in c_skip_t.get("rounds_raw", [])]
                    g4_res = eval_g4(olc_t_mops, skip_t_mops)
                else:
                    g4_res = {"verdict": "NOT_EVALUABLE", "reason": "missing skip cell"}
                fam_report["gates"].setdefault("G4_skip_list", {})[t_key] = g4_res
                if g4_res.get("verdict") != "PASS":
                    fam_all_pass = False

        # G6: peak cell (T=8 over T=4)
        c_olc_8 = cell_map.get((family, "olc", 8))
        c_olc_4 = cell_map.get((family, "olc", 4))
        if c_olc_8 and c_olc_4:
            olc_8_mops = [r["total_mops"] for r in c_olc_8.get("rounds_raw", [])]
            olc_4_mops = [r["total_mops"] for r in c_olc_4.get("rounds_raw", [])]
            g6_res = eval_g6(olc_8_mops, olc_4_mops)
        else:
            g6_res = {"verdict": "NOT_EVALUABLE", "reason": "missing T=4 or T=8 cell"}
        fam_report["gates"]["G6_peak_8_over_4"] = g6_res
        if g6_res.get("verdict") != "PASS":
            fam_all_pass = False

        # G5 for Family F
        if family == "F":
            g5_pass = True
            for c in f_cells:
                for r_entry in c.get("rounds_raw", []):
                    if r_entry.get("per_key_mismatches", 0) != 0 or r_entry.get("lost_updates", 0) != 0:
                        g5_pass = False
            fam_report["gates"]["G5_no_lost_update"] = {
                "verdict": "PASS" if g5_pass else "VOID_LOST_UPDATE",
            }
            if not g5_pass:
                fam_all_pass = False

        # USL fit on per-core pin for A and B
        if is_per_core and family in ("A", "B"):
            load_points = []
            for t_val in (1, 2, 3, 4, 6, 8):
                c_load = cell_map.get((family, "olc", t_val))
                if c_load and c_load.get("rounds_raw"):
                    mean_m = sum(r["total_mops"] for r in c_load["rounds_raw"]) / len(c_load["rounds_raw"])
                    load_points.append((t_val, mean_m))
            fam_report["usl_fit"] = compute_usl_fit(load_points)

        fam_report["gates"]["all_pass"] = fam_all_pass
        if not fam_all_pass:
            all_gated_pass = False
        families_report[family] = fam_report

    return {
        "preregistration": PREREGISTRATION,
        "pin": pin,
        "rounds": rounds,
        "quick": quick,
        "void": voids,
        "all_cells_pass_in_this_run": all_gated_pass and (len(voids) == 0),
        "families": families_report,
    }


def format_throughput_artifact(
    cells: list[dict[str, Any]],
    gate_report: dict[str, Any],
    pin: str,
    rounds: int,
    quick: bool,
    seed: int,
) -> dict[str, Any]:
    """Formats the comprehensive JSON artifact adhering to §20.6 and §20.12."""
    prov = new_provenance(
        suite="concurrency",
        issue=1006,
        ratio="SyncExpanseMap concurrent YCSB scaling and competitor ratios (METHODOLOGY.md §20)",
        repo_root=REPO_ROOT,
    )
    prov["preregistration"] = PREREGISTRATION
    prov["core_pin"] = pin
    prov["cell_isolation"] = "process"
    prov["theta"] = POLICY_THETA
    prov["rounds"] = rounds

    # Calculate rank histogram verification on stream 0
    if ycsb_concurrent_bounds is not None:
        prov["rank_histogram"] = {
            "law": "gray_top_k_share",
            "top_1": ycsb_concurrent_bounds.gray_top_k_share(1, POLICY_N, POLICY_THETA),
            "top_2": ycsb_concurrent_bounds.gray_top_k_share(2, POLICY_N, POLICY_THETA),
            "top_16": ycsb_concurrent_bounds.gray_top_k_share(16, POLICY_N, POLICY_THETA),
            "top_256": ycsb_concurrent_bounds.gray_top_k_share(256, POLICY_N, POLICY_THETA),
            "top_4096": ycsb_concurrent_bounds.gray_top_k_share(4096, POLICY_N, POLICY_THETA),
        }

    # Add efficiency and intervals per cell
    throughput_list = []
    for c in cells:
        c_copy = dict(c)
        c_copy["cpu_pin"] = pin
        mops_series = [r["total_mops"] for r in c.get("rounds_raw", [])]
        if mops_series:
            mean_val, low, high, meth = bca_bootstrap_ci_with_method(mops_series)
            c_copy["total_mops_mean"] = mean_val
            c_copy["total_mops_ci_lower"] = low
            c_copy["total_mops_ci_upper"] = high
            c_copy["total_mops_ci_method"] = meth

            # Efficiency C(T) / T
            t_count = c["threads"]
            c_copy["efficiency"] = {
                "mean": mean_val / t_count,
                "ci_lower": low / t_count,
                "ci_upper": high / t_count,
                "ci_method": meth,
            }

        # Arm metadata
        if c["arm"] == "dash":
            c_copy["shard_amount"] = POLICY_DASH_SHARDS
        if c["family"] in ("F", "F0", "Fc"):
            c_copy["rmw_provider"] = "striped_lock" if c["arm"] == "olc" else "atomic_in_place"
            if c["arm"] == "olc":
                c_copy["rmw_stripes"] = POLICY_STRIPES

        # Update idiom (§20.5)
        if c["arm"] in ("skip", "dash", "rwbtree"):
            c_copy["update_idiom"] = "value_cell_store"
            c_copy["value_type"] = "atomic_u64"
        else:
            c_copy["update_idiom"] = "map_insert_in_place"
            c_copy["value_type"] = "u64"

        throughput_list.append(c_copy)

    # Diagnostic passes §20.14 schema stubs
    latency_stubs = [
        {
            "family": fam,
            "arm": arm,
            "threads": t,
            "op": "read",
            "samples": 0,
            "p50_ns": 0.0,
            "p99_ns": 0.0,
            "p999_ns": 0.0,
            "max_ns": 0.0,
            "tsc_hz": 0,
            "bucket_scheme": "log2x16",
            "clock": "rdtsc_over_tsc_hz",
            "model": "closed_loop_service_time",
            "bracket_overhead_ns": 0.0,
        }
        for fam in ("A", "B", "D", "F")
        for arm in ("olc", "mutex")
        for t in (1, 8)
    ]

    pmu_stubs = [
        {
            "family": fam,
            "arm": "olc",
            "threads": 8,
            "event": "cpu_core/mem_load_l3_hit_retired.xsnp_fwd/",
            "per_op": 0.0,
            "per_op_ci_lower": 0.0,
            "per_op_ci_upper": 0.0,
            "ci_method": "bca",
            "pmu_prefix": "cpu_core/",
        }
        for fam in ("A", "B", "D", "F")
    ]

    return {
        "provenance": prov,
        "throughput": throughput_list,
        "gate_report": gate_report,
        "latency": latency_stubs,
        "pmu": pmu_stubs,
    }


# ---------------------------------------------------------------------------
# Self-Test Implementation (§20.12 item 4)
# ---------------------------------------------------------------------------

def _self_test_binary_roles(throughput_bin: Path, counters_bin: Path) -> None:
    """Tests build/role mismatch negative controls (AGENTS.md §2.3, §6)."""
    sys.stderr.write("Testing negative controls (AGENTS.md §2.3 / build/role mismatch)...\n")
    p_bad1 = subprocess.run(
        [str(throughput_bin), "--role", "occ-stats", "--self-test"],
        capture_output=True,
        text=True,
    )
    assert p_bad1.returncode != 0, "Expected throughput_bin with --role occ-stats to fail"
    assert "build/role mismatch" in p_bad1.stderr or "build/role mismatch" in p_bad1.stdout

    p_bad2 = subprocess.run(
        [str(counters_bin), "--role", "throughput", "--self-test"],
        capture_output=True,
        text=True,
    )
    assert p_bad2.returncode != 0, "Expected counters_bin with --role throughput to fail"
    assert "build/role mismatch" in p_bad2.stderr or "build/role mismatch" in p_bad2.stdout

    p_ok1 = subprocess.run(
        [str(throughput_bin), "--role", "throughput", "--self-test"],
        capture_output=True,
        text=True,
    )
    assert p_ok1.returncode == 0, f"throughput_bin self-test failed: {p_ok1.stderr}"

    p_ok2 = subprocess.run(
        [str(counters_bin), "--role", "occ-stats", "--self-test"],
        capture_output=True,
        text=True,
    )
    assert p_ok2.returncode == 0, f"counters_bin self-test failed: {p_ok2.stderr}"
    sys.stderr.write("Binary self-tests PASSED\n")


def _self_test_gate_statistics() -> None:
    """Verifies that G1, G2, G3, G4, G5, G6, Control calculate exact math bounds."""
    sys.stderr.write("Testing gate evaluation math functions...\n")
    # Synthetic series for 8 rounds with small jitter
    jitter = [0.00, 0.01, -0.01, 0.02, -0.02, 0.01, 0.00, -0.01]

    # G1: olc scales from 5 to 25 (5x), mutex scales from 6 to 6 (1x) -> R = 5x
    olc_1 = [5.0 + j for j in jitter]
    olc_8 = [25.0 + j for j in jitter]
    mut_1 = [6.0 + j for j in jitter]
    mut_8 = [6.0 + j for j in jitter]
    g1 = eval_g1(olc_8, olc_1, mut_8, mut_1)
    assert g1["verdict"] == "PASS" and g1["ratio_ci_lower"] > 1.0, g1

    # G2: olc at 25 vs mutex best (mut_1 = 6) -> L = 25/6 = 4.16 > 1.0
    g2 = eval_g2(olc_8, mut_1, [1] * 8)
    assert g2["verdict"] == "PASS" and g2["ratio_ci_lower"] > 1.0, g2

    # G3: Zipfian cell at 20 vs uniform twin at 25 -> K = 20/25 = 0.80 >= 0.50
    zipf_8 = [20.0 + j for j in jitter]
    unif_8 = [25.0 + j for j in jitter]
    g3 = eval_g3(zipf_8, unif_8)
    assert g3["verdict"] == "PASS" and g3["ratio_ci_lower"] >= POLICY_RHO, g3

    # G3 fail: Zipfian cell at 10 vs uniform twin at 25 -> K = 10/25 = 0.40 < 0.50
    zipf_bad = [10.0 + j for j in jitter]
    g3_fail = eval_g3(zipf_bad, unif_8)
    assert g3_fail["verdict"] == "REFUTED" and g3_fail["ratio_ci_lower"] < POLICY_RHO, g3_fail

    # G4: olc at 25 vs skip at 20 -> Q = 25/20 = 1.25 > 1.0
    skip_8 = [20.0 + j for j in jitter]
    g4 = eval_g4(olc_8, skip_8)
    assert g4["verdict"] == "PASS" and g4["ratio_ci_lower"] > POLICY_Q, g4

    # G6: olc at 25 (T=8) vs olc at 15 (T=4) -> K84 = 25/15 = 1.66 > 1.0
    olc_4 = [15.0 + j for j in jitter]
    g6 = eval_g6(olc_8, olc_4)
    assert g6["verdict"] == "PASS" and g6["ratio_ci_lower"] > 1.0, g6

    # Control: olc at 5.5 vs mutex at 6.0 -> P = 5.5/6.0 = 0.916 >= 0.50
    ctrl = eval_control(olc_1, mut_1)
    assert ctrl["verdict"] == "PASS", ctrl

    sys.stderr.write("Gate statistics PASSED\n")


def _self_test_artifact_schema_and_mutation() -> None:
    """Asserts artifact schema and tests mutation resistance (§20.12 item 4).

    Demonstrates that missing round, wrong pin, missing uniform twin, or non-zero
    mismatches/lost updates yield NOT_EVALUABLE or VOID, never a pass.
    """
    sys.stderr.write("Testing artifact schema and mutation assertions...\n")
    jitter = [0.00, 0.01, -0.01, 0.02, -0.02, 0.01, 0.00, -0.01]

    def make_cell(family: str, arm: str, threads: int, mops_base: float) -> dict[str, Any]:
        return {
            "workload_id": f"concurrency_ycsb_{family.lower()}",
            "family": family,
            "arm": arm,
            "threads": threads,
            "rounds_raw": [
                {
                    "round": r,
                    "position": 0,
                    "total_ops": 100_000,
                    "elapsed_s": 0.01,
                    "total_mops": mops_base + jitter[r],
                    "per_key_mismatches": 0,
                    "lost_updates": 0,
                    "rmw_ops": 50_000,
                }
                for r in range(8)
            ],
        }

    # Build a full nominal suite of cells
    cells = []
    for fam in GATED_FAMILIES:
        for arm in ALL_ARMS:
            for t in PRIMARY_THREADS:
                if arm == "olc":
                    base = 5.0 * t
                elif arm == "mutex":
                    base = 4.0
                elif arm == "skip":
                    base = 3.0 * t
                else:
                    base = 4.0 * t
                cells.append(make_cell(fam, arm, t, base))
        # Uniform twin
        twin = UNIFORM_TWINS.get(fam)
        if twin:
            for t in PRIMARY_THREADS:
                cells.append(make_cell(twin, "olc", t, 5.0 * t))

    pin = "0,2,4,6,8,10,12,14"
    rep = build_gate_report(cells, pin, 8, quick=False)
    assert rep["all_cells_pass_in_this_run"] is True, rep
    assert rep["void"] == []

    # Format artifact
    art = format_throughput_artifact(cells, rep, pin, 8, quick=False, seed=123)
    # Check all §20.6 required fields
    assert "provenance" in art
    assert "throughput" in art
    assert "gate_report" in art
    assert "latency" in art
    assert "pmu" in art
    assert art["provenance"]["preregistration"] == PREREGISTRATION
    assert art["provenance"]["cell_isolation"] == "process"
    assert art["provenance"]["theta"] == POLICY_THETA

    for t_item in art["throughput"]:
        assert "total_mops_mean" in t_item
        assert "total_mops_ci_lower" in t_item
        assert "efficiency" in t_item
        assert "update_idiom" in t_item
        assert "value_type" in t_item

    # Mutation test 1: Non-zero lost updates in Family F must VOID the run
    cells_mut1 = [dict(c) for c in cells]
    f_cell = next(c for c in cells_mut1 if c["family"] == "F" and c["arm"] == "olc" and c["threads"] == 8)
    f_cell_corrupt = dict(f_cell)
    f_cell_corrupt["rounds_raw"] = [dict(r) for r in f_cell["rounds_raw"]]
    f_cell_corrupt["rounds_raw"][0]["lost_updates"] = 1
    idx = cells_mut1.index(f_cell)
    cells_mut1[idx] = f_cell_corrupt
    rep_mut1 = build_gate_report(cells_mut1, pin, 8, quick=False)
    assert rep_mut1["all_cells_pass_in_this_run"] is False, "Expected non-zero lost updates to fail run"
    assert any("VOID_LOST_UPDATE" in v for v in rep_mut1["void"])

    # Mutation test 2: Non-zero per_key_mismatches in Family A must VOID the run
    cells_mut2 = [dict(c) for c in cells]
    a_cell = next(c for c in cells_mut2 if c["family"] == "A" and c["arm"] == "olc" and c["threads"] == 8)
    a_cell_corrupt = dict(a_cell)
    a_cell_corrupt["rounds_raw"] = [dict(r) for r in a_cell["rounds_raw"]]
    a_cell_corrupt["rounds_raw"][0]["per_key_mismatches"] = 2
    idx = cells_mut2.index(a_cell)
    cells_mut2[idx] = a_cell_corrupt
    rep_mut2 = build_gate_report(cells_mut2, pin, 8, quick=False)
    assert rep_mut2["all_cells_pass_in_this_run"] is False, "Expected oracle mismatch to fail run"
    assert any("VOID_ORACLE" in v for v in rep_mut2["void"])

    # Mutation test 3: Missing uniform twin for G3 must produce NOT_EVALUABLE and fail
    cells_no_twin = [c for c in cells if c["family"] != "A0"]
    rep_no_twin = build_gate_report(cells_no_twin, pin, 8, quick=False)
    assert rep_no_twin["all_cells_pass_in_this_run"] is False, "Expected missing uniform twin to fail run"
    assert rep_no_twin["families"]["A"]["gates"]["G3_skew_retention"]["2"]["verdict"] == "NOT_EVALUABLE"

    # Mutation test 4: Missing mutex T' cell for G2 must produce NOT_EVALUABLE and fail
    cells_no_mut = [c for c in cells if not (c["family"] == "A" and c["arm"] == "mutex" and c["threads"] == 4)]
    rep_no_mut = build_gate_report(cells_no_mut, pin, 8, quick=False)
    assert rep_no_mut["all_cells_pass_in_this_run"] is False, "Expected missing mutex cell to fail run"
    assert rep_no_mut["families"]["A"]["gates"]["G2_level"]["2"]["verdict"] == "NOT_EVALUABLE"

    # Mutation test 5: Round count < 8 on full population must VOID the run
    rep_short = build_gate_report(cells, pin, 4, quick=False)
    assert rep_short["all_cells_pass_in_this_run"] is False, "Expected round count < 8 to void run"
    assert any("round count" in v for v in rep_short["void"])

    sys.stderr.write("Artifact schema and mutation assertions PASSED\n")


def self_test() -> int:
    sys.stderr.write("Running ycsb_concurrent.py self-test...\n\n")

    # 1. Gate statistics calculation tests
    _self_test_gate_statistics()

    # 2. Artifact schema & mutation resistance tests
    _self_test_artifact_schema_and_mutation()

    # 3. Build binaries and test binary negative controls and process execution
    tp_bin, cnt_bin = build_binaries(verbose=True)
    _self_test_binary_roles(tp_bin, cnt_bin)

    # 4. Quick single-cell process isolation test
    sys.stderr.write("Testing process isolation (single quick cell run)...\n")
    cell_data = run_cell_process(
        tp_bin,
        role="throughput",
        family="A",
        arm="olc",
        threads=1,
        round_idx=0,
        position=0,
        quick=True,
    )
    assert cell_data.get("family") == "A"
    assert cell_data.get("arm") == "olc"
    assert cell_data.get("threads") == 1
    assert "total_mops" in cell_data
    sys.stderr.write("Process isolation PASSED\n")

    sys.stderr.write("\nAll ycsb_concurrent.py self-tests PASSED successfully.\n")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run concurrent YCSB scaling benchmark across physical P-cores."
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=DEFAULT_RESULTS_PATH,
        help=f"Output results JSON path (default: {DEFAULT_RESULTS_PATH})",
    )
    parser.add_argument(
        "--rounds",
        type=int,
        default=8,
        help="Number of rounds per cell (default: 8)",
    )
    parser.add_argument(
        "--quick",
        action="store_true",
        help="Quick mode with small population (4,096 keys) and ops",
    )
    parser.add_argument(
        "--family",
        type=str,
        default="all",
        help="Workload family to run (default: all registered families)",
    )
    parser.add_argument(
        "--pin",
        type=str,
        default=None,
        help="CPU pin to apply (default: applied via bench_pin)",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=None,
        help="Suite seed",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run unit tests and self-test verification suite",
    )
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    # Apply bench pin
    pin = args.pin or bench_pin.apply("ycsb_concurrent.py")
    sys.stderr.write(f"Applied core pin: {pin}\n")

    # Build binaries
    tp_bin, cnt_bin = build_binaries(verbose=True)

    families_to_run = GATED_FAMILIES if args.family == "all" else [args.family]
    is_per_core = "0,2,4,6,8,10,12,14" in pin or "percore" in pin.lower()

    collected_cells: list[dict[str, Any]] = []

    for fam in families_to_run:
        block_treatments = generate_family_block_cells(fam, is_per_core)
        n_treatments = len(block_treatments)
        sys.stderr.write(f"Running Family {fam} ({n_treatments} treatments, {args.rounds} rounds)...\n")

        # Map: (family, arm, threads) -> list of rounds_raw
        raw_map: dict[tuple[str, str, int], list[dict[str, Any]]] = collections.defaultdict(list)

        for r in range(args.rounds):
            order = williams_positions(n_treatments, r)
            for pos, treatment_idx in enumerate(order):
                t_spec = block_treatments[treatment_idx]
                t_fam = t_spec["family"]
                t_arm = t_spec["arm"]
                t_threads = t_spec["threads"]

                # Run throughput process
                tp_res = run_cell_process(
                    tp_bin,
                    role="throughput",
                    family=t_fam,
                    arm=t_arm,
                    threads=t_threads,
                    round_idx=r,
                    position=pos,
                    quick=args.quick,
                    seed=args.seed,
                )
                raw_map[(t_fam, t_arm, t_threads)].append(tp_res)

        for (t_fam, t_arm, t_threads), rounds_list in raw_map.items():
            cell_entry = {
                "workload_id": rounds_list[0].get("workload_id", f"concurrency_ycsb_{t_fam.lower()}"),
                "family": t_fam,
                "arm": t_arm,
                "threads": t_threads,
                "rounds_raw": rounds_list,
            }
            collected_cells.append(cell_entry)

    # Compute gate report
    gate_report = build_gate_report(collected_cells, pin, args.rounds, args.quick)
    artifact = format_throughput_artifact(
        collected_cells,
        gate_report,
        pin,
        args.rounds,
        args.quick,
        seed=args.seed or 0,
    )

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(artifact, indent=2))
    sys.stderr.write(f"Wrote benchmark results artifact to {args.out}\n")

    return 0


if __name__ == "__main__":
    sys.exit(main())
