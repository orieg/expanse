#!/usr/bin/env python3
"""Unit tests for scripts/bca_bootstrap.py."""

import random
import unittest

from bca_bootstrap import (
    bca_bootstrap_ci,
    bca_bootstrap_ratio_ci,
    _norm_ppf,
    _norm_cdf,
)


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


if __name__ == "__main__":
    unittest.main()
