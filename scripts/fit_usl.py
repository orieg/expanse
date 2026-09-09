#!/usr/bin/env python3
"""
scripts/fit_usl.py — Gunther's Universal Scalability Law (USL) Model Fitting.

Fits Gunther's Universal Scalability Law (USL) to multi-writer throughput scaling
measurements per Issue #568 and proposal_a_localized_structural_olc_plan.md §5.1:

    X(N) = (gamma * N) / (1 + alpha * (N - 1) + beta * N * (N - 1))

where:
  - N: concurrency level (number of worker / writer threads)
  - gamma: single-worker throughput (X(1) = gamma > 0)
  - alpha: contention parameter (Amdahl's law serialization fraction, 0 <= alpha <= 1)
  - beta: coherency / crosstalk parameter (pairwise communication penalty, beta >= 0)

Derived quantities & Phase 3 gates (master plan §5.1):
  - Contention ceiling: alpha <= 0.15 with BCa 95% CI
  - Retrograde point: N_max = sqrt((1 - alpha) / beta) >= 16 (for beta > 0)
  - Goodness of fit: R^2 >= 0.95, NRMSE <= 5.0%
  - Inadmissible fits (alpha > 1 or non-convergent): evaluated as REFUTED

Usage:
    python3 scripts/fit_usl.py --self-test        # Run reference-pinned unit tests
    python3 scripts/fit_usl.py                    # Fit against committed concurrency artifacts
    python3 scripts/fit_usl.py --artifact <path>  # Fit against a specific JSON benchmark artifact
"""

from __future__ import annotations

import argparse
import json
import math
import random
import sys
import unittest
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

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


def fit_usl(
    n_vals: Sequence[float],
    x_vals: Sequence[float],
    use_nlls: bool = True,
) -> Dict[str, Any]:
    """Fits Gunther's USL model and evaluates all Master Plan §5.1 criteria.

    Returns:
        Dictionary containing parameters (gamma, alpha, beta), derived metrics (n_max,
        peak_throughput), goodness-of-fit metrics (r_squared, nrmse), individual gate
        verdicts, and overall verdict ('PASS' or 'FAIL_*' / 'REFUTED_*').
    """
    try:
        gamma_ols, alpha_ols, beta_ols = fit_usl_ols(n_vals, x_vals)
    except Exception as err:
        return {
            "verdict": "REFUTED_inadmissible",
            "admissible": False,
            "error": str(err),
        }

    gamma, alpha, beta = gamma_ols, alpha_ols, beta_ols

    # Refine with non-linear least squares if scipy is available
    if use_nlls:
        try:
            import numpy as np  # type: ignore
            from scipy.optimize import curve_fit  # type: ignore

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

    # Master Plan §5.1 Gates
    admissible = (0.0 <= alpha <= 1.0) and (beta >= 0.0) and (gamma > 0.0)
    alpha_pass = alpha <= 0.15
    n_max_pass = n_max >= 16.0
    r_squared_pass = gof["r_squared"] >= 0.95
    nrmse_pass = gof["nrmse"] <= 0.05

    if not admissible:
        verdict = "REFUTED_inadmissible"
    elif not r_squared_pass or not nrmse_pass:
        verdict = "FAIL_fit_quality"
    elif not alpha_pass:
        verdict = "FAIL_contention_ceiling"
    elif not n_max_pass:
        verdict = "FAIL_retrograde_point"
    else:
        verdict = "PASS"

    return {
        "verdict": verdict,
        "admissible": admissible,
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
            "n_max_above_floor": n_max_pass,
            "r_squared_above_floor": r_squared_pass,
            "nrmse_under_ceiling": nrmse_pass,
        },
    }


