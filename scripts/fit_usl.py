#!/usr/bin/env python3
"""
scripts/fit_usl.py — Gunther's Universal Scalability Law (USL) Model Fitting.

Fits Gunther's Universal Scalability Law (USL) to multi-writer throughput scaling
measurements per Issue #568 and master_plan_updated.md §5.1 / Phase 1.5B:

    X(N) = (gamma * N) / (1 + alpha * (N - 1) + beta * N * (N - 1))

where:
  - N: concurrency level (number of worker / writer threads)
  - gamma: single-worker throughput (X(1) = gamma > 0)
  - alpha: contention parameter (Amdahl's law serialization fraction, 0 <= alpha <= 1)
  - beta: coherency / crosstalk parameter (pairwise communication penalty, beta >= 0)

Calibrated Phase 1.5B gates (master plan §5.2):
  - Contention ceiling: alpha <= 0.15 with BCa 95% CI
  - Coherency crosstalk ceiling: beta <= 0.0033 with BCa 95% CI
  - Physical core domain: fit on physical P-cores only (N <= 8) to eliminate SMT and E-core distortion
  - Goodness of fit: R^2 >= 0.95, NRMSE <= 5.0%
  - Inadmissible fits (alpha > 1 or non-convergent): evaluated as REFUTED

Usage:
    python3 scripts/fit_usl.py --self-test            # Run reference-pinned unit tests
    python3 scripts/fit_usl.py                        # Fit against committed concurrency artifacts
    python3 scripts/fit_usl.py --max-n 8              # Fit on physical P-cores (N <= 8)
    python3 scripts/fit_usl.py --artifact <path>      # Fit against a specific JSON benchmark artifact

Artifacts: the two-commit A/B schema (hot_comparison/.../baseline_concurrent_ab.json, fitted
per build) and the single-commit writer_scaling.py schema (concurrency/results/baseline_writer_scaling.json,
fitted on its rounds_raw replicates) are detected per cell; anything else raises. The `str`
arm is reported but never gated.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import math
import random
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple
from unittest import mock

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

try:
    from bca_bootstrap import _bca_from_distribution  # type: ignore
except ImportError:
    _bca_from_distribution = None


# ---------------------------------------------------------------------------
# USL Analytical Model & Mathematical Primitives
# ---------------------------------------------------------------------------

def usl_throughput(n: float, gamma: float, alpha: float, beta: float) -> float:
    """Computes modeled throughput X(N) under Gunther's Universal Scalability Law.

    Args:
        n: Concurrency level (N >= 1.0).
        gamma: Uncontended single-worker throughput (gamma > 0).
        alpha: Serialization / contention coefficient (0 <= alpha <= 1).
        beta: Coherency / crosstalk coefficient (beta >= 0).

    Returns:
        Predicted throughput X(N).
    """
    if n < 1.0:
        raise ValueError(f"Concurrency N must be >= 1.0, got {n}")
    if gamma <= 0.0:
        raise ValueError(f"Single-worker throughput gamma must be > 0, got {gamma}")
    if not (0.0 <= alpha <= 1.0):
        raise ValueError(f"Contention parameter alpha must be in [0, 1], got {alpha}")
    if beta < 0.0:
        raise ValueError(f"Coherency parameter beta must be >= 0, got {beta}")

    denom = 1.0 + alpha * (n - 1.0) + beta * n * (n - 1.0)
    if denom <= 0.0:
        raise ValueError(f"USL denominator must be positive, got {denom} at N={n}")
    return (gamma * n) / denom


def usl_n_max(alpha: float, beta: float) -> float:
    """Computes the retrograde peak concurrency point N_max = sqrt((1 - alpha) / beta).

    For physical concurrency N >= 1:
      - If beta == 0 (Amdahl asymptote with zero coherency crosstalk), returns float('inf').
      - If alpha >= 1 or (1 - alpha) / beta <= 1, throughput is strictly non-increasing
        for N >= 1, so the peak on the domain N >= 1 is at N = 1.0.
      - Otherwise, returns sqrt((1 - alpha) / beta).
    """
    if beta == 0.0:
        return float("inf")
    if alpha >= 1.0:
        return 1.0
    val = (1.0 - alpha) / beta
    if val <= 1.0:
        return 1.0
    return math.sqrt(val)


def usl_peak_throughput(gamma: float, alpha: float, beta: float) -> float:
    """Computes maximum modeled throughput across all concurrency levels N >= 1."""
    n_peak = usl_n_max(alpha, beta)
    if math.isinf(n_peak):
        if alpha == 0.0:
            return float("inf")
        return gamma / alpha
    return usl_throughput(max(1.0, n_peak), gamma, alpha, beta)


def usl_goodness_of_fit(
    n_vals: Sequence[float],
    x_vals: Sequence[float],
    gamma: float,
    alpha: float,
    beta: float,
) -> Dict[str, float]:
    """Computes R^2, RMSE, and NRMSE for a fitted USL curve against empirical data."""
    k = len(n_vals)
    if k != len(x_vals) or k < 3:
        raise ValueError(f"Need at least 3 matching data points, got {len(n_vals)} and {len(x_vals)}")

    preds = [usl_throughput(ni, gamma, alpha, beta) for ni in n_vals]
    resids = [xi - pi for xi, pi in zip(x_vals, preds)]
    ss_res = sum(r**2 for r in resids)
    mean_x = sum(x_vals) / k
    ss_tot = sum((xi - mean_x) ** 2 for xi in x_vals)

    r_squared = 1.0 - (ss_res / ss_tot) if ss_tot > 1e-12 else 1.0
    rmse = math.sqrt(ss_res / k)
    nrmse = rmse / mean_x if mean_x > 1e-12 else 0.0

    return {
        "r_squared": r_squared,
        "rmse": rmse,
        "nrmse": nrmse,
        "ss_res": ss_res,
        "ss_tot": ss_tot,
    }


# ---------------------------------------------------------------------------
# Linear & Non-Linear Parameter Estimation
# ---------------------------------------------------------------------------

def _solve_3x3(a: List[List[float]], b: List[float]) -> List[float]:
    """Solves A * x = b for a 3x3 symmetric positive-semidefinite system using Cramer's rule."""
    def det3(m: List[List[float]]) -> float:
        return (
            m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
        )

    d = det3(a)
    if abs(d) < 1e-15:
        raise ValueError("Singular matrix in 3x3 linear system")

    res = []
    for col in range(3):
        a_sub = [row[:] for row in a]
        for row in range(3):
            a_sub[row][col] = b[row]
        res.append(det3(a_sub) / d)
    return res


