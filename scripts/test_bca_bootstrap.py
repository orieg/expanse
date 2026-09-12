#!/usr/bin/env python3
"""Unit tests for scripts/bca_bootstrap.py."""

import random
import unittest

from bca_bootstrap import (
    CI_METHOD_BC,
    CI_METHOD_BCA,
    CI_METHOD_CLAMPED,
    CI_METHOD_DEGENERATE,
    bca_bootstrap_ci,
    bca_bootstrap_ci_with_method,
    bca_bootstrap_ratio_ci,
    bca_bootstrap_ratio_ci_with_method,
    _bca_from_distribution,
    _norm_ppf,
    _norm_cdf,
)

# Intervals this module produced before the method-reporting entry points were
# added, captured from the implementation at 2b4c15a8 and pinned verbatim.
#
# Six suites carry committed §8.4 intervals that came from `bca_bootstrap_ci` /
# `bca_bootstrap_ratio_ci`, and those artifacts cannot be recomputed cheaply, so
# the numerics here are frozen: the `*_with_method` entry points are the same
# construction with one more return value, not a second estimator. These
# literals are what makes that checkable. A diff here means a published figure
# has moved — re-derive it, do not re-record the literal.
#
# Each case is (name, kwargs, (point, lo, hi), method).
_PINNED_ONE_SAMPLE = [
    (
        "skewed_on_device",
        dict(
            data=[1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.9, 12.0],
            confidence=0.95,
            num_resamples=2000,
            seed=0x59_79_00,
        ),
        (2.4699999999999998, 1.35, 6.709999999999999),
        CI_METHOD_BCA,
    ),
    (
        "tight_latency_sample",
        dict(
            data=[35.2, 35.8, 36.1, 35.5, 35.9, 36.0, 35.7],
            confidence=0.95,
            num_resamples=1000,
            seed=123,
        ),
        (35.74285714285714, 35.5, 35.91428571428572),
        CI_METHOD_BCA,
    ),
    (
        "all_samples_identical",
        dict(data=[7.0] * 8),
        (7.0, 7.0, 7.0),
        CI_METHOD_DEGENERATE,
    ),
    (
        "identical_but_one",
        dict(data=[5.0, 5.0, 5.0, 5.0, 6.0]),
        (5.2, 5.0, 5.4),
        CI_METHOD_BCA,
    ),
    (
        "bca_minimum_n",
        dict(data=[1.0, 1.0, 1.5]),
        (1.1666666666666667, 1.0, 1.3333333333333333),
        CI_METHOD_BCA,
    ),
    (
        "one_wild_outlier",
        dict(data=[1.0, 1.0, 1.0, 1.0, 200.0]),
        (40.8, 1.0, 80.6),
        CI_METHOD_BCA,
    ),
    (
        "ninety_percent_confidence",
        dict(
            data=[10.0, 12.0, 9.0, 11.0, 13.0, 8.0, 10.5, 11.5, 9.5, 12.5],
            confidence=0.90,
            num_resamples=1500,
            seed=7,
        ),
        (10.7, 9.85, 11.45),
        CI_METHOD_BCA,
    ),
    (
        # sum(d**2) ** 1.5 underflows to zero at this scale, so the jackknife
        # acceleration is zeroed and what remains is the bias-corrected (BC)
        # percentile interval. The endpoints are right; the label is the point.
        "acceleration_underflows",
        dict(data=[1e-150, 1e-150, 2e-150], num_resamples=1000, seed=1),
        (1.3333333333333333e-150, 1e-150, 1.6666666666666665e-150),
        CI_METHOD_BC,
    ),
]

_PINNED_RATIO = [
    (
        "separated_arms",
        ([10.0, 11.0, 12.0, 13.0, 14.0, 15.0], [8.0, 9.0, 9.5, 10.0, 10.5, 11.0]),
        dict(num_resamples=1000, seed=5),
        (1.293103448275862, 1.1282051282051282, 1.4736842105263157),
        CI_METHOD_BCA,
    ),
    (
        "both_arms_constant",
        ([4.0] * 5, [4.0] * 5),
        dict(num_resamples=1000, seed=5),
        (1.0, 1.0, 1.0),
        CI_METHOD_DEGENERATE,
    ),
    (
        "constant_denominator",
        ([1.0, 2.0, 3.0], [1.0, 1.0, 1.0]),
        dict(num_resamples=1200, seed=11),
        (2.0, 1.0, 2.6666666666666665),
        CI_METHOD_BCA,
    ),
]


