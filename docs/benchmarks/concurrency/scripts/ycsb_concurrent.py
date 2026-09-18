#!/usr/bin/env python3
"""Expanse-native concurrent YCSB driver (Refs #1006, METHODOLOGY.md §20).

Runs `crates/expanse/examples/ycsb_concurrent.rs` across the competitor arms
(`olc`, `mutex`, `skip`, `dash`, `rwbtree`) and every cell family §20.4
registers: the gated A, B, D and F with the uniform twins A0, B0 and F0, the
read-only anchors C and C0, the contiguous-rank cells Ac and Fc, and the
`-dram` anchors.

Two builds, never one (AGENTS.md §6, METHODOLOGY.md §20.12 item 1):
- Pass 1 (throughput): the uninstrumented release build, one harness process
  per timed cell (§20.8, §15), each round running the block's cells in the
  order of that round's row of a Williams design.
- Pass 2 (counters): the `--features occ-stats` build over the `olc` cells.
  It emits counters and no timing; a counters row carrying a timing, or a
  throughput row from the counters role, stops the run.

This driver produces the throughput instrument only. The latency build, the
PMU and `perf c2c` pass (§20.14) and the untimed monotonicity pass with its
node census (§20.15) are not produced here, and an artifact carries a
`latency` or `pmu` section only when a real pass wrote one: this driver writes
neither.

What it computes, all from the artifact's own rows (AGENTS.md §8.2):
- G1–G4 and G6 as three-way verdicts (PASS / INCONCLUSIVE / REFUTED), the T = 1
  control, G5 and the §20.15 oracle as voids, every statistic paired by the
  rows' `round` field.
- E(T) = C(T) ÷ T per cell and `peak_8_over_4` per (family, arm), paired.
- Direction labels (AHEAD / BEHIND / INCONCLUSIVE) for the ungated comparisons.
- The reported scalability fit per (family, arm) on the per-core pin.

Usage:
    python3 docs/benchmarks/concurrency/scripts/ycsb_concurrent.py --run 1
    python3 docs/benchmarks/concurrency/scripts/ycsb_concurrent.py --quick --rounds 3
    python3 docs/benchmarks/concurrency/scripts/ycsb_concurrent.py --self-test
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

# Every import below is load-bearing. A missing module stops the driver
# (AGENTS.md §8.1): a gate computed without its thresholds, or a fit reported
# as "module not available", is a silent fallback.
import bench_pin  # noqa: E402
import check_bench_provenance  # noqa: E402
import fit_usl  # noqa: E402
import reader_scaling_bounds  # noqa: E402
import ycsb_concurrent_bounds as bounds  # noqa: E402
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ci_with_method  # noqa: E402
from bench_provenance import add_load, begin_cell, end_cell, new_provenance  # noqa: E402

THROUGHPUT_TARGET = REPO_ROOT / "target" / "throughput"
COUNTERS_TARGET = REPO_ROOT / "target" / "occ-stats"

RESULTS_DIR = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results"
QUICK_RESULTS_DIR = RESULTS_DIR / "quick"  # gitignored: docs/benchmarks/*/results/quick/
ARTIFACT_STEM = "gate_1006_ycsb_concurrent"

PREREGISTRATION = bounds.LOCKED_PREREGISTRATION

# §20.8: "Pins `0-15` and `0,2,4,6,8,10,12,14`, never pooled".
PIN_ALL_P = "0-15"
PIN_PER_CORE = "0,2,4,6,8,10,12,14"
REGISTERED_PINS = (PIN_ALL_P, PIN_PER_CORE)
# §20.8: "8 rounds per cell, fixed."
REGISTERED_ROUNDS = bounds.ROUNDS
# §20.4: "Each thread runs a pre-generated stream of 2^20 operations".
REGISTERED_OPS_PER_THREAD = 1 << 20
QUICK_POPULATION = 4_096
QUICK_OPS_PER_THREAD = 4_096

GATED_FAMILIES = ("A", "B", "D", "F")
UNIFORM_TWINS = {"A": "A0", "B": "B0", "F": "F0", "C": "C0"}
UNIFORM_FAMILIES = tuple(UNIFORM_TWINS.values())
RMW_FAMILIES = ("F", "F0", "Fc")
DRAM_FAMILIES = ("A-dram", "C-dram")
ALL_ARMS = ("olc", "mutex", "skip", "dash", "rwbtree")
# §20.4: "Threads. T ∈ {1, 2, 4, 8}."
PRIMARY_THREADS = (1, 2, 4, 8)
GATE_THREADS = tuple(bounds.WRITERS)  # §20.6: "Gate cells are T ∈ {2, 4, 8}"

# §20.5's table, per arm.
REGISTERED_IDIOM = {
    "olc": ("map_insert_in_place", "u64"),
    "mutex": ("map_insert_in_place", "u64"),
    "skip": ("value_cell_store", "atomic_u64"),
    "dash": ("value_cell_store", "atomic_u64"),
    "rwbtree": ("value_cell_store", "atomic_u64"),
}
REGISTERED_OLC_RMW_PROVIDER = "striped_lock"

# §20.6: "observed share on the 1, 2, 16, 256 and 4,096 lowest ranks of one stream".
RANK_K = (1, 2, 16, 256, 4096)
# §20.8 voids a histogram "outside its binomial tolerance"; §20.12 item 2 asks
# for a stated one. Stated here and in the harness's unit test: 4.5 σ.
RANK_TOLERANCE_SIGMAS = 4.5
# §20.6: "Every interval is BCa 95% over the round series, 2,000 resamples".
# The self-test lowers it for its synthetic fits only; a run never does.
USL_BOOTSTRAP_RESAMPLES = 2000

# METHODOLOGY.md §6: foreign busy CPU above 1.0 core-equivalents, or a load
# average above 12, voids.
FOREIGN_BUSY_VOID = 1.0
LOAD_AVERAGE_VOID = 12.0

# Fields only one role may emit.
TIMING_FIELDS = ("elapsed_s", "total_mops", "thread_elapsed_s")
COUNTER_FIELDS = ("lock_fallbacks", "fallback_causes_total", "lock_restarts",
                  "read_validation_failures")


class ScheduleMismatch(RuntimeError):
    """A harness row that is not the cell the driver asked for (§20.12 item 3)."""


class RoleLeak(RuntimeError):
    """A timing from the counters build, or a counters row from the timed one."""


# ---------------------------------------------------------------------------
# Builds
# ---------------------------------------------------------------------------

def get_binaries() -> tuple[Path, Path]:
    tp = THROUGHPUT_TARGET / "release" / "examples" / "ycsb_concurrent"
    cnt = COUNTERS_TARGET / "release" / "examples" / "ycsb_concurrent"
    return tp, cnt


def build_binaries(verbose: bool = True) -> tuple[Path, Path]:
    """Throughput from the default build, counters from `--features occ-stats`."""
    throughput_bin, counters_bin = get_binaries()
    for label, target, extra in (
        ("throughput binary (default features, uninstrumented)", THROUGHPUT_TARGET, []),
        ("diagnostic counters binary (--features occ-stats)", COUNTERS_TARGET,
         ["--features", "occ-stats"]),
    ):
        if verbose:
            sys.stderr.write(f"building {label} ...\n")
        env = dict(os.environ)
        env["CARGO_TARGET_DIR"] = str(target)
        subprocess.run(
            ["cargo", "build", "--release", "-p", "expanse-trie", *extra,
             "--example", "ycsb_concurrent"],
            cwd=str(REPO_ROOT), env=env, check=True,
        )
    return throughput_bin, counters_bin


# ---------------------------------------------------------------------------
# The registered cells
# ---------------------------------------------------------------------------

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


def family_blocks(per_core_layout: bool) -> dict[str, list[dict[str, Any]]]:
    """Every family block §20 registers, as (family, arm, threads) treatments.

    §20.8: "Each round runs every (arm, T) cell of a family block — the family
    and, for `olc`, its uniform twin". §20.4 opens "Common to every family" and
    fixes "T ∈ {1, 2, 4, 8}"; a family runs all five arms at those four T unless
    its row of §20.4's table narrows it.
    """
    blocks: dict[str, list[dict[str, Any]]] = {}

    def cells(family: str, arms: tuple[str, ...], threads: tuple[int, ...]) -> list[dict[str, Any]]:
        return [{"family": family, "arm": arm, "threads": t} for t in threads for arm in arms]

    # §20.4: A, B, D, F "gated"; "A0, B0, F0 | ... | G3's denominator; `olc` only".
    for fam in GATED_FAMILIES:
        block = cells(fam, ALL_ARMS, PRIMARY_THREADS)
        if fam in UNIFORM_TWINS:
            block += cells(UNIFORM_TWINS[fam], ("olc",), PRIMARY_THREADS)
        # §20.4: "On the one-thread-per-core pin only, `olc` and `mutex` also
        # run T ∈ {3, 6} in families A and B."
        if per_core_layout and fam in ("A", "B"):
            block += cells(fam, ("olc", "mutex"), tuple(bounds.LOCKED_EXTRA_THREADS))
        blocks[fam] = block

    # §20.4: "C, C0 | 100% read | Zipfian; uniform (`olc` only) | none | no —
    # anchor"; §20.6: "`dash` ... appears in A, B, C, D and F".
    blocks["C"] = cells("C", ALL_ARMS, PRIMARY_THREADS) + cells("C0", ("olc",), PRIMARY_THREADS)

    # §20.4: "Ac, Fc | as A, F | Zipfian, contiguous ranks ... | no — labels
    # only"; §20.9 names `olc`, `skip` and `dash` in them and the row narrows
    # neither arms nor T, so they run what A and F run.
    blocks["Ac"] = cells("Ac", ALL_ARMS, PRIMARY_THREADS)
    blocks["Fc"] = cells("Fc", ALL_ARMS, PRIMARY_THREADS)

    # §20.4: "A-dram, C-dram | as A, C | Zipfian over N = 2^24 ... | no —
    # anchor; `olc`, `skip`, `dash`; T ∈ {1, 8}".
    for fam in DRAM_FAMILIES:
        blocks[fam] = cells(fam, ("olc", "skip", "dash"), (1, 8))
    return blocks


def expected_population(family: str, quick: bool) -> int:
    if quick:
        return QUICK_POPULATION
    return bounds.LOCKED_POPULATION_ANCHOR if family in DRAM_FAMILIES else bounds.LOCKED_POPULATION


def expected_theta(family: str) -> float:
    return 0.0 if family in UNIFORM_FAMILIES else bounds.LOCKED_THETA


# ---------------------------------------------------------------------------
# One harness process per cell
# ---------------------------------------------------------------------------

def check_schedule(data: dict[str, Any], asked: dict[str, Any]) -> None:
    """Refuses a row disagreeing with what was asked for. Raises — never `assert`,
    which `python -O` strips."""
    for key, want in asked.items():
        got = data.get(key)
        if got != want:
            raise ScheduleMismatch(f"schedule mismatch: {key} is {got!r}, asked for {want!r}")


def check_role(data: dict[str, Any], role: str) -> None:
    if data.get("role") != role:
        raise RoleLeak(f"row reports role {data.get('role')!r}, the pass is {role!r}")
    forbidden = TIMING_FIELDS if role == "occ-stats" else COUNTER_FIELDS
    leaked = [k for k in forbidden if k in data]
    if leaked:
        raise RoleLeak(f"a {role} row carries {leaked}: timings and counters never share a build")
    required = COUNTER_FIELDS if role == "occ-stats" else TIMING_FIELDS
    missing = [k for k in required if k not in data]
    if missing:
        raise RoleLeak(f"a {role} row lacks {missing}")


def run_cell_process(bin_path: Path, role: str, family: str, arm: str, threads: int,
                     round_idx: int, position: int, quick: bool = False,
                     seed: int | None = None) -> dict[str, Any]:
    """One cell in a harness process of its own (§20.8, §15)."""
    cmd = [str(bin_path), "--role", role, "--family", family, "--arm", arm,
           "--threads", str(threads), "--round", str(round_idx), "--position", str(position)]
    if quick:
        cmd.append("--quick")
    if seed is not None:
        cmd.extend(["--seed", str(seed)])
    res = subprocess.run(cmd, cwd=str(REPO_ROOT), capture_output=True, text=True)
    if res.returncode != 0:
        raise RuntimeError(f"cell process failed (rc={res.returncode}):\ncmd: {' '.join(cmd)}\n"
                           f"stderr: {res.stderr}\nstdout: {res.stdout}")
    lines = [ln.strip() for ln in res.stdout.splitlines() if ln.strip().startswith("{")]
    if not lines:
        raise ValueError(f"no JSON emitted by cell process:\nstdout: {res.stdout}\nstderr: {res.stderr}")
    data = json.loads(lines[-1])

    asked = {
        "family": family, "arm": arm, "threads": threads, "round": round_idx,
        "position": position,
        "population": expected_population(family, quick),
        "ops_per_thread": QUICK_OPS_PER_THREAD if quick else REGISTERED_OPS_PER_THREAD,
        "theta": expected_theta(family),
    }
    if seed is not None:
        asked["seed"] = seed
    check_schedule(data, asked)
    check_role(data, role)
    return data


# ---------------------------------------------------------------------------
# Statistics
# ---------------------------------------------------------------------------

def interval(values: list[float]) -> dict[str, Any]:
    """Mean and BCa 95% interval (2,000 resamples, §20.6) with its construction label."""
    mean, lo, hi, ci_method = bca_bootstrap_ci_with_method(values)
    return {"mean": mean, "ci_lower": lo, "ci_upper": hi, "ci_method": ci_method}


def ratio_stat(values: list[float]) -> dict[str, Any]:
    iv = interval(values)
    return {"ratio_mean": iv["mean"], "ratio_ci_lower": iv["ci_lower"],
            "ratio_ci_upper": iv["ci_upper"], "ratio_ci_method": iv["ci_method"],
            "ratio_by_round": values}


def series_by_round(cell: dict[str, Any] | None, field: str = "total_mops") -> dict[int, float]:
    """`round` -> value. Pairing is by the rows' `round` field, never by list index."""
    out: dict[int, float] = {}
    if not cell:
        return out
    for row in cell.get("rounds_raw", []):
        r = row.get("round")
        if r in out:
            raise ScheduleMismatch(f"cell {cell.get('family')}/{cell.get('arm')}/T={cell.get('threads')} "
                                   f"carries round {r} twice")
        if field in row:
            out[r] = row[field]
    return out