def fit_usl_ols(n_vals: Sequence[float], x_vals: Sequence[float]) -> Tuple[float, float, float]:
    """Fits USL parameters (gamma, alpha, beta) via Gunther's linear transformation:

        Y(N) = N / X(N) = c0 + c1 * (N - 1) + c2 * N * (N - 1)
        where: gamma = 1 / c0, alpha = c1 / c0, beta = c2 / c0.
    """
    if len(n_vals) != len(x_vals) or len(n_vals) < 3:
        raise ValueError(f"Need >= 3 points for USL fitting, got {len(n_vals)}")

    # Construct normal equations for Z^T * Z * c = Z^T * Y
    a = [[0.0] * 3 for _ in range(3)]
    b = [0.0] * 3
    for ni, xi in zip(n_vals, x_vals):
        if xi <= 0.0:
            raise ValueError(f"Throughput must be positive, got {xi} at N={ni}")
        yi = ni / xi
        z = [1.0, float(ni - 1.0), float(ni * (ni - 1.0))]
        for r in range(3):
            b[r] += z[r] * yi
            for c in range(3):
                a[r][c] += z[r] * z[c]

    c0, c1, c2 = _solve_3x3(a, b)
    if c0 <= 0.0:
        raise ValueError(f"Inadmissible OLS fit: c0={c0} <= 0 (gamma must be positive)")

    gamma = 1.0 / c0
    alpha = max(0.0, min(1.0, c1 / c0))
    beta = max(0.0, c2 / c0)
    return gamma, alpha, beta


# NLLS lands on a bound to within optimiser precision (alpha = 1 - 5e-15 on the
# Phase 1.5D SWEEP), never exactly on it; one tolerance serves "at the bound"
# and "zero-width interval" alike.
_BOUND_TOL = 1e-9


def alpha_at_bound(alpha: float) -> bool:
    """True when alpha sits at its upper bound of 1, where beta is not identifiable."""
    return alpha >= 1.0 - _BOUND_TOL


def identifiability_notes(
    n_vals: Sequence[float], x_vals: Sequence[float], alpha: float
) -> List[str]:
    """Flags curves on which the fitted beta does not measure coherency.

    A retrograde curve alone is not the problem: an exact USL curve with
    alpha=0.5, beta=0.5 has C(N) < 1 at every N > 1 and both estimators recover
    it. The problem is a curve that falls and then levels off, which the
    unconstrained fit can only express with alpha > 1. alpha is then clamped to
    its bound of 1, and beta is whatever minimises the residual *given* that
    clamp. It is conditional on an imposed constraint, not a joint estimate, so
    neither beta nor its interval bounds the coherency coefficient.
    """
    notes: List[str] = []
    x1 = [x for n, x in zip(n_vals, x_vals) if n == 1.0]
    retrograde = bool(x1) and all(x < x1[0] for n, x in zip(n_vals, x_vals) if n > 1.0)
    if retrograde:
        c_n = ", ".join(f"C({n:g})={x / x1[0]:.3f}" for n, x in zip(n_vals, x_vals) if n > 1.0)
        notes.append(f"C(N) < 1 for every measured N > 1 ({c_n}): the curve is retrograde from N=2")
    if alpha_at_bound(alpha):
        notes.append(
            "beta is not identifiable: alpha pins to its bound of 1, so beta is fitted "
            "conditional on that bound, not jointly; neither beta nor its interval "
            "bounds the coherency coefficient"
        )
    return notes


def fit_usl(
    n_vals: Sequence[float],
    x_vals: Sequence[float],
    use_nlls: bool = True,
    max_n: Optional[float] = None,
) -> Dict[str, Any]:
    """Fits Gunther's USL model and evaluates all Master Plan §5.1 / Phase 1.5B criteria.

    Args:
        n_vals: Concurrency levels.
        x_vals: Measured throughput values (same length as n_vals).
        use_nlls: Whether to refine parameters using non-linear least squares.
        max_n: Optional upper bound on concurrency (e.g. 8.0 for physical P-cores).

    Returns:
        Dictionary containing parameters (gamma, alpha, beta), derived metrics (n_max,
        peak_throughput), goodness-of-fit metrics (r_squared, nrmse), individual gate
        verdicts, and overall verdict ('PASS' or 'FAIL_*' / 'REFUTED_*').
    """
    if max_n is not None:
        filtered = [(n, x) for n, x in zip(n_vals, x_vals) if n <= max_n]
        if len(filtered) < 3:
            return {
                "verdict": "REFUTED_inadmissible",
                "admissible": False,
                "error": f"Need >= 3 data points with N <= {max_n}, got {len(filtered)}",
            }
        n_vals = [p[0] for p in filtered]
        x_vals = [p[1] for p in filtered]

    try:
        gamma_ols, alpha_ols, beta_ols = fit_usl_ols(n_vals, x_vals)
    except Exception as err:
        return {
            "verdict": "REFUTED_inadmissible",
            "admissible": False,
            "error": str(err),
        }

    gamma, alpha, beta = gamma_ols, alpha_ols, beta_ols

    # Refine with non-linear least squares if scipy is available. The estimator
    # is "the lower-SSR of the OLS and NLLS candidates"; a non-convergent NLLS
    # candidate is simply absent. `estimator` names which definition ran, so a
    # bootstrap can be checked against the point fit it is reported beside.
    estimator = "ols"
    if use_nlls:
        try:
            import numpy as np  # type: ignore
            from scipy.optimize import curve_fit  # type: ignore
        except ImportError:
            estimator = "ols (nlls requested; scipy not importable)"
        else:
            estimator = "min_ssr(ols, nlls)"
            try:
                def _usl_func(n_arr, g, a, b):
                    n_arr = np.asarray(n_arr, dtype=float)
                    return (g * n_arr) / (1.0 + a * (n_arr - 1.0) + b * n_arr * (n_arr - 1.0))

                p0 = [max(1e-6, gamma_ols), max(0.0, min(0.999, alpha_ols)), max(0.0, beta_ols)]
                popt, _ = curve_fit(
                    _usl_func,
                    list(n_vals),
                    list(x_vals),
                    p0=p0,
                    bounds=([1e-6, 0.0, 0.0], [np.inf, 1.0, np.inf]),
                    maxfev=2000,
                )
                g_opt, a_opt, b_opt = float(popt[0]), float(popt[1]), float(popt[2])

                fit_ols = usl_goodness_of_fit(n_vals, x_vals, gamma, alpha, beta)
                fit_nlls = usl_goodness_of_fit(n_vals, x_vals, g_opt, a_opt, b_opt)
                if fit_nlls["ss_res"] <= fit_ols["ss_res"]:
                    gamma, alpha, beta = g_opt, a_opt, b_opt
            except Exception:
                pass

    gof = usl_goodness_of_fit(n_vals, x_vals, gamma, alpha, beta)
    n_max = usl_n_max(alpha, beta)
    peak = usl_peak_throughput(gamma, alpha, beta)

    # Master Plan §5.1 / Phase 1.5B Calibrated Gates
    admissible = (0.0 <= alpha <= 1.0) and (beta >= 0.0) and (gamma > 0.0)
    alpha_pass = alpha <= 0.15
    beta_pass = beta <= 0.0033
    # When restricted to physical cores (max_n <= 8), the coherency bound
    # beta <= 0.0033 replaces unconstrained N_max extrapolation beyond the measured domain.
    n_max_pass = True if (max_n is not None and max_n <= 8.0) else (n_max >= 16.0)
    r_squared_pass = gof["r_squared"] >= 0.95
    nrmse_pass = gof["nrmse"] <= 0.05

    if not admissible:
        verdict = "REFUTED_inadmissible"
    elif not r_squared_pass or not nrmse_pass:
        verdict = "FAIL_fit_quality"
    elif not alpha_pass:
        verdict = "FAIL_contention_ceiling"
    elif not beta_pass:
        verdict = "FAIL_coherency_crosstalk"
    elif not n_max_pass:
        verdict = "FAIL_retrograde_point"
    else:
        verdict = "PASS"

    return {
        "verdict": verdict,
        "admissible": admissible,
        "estimator": estimator,
        "notes": identifiability_notes(n_vals, x_vals, alpha),
        "gamma": gamma,
        "alpha": alpha,
        "beta": beta,
        "n_max": n_max,
        "peak_throughput": peak,
        "r_squared": gof["r_squared"],
        "rmse": gof["rmse"],
        "nrmse": gof["nrmse"],
        "gates": {
            "alpha_under_ceiling": alpha_pass,
            "beta_under_ceiling": beta_pass,
            "n_max_above_floor": n_max_pass,
            "r_squared_above_floor": r_squared_pass,
            "nrmse_under_ceiling": nrmse_pass,
        },
    }


