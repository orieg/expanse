#!/usr/bin/env python3
"""
scripts/set_algebra_bounds.py — Mathematical bounds and complexity derivations
for k-way aggregate set algebra (issue #610).

Enforces Rule 12 / GEMINI.md §1.3 (Math-first validation in committed Python
with reference-pinned unit tests).
"""

from __future__ import annotations

import unittest


def inclusion_exclusion_term_count(k: int) -> int:
    """Number of intersection terms required to compute |⋃_{i=1}^k S_i| via inclusion-exclusion.

    Reference:
        Stanley, R. P. (2011). Enumerative Combinatorics, Vol. 1, Cambridge Studies
        in Advanced Mathematics, §2.1 (The Principle of Inclusion-Exclusion).
        For k finite sets, the formula sums over all non-empty subsets of [k]:
            sum_{j=1}^k binom(k, j) = 2^k - 1.
    """
    if k < 1:
        raise ValueError(f"k must be >= 1, got {k}")
    return (1 << k) - 1


def intersection_cardinality_upper_bound(sizes: list[int]) -> int:
    """Mathematical upper bound on the cardinality of k-way intersection.

    Reference:
        Halmos, P. R. (1960). Naive Set Theory, D. Van Nostrand, §4.
        For any family of sets {S_1, ..., S_k}:
            |⋂_{i=1}^k S_i| <= min_{1 <= i <= k} |S_i|.
    """
    if not sizes:
        return 0
    for s in sizes:
        if s < 0:
            raise ValueError(f"Set size must be non-negative, got {s}")
    return min(sizes)


def union_cardinality_bounds(sizes: list[int]) -> tuple[int, int]:
    """Mathematical lower and upper bounds on the cardinality of k-way union.

    Reference:
        Halmos, P. R. (1960). Naive Set Theory, D. Van Nostrand, §4.
        Boole's Inequality and monotonicity of union:
            max_{1 <= i <= k} |S_i| <= |⋃_{i=1}^k S_i| <= sum_{i=1}^k |S_i|.
    """
    if not sizes:
        return (0, 0)
    for s in sizes:
        if s < 0:
            raise ValueError(f"Set size must be non-negative, got {s}")
    return (max(sizes), sum(sizes))


def intermediate_allocations_count(k: int, k_way: bool) -> int:
    """Number of intermediate allocated result sets required for cardinality evaluation.

    In a pairwise cascade (A ∩ B ∩ C ...), evaluating k sets requires materializing
    k - 2 intermediate result sets (or k - 1 if folding accumulator), because
    cardinality of the next step depends on having a physical set to query.
    In native k-way structural lockstep descent, 0 intermediate sets are allocated.
    """
    if k < 0:
        raise ValueError(f"k must be non-negative, got {k}")
    if k <= 2:
        return 0
    if k_way:
        return 0
    return k - 2


class TestSetAlgebraBounds(unittest.TestCase):
    """Pinned reference unit tests for k-way set algebra bounds."""

    def test_inclusion_exclusion_growth(self):
        # Pinned reference values for 2^k - 1
        self.assertEqual(inclusion_exclusion_term_count(1), 1)
        self.assertEqual(inclusion_exclusion_term_count(2), 3)
        self.assertEqual(inclusion_exclusion_term_count(3), 7)
        self.assertEqual(inclusion_exclusion_term_count(4), 15)
        self.assertEqual(inclusion_exclusion_term_count(5), 31)
        self.assertEqual(inclusion_exclusion_term_count(8), 255)
        self.assertEqual(inclusion_exclusion_term_count(16), 65535)

        with self.assertRaises(ValueError):
            inclusion_exclusion_term_count(0)

    def test_intersection_bounds(self):
        self.assertEqual(intersection_cardinality_upper_bound([]), 0)
        self.assertEqual(intersection_cardinality_upper_bound([100]), 100)
        self.assertEqual(
            intersection_cardinality_upper_bound([10_000, 50, 1_000]), 50
        )
        self.assertEqual(
            intersection_cardinality_upper_bound([1_000_000, 0, 500]), 0
        )

        with self.assertRaises(ValueError):
            intersection_cardinality_upper_bound([-5, 10])

    def test_union_bounds(self):
        self.assertEqual(union_cardinality_bounds([]), (0, 0))
        self.assertEqual(union_cardinality_bounds([42]), (42, 42))
        self.assertEqual(
            union_cardinality_bounds([100, 200, 300]), (300, 600)
        )
        self.assertEqual(
            union_cardinality_bounds([10, 0, 50]), (50, 60)
        )

        with self.assertRaises(ValueError):
            union_cardinality_bounds([-1, 10])

    def test_intermediate_allocations(self):
        # k-way structural walk never allocates intermediate sets
        for k in range(0, 20):
            self.assertEqual(intermediate_allocations_count(k, k_way=True), 0)

        # Pairwise cascade allocates k-2 intermediate structures for k >= 3
        self.assertEqual(intermediate_allocations_count(1, k_way=False), 0)
        self.assertEqual(intermediate_allocations_count(2, k_way=False), 0)
        self.assertEqual(intermediate_allocations_count(3, k_way=False), 1)
        self.assertEqual(intermediate_allocations_count(5, k_way=False), 3)
        self.assertEqual(intermediate_allocations_count(8, k_way=False), 6)


if __name__ == "__main__":
    unittest.main()