def not_evaluable(reason: str) -> dict[str, Any]:
    return {"verdict": "NOT_EVALUABLE", "reason": reason}


def paired(rounds: int, **named: dict[int, float]) -> tuple[list[int], str | None]:
    """The registered rounds, or why the named series cannot be paired over them."""
    want = list(range(rounds))
    for name, series in named.items():
        if not series:
            return want, f"{name}: cell absent"
        if sorted(series) != want:
            missing = sorted(set(want) - set(series))
            extra = sorted(set(series) - set(want))
            return want, (f"{name}: rounds {sorted(series)} are not the registered {want} "
                          f"(missing {missing}, unexpected {extra})")
    return want, None


def three_way(stat: dict[str, Any], threshold: float, strict: bool) -> str:
    """§20.10: PASS, `REFUTED` "(interval wholly on the wrong side of the cell's
    threshold)", `INCONCLUSIVE` "(threshold inside the interval)"."""
    lo, hi = stat["ratio_ci_lower"], stat["ratio_ci_upper"]
    if (lo > threshold) if strict else (lo >= threshold):
        return "PASS"
    if hi < threshold:
        return "REFUTED"
    return "INCONCLUSIVE"


def gate(values: list[float], threshold: float, strict: bool) -> dict[str, Any]:
    stat = ratio_stat(values)
    stat["threshold"] = threshold
    stat["lower_bound_strictly_above"] = strict
    stat["verdict"] = three_way(stat, threshold, strict)
    return stat


def eval_g1(rounds: int, olc_t, olc_1, mut_t, mut_1) -> dict[str, Any]:
    """G1: R = [X_olc(T) ÷ X_olc(1)] ÷ [X_mutex(T) ÷ X_mutex(1)], lower bound strictly above 1.0."""
    rs, why = paired(rounds, olc_T=olc_t, olc_1=olc_1, mutex_T=mut_t, mutex_1=mut_1)
    if why:
        return not_evaluable(why)
    return gate([(olc_t[r] / olc_1[r]) / (mut_t[r] / mut_1[r]) for r in rs], 1.0, True)


def eval_g2(rounds: int, olc_t, mutex_by_t: dict[int, dict[int, float]]) -> dict[str, Any]:
    """G2: L = X_olc(T) ÷ max over T′ ∈ {1, 2, 4, 8} of X_mutex(T′), strictly above 1.0."""
    named = {"olc_T": olc_t}
    for t_prime in PRIMARY_THREADS:
        named[f"mutex_T{t_prime}"] = mutex_by_t.get(t_prime, {})
    rs, why = paired(rounds, **named)
    if why:
        return not_evaluable(why)
    best_t = [max(PRIMARY_THREADS, key=lambda tp: mutex_by_t[tp][r]) for r in rs]
    out = gate([olc_t[r] / mutex_by_t[bt][r] for r, bt in zip(rs, best_t)], 1.0, True)
    out["best_mutex_t_by_round"] = best_t
    return out


def eval_g3(rounds: int, zipf_t, unif_t, zipf_1, unif_1) -> dict[str, Any]:
    """G3: K = X_olc(f, T) ÷ X_olc(f0, T), lower bound at least ρ; scaling retention beside it."""
    rs, why = paired(rounds, zipfian_T=zipf_t, uniform_twin_T=unif_t)
    if why:
        return not_evaluable(why)
    out = gate([zipf_t[r] / unif_t[r] for r in rs], bounds.LOCKED_SKEW_RETENTION_FLOOR, False)
    _, why1 = paired(rounds, zipfian_1=zipf_1, uniform_twin_1=unif_1)
    if why1:
        out["scaling_retention"] = not_evaluable(why1)
    else:
        out["scaling_retention"] = ratio_stat(
            [(zipf_t[r] / zipf_1[r]) / (unif_t[r] / unif_1[r]) for r in rs])
    return out


def eval_g4(rounds: int, olc_t, skip_t) -> dict[str, Any]:
    """G4: Q = X_olc ÷ X_skip, lower bound strictly above q = 1.0. A, B and D only."""
    rs, why = paired(rounds, olc_T=olc_t, skip_T=skip_t)
    if why:
        return not_evaluable(why)
    return gate([olc_t[r] / skip_t[r] for r in rs], bounds.LOCKED_SKIPLIST_FLOOR, True)


def eval_peak(rounds: int, t8, t4) -> dict[str, Any]:
    """G6's statistic: K₈₄ = X(8) ÷ X(4), strictly above 1.0."""
    rs, why = paired(rounds, T8=t8, T4=t4)
    if why:
        return not_evaluable(why)
    return gate([t8[r] / t4[r] for r in rs], 1.0, True)


def eval_control(rounds: int, olc_1, mut_1) -> dict[str, Any]:
    """T = 1 control: P = X_olc(1) ÷ X_mutex(1). "Fails iff the interval lies
    wholly below F₁" (§20.6)."""
    rs, why = paired(rounds, olc_1=olc_1, mutex_1=mut_1)
    if why:
        return not_evaluable(why)
    stat = ratio_stat([olc_1[r] / mut_1[r] for r in rs])
    stat["threshold"] = bounds.LOCKED_COLLAPSE_GUARD
    stat["verdict"] = "FAIL" if stat["ratio_ci_upper"] < bounds.LOCKED_COLLAPSE_GUARD else "PASS"
    return stat


def eval_competitor_comparison(rounds: int, olc, comp) -> dict[str, Any]:
    """Paired X_olc ÷ X_arm with §20.6's direction label; no floor."""
    rs, why = paired(rounds, olc=olc, comparator=comp)
    if why:
        return {"label": "NOT_EVALUABLE", "reason": why}
    stat = ratio_stat([olc[r] / comp[r] for r in rs])
    if stat["ratio_ci_lower"] > 1.0:
        stat["label"] = "AHEAD"
    elif stat["ratio_ci_upper"] < 1.0:
        stat["label"] = "BEHIND"
    else:
        stat["label"] = "INCONCLUSIVE"
    return stat


def eval_efficiency(rounds: int, x_t, x_1, threads: int) -> dict[str, Any]:
    """§20.6: "the per-point efficiency E(T) = C(T) ÷ T with its interval, for
    every arm" — C(T) = X(T) ÷ X(1), paired within each round."""
    rs, why = paired(rounds, X_T=x_t, X_1=x_1)
    if why:
        return not_evaluable(why)
    out = interval([x_t[r] / x_1[r] / threads for r in rs])
    out["definition"] = "X(T) / X(1) / T, paired per round"
    return out


def compute_usl_fit(rounds: int, by_t: dict[int, dict[int, float]]) -> dict[str, Any]:
    """The reported fit (§20.6): α and β with BCa intervals, the estimator that
    actually ran, and whether the β ≥ 0 constraint binds. Decides nothing."""
    load_points = sorted(by_t)
    if len(load_points) < 4:
        return {"fit_status": "NOT_EVALUABLE", "reason": f"{len(load_points)} load points",
                "load_points": load_points}
    named = {f"T{t}": by_t[t] for t in load_points}
    rs, why = paired(rounds, **named)
    if why:
        return {"fit_status": "NOT_EVALUABLE", "reason": why, "load_points": load_points}
    n_vals = [float(t) for t in load_points]
    reps = [[by_t[t][r] for r in rs] for t in load_points]
    means = [sum(x) / len(x) for x in reps]
    # `fit_usl_with_bootstrap` indexes the point fit's `alpha` before it looks
    # at `admissible`; a curve the model cannot express is reported as such.
    point = fit_usl.fit_usl(n_vals, means)
    if not point.get("admissible", False) or "alpha" not in point:
        return {"fit_status": "INADMISSIBLE", "load_points": load_points,
                "reason": str(point.get("error") or point.get("verdict"))}
    try:
        fit = fit_usl.fit_usl_with_bootstrap(n_vals, reps, num_resamples=USL_BOOTSTRAP_RESAMPLES)
    except ValueError as err:
        return {"fit_status": "NOT_EVALUABLE", "reason": str(err), "load_points": load_points}
    out: dict[str, Any] = {
        "fit_status": "REPORTED",
        "alpha": fit["alpha"], "beta": fit["beta"], "gamma": fit["gamma"],
        # `fit_usl` names what ran: NLLS only where scipy imports, else OLS.
        "estimator": fit["estimator"],
        "load_points": load_points,
        "unclamped_beta": bounds.usl_unclamped_beta(n_vals, means),
    }
    out["beta_constraint_binds"] = out["unclamped_beta"] < 0.0
    for name in ("alpha", "beta"):
        ci = fit[f"{name}_ci"]
        if ci["estimator"] != fit["estimator"]:
            raise RuntimeError(f"{name} interval came from {ci['estimator']!r}, "
                               f"the point fit from {fit['estimator']!r}")
        out[f"{name}_ci_lower"] = ci["ci_lower"]
        out[f"{name}_ci_upper"] = ci["ci_upper"]
        out[f"{name}_ci_method"] = ci["method"]
        out[f"{name}_ci_usable"] = ci["usable"]
        out[f"{name}_ci_problems"] = ci["problems"]
    return out


# ---------------------------------------------------------------------------
# Voids: what the harness reported, checked against the registration
# ---------------------------------------------------------------------------

def expected_rank_share(k: int, population: int, theta: float) -> float:
    if theta == 0.0:
        return min(1.0, k / population)
    return bounds.gray_top_k_share(min(k, population), population, theta)