def _ci_bounds(
    theta_hat: float,
    boot_thetas: Sequence[float],
    jackknife_thetas: Sequence[float],
    confidence: float,
) -> Tuple[float, float, str]:
    """BCa bounds, or percentile bounds when BCa cannot be computed. Returns (lo, hi, method).

    `_bca_from_distribution` returns the construction it used as a third value
    (`CI_METHOD_*`, #882), so the label is read from it rather than asserted
    here: a sample whose acceleration or bias correction degenerated is not a
    `bca` interval and must not be published as one (AGENTS.md §8.1). This
    unpacked two values until #882 widened the return, which made the `try`
    raise `ValueError` on every call and silently took the percentile branch
    below — the reason `test_ci_bounds_reaches_the_shared_bca_construction`
    asserts the label rather than only the bounds.
    """
    if _bca_from_distribution is not None and len(jackknife_thetas) > 0:
        try:
            lo, hi, method = _bca_from_distribution(
                theta_hat, list(boot_thetas), list(jackknife_thetas), confidence
            )
            return lo, hi, method
        except Exception:
            pass
    ordered = sorted(boot_thetas)
    idx_low = int((1.0 - confidence) / 2.0 * len(ordered))
    idx_high = int((1.0 + confidence) / 2.0 * len(ordered))
    return ordered[idx_low], ordered[min(idx_high, len(ordered) - 1)], "percentile"


def interval_problems(point: float, lo: float, hi: float) -> List[str]:
    """Reasons a bootstrap interval cannot be reported beside `point`; empty when usable.

    Two failure shapes, both seen on the Phase 1.5D SWEEP (map arm, beta 0.101003
    reported with CI [0.026496, 0.026496]):
      - zero width: every replicate landed on one value (a parameter bound), or
        the BCa bias correction saturated. When the point estimate lies outside
        the whole bootstrap distribution, z0 pins at its clamp and both BCa
        quantiles collapse onto the extreme replicate.
      - excludes the point estimate: AGENTS.md §8.4 requires the point estimate
        and the interval to share one definition, so the point is always enclosed.
    """
    tol = _BOUND_TOL * max(1.0, abs(point))
    problems: List[str] = []
    if hi - lo <= tol:
        problems.append("zero_width")
    if not (lo - tol <= point <= hi + tol):
        problems.append("excludes_point_estimate")
    return problems


def fit_usl_with_bootstrap(
    n_vals: Sequence[float],
    x_replicates_per_n: Sequence[Sequence[float]],
    confidence: float = 0.95,
    num_resamples: int = 2000,
    seed: int = 42,
    max_n: Optional[float] = None,
    use_nlls: bool = True,
) -> Dict[str, Any]:
    """Fits USL model and computes 95% BCa confidence intervals for contention (alpha) and coherency (beta).

    Every bootstrap resample and jackknife refit uses the same estimator as the
    point fit (`use_nlls` is passed through to all three), so the interval
    describes the estimator it is reported beside (AGENTS.md §8.4).

    An interval that is zero-width or does not enclose its point estimate is
    labelled unusable rather than reported (§8.1), as is the beta interval when
    alpha pins to its bound of 1 (see `identifiability_notes`): `ci_lower` / `ci_upper` are
    None, the rejected bounds are kept under `rejected_ci_*` for diagnosis, the
    ceiling check reads False, and a PASS verdict becomes `FAIL_ci_unusable`.
    """
    if len(n_vals) != len(x_replicates_per_n):
        raise ValueError("n_vals and x_replicates_per_n length mismatch")

    if max_n is not None:
        filtered = [(n, reps) for n, reps in zip(n_vals, x_replicates_per_n) if n <= max_n]
        if len(filtered) < 3:
            raise ValueError(f"Need >= 3 concurrency levels with N <= {max_n}, got {len(filtered)}")
        n_vals = [p[0] for p in filtered]
        x_replicates_per_n = [p[1] for p in filtered]

    k = len(n_vals)
    round_counts = [len(reps) for reps in x_replicates_per_n]
    if any(rc < 3 for rc in round_counts):
        raise ValueError(f"Each concurrency level needs >= 3 rounds for bootstrap, got {round_counts}")

    means = [sum(reps) / len(reps) for reps in x_replicates_per_n]
    point_fit = fit_usl(n_vals, means, use_nlls=use_nlls, max_n=max_n)
    alpha_hat = point_fit["alpha"]
    beta_hat = point_fit["beta"]

    rng = random.Random(seed)
    boot_alphas: List[float] = []
    boot_betas: List[float] = []
    for _ in range(num_resamples):
        resampled_means = []
        for reps, rc in zip(x_replicates_per_n, round_counts):
            s = [reps[rng.randint(0, rc - 1)] for _ in range(rc)]
            resampled_means.append(sum(s) / rc)
        b_fit = fit_usl(n_vals, resampled_means, use_nlls=use_nlls, max_n=max_n)
        if b_fit.get("admissible", False):
            boot_alphas.append(b_fit["alpha"])
            boot_betas.append(b_fit["beta"])

    if len(boot_alphas) < num_resamples // 2:
        raise ValueError("Too many inadmissible bootstrap fits during resampling")

    jackknife_alphas: List[float] = []
    jackknife_betas: List[float] = []
    for i in range(k):
        reps = x_replicates_per_n[i]
        rc = len(reps)
        for j in range(rc):
            jk_means = list(means)
            jk_means[i] = sum(reps[m] for m in range(rc) if m != j) / (rc - 1)
            jk_fit = fit_usl(n_vals, jk_means, use_nlls=use_nlls, max_n=max_n)
            if jk_fit.get("admissible", False):
                jackknife_alphas.append(jk_fit["alpha"])
                jackknife_betas.append(jk_fit["beta"])

    def _ci_entry(
        point: float, boots: List[float], jacks: List[float], ceiling: float, extra: List[str]
    ) -> Dict[str, Any]:
        lo, hi, method = _ci_bounds(point, boots, jacks, confidence)
        problems = interval_problems(point, lo, hi) + extra
        entry: Dict[str, Any] = {
            "point_estimate": point,
            "confidence": confidence,
            "method": method,
            "estimator": point_fit["estimator"],
            "usable": not problems,
            "problems": problems,
        }
        if problems:
            entry.update(ci_lower=None, ci_upper=None, rejected_ci_lower=lo, rejected_ci_upper=hi,
                         passes_ceiling=False)
        else:
            entry.update(ci_lower=lo, ci_upper=hi, passes_ceiling=hi <= ceiling)
        return entry

    # With alpha clamped at 1, beta is fitted conditional on the clamp: its
    # interval may be well-formed, but it does not bound the coherency coefficient.
    beta_extra = ["not_identifiable_alpha_at_bound"] if alpha_at_bound(alpha_hat) else []
    point_fit["alpha_ci"] = _ci_entry(alpha_hat, boot_alphas, jackknife_alphas, 0.15, [])
    point_fit["beta_ci"] = _ci_entry(beta_hat, boot_betas, jackknife_betas, 0.0033, beta_extra)

    if point_fit["verdict"] == "PASS":
        if not (point_fit["alpha_ci"]["usable"] and point_fit["beta_ci"]["usable"]):
            point_fit["verdict"] = "FAIL_ci_unusable"
        elif not point_fit["alpha_ci"]["passes_ceiling"]:
            point_fit["verdict"] = "FAIL_contention_ci_overlaps_floor"
        elif not point_fit["beta_ci"]["passes_ceiling"]:
            point_fit["verdict"] = "FAIL_coherency_ci_overlaps_ceiling"

    return point_fit