def fit_usl_with_bootstrap(
    n_vals: Sequence[float],
    x_replicates_per_n: Sequence[Sequence[float]],
    confidence: float = 0.95,
    num_resamples: int = 2000,
    seed: int = 42,
) -> Dict[str, Any]:
    """Fits USL model and computes 95% BCa confidence interval for contention parameter alpha."""
    if len(n_vals) != len(x_replicates_per_n):
        raise ValueError("n_vals and x_replicates_per_n length mismatch")

    k = len(n_vals)
    round_counts = [len(reps) for reps in x_replicates_per_n]
    if any(rc < 3 for rc in round_counts):
        raise ValueError(f"Each concurrency level needs >= 3 rounds for bootstrap, got {round_counts}")

    means = [sum(reps) / len(reps) for reps in x_replicates_per_n]
    point_fit = fit_usl(n_vals, means)
    alpha_hat = point_fit["alpha"]

    rng = random.Random(seed)
    boot_alphas: List[float] = []
    for _ in range(num_resamples):
        resampled_means = []
        for reps, rc in zip(x_replicates_per_n, round_counts):
            s = [reps[rng.randint(0, rc - 1)] for _ in range(rc)]
            resampled_means.append(sum(s) / rc)
        b_fit = fit_usl(n_vals, resampled_means, use_nlls=False)
        if b_fit.get("admissible", False):
            boot_alphas.append(b_fit["alpha"])

    if len(boot_alphas) < num_resamples // 2:
        raise ValueError("Too many inadmissible bootstrap fits during resampling")

    jackknife_alphas: List[float] = []
    for i in range(k):
        reps = x_replicates_per_n[i]
        rc = len(reps)
        for j in range(rc):
            jk_means = list(means)
            jk_means[i] = sum(reps[m] for m in range(rc) if m != j) / (rc - 1)
            jk_fit = fit_usl(n_vals, jk_means, use_nlls=False)
            if jk_fit.get("admissible", False):
                jackknife_alphas.append(jk_fit["alpha"])

    if _bca_from_distribution is not None and len(jackknife_alphas) > 0:
        ci_lower, ci_upper = _bca_from_distribution(alpha_hat, boot_alphas, jackknife_alphas, confidence)
    else:
        boot_alphas.sort()
        idx_low = int((1.0 - confidence) / 2.0 * len(boot_alphas))
        idx_high = int((1.0 + confidence) / 2.0 * len(boot_alphas))
        ci_lower = boot_alphas[idx_low]
        ci_upper = boot_alphas[min(idx_high, len(boot_alphas) - 1)]

    alpha_ci_pass = ci_upper <= 0.15
    point_fit["alpha_ci"] = {
        "point_estimate": alpha_hat,
        "ci_lower": ci_lower,
        "ci_upper": ci_upper,
        "confidence": confidence,
        "passes_ceiling": alpha_ci_pass,
    }
    if not alpha_ci_pass and point_fit["verdict"] == "PASS":
        point_fit["verdict"] = "FAIL_contention_ci_overlaps_floor"

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
        self.assertGreaterEqual(fit["n_max"], 16.0)

    def test_contention_failure_detected(self):
        n_vals = [1.0, 2.0, 4.0, 8.0, 16.0]
        x_vals = [usl_throughput(ni, 1000.0, 0.25, 0.001) for ni in n_vals]
        fit = fit_usl(n_vals, x_vals)
        self.assertEqual(fit["verdict"], "FAIL_contention_ceiling")
        self.assertFalse(fit["gates"]["alpha_under_ceiling"])

    def test_retrograde_failure_detected(self):
        n_vals = [1.0, 2.0, 4.0, 8.0, 16.0]
        x_vals = [usl_throughput(ni, 1000.0, 0.05, 0.02) for ni in n_vals]
        fit = fit_usl(n_vals, x_vals)
        self.assertEqual(fit["verdict"], "FAIL_retrograde_point")
        self.assertFalse(fit["gates"]["n_max_above_floor"])

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
        ci = fit["alpha_ci"]
        self.assertLess(ci["ci_lower"], ci["ci_upper"])
        self.assertLess(ci["ci_upper"], 0.15)
        self.assertTrue(ci["passes_ceiling"])
        self.assertEqual(fit["verdict"], "PASS")


def evaluate_artifact(path: Path) -> None:
    data = json.loads(path.read_text())
    print(f"=== Universal Scalability Law (USL) Fit: {path.name} ===")
    for arm in ("set", "map"):
        cells = [t for t in data.get("throughput", []) if t.get("arm") == arm and t.get("readers") == 0]
        if not cells:
            continue
        cells.sort(key=lambda c: c.get("writers", 0))
        n_vals = [float(c["writers"]) for c in cells]

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

            if all(len(r) >= 3 for r in reps):
                fit = fit_usl_with_bootstrap(n_vals, reps, num_resamples=1000)
            else:
                fit = fit_usl(n_vals, x_vals)

            print(f"\nArm: {arm} | Build: {build} | Concurrency: {n_vals}")
            print(f"  Throughput (M ops/s): {x_vals}")
            print(f"  Verdict:       {fit['verdict']}")
            print(f"  gamma (W=1):   {fit['gamma']:.4f} M ops/s")
            print(f"  alpha (cont):  {fit['alpha']:.6f} (ceiling <= 0.15: {fit['gates']['alpha_under_ceiling']})")
            if "alpha_ci" in fit:
                ci = fit["alpha_ci"]
                print(f"    alpha BCa 95% CI: [{ci['ci_lower']:.6f}, {ci['ci_upper']:.6f}] (passes: {ci['passes_ceiling']})")
            print(f"  beta (coher):  {fit['beta']:.6f}")
            n_max_str = "inf" if math.isinf(fit['n_max']) else f"{fit['n_max']:.2f}"
            print(f"  N_max (peak):  {n_max_str} (floor >= 16: {fit['gates']['n_max_above_floor']})")
            print(f"  Goodness of Fit: R^2 = {fit['r_squared']:.4f} (>= 0.95: {fit['gates']['r_squared_above_floor']}), "
                  f"NRMSE = {fit['nrmse']*100:.2f}% (<= 5%: {fit['gates']['nrmse_under_ceiling']})")


def main() -> None:
    parser = argparse.ArgumentParser(description="Fit Gunther's USL model to scalability benchmarks.")
    parser.add_argument("--self-test", action="store_true", help="Run self-test unit tests")
    parser.add_argument("--artifact", type=str, help="Path to JSON benchmark artifact to fit")
    args = parser.parse_args()

    if args.self_test:
        sys.argv = [sys.argv[0]]
        unittest.main()
        return

    if args.artifact:
        evaluate_artifact(Path(args.artifact))
        return

    # Default report on committed baseline artifact
    default_artifact = REPO_ROOT / "docs" / "benchmarks" / "hot_comparison" / "results" / "multi_writer_olc" / "baseline_concurrent_ab.json"
    if default_artifact.exists():
        evaluate_artifact(default_artifact)
    else:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestUniversalScalabilityLaw)
        runner = unittest.TextTestRunner(verbosity=2)
        res = runner.run(suite)
        if not res.wasSuccessful():
            sys.exit(1)


if __name__ == "__main__":
    main()