def rank_histogram_problems(hist: dict[str, Any] | None, population: int, theta: float) -> list[str]:
    if not isinstance(hist, dict):
        return ["no rank_histogram"]
    if tuple(hist.get("k", ())) != RANK_K:
        return [f"rank_histogram k set {hist.get('k')} is not the registered {list(RANK_K)}"]
    draws = hist.get("draws", 0)
    shares = hist.get("observed_share", [])
    if draws <= 0 or len(shares) != len(RANK_K):
        return ["rank_histogram carries no draws or a short share list"]
    out = []
    for k, got in zip(RANK_K, shares):
        want = expected_rank_share(k, population, theta)
        sigma = math.sqrt(max(want * (1.0 - want), 0.0) / draws)
        if abs(got - want) > RANK_TOLERANCE_SIGMAS * sigma + 1e-12:
            out.append(f"rank_histogram k={k}: observed {got:.6f} against {want:.6f} "
                       f"(tolerance {RANK_TOLERANCE_SIGMAS} sigma = {RANK_TOLERANCE_SIGMAS * sigma:.6f})")
    return out


def row_voids(row: dict[str, Any], family: str, arm: str, quick: bool) -> list[str]:
    """Every void one throughput row can raise, from what the harness observed."""
    v: list[str] = []
    population = expected_population(family, quick)
    ops = QUICK_OPS_PER_THREAD if quick else REGISTERED_OPS_PER_THREAD
    theta = expected_theta(family)
    if row.get("population") != population:
        v.append(f"population {row.get('population')} is not {population}")
    if row.get("ops_per_thread") != ops:
        v.append(f"ops_per_thread {row.get('ops_per_thread')} is not {ops}")
    if row.get("theta") != theta:
        v.append(f"theta {row.get('theta')} is not {theta}")
    idiom, value_type = REGISTERED_IDIOM[arm]
    if row.get("update_idiom") != idiom:
        v.append(f"update_idiom {row.get('update_idiom')!r} is not {idiom!r} (§20.5)")
    if row.get("value_type") != value_type:
        v.append(f"value_type {row.get('value_type')!r} is not {value_type!r} (§20.5)")
    if arm == "dash" and row.get("shard_amount") != bounds.LOCKED_DASH_SHARDS:
        v.append(f"shard_amount {row.get('shard_amount')} is not {bounds.LOCKED_DASH_SHARDS}")
    if family in RMW_FAMILIES:
        if not row.get("rmw_provider"):
            v.append("an RMW cell names no rmw_provider")
        if arm == "olc":
            if row.get("rmw_provider") != REGISTERED_OLC_RMW_PROVIDER:
                v.append(f"rmw_provider {row.get('rmw_provider')!r} is not "
                         f"{REGISTERED_OLC_RMW_PROVIDER!r} (D1)")
            if row.get("rmw_stripes") != bounds.LOCKED_RMW_STRIPES:
                v.append(f"rmw_stripes {row.get('rmw_stripes')} is not {bounds.LOCKED_RMW_STRIPES} (D1)")
    v.extend(rank_histogram_problems(row.get("rank_histogram"), population, theta))

    # §20.4: "every read in every family names a present key, so `hit_rate` is 100%".
    for field in ("read_misses", "write_misses"):
        if row.get(field) != 0:
            v.append(f"{field} is {row.get(field)!r}, not 0, against a registered 100% hit rate")

    # G5 (§20.5) and the §20.15 oracle. Every field is required: an absent
    # count is not a zero.
    if family in RMW_FAMILIES:
        if row.get("value_sum") is None or row.get("value_sum") != row.get("rmw_ops"):
            v.append(f"VOID_LOST_UPDATE: value_sum {row.get('value_sum')!r} against "
                     f"rmw_ops {row.get('rmw_ops')!r}")
        if row.get("per_key_mismatches") != 0:
            v.append(f"VOID_LOST_UPDATE: per_key_mismatches {row.get('per_key_mismatches')!r}")
        if row.get("lost_updates") != 0:
            v.append(f"VOID_LOST_UPDATE: lost_updates {row.get('lost_updates')!r}")
    elif row.get("per_key_mismatches") != 0:
        v.append(f"VOID_ORACLE: per_key_mismatches {row.get('per_key_mismatches')!r}")
    if row.get("missing_population_keys") != 0:
        v.append(f"VOID_ORACLE: missing_population_keys {row.get('missing_population_keys')!r}")
    if row.get("final_count") is None or row.get("final_count") != row.get("expected_final_count"):
        v.append(f"VOID_ORACLE: final_count {row.get('final_count')!r} against "
                 f"{row.get('expected_final_count')!r}")
    if row.get("successful_inserts") != row.get("insert_ops"):
        v.append(f"VOID_ORACLE: successful_inserts {row.get('successful_inserts')!r} of "
                 f"{row.get('insert_ops')!r} inserts")
    if row.get("oracle") != "PASS" and not any(x.startswith("VOID_") for x in v):
        v.append(f"VOID_ORACLE: harness oracle reads {row.get('oracle')!r}")
    return v


def load_voids(prov: dict[str, Any], cells: list[dict[str, Any]], quick: bool) -> list[str]:
    """METHODOLOGY.md §6's load rules, from the snapshots the run recorded."""
    v: list[str] = []
    for snap in prov.get("loads", []):
        one = snap.get("load1")
        if isinstance(one, (int, float)) and one > LOAD_AVERAGE_VOID:
            v.append(f"load average {one} above {LOAD_AVERAGE_VOID} at snapshot {snap.get('label')!r}")
    seen: set[str] = set()
    for c in cells:
        load = c.get("load") or {}
        label = str(load.get("since"))
        if label in seen:
            continue
        seen.add(label)
        foreign = load.get("foreign_busy_cpus")
        if isinstance(foreign, (int, float)) and not isinstance(foreign, bool):
            if foreign > FOREIGN_BUSY_VOID:
                v.append(f"foreign busy CPU {foreign} above {FOREIGN_BUSY_VOID} over {label}")
        elif not quick:
            v.append(f"no numeric load.foreign_busy_cpus over {label}: the block cannot be "
                     f"shown quiet (AGENTS.md §8.17)")
    return v


# ---------------------------------------------------------------------------
# The gate report
# ---------------------------------------------------------------------------

def run_level_voids(pin: str, rounds: int, quick: bool, prov: dict[str, Any] | None) -> list[str]:
    v: list[str] = []
    if quick:
        v.append("quick run: population and stream length are not the registered 1,048,576")
        return v
    if pin not in REGISTERED_PINS:
        v.append(f"pin {pin!r} is not one of the registered {list(REGISTERED_PINS)} (§20.8)")
    if rounds != REGISTERED_ROUNDS:
        v.append(f"round count {rounds} is not the registered {REGISTERED_ROUNDS} (§20.8)")
    if prov is not None:
        if prov.get("cell_isolation") != "process":
            v.append(f"cell_isolation {prov.get('cell_isolation')!r} is not 'process' (§20.8)")
        if prov.get("preregistration") != PREREGISTRATION:
            v.append(f"artifact names registration {prov.get('preregistration')!r}")
    return v


def build_gate_report(cells: list[dict[str, Any]], pin: str, rounds: int, quick: bool,
                      prov: dict[str, Any] | None = None) -> dict[str, Any]:
    """§20.6's gates over the throughput cells, with every void that can fire."""
    voids = run_level_voids(pin, rounds, quick, prov)
    cell_map = {(c["family"], c["arm"], c["threads"]): c for c in cells}

    for c in cells:
        where = f"{c['family']}/{c['arm']}/T={c['threads']}"
        rows = c.get("rounds_raw", [])
        if sorted(r.get("round") for r in rows) != list(range(rounds)):
            voids.append(f"{where}: rounds {[r.get('round') for r in rows]} are not 0..{rounds - 1}")
        for row in rows:
            for problem in row_voids(row, c["family"], c["arm"], quick):
                voids.append(f"{where} round {row.get('round')}: {problem}")
        for field in ("seed", "population", "update_idiom", "value_type"):
            if len({json.dumps(r.get(field)) for r in rows}) > 1:
                voids.append(f"{where}: `{field}` differs between rounds")
    seeds = {r.get("seed") for c in cells for r in c.get("rounds_raw", [])}
    if len(seeds) > 1:
        voids.append(f"cells ran under different seeds: {sorted(map(str, seeds))}")
    if prov is not None:
        voids.extend(load_voids(prov, cells, quick))

    def s(family: str, arm: str, t: int) -> dict[int, float]:
        return series_by_round(cell_map.get((family, arm, t)))

    families_report: dict[str, Any] = {}
    for family in GATED_FAMILIES:
        gates: dict[str, Any] = {}
        mutex_by_t = {t: s(family, "mutex", t) for t in PRIMARY_THREADS}
        olc_1, mut_1 = s(family, "olc", 1), mutex_by_t[1]

        # A registered olc or mutex cell that is absent is said so, per T, and voids.
        for arm in ("olc", "mutex"):
            for t in PRIMARY_THREADS:
                if (family, arm, t) not in cell_map:
                    voids.append(f"NOT_EVALUABLE: registered cell {family}/{arm}/T={t} is absent")

        control = eval_control(rounds, olc_1, mut_1)
        for t in GATE_THREADS:
            olc_t = s(family, "olc", t)
            gates.setdefault("G1_scaling", {})[str(t)] = eval_g1(
                rounds, olc_t, olc_1, mutex_by_t[t], mut_1)
            gates.setdefault("G2_level", {})[str(t)] = eval_g2(rounds, olc_t, mutex_by_t)
            if family in UNIFORM_TWINS:
                twin = UNIFORM_TWINS[family]
                gates.setdefault("G3_skew_retention", {})[str(t)] = eval_g3(
                    rounds, olc_t, s(twin, "olc", t), olc_1, s(twin, "olc", 1))
            if family in bounds.LOCKED_SKIPLIST_GATED_FAMILIES:
                gates.setdefault("G4_skip_list", {})[str(t)] = eval_g4(
                    rounds, olc_t, s(family, "skip", t))
        gates["G6_peak_8_over_4"] = eval_peak(rounds, s(family, "olc", 8), s(family, "olc", 4))
        if family == "F":
            bad = [x for x in voids if "VOID_LOST_UPDATE" in x and x.startswith("F/")]
            f_rows = [r for c in cells if c["family"] == "F" for r in c.get("rounds_raw", [])]
            if not f_rows:
                gates["G5_no_lost_update"] = not_evaluable("no F round present")
            else:
                gates["G5_no_lost_update"] = {
                    "verdict": "VOID_LOST_UPDATE" if bad else "PASS", "rounds_checked": len(f_rows)}

        verdicts = [g["verdict"] for g in _leaf_gates(gates)]
        # §20.8 voids "a method other than `bca`": a gate interval a defensive
        # clamp produced is not the registered statistic.
        if not quick:
            for g in [*_leaf_gates(gates), control]:
                method = g.get("ratio_ci_method")
                if method is not None and method != CI_METHOD_BCA:
                    voids.append(f"family {family}: a gate interval's method is {method!r}, "
                                 f"not {CI_METHOD_BCA!r} (§20.8)")
        families_report[family] = {
            "gates": gates,
            "controls": {"T1": control},
            "all_gate_cells_pass": all(x == "PASS" for x in verdicts),
            "claim_licensed_in_this_run": (all(x == "PASS" for x in verdicts)
                                           and control["verdict"] == "PASS" and not voids),
        }

    report = {
        "preregistration": PREREGISTRATION,
        "pin": pin,
        "rounds": rounds,
        "quick": quick,
        "void": voids,
        "evaluation": "VOID" if voids else "EVALUATED",
        "families": families_report,
        "p_a": eval_p_a(rounds, cell_map),
    }
    report["all_cells_pass_in_this_run"] = (not voids) and all(
        f["claim_licensed_in_this_run"] for f in families_report.values())
    return report


