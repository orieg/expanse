#!/usr/bin/env python3
"""Unit tests for scripts/bca_bootstrap.py."""

import unittest
from bca_bootstrap import bca_bootstrap_ci, _norm_ppf, _norm_cdf


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


if __name__ == "__main__":
    unittest.main()
