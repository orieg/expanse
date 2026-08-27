#!/usr/bin/env python3
"""
scripts/bca_bootstrap.py — BCa (Bias-Corrected and Accelerated) Bootstrap Confidence Interval.

Computes 95% BCa bootstrap confidence intervals for continuous benchmark latency/throughput
samples per AGENTS.md §8.4 and Rule 1.1 / B-9.
"""

from __future__ import annotations

import random
import statistics
from typing import List, Sequence, Tuple

_NORMAL = statistics.NormalDist()


def _norm_ppf(p: float) -> float:
    """Standard normal percent point function (inverse CDF)."""
    if p <= 0.0 or p >= 1.0:
        raise ValueError(f"Probability p must be in (0, 1), got {p}")
    return _NORMAL.inv_cdf(p)


def _norm_cdf(x: float) -> float:
    """Standard normal cumulative distribution function."""
    return _NORMAL.cdf(x)


def bca_bootstrap_ci(
    data: Sequence[float],
    confidence: float = 0.95,
    num_resamples: int = 2000,
    seed: int = 42,
) -> Tuple[float, float, float]:
    """Computes the (point_estimate, ci_lower, ci_upper) using BCa bootstrap.

    Args:
        data: Sequence of numeric samples (n >= 3).
        confidence: Desired confidence level (default 0.95).
        num_resamples: Number of bootstrap resamples (>= 1000).
        seed: PRNG seed for reproducibility.

    Returns:
        (mean, ci_lower, ci_upper)
    """
    n = len(data)
    if n < 3:
        raise ValueError(f"Need at least 3 data points for BCa bootstrap, got {n}")

    theta_hat = sum(data) / n
    rng = random.Random(seed)

    # 1. Bootstrap resamples
    boot_means: List[float] = []
    for _ in range(num_resamples):
        sample = [data[rng.randint(0, n - 1)] for _ in range(n)]
        boot_means.append(sum(sample) / n)
    boot_means.sort()

    # 2. Bias-correction parameter z0
    less_count = sum(1 for b in boot_means if b < theta_hat)
    prop_less = max(1e-6, min(1.0 - 1e-6, less_count / num_resamples))
    z0 = _norm_ppf(prop_less)

    # 3. Acceleration parameter a (via jackknife)
    jackknife_means: List[float] = []
    for i in range(n):
        # Sample with item i removed
        jack_sum = sum(data[j] for j in range(n) if j != i)
        jackknife_means.append(jack_sum / (n - 1))

    jack_mean_bar = sum(jackknife_means) / n
    diffs = [jack_mean_bar - m for m in jackknife_means]
    num = sum(d**3 for d in diffs)
    den = 6.0 * (sum(d**2 for d in diffs) ** 1.5)
    a = num / den if abs(den) > 1e-12 else 0.0

    # 4. Adjusted percentiles
    alpha = (1.0 - confidence) / 2.0
    z_alpha = _norm_ppf(alpha)
    z_1_minus_alpha = _norm_ppf(1.0 - alpha)

    def _adjusted_p(z_val: float) -> float:
        denom = 1.0 - a * (z0 + z_val)
        if abs(denom) < 1e-6:
            denom = 1e-6 if denom >= 0 else -1e-6
        val = z0 + (z0 + z_val) / denom
        return _norm_cdf(val)

    p1 = max(0.0, min(1.0, _adjusted_p(z_alpha)))
    p2 = max(0.0, min(1.0, _adjusted_p(z_1_minus_alpha)))

    idx1 = max(0, min(num_resamples - 1, int(p1 * num_resamples)))
    idx2 = max(0, min(num_resamples - 1, int(p2 * num_resamples)))

    ci_lower = boot_means[idx1]
    ci_upper = boot_means[idx2]

    return (theta_hat, ci_lower, ci_upper)