def _leaf_gates(gates: dict[str, Any]) -> list[dict[str, Any]]:
    out = []
    for g in gates.values():
        out.extend([g] if "verdict" in g else list(g.values()))
    return out


def eval_p_a(rounds: int, cell_map: dict[tuple[str, str, int], dict[str, Any]]) -> dict[str, Any]:
    """P-A (§20.7 (b′)): family A's scaling retention at T = 8 against the
    stall-only ceiling. Refuted only by "the BCa 95% interval of that statistic
    lying wholly below 0.9715, in both runs of a pin", so one artifact states
    its own reading and never the verdict."""
    cover = bounds.same_leaf_pair_scattered(bounds.LOCKED_POPULATION, bounds.LOCKED_THETA,
                                            bounds.COVER_BINS_64)
    bound = 1.0 - bounds.coincidence_loss_bound(8, bounds.write_involving_pair_fraction(0.5), cover)

    def s(family: str, t: int) -> dict[int, float]:
        return series_by_round(cell_map.get((family, "olc", t)))

    rs, why = paired(rounds, A_8=s("A", 8), A_1=s("A", 1), A0_8=s("A0", 8), A0_1=s("A0", 1))
    if why:
        return {"bound": bound, "reading_this_run": "NOT_EVALUABLE", "reason": why}
    stat = ratio_stat([(s("A", 8)[r] / s("A", 1)[r]) / (s("A0", 8)[r] / s("A0", 1)[r]) for r in rs])
    stat["bound"] = bound
    stat["reading_this_run"] = ("INTERVAL_WHOLLY_BELOW_BOUND" if stat["ratio_ci_upper"] < bound
                                else "INTERVAL_NOT_WHOLLY_BELOW_BOUND")
    stat["note"] = "P-A's verdict needs both runs of a pin (§20.7 (b′))"
    return stat


def build_comparisons(cells: list[dict[str, Any]], rounds: int) -> list[dict[str, Any]]:
    """§20.6: X_olc ÷ X_arm with AHEAD / BEHIND / INCONCLUSIVE and no floor —
    `dash` and `rwbtree` everywhere, `skip` where G4 does not gate it, every arm
    of C, Ac, Fc and the `-dram` anchors, and C against its uniform twin."""
    cell_map = {(c["family"], c["arm"], c["threads"]): c for c in cells}
    out = []
    for (family, arm, t), c in sorted(cell_map.items()):
        if arm == "olc" or family in UNIFORM_FAMILIES:
            continue
        gated_elsewhere = (family in GATED_FAMILIES and arm == "mutex") or (
            family in bounds.LOCKED_SKIPLIST_GATED_FAMILIES and arm == "skip")
        if gated_elsewhere:
            continue
        entry = {"family": family, "threads": t, "numerator": "olc", "denominator": arm,
                 "gated": False}
        entry.update(eval_competitor_comparison(
            rounds, series_by_round(cell_map.get((family, "olc", t))), series_by_round(c)))
        out.append(entry)
    for t in PRIMARY_THREADS:
        if ("C", "olc", t) in cell_map or ("C0", "olc", t) in cell_map:
            entry = {"family": "C", "threads": t, "numerator": "olc under C",
                     "denominator": "olc under C0", "gated": False}
            entry.update(eval_competitor_comparison(
                rounds, series_by_round(cell_map.get(("C", "olc", t))),
                series_by_round(cell_map.get(("C0", "olc", t)))))
            out.append(entry)
    return out


def build_peaks(cells: list[dict[str, Any]], rounds: int) -> list[dict[str, Any]]:
    """§20.6, G6: "Computed and published for every arm; gated for `olc` only."""
    cell_map = {(c["family"], c["arm"], c["threads"]): c for c in cells}
    out = []
    for family, arm in sorted({(f, a) for f, a, _ in cell_map}):
        if (family, arm, 8) not in cell_map and (family, arm, 4) not in cell_map:
            continue
        entry = {"family": family, "arm": arm,
                 "gated": family in GATED_FAMILIES and arm == "olc"}
        entry.update(eval_peak(rounds, series_by_round(cell_map.get((family, arm, 8))),
                               series_by_round(cell_map.get((family, arm, 4)))))
        if not entry["gated"]:
            # Published, never a verdict: an ungated arm carries the statistic only.
            entry["label"] = entry.pop("verdict")
        out.append(entry)
    return out


def build_usl_fits(cells: list[dict[str, Any]], rounds: int, per_core_layout: bool) -> list[dict[str, Any]]:
    """§20.6's field: "`usl_fit` per (family, arm) on the per-core pin"."""
    if not per_core_layout:
        return []
    by_pair: dict[tuple[str, str], dict[int, dict[int, float]]] = {}
    for c in cells:
        by_pair.setdefault((c["family"], c["arm"]), {})[c["threads"]] = series_by_round(c)
    out = []
    for (family, arm), by_t in sorted(by_pair.items()):
        entry = {"family": family, "arm": arm, "gated": False}
        entry.update(compute_usl_fit(rounds, by_t))
        out.append(entry)
    return out


# ---------------------------------------------------------------------------
# The artifact
# ---------------------------------------------------------------------------

def summarize_cell(c: dict[str, Any], cell_map: dict[tuple[str, str, int], dict[str, Any]],
                   pin: str, rounds: int) -> dict[str, Any]:
    out = dict(c)
    rows = c.get("rounds_raw", [])
    first = rows[0] if rows else {}
    out["cpu_pin"] = pin
    out["gated"] = c["family"] in GATED_FAMILIES and c["threads"] in PRIMARY_THREADS
    # What the harness observed, copied from its rows — never the driver's constants.
    for field in ("workload_id", "population", "ops_per_thread", "seed", "theta", "update_idiom",
                  "value_type", "shard_amount", "rmw_provider", "rmw_stripes", "mem_used"):
        if field in first:
            out[field] = first[field]
    mops = [r["total_mops"] for r in rows if "total_mops" in r]
    if mops:
        iv = interval(mops)
        out["total_mops_mean"] = iv["mean"]
        out["total_mops_ci_lower"] = iv["ci_lower"]
        out["total_mops_ci_upper"] = iv["ci_upper"]
        out["total_mops_ci_method"] = iv["ci_method"]
    out["efficiency"] = eval_efficiency(
        rounds, series_by_round(c),
        series_by_round(cell_map.get((c["family"], c["arm"], 1))), c["threads"])
    # §20.4: "the artifact publishes `reader_scaling_bounds.max_over_mean_bias`
    # over them beside every cell ... No threshold is set on it."
    biases = [reader_scaling_bounds.max_over_mean_bias(r["thread_elapsed_s"])
              for r in rows if r.get("thread_elapsed_s")]
    if biases:
        out["max_over_mean_bias"] = {"by_round": biases, "mean": sum(biases) / len(biases),
                                     "max": max(biases)}
    return out