# ---------------------------------------------------------------------------
# Unit Testing & Self-Test Verification
# ---------------------------------------------------------------------------

class TestUniversalScalabilityLaw(unittest.TestCase):
    """Unit tests pinning mathematical derivations, edge cases, and known reference values."""

    def test_analytical_primitives(self):
        self.assertAlmostEqual(usl_throughput(1.0, 5000.0, 0.05, 0.001), 5000.0)
        self.assertAlmostEqual(usl_throughput(4.0, 1000.0, 0.0, 0.0), 4000.0)
        self.assertEqual(usl_n_max(0.10, 0.0), float("inf"))
        self.assertAlmostEqual(usl_peak_throughput(100.0, 0.10, 0.0), 1000.0)

        expected_n_max = math.sqrt(0.95 / 0.002)
        self.assertAlmostEqual(usl_n_max(0.05, 0.002), expected_n_max, places=5)

    def test_synthetic_exact_recovery(self):
        true_gamma, true_alpha, true_beta = 1250.0, 0.045, 0.0015
        n_vals = [1.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0]
        x_vals = [usl_throughput(ni, true_gamma, true_alpha, true_beta) for ni in n_vals]

        fit = fit_usl(n_vals, x_vals)
        self.assertEqual(fit["verdict"], "PASS")
        self.assertTrue(fit["admissible"])
        self.assertAlmostEqual(fit["gamma"], true_gamma, places=3)
        self.assertAlmostEqual(fit["alpha"], true_alpha, places=4)
        self.assertAlmostEqual(fit["beta"], true_beta, places=5)
        self.assertAlmostEqual(fit["r_squared"], 1.0, places=5)
        self.assertLess(fit["nrmse"], 1e-4)
        self.assertTrue(fit["gates"]["alpha_under_ceiling"])
        self.assertTrue(fit["gates"]["beta_under_ceiling"])
        self.assertGreaterEqual(fit["n_max"], 16.0)

    def test_contention_failure_detected(self):
        n_vals = [1.0, 2.0, 4.0, 8.0, 16.0]
        x_vals = [usl_throughput(ni, 1000.0, 0.25, 0.001) for ni in n_vals]
        fit = fit_usl(n_vals, x_vals)
        self.assertEqual(fit["verdict"], "FAIL_contention_ceiling")
        self.assertFalse(fit["gates"]["alpha_under_ceiling"])

    def test_coherency_failure_detected(self):
        n_vals = [1.0, 2.0, 4.0, 8.0]
        x_vals = [usl_throughput(ni, 1000.0, 0.05, 0.005) for ni in n_vals]
        fit = fit_usl(n_vals, x_vals)
        self.assertEqual(fit["verdict"], "FAIL_coherency_crosstalk")
        self.assertFalse(fit["gates"]["beta_under_ceiling"])

    def test_physical_core_filtering(self):
        true_gamma, true_alpha, true_beta = 2000.0, 0.04, 0.001
        n_vals = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0]
        x_vals = [usl_throughput(ni, true_gamma, true_alpha, true_beta) for ni in n_vals]
        fit = fit_usl(n_vals, x_vals, max_n=8.0)
        self.assertEqual(fit["verdict"], "PASS")
        self.assertTrue(fit["gates"]["alpha_under_ceiling"])
        self.assertTrue(fit["gates"]["beta_under_ceiling"])

    def test_invalid_parameters_raise(self):
        with self.assertRaises(ValueError):
            usl_throughput(0.5, 100.0, 0.1, 0.01)
        with self.assertRaises(ValueError):
            usl_throughput(2.0, -50.0, 0.1, 0.01)
        with self.assertRaises(ValueError):
            usl_throughput(2.0, 100.0, 1.5, 0.01)
        with self.assertRaises(ValueError):
            usl_throughput(2.0, 100.0, 0.1, -0.01)

    def test_bootstrap_ci_computation(self):
        true_gamma, true_alpha, true_beta = 2000.0, 0.06, 0.001
        n_vals = [1.0, 2.0, 4.0, 8.0, 16.0]
        rng = random.Random(1234)
        reps = []
        for ni in n_vals:
            base = usl_throughput(ni, true_gamma, true_alpha, true_beta)
            reps.append([base * (1.0 + rng.uniform(-0.02, 0.02)) for _ in range(5)])

        fit = fit_usl_with_bootstrap(n_vals, reps, num_resamples=500, seed=42)
        self.assertIn("alpha_ci", fit)
        ci_alpha = fit["alpha_ci"]
        self.assertLess(ci_alpha["ci_lower"], ci_alpha["ci_upper"])
        self.assertLess(ci_alpha["ci_upper"], 0.15)
        self.assertTrue(ci_alpha["passes_ceiling"])

        self.assertIn("beta_ci", fit)
        ci_beta = fit["beta_ci"]
        self.assertLessEqual(ci_beta["ci_lower"], ci_beta["ci_upper"])
        self.assertLessEqual(ci_beta["ci_upper"], 0.0033)
        self.assertTrue(ci_beta["passes_ceiling"])

        self.assertEqual(fit["verdict"], "PASS")

        # A well-conditioned curve keeps a non-degenerate interval that encloses
        # its point estimate, from the point fit's own estimator.
        for ci in (ci_alpha, ci_beta):
            self.assertTrue(ci["usable"], ci)
            self.assertEqual(ci["problems"], [])
            self.assertLess(ci["ci_lower"], ci["ci_upper"])
            self.assertLessEqual(ci["ci_lower"], ci["point_estimate"])
            self.assertLessEqual(ci["point_estimate"], ci["ci_upper"])
            self.assertEqual(ci["estimator"], fit["estimator"])
        self.assertEqual(fit["notes"], [])

    @staticmethod
    def _retrograde_replicates() -> Tuple[List[float], List[List[float]]]:
        """Synthetic curve shaped like the Phase 1.5D SWEEP map arm: it falls from
        N=1 and then levels off, so C(N) < 1 at every N > 1 and alpha pins to 1."""
        n_vals = [1.0, 2.0, 4.0, 8.0]
        shape = [4.97, 4.16, 3.265, 3.103]
        rng = random.Random(568)
        return n_vals, [[m * (1.0 + rng.uniform(-0.04, 0.04)) for _ in range(8)] for m in shape]

    def test_retrograde_interval_encloses_point_or_is_flagged(self):
        n_vals, reps = self._retrograde_replicates()
        fit = fit_usl_with_bootstrap(n_vals, reps, num_resamples=300, seed=42, max_n=8.0)

        for name in ("alpha_ci", "beta_ci"):
            ci = fit[name]
            if ci["ci_lower"] is not None:
                # Reported bounds must be an interval around their own point estimate.
                self.assertLess(ci["ci_lower"], ci["ci_upper"], name)
                self.assertLessEqual(ci["ci_lower"], ci["point_estimate"], name)
                self.assertLessEqual(ci["point_estimate"], ci["ci_upper"], name)
            else:
                self.assertFalse(ci.get("usable", True), name)
                self.assertTrue(ci["problems"], name)
                self.assertIsNone(ci["ci_upper"], name)
                self.assertFalse(ci["passes_ceiling"], name)

        # An unusable interval never carries bounds, even rejected ones that happen
        # to enclose the point (the alpha bounds here are 2e-14 wide).
        for name in ("alpha_ci", "beta_ci"):
            self.assertEqual(fit[name]["ci_lower"] is None, not fit[name]["usable"], name)
            self.assertEqual(fit[name]["ci_upper"] is None, not fit[name]["usable"], name)

        # On this curve both intervals are flagged, each for its own reason.
        self.assertIn("zero_width", fit["alpha_ci"]["problems"])
        self.assertIn("not_identifiable_alpha_at_bound", fit["beta_ci"]["problems"])
        self.assertTrue(alpha_at_bound(fit["alpha"]))
        self.assertNotEqual(fit["verdict"], "PASS")
        self.assertTrue(any(n.startswith("C(N) < 1 for every measured N > 1") for n in fit["notes"]))
        self.assertTrue(any(n.startswith("beta is not identifiable") for n in fit["notes"]))

    def test_interval_problems_pins_sweep_figures(self):
        # Verbatim from the Phase 1.5D SWEEP (run 34554671912, main dde0ac0b).
        self.assertEqual(interval_problems(0.101003, 0.026496, 0.026496), ["zero_width", "excludes_point_estimate"])
        self.assertEqual(interval_problems(0.343331, 0.047369, 0.047369), ["zero_width", "excludes_point_estimate"])
        self.assertEqual(interval_problems(1.0, 1.0, 1.0), ["zero_width"])
        self.assertEqual(interval_problems(0.5, 0.1, 0.3), ["excludes_point_estimate"])
        self.assertEqual(interval_problems(0.2, 0.1, 0.3), [])

    def test_bootstrap_refits_use_point_estimator(self):
        # Pins the call sites: every refit must be passed the point fit's use_nlls.
        n_vals, reps = self._retrograde_replicates()
        module = sys.modules[__name__]
        for use_nlls in (True, False):
            with mock.patch.object(module, "fit_usl", wraps=module.fit_usl) as spy:
                fit_usl_with_bootstrap(n_vals, reps, num_resamples=20, seed=1, use_nlls=use_nlls)
            seen = {call.kwargs.get("use_nlls", True) for call in spy.call_args_list}
            self.assertEqual(seen, {use_nlls})
            self.assertEqual(spy.call_count, 1 + 20 + sum(len(r) for r in reps))

    def test_ci_bounds_reaches_the_shared_bca_construction(self):
        """The BCa branch must run, and its own label must be what is reported.

        THE DEFECT THIS PINS: `_ci_bounds` unpacked two values from
        `_bca_from_distribution` and returned a hard-coded `"bca"`. #882 widened
        that return to `(lo, hi, method)`, so the unpack raised `ValueError`
        inside the `except Exception: pass` and every interval silently became
        the plain-percentile fallback while the report kept saying BCa. A test
        that only checked the bounds stayed green, because the fallback also
        returns bounds — so this asserts the label, and that the shared
        construction was actually called.
        """
        if _bca_from_distribution is None:
            self.skipTest("scripts/bca_bootstrap.py not importable")
        boots = [1.0 + 0.01 * i for i in range(200)]
        jacks = [1.0, 1.02, 0.97, 1.05, 0.99]
        lo, hi, method = _ci_bounds(1.5, boots, jacks, 0.95)
        self.assertEqual(method, "bca", "the shared BCa branch did not run")
        self.assertLess(lo, hi)
        # A point estimate outside the bootstrap support clamps the bias
        # correction, and that is reported rather than passed off as `bca`.
        self.assertEqual(_ci_bounds(0.5, boots, jacks, 0.95)[2], "clamped")
        # And a degenerate sample is reported as degenerate, not as `bca`: the
        # label is read from the construction, never asserted by this module.
        flat = [7.0] * 200
        self.assertEqual(_ci_bounds(7.0, flat, [7.0] * 5, 0.95)[2], "degenerate")
        # Every label the construction can return must be printable (§8.1).
        for label in ("bca", "bc", "clamped", "degenerate", "percentile"):
            self.assertIn(label, _CI_METHOD_LABELS)

    def test_unusable_interval_blocks_pass(self):
        # A curve that PASSes must not keep PASS beside an interval that was rejected.
        n_vals = [1.0, 2.0, 4.0, 8.0, 16.0]
        rng = random.Random(1234)
        reps = [[usl_throughput(ni, 2000.0, 0.06, 0.001) * (1.0 + rng.uniform(-0.02, 0.02)) for _ in range(5)]
                for ni in n_vals]
        module = sys.modules[__name__]
        collapsed = lambda point, boots, jacks, conf: (point, point, "bca")  # noqa: E731
        with mock.patch.object(module, "_ci_bounds", side_effect=collapsed):
            fit = fit_usl_with_bootstrap(n_vals, reps, num_resamples=50, seed=42)
        self.assertEqual(fit["alpha_ci"]["problems"], ["zero_width"])
        self.assertIsNone(fit["alpha_ci"]["ci_upper"])
        self.assertEqual(fit["verdict"], "FAIL_ci_unusable")

    def test_identifiability_notes(self):
        n_vals = [1.0, 2.0, 4.0, 8.0]
        # Retrograde from N=2, yet exact USL data: alpha is recovered, so beta is identifiable.
        exact = [usl_throughput(ni, 1.0, 0.5, 0.5) for ni in n_vals]
        fit = fit_usl(n_vals, exact)
        self.assertAlmostEqual(fit["alpha"], 0.5, places=6)
        self.assertEqual(len(fit["notes"]), 1)
        self.assertTrue(fit["notes"][0].startswith("C(N) < 1 for every measured N > 1"))
        # A scaling curve carries no note.
        scaling = [usl_throughput(ni, 1.0, 0.05, 0.001) for ni in n_vals]
        self.assertEqual(fit_usl(n_vals, scaling)["notes"], [])

    def test_single_commit_artifact_loader(self):
        arms = {"map": (6.0, 0.05, 0.001), "set": (7.0, 0.40, 0.001), "str": (3.0, 1.0, 0.0)}
        data = _synthetic_single_commit_artifact(arms, writers=[1, 2, 4, 8, 16])
        inputs = artifact_fit_inputs(data, max_n=8.0)
        self.assertEqual(inputs["schema"], SCHEMA_SINGLE_COMMIT)
        # `str` is excluded from the gate, and says so.
        self.assertEqual([s["arm"] for s in inputs["series"]], ["set", "map"])
        self.assertTrue(any("str" in n and "excluded" in n for n in inputs["notices"]))

        by_arm = {s["arm"]: s for s in inputs["series"]}
        map_cells = {c["writers"]: c for c in data["throughput"] if c["arm"] == "map"}
        self.assertEqual(by_arm["map"]["build"], "commit 0123456789ab")
        self.assertEqual(by_arm["map"]["n_vals"], [1.0, 2.0, 4.0, 8.0])
        self.assertEqual(
            by_arm["map"]["reps"],
            [[r["writer_mops"] for r in map_cells[w]["rounds_raw"]] for w in (1, 2, 4, 8)],
        )
        self.assertEqual(
            by_arm["map"]["x_vals"], [map_cells[w]["expanse_writer_mops_mean"] for w in (1, 2, 4, 8)]
        )

        # End to end through the file loader that raised KeyError on c["base"].
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "writer_scaling.json"
            path.write_text(json.dumps(data))
            with contextlib.redirect_stdout(io.StringIO()) as out:
                results = evaluate_artifact(path, max_n=8.0)
        self.assertIn("str", out.getvalue())
        fits = {r["arm"]: r["fit"] for r in results}
        # Same estimator and ceilings as the A/B path: the loader only supplies replicates.
        direct = fit_usl_with_bootstrap(
            by_arm["map"]["n_vals"], by_arm["map"]["reps"], num_resamples=1000, max_n=8.0
        )
        self.assertEqual(fits["map"]["alpha_ci"], direct["alpha_ci"])
        self.assertEqual(fits["map"]["beta_ci"], direct["beta_ci"])
        self.assertEqual(fits["map"]["verdict"], "PASS")
        self.assertEqual(fits["set"]["verdict"], "FAIL_contention_ceiling")

    def test_single_commit_artifact_rejects_malformed(self):
        def fresh():
            return _synthetic_single_commit_artifact({"map": (6.0, 0.05, 0.001)}, writers=[1, 2, 4, 8])

        cases = {}
        d = fresh(); del d["throughput"][1]["expanse_writer_mops_mean"]
        cases["cell matching neither schema"] = d
        d = fresh(); d["throughput"][1]["base"] = {}; d["throughput"][1]["head"] = {}
        cases["mixed schemas"] = d
        d = fresh(); cell = d["throughput"][2]; cell["rounds_raw"].pop()
        # Keep the headline consistent with the remaining rows so only the count check can fire.
        kept = [r["writer_mops"] for r in cell["rounds_raw"]]
        cell["expanse_writer_mops_mean"] = round(sum(kept) / len(kept), 4)
        cases["rounds_raw shorter than rounds"] = d
        d = fresh(); d["throughput"][2]["expanse_writer_mops_mean"] += 0.01
        cases["headline mean not from rounds_raw"] = d
        d = fresh(); d["throughput"].append(dict(d["throughput"][3]))
        cases["duplicate writer count"] = d
        d = fresh(); d["throughput"][0]["arm"] = "hot"
        cases["unrecognised arm"] = d
        d = fresh()
        for c in d["throughput"]:
            c["arm"] = "str"
        cases["no gate arm"] = d
        d = fresh(); d["throughput"][0]["rounds_raw"][0]["writer_mops"] = 0.0
        cases["non-positive replicate"] = d
        for name, bad in cases.items():
            with self.subTest(name), self.assertRaises(ValueError):
                artifact_fit_inputs(bad)

    def test_ab_artifact_loader_unchanged(self):
        n_vals = [1, 2, 4, 8]
        cells = []
        for arm, gamma in (("set", 7.0), ("map", 5.0)):
            for w in n_vals:
                cell = {"arm": arm, "writers": w, "readers": 0, "rounds_raw": []}
                for build, scale in (("base", 1.0), ("head", 0.8)):
                    x = usl_throughput(float(w), gamma * scale, 0.05, 0.001)
                    samples = [round(x * m, 4) for m in (0.99, 1.01, 0.995, 1.005)]
                    cell[build] = {"expanse_writer_mops_median": sorted(samples)[2]}
                    cell["rounds_raw"] += [
                        {"round": i, "build": build, "expanse_writer_mops": v} for i, v in enumerate(samples)
                    ]
                # Rows the A/B loader always discarded: a null sample.
                cell["rounds_raw"].append({"round": 4, "build": "base", "expanse_writer_mops": None})
                cells.append(cell)
            cells.append({"arm": arm, "writers": 0, "readers": 8, "base": {}, "head": {}, "rounds_raw": []})
        data = {"throughput": cells}

        inputs = artifact_fit_inputs(data)
        self.assertEqual(inputs["schema"], SCHEMA_AB)
        self.assertEqual(
            [(s["arm"], s["build"]) for s in inputs["series"]],
            [("set", "base"), ("set", "head"), ("map", "base"), ("map", "head")],
        )
        for s in inputs["series"]:
            arm_cells = [c for c in cells if c["arm"] == s["arm"] and c["readers"] == 0]
            self.assertEqual(s["n_vals"], [1.0, 2.0, 4.0, 8.0])
            self.assertEqual(s["x_vals"], [c[s["build"]]["expanse_writer_mops_median"] for c in arm_cells])
            self.assertEqual(
                s["reps"],
                [
                    [r["expanse_writer_mops"] for r in c["rounds_raw"]
                     if r["build"] == s["build"] and r["expanse_writer_mops"] is not None]
                    for c in arm_cells
                ],
            )

        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "baseline_concurrent_ab.json"
            path.write_text(json.dumps(data))
            with contextlib.redirect_stdout(io.StringIO()):
                results = evaluate_artifact(path)
        set_base = inputs["series"][0]
        direct = fit_usl_with_bootstrap(set_base["n_vals"], set_base["reps"], num_resamples=1000)
        self.assertEqual(results[0]["fit"]["alpha_ci"], direct["alpha_ci"])
        self.assertEqual(results[0]["fit"]["beta_ci"], direct["beta_ci"])


