#!/usr/bin/env python3
"""
scripts/bca_bootstrap.py — BCa (Bias-Corrected and Accelerated) Bootstrap Confidence Interval.

Computes 95% BCa bootstrap confidence intervals for continuous benchmark latency/throughput
samples per AGENTS.md §8.4 and Rule 1.1 / B-9.

This is the repository's single BCa implementation: every wall-clock gate, the
per-suite drivers under `docs/benchmarks/**/scripts/`, and the on-device
harvesters reach it from here. `_bca_from_distribution` is the one place the
construction is written down — the one-sample, ratio and on-device paths differ
only in which bootstrap and jackknife replicates they hand it.

Reporting the construction
--------------------------
BCa's two corrections can both degenerate on a sample, and this module's
defensive clamps then bound the result rather than failing. The clamps land on
the right answer (an all-identical sample gets its point as both endpoints),
but a three-value return cannot say *which* construction produced an interval,
so "this is a BCa interval" becomes an assumption rather than something the
artifact states. The `*_with_method` entry points return a fourth value naming
it, from the `CI_METHOD_*` vocabulary below (AGENTS.md §8.1: a degradation is
reported, never silently substituted).

The three-value entry points are unchanged — same numerics, same signature,
same returns — because six suites carry committed intervals that came from
them.
"""

from __future__ import annotations

import random
import statistics
from typing import List, Sequence, Tuple

_NORMAL = statistics.NormalDist()

# How an interval was constructed, most degraded first. `method` carries one of
# these; the precedence when several apply is exactly this order, so the label
# always names the worst degradation the sample caused.
#
#   degenerate — the bootstrap distribution is a single point (every resample
#                gave the same statistic, which for the mean means every datum
#                is identical). Both endpoints are that point. The corrections
#                are vacuous rather than wrong.
#   clamped    — a defensive clamp bound the result: the bias-correction
#                proportion hit `[1e-6, 1-1e-6]` (the observed statistic sits
#                outside the bootstrap support, so `z0` is not finite), or the
#                adjusted-percentile denominator `1 - a*(z0+z)` was pinned away
#                from zero. The endpoints are that clamped construction's, not
#                a BCa interval as written.
#   bc         — the jackknife acceleration denominator vanished, so `a = 0`.
#                What remains is Efron's bias-corrected (BC) percentile
#                interval, one correction short of BCa.
#   bca        — neither correction degenerated.
CI_METHOD_BCA = "bca"
CI_METHOD_BC = "bc"
CI_METHOD_CLAMPED = "clamped"
CI_METHOD_DEGENERATE = "degenerate"


def _norm_ppf(p: float) -> float:
    """Standard normal percent point function (inverse CDF)."""
    if p <= 0.0 or p >= 1.0:
        raise ValueError(f"Probability p must be in (0, 1), got {p}")
    return _NORMAL.inv_cdf(p)


def _norm_cdf(x: float) -> float:
    """Standard normal cumulative distribution function."""
    return _NORMAL.cdf(x)


def _bca_from_distribution(
    theta_hat: float,
    boot_thetas: List[float],
    jackknife_thetas: List[float],
    confidence: float,
) -> Tuple[float, float, str]:
    """Turns a bootstrap distribution plus jackknife replicates into BCa bounds.

    Returns ``(ci_lower, ci_upper, method)``, where ``method`` is one of the
    ``CI_METHOD_*`` constants and names the construction that produced the
    endpoints. The arithmetic below is the module's original BCa construction,
    unchanged; the flags only observe which branches it took.
    """
    num_resamples = len(boot_thetas)
    boot_thetas = sorted(boot_thetas)

    less_count = sum(1 for b in boot_thetas if b < theta_hat)
    raw_prop_less = less_count / num_resamples
    prop_less = max(1e-6, min(1.0 - 1e-6, raw_prop_less))
    bias_clamped = prop_less != raw_prop_less
    z0 = _norm_ppf(prop_less)

    m = len(jackknife_thetas)
    jack_bar = sum(jackknife_thetas) / m
    diffs = [jack_bar - v for v in jackknife_thetas]
    num = sum(d**3 for d in diffs)
    den = 6.0 * (sum(d**2 for d in diffs) ** 1.5)
    if abs(den) > 1e-12:
        a = num / den
        acceleration_zeroed = False
    else:
        a = 0.0
        acceleration_zeroed = True

    alpha = (1.0 - confidence) / 2.0

    denominator_pinned = False

    def _adjusted_p(z_val: float) -> float:
        nonlocal denominator_pinned
        denom = 1.0 - a * (z0 + z_val)
        if abs(denom) < 1e-6:
            denom = 1e-6 if denom >= 0 else -1e-6
            denominator_pinned = True
        return _norm_cdf(z0 + (z0 + z_val) / denom)

    p1 = max(0.0, min(1.0, _adjusted_p(_norm_ppf(alpha))))
    p2 = max(0.0, min(1.0, _adjusted_p(_norm_ppf(1.0 - alpha))))

    idx1 = max(0, min(num_resamples - 1, int(p1 * num_resamples)))
    idx2 = max(0, min(num_resamples - 1, int(p2 * num_resamples)))

    if boot_thetas[0] == boot_thetas[-1]:
        method = CI_METHOD_DEGENERATE
    elif bias_clamped or denominator_pinned:
        method = CI_METHOD_CLAMPED
    elif acceleration_zeroed:
        method = CI_METHOD_BC
    else:
        method = CI_METHOD_BCA

    return (boot_thetas[idx1], boot_thetas[idx2], method)