class TestBcaBootstrap(unittest.TestCase):
    def test_norm_cdf_and_ppf_roundtrip(self):
        for p in [0.001, 0.01, 0.05, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95, 0.99, 0.999]:
            z = _norm_ppf(p)
            p_rec = _norm_cdf(z)
            self.assertAlmostEqual(p, p_rec, places=5)

    def test_bca_ci_bounds(self):
        data = [35.2, 35.8, 36.1, 35.5, 35.9, 36.0, 35.7]
        mean, lo, hi = bca_bootstrap_ci(data, confidence=0.95, num_resamples=1000, seed=123)
        self.assertTrue(lo <= mean <= hi)
        self.assertGreater(lo, 34.5)
        self.assertLess(hi, 37.0)

    def test_bca_requires_min_samples(self):
        with self.assertRaises(ValueError):
            bca_bootstrap_ci([1.0, 2.0])

    def test_ratio_ci_encloses_point_and_recovers_known_ratio(self):
        # Two well-separated arms with a known population ratio of 1.25.
        rng = random.Random(4242)
        numerator = [rng.gauss(100.0, 2.0) for _ in range(80)]
        denominator = [rng.gauss(80.0, 2.0) for _ in range(80)]
        ratio, lo, hi = bca_bootstrap_ratio_ci(
            numerator, denominator, num_resamples=2000, seed=7
        )
        self.assertTrue(lo <= ratio <= hi)
        self.assertAlmostEqual(ratio, 1.25, delta=0.03)
        # The interval must actually cover the population ratio.
        self.assertLess(lo, 1.25)
        self.assertGreater(hi, 1.25)

    def test_ratio_ci_of_identical_arms_brackets_unity(self):
        rng = random.Random(99)
        arm = [rng.gauss(50.0, 3.0) for _ in range(60)]
        other = [rng.gauss(50.0, 3.0) for _ in range(60)]
        ratio, lo, hi = bca_bootstrap_ratio_ci(arm, other, num_resamples=2000, seed=7)
        self.assertTrue(lo <= ratio <= hi)
        self.assertLess(lo, 1.0)
        self.assertGreater(hi, 1.0)

    def test_ratio_is_deterministic_for_a_fixed_seed(self):
        a = [10.0, 11.0, 12.0, 13.0, 14.0, 15.0]
        b = [8.0, 9.0, 9.5, 10.0, 10.5, 11.0]
        first = bca_bootstrap_ratio_ci(a, b, num_resamples=1000, seed=5)
        second = bca_bootstrap_ratio_ci(a, b, num_resamples=1000, seed=5)
        self.assertEqual(first, second)

    def test_ratio_requires_min_samples_in_both_arms(self):
        with self.assertRaises(ValueError):
            bca_bootstrap_ratio_ci([1.0, 2.0], [1.0, 2.0, 3.0])
        with self.assertRaises(ValueError):
            bca_bootstrap_ratio_ci([1.0, 2.0, 3.0], [1.0, 2.0])

    def test_ratio_rejects_zero_mean_denominator(self):
        with self.assertRaises(ValueError):
            bca_bootstrap_ratio_ci([1.0, 2.0, 3.0], [0.0, 0.0, 0.0])