def _synthetic_single_commit_artifact(
    arms: Dict[str, Tuple[float, float, float]], writers: Sequence[int]
) -> Dict[str, Any]:
    """A baseline_writer_scaling.json-shaped artifact whose replicates lie on a known USL curve.

    The four multipliers average to exactly 1.0, so each cell's replicate mean is the
    model value and the headline mean is its 4-decimal rounding, as writer_scaling.py emits.
    """
    multipliers = (0.99, 1.01, 0.995, 1.005)
    cells = []
    for arm, (gamma, alpha, beta) in arms.items():
        for w in writers:
            x = usl_throughput(float(w), gamma, alpha, beta)
            samples = [x * m for m in multipliers]
            cells.append({
                "arm": arm,
                "writers": w,
                "readers": 0,
                "rounds": len(samples),
                "expanse_writer_mops_mean": round(sum(samples) / len(samples), 4),
                "expanse_writer_mops_median": round(sorted(samples)[len(samples) // 2], 4),
                "rounds_raw": [
                    {"round": i, "position": i, "writer_mops": v, "writer_elapsed_s": 1.0, "write_ops": 1000}
                    for i, v in enumerate(samples)
                ],
            })
    return {"provenance": {"commit": "0123456789abcdef"}, "throughput": cells}


# ---------------------------------------------------------------------------
# Artifact Loading
# ---------------------------------------------------------------------------

# Arms the Phase 1.5B gate is evaluated on.
GATE_ARMS = ("set", "map")
# `str` is a reference curve, not a candidate: SyncExpanseStrMap::insert takes the
# writer mutex, so its contention coefficient is 1 by construction.
REFERENCE_ARMS = ("str",)

# Two-commit A/B artifact (hot_comparison/results/multi_writer_olc/baseline_concurrent_ab.json):
# each cell carries `base`/`head` sub-dicts and build-tagged `rounds_raw` rows.
SCHEMA_AB = "ab"
# Single-commit artifact (concurrency/results/baseline_writer_scaling.json): each cell carries a
# top-level `expanse_writer_mops_mean` and untagged `rounds_raw[].writer_mops` rows.
SCHEMA_SINGLE_COMMIT = "single_commit"

# writer_scaling.py rounds the headline mean to 4 decimals.
_MEAN_ROUNDING_TOL = 5e-5 + 1e-9


def _cell_schema(cell: Dict[str, Any]) -> Optional[str]:
    if isinstance(cell.get("base"), dict) and isinstance(cell.get("head"), dict):
        return SCHEMA_AB
    rounds = cell.get("rounds_raw")
    if (
        "expanse_writer_mops_mean" in cell
        and isinstance(rounds, list)
        and rounds
        and all(isinstance(r, dict) and "writer_mops" in r for r in rounds)
    ):
        return SCHEMA_SINGLE_COMMIT
    return None


def _cell_label(cell: Dict[str, Any]) -> str:
    return f"arm={cell.get('arm')!r} writers={cell.get('writers')!r} readers={cell.get('readers')!r}"


def detect_schema(data: Any) -> str:
    """Returns SCHEMA_AB or SCHEMA_SINGLE_COMMIT; raises ValueError on anything else (§8.1).

    Every writer-only cell must name a known arm and match one schema, and all of them
    must match the same one — a cell that is skipped instead is a silently dropped arm.
    """
    throughput = data.get("throughput") if isinstance(data, dict) else None
    if not isinstance(throughput, list):
        raise ValueError("unrecognised artifact schema: no `throughput` list")

    writer_only = [c for c in throughput if isinstance(c, dict) and c.get("readers") == 0]
    unknown = sorted({str(c.get("arm")) for c in writer_only} - set(GATE_ARMS + REFERENCE_ARMS))
    if unknown:
        raise ValueError(
            f"unrecognised arm(s) {unknown}: expected gate arms {list(GATE_ARMS)} "
            f"or reference arms {list(REFERENCE_ARMS)}"
        )
    gate_cells = [c for c in writer_only if c.get("arm") in GATE_ARMS]
    if not gate_cells:
        raise ValueError(f"no writer-only (readers == 0) cells for gate arms {list(GATE_ARMS)}")

    schemas = {}
    for c in gate_cells:
        schema = _cell_schema(c)
        if schema is None:
            raise ValueError(
                f"unrecognised cell schema ({_cell_label(c)}): expected `base`/`head` sub-dicts "
                f"(A/B) or `expanse_writer_mops_mean` with `rounds_raw[].writer_mops` "
                f"(single-commit); cell keys {sorted(c)}"
            )
        schemas.setdefault(schema, _cell_label(c))
    if len(schemas) > 1:
        raise ValueError(f"mixed cell schemas in one artifact: {schemas}")
    return next(iter(schemas))


def _single_commit_replicates(cell: Dict[str, Any]) -> List[float]:
    reps = [float(r["writer_mops"]) for r in cell["rounds_raw"]]
    label = _cell_label(cell)
    if len(reps) < 3:
        raise ValueError(f"{label}: {len(reps)} rounds_raw rows, the bootstrap needs >= 3")
    if "rounds" in cell and len(reps) != int(cell["rounds"]):
        raise ValueError(f"{label}: {len(reps)} rounds_raw rows but rounds = {cell['rounds']}")
    if any(not math.isfinite(x) or x <= 0.0 for x in reps):
        raise ValueError(f"{label}: non-positive or non-finite writer_mops in rounds_raw: {reps}")
    mean = sum(reps) / len(reps)
    headline = float(cell["expanse_writer_mops_mean"])
    if abs(mean - headline) > _MEAN_ROUNDING_TOL:
        raise ValueError(
            f"{label}: rounds_raw mean {mean:.6f} != expanse_writer_mops_mean {headline} — "
            f"the replicates are not the population the headline was computed from"
        )
    return reps


def artifact_fit_inputs(data: Any, max_n: Optional[float] = None) -> Dict[str, Any]:
    """Extracts per-arm USL fit inputs from an A/B or single-commit artifact.

    Returns {"schema", "series", "notices"}; each series is
    {"arm", "build", "n_vals", "x_vals", "reps"}. Arms that are absent or have fewer
    than three concurrency levels produce a notice rather than disappearing.
    """
    schema = detect_schema(data)
    throughput = data["throughput"]
    notices: List[str] = []
    series: List[Dict[str, Any]] = []

    for arm in REFERENCE_ARMS:
        if any(c.get("arm") == arm and c.get("readers") == 0 for c in throughput):
            notices.append(
                f"arm {arm}: excluded from the gate — coarse-mutex reference curve (alpha = 1 by construction)"
            )

    for arm in GATE_ARMS:
        cells = [t for t in throughput if t.get("arm") == arm and t.get("readers") == 0]
        if not cells:
            notices.append(f"arm {arm}: no writer-only cells in artifact — not evaluated")
            continue
        cells.sort(key=lambda c: c.get("writers", 0))
        if max_n is not None:
            cells = [c for c in cells if float(c.get("writers", 0)) <= max_n]
        if len(cells) < 3:
            notices.append(
                f"arm {arm}: {len(cells)} concurrency level(s) with N <= {max_n}, the fit needs >= 3 — not evaluated"
            )
            continue
        n_vals = [float(c["writers"]) for c in cells]

        if schema == SCHEMA_AB:
            for build in ("base", "head"):
                x_vals = [float(c[build]["expanse_writer_mops_median"]) for c in cells]
                reps = []
                for c in cells:
                    round_samples = [
                        float(r["expanse_writer_mops"])
                        for r in c.get("rounds_raw", [])
                        if r.get("build") == build and r.get("expanse_writer_mops") is not None
                    ]
                    reps.append(round_samples)
                series.append({"arm": arm, "build": build, "n_vals": n_vals, "x_vals": x_vals, "reps": reps})
        else:
            if len(set(n_vals)) != len(n_vals):
                raise ValueError(f"arm {arm}: duplicate writer counts {n_vals}")
            commit = (data.get("provenance") or {}).get("commit")
            build = f"commit {commit[:12]}" if isinstance(commit, str) and commit else "single-commit"
            reps = [_single_commit_replicates(c) for c in cells]
            x_vals = [float(c["expanse_writer_mops_mean"]) for c in cells]
            series.append({"arm": arm, "build": build, "n_vals": n_vals, "x_vals": x_vals, "reps": reps})

    return {"schema": schema, "series": series, "notices": notices}


# How a `_ci_bounds` label prints. The first four are the shared estimator's
# `CI_METHOD_*` vocabulary (#882) and name which of BCa's two corrections
# survived the sample; `percentile` is this module's own fallback, taken when
# the shared module is absent or its construction raised. A label outside this
# map is a vocabulary the reporter has not been taught, and the KeyError saying
# so is the intended behaviour (AGENTS.md §8.1) — never a silent "BCa".
_CI_METHOD_LABELS = {
    "bca": "BCa",
    "bc": "bias-corrected (BCa acceleration degenerated)",
    "clamped": "clamped (a BCa correction was bounded)",
    "degenerate": "degenerate (the bootstrap distribution is one point)",
    "percentile": "percentile",
}


def _format_ci(ci: Dict[str, Any]) -> str:
    """One report line for an interval; an unusable one is never printed as bounds."""
    method = _CI_METHOD_LABELS[ci["method"]]
    label = f"{method} {ci['confidence'] * 100:g}% CI"
    if not ci["usable"]:
        return (f"{label}: ⚠️ UNUSABLE ({', '.join(ci['problems'])}); rejected bounds "
                f"[{ci['rejected_ci_lower']:.6f}, {ci['rejected_ci_upper']:.6f}] are not reported")
    return f"{label}: [{ci['ci_lower']:.6f}, {ci['ci_upper']:.6f}] (passes: {ci['passes_ceiling']})"


def evaluate_artifact(path: Path, max_n: Optional[float] = None) -> List[Dict[str, Any]]:
    data = json.loads(path.read_text())
    inputs = artifact_fit_inputs(data, max_n=max_n)
    title_suffix = f" (N <= {max_n})" if max_n is not None else ""
    print(f"=== Universal Scalability Law (USL) Fit: {path.name}{title_suffix} ===")
    for notice in inputs["notices"]:
        print(f"notice: {notice}")

    # The single-commit headline is the mean (writer_scaling.py); the A/B headline is the median.
    x_label = "Throughput mean (M ops/s)" if inputs["schema"] == SCHEMA_SINGLE_COMMIT else "Throughput (M ops/s)"
    results: List[Dict[str, Any]] = []
    for s in inputs["series"]:
        arm, build, n_vals, x_vals, reps = s["arm"], s["build"], s["n_vals"], s["x_vals"], s["reps"]
        if all(len(r) >= 3 for r in reps):
            fit = fit_usl_with_bootstrap(n_vals, reps, num_resamples=1000, max_n=max_n)
        else:
            fit = fit_usl(n_vals, x_vals, max_n=max_n)
        results.append({"arm": arm, "build": build, "fit": fit})

        print(f"\nArm: {arm} | Build: {build} | Concurrency: {n_vals}")
        print(f"  {x_label}: {x_vals}")
        print(f"  Verdict:       {fit['verdict']}")
        print(f"  gamma (W=1):   {fit['gamma']:.4f} M ops/s")
        print(f"  alpha (cont):  {fit['alpha']:.6f} (ceiling <= 0.15: {fit['gates']['alpha_under_ceiling']})")
        if "alpha_ci" in fit:
            print(f"    alpha {_format_ci(fit['alpha_ci'])}")
        print(f"  beta (coher):  {fit['beta']:.6f} (ceiling <= 0.0033: {fit['gates']['beta_under_ceiling']})")
        if "beta_ci" in fit:
            print(f"    beta  {_format_ci(fit['beta_ci'])}")
        n_max_str = "inf" if math.isinf(fit['n_max']) else f"{fit['n_max']:.2f}"
        print(f"  N_max (peak):  {n_max_str} (floor >= 16: {fit['gates']['n_max_above_floor']})")
        print(f"  Goodness of Fit: R^2 = {fit['r_squared']:.4f} (>= 0.95: {fit['gates']['r_squared_above_floor']}), "
              f"NRMSE = {fit['nrmse']*100:.2f}% (<= 5%: {fit['gates']['nrmse_under_ceiling']})")
        print(f"  Estimator:     {fit['estimator']}")
        for note in fit["notes"]:
            print(f"  Note: {note}")
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description="Fit Gunther's USL model to scalability benchmarks.")
    parser.add_argument("--self-test", action="store_true", help="Run self-test unit tests")
    parser.add_argument("--artifact", type=str, help="Path to JSON benchmark artifact to fit")
    parser.add_argument("--max-n", type=float, default=None, help="Maximum concurrency N to include in fit (e.g. 8 for physical P-cores)")
    args = parser.parse_args()

    if args.self_test:
        sys.argv = [sys.argv[0]]
        unittest.main()
        return

    if args.artifact:
        evaluate_artifact(Path(args.artifact), max_n=args.max_n)
        return

    # Default report on committed baseline artifact
    default_artifact = REPO_ROOT / "docs" / "benchmarks" / "hot_comparison" / "results" / "multi_writer_olc" / "baseline_concurrent_ab.json"
    if default_artifact.exists():
        evaluate_artifact(default_artifact, max_n=args.max_n)
    else:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestUniversalScalabilityLaw)
        runner = unittest.TextTestRunner(verbosity=2)
        res = runner.run(suite)
        if not res.wasSuccessful():
            sys.exit(1)


if __name__ == "__main__":
    main()