def bca_bootstrap_ci_with_method(
    data: Sequence[float],
    confidence: float = 0.95,
    num_resamples: int = 2000,
    seed: int = 42,
) -> Tuple[float, float, float, str]:
    """:func:`bca_bootstrap_ci`, plus the construction that produced the interval.

    Same numerics as :func:`bca_bootstrap_ci` — that function is this one with
    the fourth value dropped, and `test_bca_bootstrap.py` pins both against
    literal reference values so the two cannot drift.

    Args:
        data: Sequence of numeric samples (n >= 3).
        confidence: Desired confidence level (default 0.95).
        num_resamples: Number of bootstrap resamples (>= 1000).
        seed: PRNG seed for reproducibility.

    Returns:
        (mean, ci_lower, ci_upper, method), method being a ``CI_METHOD_*`` value.
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

    # 2. Jackknife replicates (the acceleration's input)
    jackknife_means: List[float] = []
    for i in range(n):
        # Sample with item i removed
        jack_sum = sum(data[j] for j in range(n) if j != i)
        jackknife_means.append(jack_sum / (n - 1))

    # 3. Bias correction, acceleration and adjusted percentiles
    ci_lower, ci_upper, method = _bca_from_distribution(
        theta_hat, boot_means, jackknife_means, confidence
    )
    return (theta_hat, ci_lower, ci_upper, method)


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

    Use :func:`bca_bootstrap_ci_with_method` to also learn whether a defensive
    clamp bound the interval. This signature and its numbers are load-bearing
    for every committed §8.4 artifact and do not change.
    """
    point, ci_lower, ci_upper, _method = bca_bootstrap_ci_with_method(
        data, confidence, num_resamples, seed
    )
    return (point, ci_lower, ci_upper)


def bca_bootstrap_ratio_ci_with_method(
    numerator: Sequence[float],
    denominator: Sequence[float],
    confidence: float = 0.95,
    num_resamples: int = 2000,
    seed: int = 42,
) -> Tuple[float, float, float, str]:
    """:func:`bca_bootstrap_ratio_ci`, plus the construction that produced it.

    Returns:
        (ratio, ci_lower, ci_upper, method), method being a ``CI_METHOD_*`` value.
    """
    n_num = len(numerator)
    n_den = len(denominator)
    if n_num < 3 or n_den < 3:
        raise ValueError(
            f"Need at least 3 data points in each arm for a two-sample BCa bootstrap, "
            f"got {n_num} and {n_den}"
        )

    mean_den = sum(denominator) / n_den
    if mean_den == 0.0:
        raise ValueError("Denominator arm has a zero mean; the ratio is undefined")
    theta_hat = (sum(numerator) / n_num) / mean_den

    rng = random.Random(seed)
    boot: List[float] = []
    for _ in range(num_resamples):
        num_sum = 0.0
        for _ in range(n_num):
            num_sum += numerator[rng.randint(0, n_num - 1)]
        den_sum = 0.0
        for _ in range(n_den):
            den_sum += denominator[rng.randint(0, n_den - 1)]
        den_mean = den_sum / n_den
        if den_mean == 0.0:
            raise ValueError("Bootstrap resample produced a zero-mean denominator")
        boot.append((num_sum / n_num) / den_mean)

    total_num = sum(numerator)
    total_den = sum(denominator)
    jackknife: List[float] = []
    for i in range(n_num):
        jackknife.append(((total_num - numerator[i]) / (n_num - 1)) / mean_den)
    for j in range(n_den):
        den_mean = (total_den - denominator[j]) / (n_den - 1)
        if den_mean == 0.0:
            raise ValueError("Jackknife replicate produced a zero-mean denominator")
        jackknife.append((total_num / n_num) / den_mean)

    lower, upper, method = _bca_from_distribution(theta_hat, boot, jackknife, confidence)
    return (theta_hat, lower, upper, method)


def bca_bootstrap_ratio_ci(
    numerator: Sequence[float],
    denominator: Sequence[float],
    confidence: float = 0.95,
    num_resamples: int = 2000,
    seed: int = 42,
) -> Tuple[float, float, float]:
    """Two-sample BCa interval for the ratio mean(numerator) / mean(denominator).

    A head-vs-baseline speedup claim is a ratio of two independently sampled
    means, so gating it on the CI lower bound (AGENTS.md §8.4) needs a
    two-sample bootstrap: the two arms are resampled independently and the
    statistic is recomputed on each pair. Acceleration comes from the standard
    two-sample jackknife -- leave one observation out of either arm in turn.

    Args:
        numerator: samples whose mean forms the ratio's numerator (n >= 3).
        denominator: samples whose mean forms the ratio's denominator (n >= 3).
        confidence: desired confidence level (default 0.95).
        num_resamples: number of bootstrap resamples (>= 1000).
        seed: PRNG seed for reproducibility.

    Returns:
        (ratio, ci_lower, ci_upper)

    Use :func:`bca_bootstrap_ratio_ci_with_method` to also learn whether a
    defensive clamp bound the interval.
    """
    ratio, lower, upper, _method = bca_bootstrap_ratio_ci_with_method(
        numerator, denominator, confidence, num_resamples, seed
    )
    return (ratio, lower, upper)