class TestNumericsAreFrozen(unittest.TestCase):
    """The published-interval guarantee: these numbers do not move (#880)."""

    def test_one_sample_intervals_match_pinned_references(self):
        for name, kwargs, expected, _method in _PINNED_ONE_SAMPLE:
            with self.subTest(case=name):
                self.assertEqual(bca_bootstrap_ci(**kwargs), expected)

    def test_ratio_intervals_match_pinned_references(self):
        for name, arms, kwargs, expected, _method in _PINNED_RATIO:
            with self.subTest(case=name):
                self.assertEqual(bca_bootstrap_ratio_ci(*arms, **kwargs), expected)

    def test_with_method_returns_the_same_three_values(self):
        # The additive entry point must be the existing one plus a label, not a
        # second estimator: identical `(point, lo, hi)` on every shape above,
        # degenerate ones included.
        for name, kwargs, _expected, _method in _PINNED_ONE_SAMPLE:
            with self.subTest(case=name):
                self.assertEqual(
                    bca_bootstrap_ci_with_method(**kwargs)[:3],
                    bca_bootstrap_ci(**kwargs),
                )
        for name, arms, kwargs, _expected, _method in _PINNED_RATIO:
            with self.subTest(case=f"ratio_{name}"):
                self.assertEqual(
                    bca_bootstrap_ratio_ci_with_method(*arms, **kwargs)[:3],
                    bca_bootstrap_ratio_ci(*arms, **kwargs),
                )

    def test_with_method_still_refuses_too_few_samples(self):
        with self.assertRaises(ValueError):
            bca_bootstrap_ci_with_method([1.0, 2.0])
        with self.assertRaises(ValueError):
            bca_bootstrap_ratio_ci_with_method([1.0, 2.0], [1.0, 2.0, 3.0])


class TestReportedConstruction(unittest.TestCase):
    """Which construction produced an interval must be stated, not assumed."""

    def test_one_sample_methods_match_pinned_references(self):
        for name, kwargs, _expected, method in _PINNED_ONE_SAMPLE:
            with self.subTest(case=name):
                self.assertEqual(bca_bootstrap_ci_with_method(**kwargs)[3], method)

    def test_ratio_methods_match_pinned_references(self):
        for name, arms, kwargs, _expected, method in _PINNED_RATIO:
            with self.subTest(case=name):
                self.assertEqual(
                    bca_bootstrap_ratio_ci_with_method(*arms, **kwargs)[3], method
                )

    def test_an_undegraded_sample_is_not_labelled_degraded(self):
        # The discriminating pair: a healthy sample and a degenerate one must
        # not carry the same label. A reporter stuck on one value fails here.
        healthy = bca_bootstrap_ci_with_method(
            [35.2, 35.8, 36.1, 35.5, 35.9, 36.0, 35.7], num_resamples=1000, seed=123
        )[3]
        degenerate = bca_bootstrap_ci_with_method([7.0] * 8)[3]
        underflowed = bca_bootstrap_ci_with_method(
            [1e-150, 1e-150, 2e-150], num_resamples=1000, seed=1
        )[3]
        self.assertEqual(healthy, CI_METHOD_BCA)
        self.assertEqual(degenerate, CI_METHOD_DEGENERATE)
        self.assertEqual(underflowed, CI_METHOD_BC)
        self.assertEqual(len({healthy, degenerate, underflowed}), 3)

    def test_bias_correction_clamp_is_reported(self):
        # `CI_METHOD_CLAMPED` is the label for the `prop_less` clamp and the
        # pinned adjusted-percentile denominator. No sample shape tried reaches
        # it through the public entry points -- for a mean, the observed
        # statistic falling outside the bootstrap support implies a degenerate
        # sample, which takes precedence -- so the guard is driven directly.
        # It stays because the clamps are in the code: if one ever fires, the
        # cell must say so rather than claim "bca" (§8.1).
        boot = [float(i) for i in range(1, 101)]
        jackknife = [1.0, 2.0, 4.0]
        # theta_hat below every resample: less_count == 0, so `prop_less` is
        # clamped off zero and `z0` is the clamp's value, not the sample's.
        _lo, _hi, method = _bca_from_distribution(0.0, boot, jackknife, 0.95)
        self.assertEqual(method, CI_METHOD_CLAMPED)
        # And above every resample, for the upper clamp.
        _lo, _hi, method = _bca_from_distribution(1000.0, boot, jackknife, 0.95)
        self.assertEqual(method, CI_METHOD_CLAMPED)

    def test_degeneracy_outranks_the_other_labels(self):
        # An all-identical sample trips the bias clamp and zeroes the
        # acceleration at once; `degenerate` is the informative label and must
        # win, because the interval really is the exact answer there.
        boot = [5.0] * 50
        jackknife = [5.0, 5.0, 5.0]
        _lo, _hi, method = _bca_from_distribution(5.0, boot, jackknife, 0.95)
        self.assertEqual(method, CI_METHOD_DEGENERATE)


if __name__ == "__main__":
    unittest.main()