def summarize_counters(cells: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Per-operation diagnostic counters (§20.6's last row). Never timed, never gated."""
    out = []
    for c in cells:
        rows = c.get("rounds_raw", [])
        ops = sum(r.get("ops", 0) for r in rows)
        entry = dict(c)
        entry["gated"] = False
        entry["ops_total"] = ops
        for field in ("lock_fallbacks", "lock_restarts", "read_validation_failures", "read_fallbacks"):
            total = sum(r.get(field, 0) for r in rows)
            entry[f"{field}_total"] = total
            entry[f"{field}_per_op"] = (total / ops) if ops else None
        causes: dict[str, int] = {}
        for r in rows:
            for name, n in r.get("fallback_causes_total", {}).items():
                causes[name] = causes.get(name, 0) + n
        entry["fallback_causes_total"] = causes
        entry["fallback_causes_partition_ok"] = sum(causes.values()) == entry["lock_fallbacks_total"]
        rmws = sum(r.get("rmw_ops", 0) for r in rows)
        if any("stripe_contended_acquisitions" in r for r in rows):
            contended = sum(r.get("stripe_contended_acquisitions", 0) for r in rows)
            entry["stripe_contended_per_rmw"] = (contended / rmws) if rmws else None
            entry["hottest_stripe_share"] = bounds.hottest_stripe_share(
                bounds.LOCKED_POPULATION, bounds.LOCKED_THETA, bounds.LOCKED_RMW_STRIPES)
        out.append(entry)
    return out


def first_rank_histogram(cells: list[dict[str, Any]]) -> dict[str, Any] | None:
    """`provenance.rank_histogram`: one Zipfian stream's observed shares beside
    `gray_top_k_share` of each (§20.6). Every row's histogram is checked by
    `row_voids`; this is the one the header shows."""
    for c in cells:
        if c["family"] in UNIFORM_FAMILIES:
            continue
        for row in c.get("rounds_raw", []):
            hist = row.get("rank_histogram")
            if isinstance(hist, dict) and hist.get("draws"):
                n = row["population"]
                return {
                    "from_cell": f"{c['family']}/{c['arm']}/T={c['threads']} round {row.get('round')}",
                    "stream": hist.get("stream"), "draws": hist["draws"], "population": n,
                    "k": list(hist.get("k", [])),
                    "observed_share": list(hist.get("observed_share", [])),
                    "gray_top_k_share": [expected_rank_share(k, n, bounds.LOCKED_THETA)
                                         for k in hist.get("k", [])],
                    "tolerance_sigmas": RANK_TOLERANCE_SIGMAS,
                }
    return None


def driver_sha256() -> str:
    return hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


def new_run_provenance(pin: str, rounds: int, quick: bool, run_index: int | None) -> dict[str, Any]:
    prov = new_provenance(
        suite="concurrency", issue=1006,
        ratio="every ratio is the mean of per-round quotients paired by `round` within one "
              "interleaved run (METHODOLOGY.md §20.6); its interval is BCa 95% over those quotients",
        repo_root=REPO_ROOT,
        core_pin=pin,
        cell_isolation="process",
        preregistration=PREREGISTRATION,
    )
    prov["rounds"] = rounds
    prov["quick"] = quick
    prov["run_index"] = run_index
    prov["dispatch_run_id"] = os.environ.get("GITHUB_RUN_ID")
    prov["driver_sha256"] = driver_sha256()
    prov["cell_schedule"] = "williams_rows_per_family_block"
    return prov


def format_artifact(cells: list[dict[str, Any]], counter_cells: list[dict[str, Any]],
                    prov: dict[str, Any], pin: str, rounds: int, quick: bool,
                    per_core_layout: bool) -> dict[str, Any]:
    """The artifact of §20.6. No `latency` and no `pmu` section: this driver runs neither pass."""
    cell_map = {(c["family"], c["arm"], c["threads"]): c for c in cells}
    seeds = sorted({r.get("seed") for c in cells for r in c.get("rounds_raw", [])
                    if r.get("seed") is not None})
    prov["theta"] = bounds.LOCKED_THETA
    prov["seed"] = seeds[0] if len(seeds) == 1 else seeds
    prov["rank_histogram"] = first_rank_histogram(cells)
    return {
        "provenance": prov,
        "throughput": [summarize_cell(c, cell_map, pin, rounds) for c in cells],
        "peak_8_over_4": build_peaks(cells, rounds),
        "comparisons": build_comparisons(cells, rounds),
        "usl_fit": build_usl_fits(cells, rounds, per_core_layout),
        "counters": summarize_counters(counter_cells),
        "gate_report": build_gate_report(cells, pin, rounds, quick, prov),
    }


# ---------------------------------------------------------------------------
# The run
# ---------------------------------------------------------------------------

def resolve_pin(requested: str | None) -> str:
    """Every path goes through `bench_pin.apply(`; the recorded pin is what it returned."""
    if requested:
        inherited = os.environ.get("EXPANSE_BENCH_PIN_APPLIED")
        if inherited and inherited != requested:
            raise SystemExit(f"error: --pin {requested} but the runner already applied "
                             f"{inherited}; one pin per run")
        os.environ["EXPANSE_BENCH_PIN"] = requested
    applied = bench_pin.apply("ycsb_concurrent.py")
    if requested and applied != requested:
        raise SystemExit(f"error: asked for pin {requested}, bench_pin applied {applied}")
    return applied


def pin_tag(pin: str) -> str:
    return {PIN_ALL_P: "pin0to15", PIN_PER_CORE: "pinpercore"}.get(
        pin, "pin" + "".join(ch if ch.isalnum() else "_" for ch in pin))


def default_out_path(pin: str, commit: str, run_index: int | None, quick: bool) -> tuple[Path, int | None]:
    """`gate_1006_ycsb_concurrent_<pin>_<commit>_run<N>.json`, as the #929 gate
    artifacts are named per pin and run; N is `--run`, else the first free."""
    if quick:
        return QUICK_RESULTS_DIR / f"{ARTIFACT_STEM}_quick.json", run_index
    n = run_index
    if n is None:
        n = 1
        while (RESULTS_DIR / f"{ARTIFACT_STEM}_{pin_tag(pin)}_{commit}_run{n}.json").exists():
            n += 1
    return RESULTS_DIR / f"{ARTIFACT_STEM}_{pin_tag(pin)}_{commit}_run{n}.json", n


def quick_out_refusal(out: Path, quick: bool, force: bool) -> str | None:
    """A `--quick` run never writes beside the committed results (AGENTS.md §8.5)."""
    if not quick or force:
        return None
    resolved = out.resolve()
    committed = RESULTS_DIR.resolve()
    scratch = QUICK_RESULTS_DIR.resolve()
    if committed in resolved.parents and scratch not in resolved.parents:
        return (f"error: --quick output cannot be written under the committed results path "
                f"{committed} (got {resolved}) without --force-quick-out; the default is {scratch}")
    return None


def run_block(name: str, treatments: list[dict[str, Any]], tp_bin: Path, cnt_bin: Path,
              rounds: int, quick: bool, seed: int | None, prov: dict[str, Any]
              ) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    n = len(treatments)
    sys.stderr.write(f"block {name}: {n} cells x {rounds} rounds\n")
    raw: dict[tuple[str, str, int], list[dict[str, Any]]] = {}
    # §20.8: load snapshots "before the first cell, between family blocks and mid-block".
    start = begin_cell(prov, f"block:{name}:throughput")
    for r in range(rounds):
        for pos, idx in enumerate(williams_positions(n, r)):
            t = treatments[idx]
            row = run_cell_process(tp_bin, "throughput", t["family"], t["arm"], t["threads"],
                                   r, pos, quick=quick, seed=seed)
            raw.setdefault((t["family"], t["arm"], t["threads"]), []).append(row)
        if r == (rounds - 1) // 2 and rounds > 1:
            add_load(prov, f"block:{name}:mid")
    # Closed before the counters pass, so that pass does not dilute the timed window.
    load = end_cell(start)

    cells = [{"family": f, "arm": a, "threads": t, "block": name, "load": load, "rounds_raw": rows}
             for (f, a, t), rows in raw.items()]

    counter_cells = []
    for t in (x for x in treatments if x["arm"] == "olc"):
        rows = [run_cell_process(cnt_bin, "occ-stats", t["family"], t["arm"], t["threads"],
                                 r, 0, quick=quick, seed=seed) for r in range(rounds)]
        counter_cells.append({"family": t["family"], "arm": t["arm"], "threads": t["threads"],
                              "block": name, "build": "occ-stats", "rounds_raw": rows})
    return cells, counter_cells


def print_summary(artifact: dict[str, Any]) -> None:
    rep = artifact["gate_report"]
    print(f"\nconcurrent YCSB ({PREREGISTRATION}) — pin {rep['pin']}, rounds {rep['rounds']}, "
          f"evaluation {rep['evaluation']}")
    for v in rep["void"][:20]:
        print(f"  VOID: {v}")
    if len(rep["void"]) > 20:
        print(f"  ... and {len(rep['void']) - 20} more")
    for fam, body in rep["families"].items():
        print(f"  family {fam}: control {body['controls']['T1']['verdict']}")
        for gname, g in body["gates"].items():
            leaves = {"": g} if "verdict" in g else g
            for t, leaf in leaves.items():
                iv = (f" [{leaf['ratio_ci_lower']:.4f}, {leaf['ratio_ci_upper']:.4f}]"
                      if "ratio_ci_lower" in leaf else "")
                print(f"    {gname}{' T=' + t if t else ''}: {leaf['verdict']}{iv}")
    labels: dict[str, int] = {}
    for c in artifact["comparisons"]:
        labels[c["label"]] = labels.get(c["label"], 0) + 1
    print(f"  direction labels: {labels}")
    print(f"  counters cells: {len(artifact['counters'])}; throughput cells: {len(artifact['throughput'])}")


def run(args: argparse.Namespace) -> int:
    if args.rounds < 3:
        sys.stderr.write("error: --rounds must be at least 3 for a BCa interval\n")
        return 1
    if args.run is not None and args.run < 1:
        sys.stderr.write("error: --run counts from 1\n")
        return 1
    pin = resolve_pin(args.pin)
    # `--quick` exercises the per-core block layout (the T ∈ {3, 6} cells and the
    # fit) wherever the pin is not `0-15`; a registered run takes it from the pin.
    per_core_layout = (pin == PIN_PER_CORE) or (args.quick and pin != PIN_ALL_P)
    prov = new_run_provenance(pin, args.rounds, args.quick, args.run)

    if args.out is not None:
        out, run_index = args.out, args.run
    else:
        out, run_index = default_out_path(pin, prov["commit"], args.run, args.quick)
    prov["run_index"] = run_index
    refusal = quick_out_refusal(out, args.quick, args.force_quick_out)
    if refusal:
        sys.stderr.write(refusal + "\n")
        return 1
    if out.exists() and not args.force and not args.quick:
        sys.stderr.write(f"error: {out} exists; a second dispatch takes another --run or --out "
                         f"(--force overwrites)\n")
        return 1

    blocks = family_blocks(per_core_layout)
    if args.family != "all":
        if args.family not in blocks:
            sys.stderr.write(f"error: --family {args.family!r} is not one of {list(blocks)}\n")
            return 1
        blocks = {args.family: blocks[args.family]}

    tp_bin, cnt_bin = build_binaries(verbose=True)
    cells: list[dict[str, Any]] = []
    counter_cells: list[dict[str, Any]] = []
    for name, treatments in blocks.items():
        got, got_counters = run_block(name, treatments, tp_bin, cnt_bin, args.rounds,
                                      args.quick, args.seed, prov)
        cells.extend(got)
        counter_cells.extend(got_counters)
    add_load(prov, "end")
    prov["blocks_run"] = list(blocks)

    artifact = format_artifact(cells, counter_cells, prov, pin, args.rounds, args.quick,
                               per_core_layout)
    if args.family != "all":
        artifact["gate_report"]["void"].append(
            f"only block {args.family} ran: not an evaluation of the registration")
        artifact["gate_report"]["evaluation"] = "VOID"
        artifact["gate_report"]["all_cells_pass_in_this_run"] = False
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(artifact, indent=2) + "\n")
    print_summary(artifact)
    sys.stderr.write(f"wrote {out}\n")
    return 0


# ---------------------------------------------------------------------------
# Self-test (§20.12 item 4). Every assertion below guards a production line and
# is mutation-tested: the PR records which line, disabled, turns which one red.
# ---------------------------------------------------------------------------

_JITTER = [0.00, 0.01, -0.01, 0.02, -0.02, 0.01, 0.00, -0.01]


def _synthetic_row(family: str, arm: str, threads: int, r: int, mops: float,
                   quick: bool = False) -> dict[str, Any]:
    population = expected_population(family, quick)
    theta = expected_theta(family)
    ops = QUICK_OPS_PER_THREAD if quick else REGISTERED_OPS_PER_THREAD
    rmw = ops * threads // 2 if family in RMW_FAMILIES else 0
    inserts = ops * threads // 20 if family == "D" else 0
    row: dict[str, Any] = {
        "workload_id": f"concurrency_ycsb_{family.lower().replace('-', '_')}",
        "family": family, "arm": arm, "threads": threads, "round": r, "position": 0,
        "role": "throughput", "population": population, "ops_per_thread": ops, "seed": 7,
        "theta": theta, "update_idiom": REGISTERED_IDIOM[arm][0],
        "value_type": REGISTERED_IDIOM[arm][1],
        "rank_histogram": {"stream": 0, "draws": ops, "k": list(RANK_K),
                           "observed_share": [expected_rank_share(k, population, theta) for k in RANK_K]},
        "ops": ops * threads, "read_ops": ops * threads - rmw - inserts, "write_ops": rmw + inserts,
        "rmw_ops": rmw, "insert_ops": inserts, "read_misses": 0, "write_misses": 0,
        "successful_inserts": inserts, "value_sum": rmw, "per_key_mismatches": 0,
        "lost_updates": 0, "missing_population_keys": 0, "final_count": population + inserts,
        "expected_final_count": population + inserts, "oracle": "PASS",
        "elapsed_s": 1.0, "total_mops": mops,
        "thread_elapsed_s": [1.0 - 0.01 * i for i in range(threads)],
    }
    if arm == "dash":
        row["shard_amount"] = bounds.LOCKED_DASH_SHARDS
    if family in RMW_FAMILIES:
        row["rmw_provider"] = REGISTERED_OLC_RMW_PROVIDER if arm == "olc" else "atomic_fetch_add"
        if arm == "olc":
            row["rmw_stripes"] = bounds.LOCKED_RMW_STRIPES
    return row


_GOOD_LOAD = {"since": "block:synthetic", "wall_s": 10.0, "busy_cpus_since_prev": 8.1,
              "own_busy_cpus": 8.0, "foreign_busy_cpus": 0.1}


def _synthetic_cells(per_core_layout: bool = True, rounds: int = REGISTERED_ROUNDS) -> list[dict[str, Any]]:
    base = {"olc": 5.0, "mutex": 0.0, "skip": 3.0, "dash": 6.0, "rwbtree": 1.0}
    cells = []
    for block, treatments in family_blocks(per_core_layout).items():
        for t in treatments:
            level = 4.0 if t["arm"] == "mutex" else base[t["arm"]] * t["threads"] ** 0.9
            if t["family"] in UNIFORM_FAMILIES:
                level *= 1.25
            # Each cell takes its own phase of the jitter, so that a paired
            # ratio varies between rounds and its interval is a BCa one.
            shift = 3 * len(cells) + len(t["family"])
            cells.append({
                "family": t["family"], "arm": t["arm"], "threads": t["threads"], "block": block,
                "load": dict(_GOOD_LOAD),
                "rounds_raw": [_synthetic_row(t["family"], t["arm"], t["threads"], r,
                                              level * (1 + _JITTER[(r + shift) % 8] / 10))
                               for r in range(rounds)],
            })
    return cells


def _mutated(cells: list[dict[str, Any]], family: str, arm: str, threads: int, **fields: Any
             ) -> list[dict[str, Any]]:
    """A deep copy with `fields` set on round 0 of one cell."""
    out = json.loads(json.dumps(cells))
    cell = next(c for c in out if (c["family"], c["arm"], c["threads"]) == (family, arm, threads))
    cell["rounds_raw"][0].update(fields)
    return out


def _report(cells: list[dict[str, Any]], pin: str = PIN_PER_CORE, rounds: int = REGISTERED_ROUNDS,
            quick: bool = False) -> dict[str, Any]:
    return build_gate_report(cells, pin, rounds, quick)


def _has_void(rep: dict[str, Any], needle: str) -> bool:
    return any(needle in v for v in rep["void"])


def _self_test_blocks() -> None:
    per_core, all_p = family_blocks(True), family_blocks(False)
    assert list(per_core) == ["A", "B", "D", "F", "C", "Ac", "Fc", "A-dram", "C-dram"], list(per_core)
    sizes = {k: len(v) for k, v in per_core.items()}
    # §20.8: "A block has up to 28 cells (five arms × four T, the uniform twin's
    # four, and the four per-core T ∈ {3, 6} cells)".
    assert sizes == {"A": 28, "B": 28, "D": 20, "F": 24, "C": 24, "Ac": 20, "Fc": 20,
                     "A-dram": 6, "C-dram": 6}, sizes
    assert len(all_p["A"]) == 24 and len(all_p["B"]) == 24, "T in {3, 6} is per-core only"
    assert {(c["arm"], c["threads"]) for c in per_core["A-dram"]} == {
        (a, t) for a in ("olc", "skip", "dash") for t in (1, 8)}
    assert all(c["arm"] == "olc" for c in per_core["C"] if c["family"] == "C0")
    for n in (6, 20, 24, 28):
        for r in range(8):
            assert sorted(williams_positions(n, r)) == list(range(n))


def _self_test_three_way() -> None:
    def g(lo: float, hi: float, thr: float, strict: bool) -> str:
        return three_way({"ratio_ci_lower": lo, "ratio_ci_upper": hi}, thr, strict)
    assert g(1.01, 1.2, 1.0, True) == "PASS"
    assert g(0.95, 1.2, 1.0, True) == "INCONCLUSIVE", "threshold inside the interval"
    assert g(0.80, 0.9, 1.0, True) == "REFUTED"
    assert g(1.00, 1.2, 1.0, True) == "INCONCLUSIVE", "G1/G2/G4/G6 need the bound strictly above"
    assert g(0.50, 0.6, 0.5, False) == "PASS", "G3 passes at a lower bound of exactly rho"

    rounds = REGISTERED_ROUNDS
    flat = {r: 10.0 + _JITTER[r] for r in range(rounds)}
    wide = {r: 10.0 + (3.0 if r % 2 else -3.0) for r in range(rounds)}
    low = {r: 4.0 + _JITTER[r] for r in range(rounds)}
    one = {r: 1.0 for r in range(rounds)}
    # G1..G4 each read INCONCLUSIVE when the threshold lies inside the interval.
    assert eval_g1(rounds, wide, flat, flat, flat)["verdict"] == "INCONCLUSIVE"
    assert eval_g1(rounds, low, flat, flat, flat)["verdict"] == "REFUTED"
    assert eval_g2(rounds, wide, {t: flat for t in PRIMARY_THREADS})["verdict"] == "INCONCLUSIVE"
    assert eval_g2(rounds, low, {t: flat for t in PRIMARY_THREADS})["verdict"] == "REFUTED"
    half = {r: 5.0 + (2.0 if r % 2 else -2.0) for r in range(rounds)}
    assert eval_g3(rounds, half, flat, flat, flat)["verdict"] == "INCONCLUSIVE"
    assert eval_g3(rounds, one, flat, flat, flat)["verdict"] == "REFUTED"
    assert eval_g3(rounds, flat, flat, flat, flat)["verdict"] == "PASS"
    assert eval_g4(rounds, wide, flat)["verdict"] == "INCONCLUSIVE"
    assert eval_g4(rounds, low, flat)["verdict"] == "REFUTED"
    assert eval_peak(rounds, wide, flat)["verdict"] == "INCONCLUSIVE"
    # G2 takes the control's best cell in each round, not a fixed T′.
    rising = {t: {r: float(t) for r in range(rounds)} for t in PRIMARY_THREADS}
    g2 = eval_g2(rounds, {r: 4.0 + _JITTER[r] for r in range(rounds)}, rising)
    assert g2["best_mutex_t_by_round"] == [8] * rounds and g2["verdict"] == "REFUTED", g2
    # The control fails only when the interval is wholly below F1.
    assert eval_control(rounds, one, flat)["verdict"] == "FAIL"
    assert eval_control(rounds, half, flat)["verdict"] == "PASS", "F1 inside the interval is not a failure"
    assert eval_competitor_comparison(rounds, low, flat)["label"] == "BEHIND"
    assert eval_competitor_comparison(rounds, flat, low)["label"] == "AHEAD"
    assert eval_competitor_comparison(rounds, wide, flat)["label"] == "INCONCLUSIVE"
    # A mean above 1.0 is not AHEAD while the interval still contains 1.0.
    lopsided = {r: 10.0 + (5.0 if r % 2 else -3.0) for r in range(rounds)}
    leaning = eval_competitor_comparison(rounds, lopsided, flat)
    assert leaning["ratio_mean"] > 1.0 and leaning["label"] == "INCONCLUSIVE", leaning


def _self_test_pairing_and_rounds() -> None:
    rounds = REGISTERED_ROUNDS
    # Pairing is by `round`: a cell whose rows arrive in another order gives the same ratio.
    a = {"family": "A", "arm": "olc", "threads": 2,
         "rounds_raw": [{"round": r, "total_mops": float(r + 1)} for r in range(rounds)]}
    b = {"family": "A", "arm": "skip", "threads": 2,
         "rounds_raw": [{"round": r, "total_mops": 2.0 * (r + 1)} for r in reversed(range(rounds))]}
    stat = eval_g4(rounds, series_by_round(a), series_by_round(b))
    assert stat["ratio_by_round"] == [0.5] * rounds, stat["ratio_by_round"]
    # Exactly the registered rounds: one short, or one renumbered, is NOT_EVALUABLE.
    short = {r: 1.0 for r in range(rounds - 1)}
    full = {r: 1.0 for r in range(rounds)}
    assert eval_g4(rounds, short, full)["verdict"] == "NOT_EVALUABLE"
    shifted = {r + 1: 1.0 for r in range(rounds)}
    assert eval_g4(rounds, shifted, full)["verdict"] == "NOT_EVALUABLE"
    try:
        series_by_round({"rounds_raw": [{"round": 0, "total_mops": 1.0}, {"round": 0, "total_mops": 2.0}]})
    except ScheduleMismatch:
        pass
    else:
        raise AssertionError("a round carried twice must be refused")


def _self_test_report_and_voids() -> None:
    cells = _synthetic_cells()
    rep = _report(cells)
    assert rep["void"] == [], rep["void"][:3]
    assert rep["evaluation"] == "EVALUATED" and rep["all_cells_pass_in_this_run"] is True, rep["families"]
    assert set(rep["families"]) == set(GATED_FAMILIES)
    assert "G4_skip_list" not in rep["families"]["F"]["gates"], "F has no G4 cell (§20.5)"
    assert "G3_skew_retention" not in rep["families"]["D"]["gates"]
    assert rep["families"]["F"]["gates"]["G5_no_lost_update"]["verdict"] == "PASS"
    assert abs(rep["p_a"]["bound"] - 0.971509) < 1e-6, rep["p_a"]["bound"]

    # §20.8: an interval whose method is not `bca` voids. Identical jitter in
    # two cells makes their paired ratio constant, which the estimator labels
    # `degenerate`.
    same = json.loads(json.dumps(cells))
    src = next(c for c in same if (c["family"], c["arm"], c["threads"]) == ("D", "olc", 2))
    dst = next(c for c in same if (c["family"], c["arm"], c["threads"]) == ("D", "skip", 2))
    for a, b in zip(src["rounds_raw"], dst["rounds_raw"]):
        b["total_mops"] = a["total_mops"] / 2
    assert _has_void(_report(same), "a gate interval's method is 'degenerate'"), _report(same)["void"]

    # Run-level voids.
    assert _has_void(_report(cells, pin="0-7"), "pin '0-7' is not one of the registered")
    assert _has_void(_report(cells, pin="none"), "is not one of the registered")
    assert not _report(cells, pin=PIN_ALL_P)["void"]
    assert _has_void(_report(_synthetic_cells(rounds=9), rounds=9), "round count 9 is not the registered 8")
    assert _has_void(_report(_synthetic_cells(rounds=4), rounds=4), "round count 4 is not the registered 8")
    assert _has_void(_report(cells, quick=True), "quick run")

    # A missing olc or mutex cell at a registered T: an explicit NOT_EVALUABLE and a void.
    no_mutex4 = [c for c in cells if (c["family"], c["arm"], c["threads"]) != ("A", "mutex", 4)]
    r = _report(no_mutex4)
    assert _has_void(r, "NOT_EVALUABLE: registered cell A/mutex/T=4 is absent"), r["void"]
    assert r["families"]["A"]["gates"]["G1_scaling"]["4"]["verdict"] == "NOT_EVALUABLE"
    assert r["families"]["A"]["gates"]["G2_level"]["2"]["verdict"] == "NOT_EVALUABLE", "an absent T′ cell"
    assert r["all_cells_pass_in_this_run"] is False
    no_olc8 = [c for c in cells if (c["family"], c["arm"], c["threads"]) != ("D", "olc", 8)]
    r = _report(no_olc8)
    assert _has_void(r, "NOT_EVALUABLE: registered cell D/olc/T=8 is absent")
    for gname in ("G1_scaling", "G2_level", "G4_skip_list"):
        assert r["families"]["D"]["gates"][gname]["8"]["verdict"] == "NOT_EVALUABLE", gname
    assert r["families"]["D"]["gates"]["G6_peak_8_over_4"]["verdict"] == "NOT_EVALUABLE"
    no_twin = [c for c in cells if c["family"] != "A0"]
    r = _report(no_twin)
    assert r["families"]["A"]["gates"]["G3_skew_retention"]["2"]["verdict"] == "NOT_EVALUABLE"
    assert r["families"]["A"]["claim_licensed_in_this_run"] is False
    # A missing round in one cell.
    short = json.loads(json.dumps(cells))
    next(c for c in short if (c["family"], c["arm"], c["threads"]) == ("B", "olc", 2))["rounds_raw"].pop()
    r = _report(short)
    assert _has_void(r, "B/olc/T=2: rounds")
    assert r["families"]["B"]["gates"]["G1_scaling"]["2"]["verdict"] == "NOT_EVALUABLE"

    # Row voids — one per clause, each from what the harness reported.
    cases = [
        ("F", "olc", 8, {"value_sum": 1}, "VOID_LOST_UPDATE: value_sum"),
        ("F", "skip", 8, {"per_key_mismatches": 1}, "VOID_LOST_UPDATE: per_key_mismatches"),
        ("F", "olc", 4, {"lost_updates": 3}, "VOID_LOST_UPDATE: lost_updates"),
        ("A", "olc", 8, {"per_key_mismatches": 2}, "VOID_ORACLE: per_key_mismatches"),
        ("C", "dash", 2, {"per_key_mismatches": 1}, "VOID_ORACLE: per_key_mismatches"),
        ("A", "olc", 8, {"missing_population_keys": 1}, "VOID_ORACLE: missing_population_keys"),
        ("D", "skip", 8, {"final_count": 5}, "VOID_ORACLE: final_count"),
        ("D", "olc", 8, {"successful_inserts": 1}, "VOID_ORACLE: successful_inserts"),
        ("B", "olc", 2, {"oracle": "VOID_ORACLE: something"}, "VOID_ORACLE: harness oracle reads"),
        ("A", "olc", 1, {"read_misses": 1}, "read_misses is 1"),
        ("A", "skip", 1, {"write_misses": 4}, "write_misses is 4"),
        ("A", "olc", 2, {"population": 4096}, "population 4096 is not 1048576"),
        ("A-dram", "olc", 8, {"population": 1 << 20}, "population 1048576 is not 16777216"),
        ("A", "olc", 2, {"ops_per_thread": 4096}, "ops_per_thread 4096 is not 1048576"),
        ("A", "olc", 2, {"theta": 0.5}, "theta 0.5 is not 0.99"),
        ("A0", "olc", 2, {"theta": 0.99}, "theta 0.99 is not 0.0"),
        ("A", "olc", 2, {"seed": 8}, "`seed` differs between rounds"),
        ("A", "skip", 2, {"update_idiom": "map_insert_in_place"},
         "update_idiom 'map_insert_in_place' is not 'value_cell_store'"),
        ("A", "olc", 2, {"value_type": "atomic_u64"}, "value_type 'atomic_u64' is not 'u64'"),
        ("A", "dash", 2, {"shard_amount": 32}, "shard_amount 32 is not 64"),
        ("F", "olc", 2, {"rmw_stripes": 64}, "rmw_stripes 64 is not 1024"),
        ("F", "olc", 2, {"rmw_provider": "none"}, "rmw_provider 'none' is not 'striped_lock'"),
        ("F", "skip", 2, {"rmw_provider": ""}, "names no rmw_provider"),
    ]
    for family, arm, t, fields, needle in cases:
        r = _report(_mutated(cells, family, arm, t, **fields))
        assert _has_void(r, needle), (family, arm, t, fields, r["void"][:3])
        assert r["all_cells_pass_in_this_run"] is False
    r = _report(_mutated(cells, "F", "olc", 8, value_sum=1))
    assert r["families"]["F"]["gates"]["G5_no_lost_update"]["verdict"] == "VOID_LOST_UPDATE"
    # A field the harness did not emit is a void, never a default.
    dropped = json.loads(json.dumps(cells))
    del next(c for c in dropped if (c["family"], c["arm"], c["threads"]) == ("A", "olc", 2)
             )["rounds_raw"][0]["read_misses"]
    assert _has_void(_report(dropped), "read_misses is None")
    # The rank histogram is held to the generator's law, and to the registered k set.
    hist = dict(cells[0]["rounds_raw"][0]["rank_histogram"])
    hist["observed_share"] = [x + 0.01 for x in hist["observed_share"]]
    assert _has_void(_report(_mutated(cells, "A", "olc", 1, rank_histogram=hist)), "rank_histogram k=1")
    hist["k"] = [1, 2, 5, 10, 50]
    assert _has_void(_report(_mutated(cells, "A", "olc", 1, rank_histogram=hist)), "is not the registered")
    assert _has_void(_report(_mutated(cells, "A", "olc", 1, rank_histogram=None)), "no rank_histogram")
    assert abs(expected_rank_share(16, bounds.LOCKED_POPULATION, bounds.LOCKED_THETA) - 0.232078) < 1e-6
    assert expected_rank_share(16, 1 << 20, 0.0) == 16 / (1 << 20)


def _self_test_schedule_and_roles() -> None:
    row = _synthetic_row("A", "olc", 2, 3, 10.0)
    asked = {"family": "A", "arm": "olc", "threads": 2, "round": 3, "position": 0,
             "population": bounds.LOCKED_POPULATION, "ops_per_thread": REGISTERED_OPS_PER_THREAD,
             "theta": bounds.LOCKED_THETA, "seed": 7}
    check_schedule(row, asked)
    for key, wrong in (("family", "B"), ("arm", "skip"), ("threads", 4), ("round", 2), ("position", 1),
                       ("population", 4096), ("ops_per_thread", 4096), ("theta", 0.0), ("seed", 8)):
        try:
            check_schedule(row, {**asked, key: wrong})
        except ScheduleMismatch as err:
            assert key in str(err), err
        else:
            raise AssertionError(f"a row disagreeing on `{key}` was accepted")
    check_role(row, "throughput")
    counters = {k: v for k, v in row.items() if k not in TIMING_FIELDS}
    counters.update({"role": "occ-stats", "lock_fallbacks": 0, "fallback_causes_total": {},
                     "lock_restarts": 0, "read_validation_failures": 0})
    check_role(counters, "occ-stats")
    for bad, role in (({**counters, "total_mops": 1.0}, "occ-stats"),
                      ({**counters, "elapsed_s": 1.0}, "occ-stats"),
                      ({**row, "lock_fallbacks": 0}, "throughput"),
                      ({**row, "role": "occ-stats"}, "throughput"),
                      ({k: v for k, v in counters.items() if k != "lock_restarts"}, "occ-stats")):
        try:
            check_role(bad, role)
        except RoleLeak:
            pass
        else:
            raise AssertionError(f"role leak accepted: {sorted(set(bad) ^ set(row))}")


def _self_test_artifact_shape() -> None:
    cells = _synthetic_cells()
    counter_cells = []
    for c in cells:
        if c["arm"] != "olc":
            continue
        rows = []
        for row in c["rounds_raw"]:
            cr = {k: v for k, v in row.items() if k not in TIMING_FIELDS}
            cr.update({"role": "occ-stats", "lock_fallbacks": 3,
                       "fallback_causes_total": {"contention": 2, "branch_split": 1},
                       "lock_restarts": 5, "read_validation_failures": 7, "read_fallbacks": 0})
            if c["family"] in RMW_FAMILIES:
                cr["stripe_contended_acquisitions"] = 11
            rows.append(cr)
        counter_cells.append({"family": c["family"], "arm": "olc", "threads": c["threads"],
                              "build": "occ-stats", "rounds_raw": rows})
    prov = new_run_provenance(PIN_PER_CORE, REGISTERED_ROUNDS, False, 1)
    art = format_artifact(cells, counter_cells, prov, PIN_PER_CORE, REGISTERED_ROUNDS, False, True)

    # No section is present unless a real pass produced it.
    assert "latency" not in art and "pmu" not in art and "c2c" not in art, sorted(art)
    assert sorted(art) == ["comparisons", "counters", "gate_report", "peak_8_over_4", "provenance",
                           "throughput", "usl_fit"], sorted(art)
    p = art["provenance"]
    assert p["preregistration"] == PREREGISTRATION and p["cell_isolation"] == "process"
    assert p["core_pin"] == PIN_PER_CORE and p["run_index"] == 1 and p["theta"] == bounds.LOCKED_THETA
    assert p["seed"] == 7 and len(p["driver_sha256"]) == 64
    rh = p["rank_histogram"]
    assert rh["k"] == list(RANK_K) and len(rh["observed_share"]) == 5 and len(rh["gray_top_k_share"]) == 5

    for cell in art["throughput"]:
        for field in ("family", "arm", "threads", "gated", "workload_id", "cpu_pin", "population",
                      "ops_per_thread", "seed", "update_idiom", "value_type", "load",
                      "total_mops_mean", "total_mops_ci_lower", "total_mops_ci_upper",
                      "total_mops_ci_method", "efficiency", "max_over_mean_bias"):
            assert field in cell, (field, cell["family"], cell["arm"])
        for row in cell["rounds_raw"]:
            for field in ("round", "total_mops", "elapsed_s", "ops", "read_ops", "write_ops",
                          "thread_elapsed_s", "rmw_ops", "value_sum", "per_key_mismatches"):
                assert field in row, field
        if cell["arm"] == "dash":
            assert cell["shard_amount"] == 64
        if cell["family"] in RMW_FAMILIES:
            assert cell["rmw_provider"]
            assert (cell["arm"] != "olc") or cell["rmw_stripes"] == 1024
        eff = cell["efficiency"]
        assert "ci_lower" in eff and "ci_method" in eff, eff
    # E(T) = X(T)/X(1)/T, paired: the olc arm's synthetic level is 5·T^0.9.
    a8 = next(c for c in art["throughput"] if (c["family"], c["arm"], c["threads"]) == ("A", "olc", 8))
    assert abs(a8["efficiency"]["mean"] - 8 ** 0.9 / 8) < 0.005, a8["efficiency"]
    a8_rows = {r["round"]: r["total_mops"] for r in a8["rounds_raw"]}
    a1_rows = {r["round"]: r["total_mops"] for c in art["throughput"]
               if (c["family"], c["arm"], c["threads"]) == ("A", "olc", 1) for r in c["rounds_raw"]}
    want = sum(a8_rows[r] / a1_rows[r] / 8 for r in a8_rows) / len(a8_rows)
    assert abs(a8["efficiency"]["mean"] - want) < 1e-12, (a8["efficiency"]["mean"], want)
    assert abs(a8["max_over_mean_bias"]["max"] - (1.0 / 0.965 - 1.0)) < 1e-9

    # peak_8_over_4 for every (family, arm) with those cells; gated for olc in A, B, D, F only.
    peaks = {(x["family"], x["arm"]): x for x in art["peak_8_over_4"]}
    assert ("A", "skip") in peaks and ("Ac", "rwbtree") in peaks and ("C0", "olc") in peaks
    assert peaks[("A", "olc")]["gated"] and "verdict" in peaks[("A", "olc")]
    assert not peaks[("A", "skip")]["gated"] and "verdict" not in peaks[("A", "skip")]
    assert not peaks[("C", "olc")]["gated"]
    assert peaks[("A-dram", "olc")]["label"] == "NOT_EVALUABLE", "no T = 4 anchor cell is registered"

    # Direction labels: dash and rwbtree everywhere, skip in F, every arm of the ungated families.
    comps = {(x["family"], x["denominator"], x["threads"]): x for x in art["comparisons"]}
    for key in (("A", "dash", 8), ("D", "rwbtree", 2), ("F", "skip", 4), ("C", "mutex", 8),
                ("Ac", "skip", 8), ("Fc", "dash", 1), ("A-dram", "dash", 8), ("C-dram", "skip", 1),
                ("C", "olc under C0", 8)):
        assert comps[key]["label"] in ("AHEAD", "BEHIND", "INCONCLUSIVE"), (key, comps.get(key))
    assert ("A", "skip", 8) not in comps and ("A", "mutex", 8) not in comps, "gated elsewhere"
    assert comps[("A", "dash", 8)]["label"] == "BEHIND" and comps[("A", "rwbtree", 8)]["label"] == "AHEAD"

    # The fit: per (family, arm) on the per-core pin, six load points for olc and mutex in A and B.
    fits = {(x["family"], x["arm"]): x for x in art["usl_fit"]}
    assert fits[("A", "olc")]["load_points"] == [1, 2, 3, 4, 6, 8], fits[("A", "olc")]
    assert fits[("A", "skip")]["load_points"] == [1, 2, 4, 8]
    assert fits[("A-dram", "olc")]["fit_status"] == "NOT_EVALUABLE"
    fit = fits[("B", "olc")]
    assert fit["fit_status"] == "REPORTED", fit
    assert fit["estimator"] in ("min_ssr(ols, nlls)", "ols (nlls requested; scipy not importable)"), fit
    for field in ("alpha", "beta", "alpha_ci_lower", "alpha_ci_upper", "alpha_ci_method",
                  "beta_ci_lower", "beta_ci_upper", "beta_ci_method", "beta_constraint_binds"):
        assert field in fit, field
    assert format_artifact(cells, [], new_run_provenance(PIN_ALL_P, 8, False, 1), PIN_ALL_P, 8,
                           False, False)["usl_fit"] == []

    # Counters: per operation, never a timing.
    cnt = next(x for x in art["counters"] if (x["family"], x["threads"]) == ("F", 8))
    assert cnt["lock_fallbacks_total"] == 24 and cnt["fallback_causes_partition_ok"] is True
    assert cnt["lock_restarts_per_op"] == 40 / (8 * REGISTERED_OPS_PER_THREAD * 8)
    assert cnt["stripe_contended_per_rmw"] == 88 / (8 * REGISTERED_OPS_PER_THREAD * 4)
    assert not any(k in row for x in art["counters"] for row in x["rounds_raw"] for k in TIMING_FIELDS)

    # The provenance gate, with no grandfather entry, under both artifact names.
    for rel in ("concurrency/results/gate_1006_ycsb_concurrent_pinpercore_0000000_run1.json",
                "concurrency/results/baseline_ycsb_concurrent.json"):
        assert check_bench_provenance.is_concurrent(rel), rel
        assert rel not in check_bench_provenance.GRANDFATHERED
        assert rel not in check_bench_provenance.ATTRIBUTION_GRANDFATHERED
        findings = check_bench_provenance.findings_for(rel, json.loads(json.dumps(art)))
        # Off Linux the host block has no governor map; every other finding is ours.
        findings = [f for f in findings if "scaling_governor_by_cpu" not in f or sys.platform == "linux"]
        assert findings == [], findings
    # ... and the gate does fire on this artifact when a cell loses its attribution.
    broken = json.loads(json.dumps(art))
    broken["throughput"][0]["load"]["foreign_busy_cpus"] = None
    assert any("foreign_busy_cpus" in f for f in check_bench_provenance.findings_for(
        "concurrency/results/baseline_ycsb_concurrent.json", broken))

    # Load voids, from the recorded snapshots.
    busy = json.loads(json.dumps(cells))
    for c in busy:
        if c["block"] == "A":
            c["load"]["foreign_busy_cpus"] = 2.5
    assert _has_void(build_gate_report(busy, PIN_PER_CORE, 8, False, prov), "foreign busy CPU 2.5 above 1.0")
    blind = json.loads(json.dumps(cells))
    blind[0]["load"]["foreign_busy_cpus"] = None
    assert _has_void(build_gate_report(blind, PIN_PER_CORE, 8, False, prov), "no numeric load.foreign_busy_cpus")
    loud = json.loads(json.dumps(prov))
    loud["loads"].append({"label": "block:A:mid", "load1": 13.5})
    assert _has_void(build_gate_report(cells, PIN_PER_CORE, 8, False, loud), "load average 13.5 above 12.0")
    other = dict(prov, preregistration="docs/benchmarks/concurrency/METHODOLOGY.md §19")
    assert _has_void(build_gate_report(cells, PIN_PER_CORE, 8, False, other), "artifact names registration")
    shared = dict(prov, cell_isolation="shared")
    assert _has_void(build_gate_report(cells, PIN_PER_CORE, 8, False, shared), "cell_isolation 'shared'")


def _self_test_output_paths() -> None:
    committed = RESULTS_DIR / "baseline_ycsb_concurrent.json"
    assert quick_out_refusal(committed, quick=True, force=False), "quick must not write committed results"
    assert quick_out_refusal(RESULTS_DIR / "anything.json", quick=True, force=False)
    assert quick_out_refusal(QUICK_RESULTS_DIR / "x.json", quick=True, force=False) is None
    assert quick_out_refusal(Path(tempfile.gettempdir()) / "x.json", quick=True, force=False) is None
    assert quick_out_refusal(committed, quick=False, force=False) is None
    assert quick_out_refusal(committed, quick=True, force=True) is None
    out, n = default_out_path(PIN_PER_CORE, "abc1234", 2, quick=False)
    assert out.name == "gate_1006_ycsb_concurrent_pinpercore_abc1234_run2.json" and n == 2, out
    out, n = default_out_path(PIN_ALL_P, "abc1234", None, quick=False)
    assert out.name == "gate_1006_ycsb_concurrent_pin0to15_abc1234_run1.json" and n == 1, out
    out, _ = default_out_path(PIN_PER_CORE, "abc1234", None, quick=True)
    assert out.parent == QUICK_RESULTS_DIR, out
    ignored = subprocess.run(["git", "check-ignore", "-q", str(out)], cwd=str(REPO_ROOT))
    assert ignored.returncode == 0, f"{out} is not gitignored (git check-ignore rc={ignored.returncode})"


def _self_test_pin_goes_through_apply() -> None:
    calls: list[str] = []
    saved_apply, saved_env = bench_pin.apply, dict(os.environ)

    def fake_apply(who: str = "benchmark") -> str:
        calls.append(os.environ.get("EXPANSE_BENCH_PIN", ""))
        return os.environ.get("EXPANSE_BENCH_PIN") or "none"

    bench_pin.apply = fake_apply
    try:
        os.environ.pop("EXPANSE_BENCH_PIN", None)
        os.environ.pop("EXPANSE_BENCH_PIN_APPLIED", None)
        assert resolve_pin(PIN_PER_CORE) == PIN_PER_CORE
        assert calls == [PIN_PER_CORE], "--pin must reach bench_pin.apply through EXPANSE_BENCH_PIN"
        os.environ.pop("EXPANSE_BENCH_PIN", None)
        assert resolve_pin(None) == "none" and len(calls) == 2
        os.environ["EXPANSE_BENCH_PIN_APPLIED"] = PIN_ALL_P
        try:
            resolve_pin(PIN_PER_CORE)
        except SystemExit:
            pass
        else:
            raise AssertionError("a --pin disagreeing with the inherited pin was accepted")
    finally:
        bench_pin.apply = saved_apply
        os.environ.clear()
        os.environ.update(saved_env)


def _self_test_binaries(throughput_bin: Path, counters_bin: Path) -> None:
    def refused(binary: Path, role: str, needle: str) -> None:
        p = subprocess.run([str(binary), "--role", role, "--self-test"], capture_output=True, text=True)
        # The diagnostic string, not the exit code alone (AGENTS.md §5).
        assert needle in p.stderr, (binary.name, role, p.stderr, p.stdout)
        assert p.returncode != 0 and "self-test: OK" not in p.stdout, (role, p.stdout)

    refused(throughput_bin, "occ-stats", "build/role mismatch")
    refused(counters_bin, "throughput", "build/role mismatch")
    refused(throughput_bin, "latency", "NOT_IMPLEMENTED")
    refused(counters_bin, "latency", "NOT_IMPLEMENTED")
    p = subprocess.run([str(throughput_bin), "--role", "latency", "--family", "A", "--quick"],
                       capture_output=True, text=True)
    assert "NOT_IMPLEMENTED" in p.stderr and "total_mops" not in p.stdout, (p.stdout, p.stderr)
    for binary, role in ((throughput_bin, "throughput"), (counters_bin, "occ-stats")):
        p = subprocess.run([str(binary), "--role", role, "--self-test"], capture_output=True, text=True)
        assert p.returncode == 0 and "self-test: OK" in p.stdout, (role, p.stderr)

    # One real row from each build, through the production checks.
    for family, arm, threads in (("A", "olc", 2), ("F", "olc", 2), ("D", "skip", 2), ("C0", "olc", 1),
                                 ("Fc", "dash", 2), ("A-dram", "dash", 1)):
        row = run_cell_process(throughput_bin, "throughput", family, arm, threads, 0, 0, quick=True)
        problems = row_voids(row, family, arm, quick=True)
        assert problems == [], (family, arm, problems)
        assert row["total_mops"] > 0 and len(row["thread_elapsed_s"]) == threads
        assert row["read_ops"] + row["write_ops"] == row["ops"] == threads * QUICK_OPS_PER_THREAD
    cnt = run_cell_process(counters_bin, "occ-stats", "F", "olc", 2, 0, 0, quick=True)
    assert set(cnt["fallback_causes_total"]) == {"cap_expansion", "immediate_conversion", "branch_split",
                                                 "root_growth", "contention", "unknown_tag"}
    assert sum(cnt["fallback_causes_total"].values()) == cnt["lock_fallbacks"], cnt
    assert "stripe_contended_acquisitions" in cnt and cnt["occ_read_ops"] > 0, cnt
    try:
        run_cell_process(throughput_bin, "throughput", "A", "olc", 1, 0, 0, quick=True, seed=1)
        run_cell_process(throughput_bin, "occ-stats", "A", "olc", 1, 0, 0, quick=True)
    except RuntimeError as err:
        assert "build/role mismatch" in str(err), err
    else:
        raise AssertionError("the throughput build served the occ-stats role")

    # One real block through the production path — processes, load snapshots,
    # counters pass — and the provenance gate over what it wrote.
    prov = new_run_provenance("none", 3, True, None)
    cells, counter_cells = run_block("D", family_blocks(False)["D"], throughput_bin, counters_bin,
                                     3, True, None, prov)
    add_load(prov, "end")
    art = format_artifact(cells, counter_cells, prov, "none", 3, True, False)
    assert "latency" not in art and "pmu" not in art
    assert len(art["throughput"]) == 20 and len(art["counters"]) == 4, (len(art["throughput"]),
                                                                        len(art["counters"]))
    labels = [s["label"] for s in prov["loads"]]
    assert labels == ["start", "block:D:throughput", "block:D:mid", "end"], labels
    assert all(c["load"]["since"] == "block:D:throughput" for c in art["throughput"])
    # Block D alone: void as a quick run and for the absent A, B and F cells,
    # and for nothing the D rows themselves reported. A busy host may add a
    # load void of its own; that is the rule working, not a defect here.
    assert all(v.startswith(("quick run", "NOT_EVALUABLE: registered cell", "load average",
                             "foreign busy CPU"))
               for v in art["gate_report"]["void"]), art["gate_report"]["void"]
    assert not _has_void(art["gate_report"], "registered cell D/")
    findings = check_bench_provenance.findings_for(
        "concurrency/results/baseline_ycsb_concurrent.json", json.loads(json.dumps(art)))
    if sys.platform == "linux":
        assert findings == [], findings
    else:
        # No /proc/stat off Linux, so host busy CPU cannot be attributed there.
        assert all("foreign_busy_cpus" in f or "scaling_governor" in f for f in findings), findings


def self_test(build: bool = True) -> int:
    global USL_BOOTSTRAP_RESAMPLES
    assert USL_BOOTSTRAP_RESAMPLES == 2000, "§20.6 registers 2,000 resamples"
    # The synthetic artifacts fit dozens of curves; their intervals are shape
    # checks, so the self-test alone resamples less. `run` never reaches here.
    USL_BOOTSTRAP_RESAMPLES = 100
    steps = [
        ("family blocks and Williams rows", _self_test_blocks),
        ("three-way verdicts and direction labels", _self_test_three_way),
        ("pairing by round and the exact round count", _self_test_pairing_and_rounds),
        ("gate report and voids", _self_test_report_and_voids),
        ("schedule check and role separation", _self_test_schedule_and_roles),
        ("artifact shape, efficiency, fit, counters and the provenance gate", _self_test_artifact_shape),
        ("quick output guard and artifact naming", _self_test_output_paths),
        ("pin goes through bench_pin.apply", _self_test_pin_goes_through_apply),
    ]
    for label, fn in steps:
        fn()
        sys.stderr.write(f"ok: {label}\n")
    if build:
        tp_bin, cnt_bin = build_binaries(verbose=True)
        _self_test_binaries(tp_bin, cnt_bin)
        sys.stderr.write("ok: both builds, role refusals and real rows\n")
    else:
        sys.stderr.write("SKIPPED (--no-build): both builds, role refusals and real rows\n")
    sys.stderr.write(f"ycsb_concurrent.py self-test: {len(steps) + (1 if build else 0)} of "
                     f"{len(steps) + 1} steps ran, all passed\n")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Concurrent YCSB suite for SyncExpanseMap (METHODOLOGY.md §20).")
    parser.add_argument("--out", type=Path, default=None,
                        help=f"artifact path (default: results/{ARTIFACT_STEM}_<pin>_<commit>_run<N>.json; "
                             f"--quick: results/quick/, gitignored)")
    parser.add_argument("--run", type=int, default=None,
                        help="run index at this pin and head (§20.8: two independent runs per pin)")
    parser.add_argument("--rounds", type=int, default=REGISTERED_ROUNDS,
                        help=f"rounds per cell; anything but {REGISTERED_ROUNDS} voids outside --quick")
    parser.add_argument("--quick", action="store_true", help="4,096 keys and operations; never an evaluation")
    parser.add_argument("--force-quick-out", action="store_true",
                        help="allow --quick to write under the committed results path")
    parser.add_argument("--force", action="store_true", help="overwrite an existing artifact")
    parser.add_argument("--family", type=str, default="all", help="one family block (voids the evaluation)")
    parser.add_argument("--pin", type=str, default=None,
                        help="CPU list, applied through bench_pin (default: bench_pin's own choice)")
    parser.add_argument("--seed", type=int, default=None, help="suite seed (default: the harness's)")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--no-build", action="store_true",
                        help="with --self-test: skip the two cargo builds and the steps that need them")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.self_test:
        return self_test(build=not args.no_build)
    return run(args)


if __name__ == "__main__":
    sys.exit(main())
