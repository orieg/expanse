#!/usr/bin/env python3
"""
scripts/olc_bounds.py — Mathematical bounds and contention ceilings for
Stage B multi-writer Optimistic Lock Coupling (OLC) on Expanse (issue #568).

Implements Rule 12 / GEMINI.md §1.3 (Math-first validation in committed Python
with reference-pinned unit tests) for PR 4 (Stage B design gate).

Primary sources:
  1. Leis, V., Scheibner, F., Kemper, A., & Neumann, T. (2016). The ART of
     Practical Synchronization. In Proceedings of the 12th International
     Workshop on Data Management on New Hardware (DaMoN '16).
     DOI: 10.1145/2933349.2933352.
  2. Gunther, N. J. (2007). Guerrilla Capacity Planning: A Metric Approach to
     Designing and Managing Systems. Springer. Chapter 6: Universal Scalability
     Law (USL).
  3. Hennessy, J. L., & Patterson, D. A. (2017). Computer Architecture:
     A Quantitative Approach (6th ed.), Morgan Kaufmann. §5.2 (Multiprocessor
     Cache Coherence and Invalidation Latencies).
  4. orieg/expanse line-transfer calibration:
     docs/benchmarks/concurrency/results/line_transfer.json (commit a1982ff2,
     12th Gen Intel Core i9-12900F reference host).
"""

from __future__ import annotations

import math
import unittest

# ---------------------------------------------------------------------------
# 1. Pinned Reference Constants from Empirical Harnesses & Hardware Profile
# ---------------------------------------------------------------------------

# Measured pairwise cache line transfer latencies (docs/benchmarks/concurrency/results/line_transfer.json)
T_LINE_SPIN_MEDIAN_NS = 33.48571428571429
T_LINE_SPIN_MIN_NS = 19.797142857142855  # SMT sibling pairs
T_LINE_SPIN_MAX_NS = 36.129999999999995  # Distant physical core pairs
T_LINE_PAUSE_CALIBRATION_NS = 34.07       # Single pause calibration on ref host

# Single-writer baseline throughput (reference host, 64-bit integer map inserts)
# Commit 64f8a3af: ~5.7 M inserts/s at W=1 -> ~175.44 ns per insert operation
W1_BASELINE_INSERTS_PER_SEC = 5_700_000.0
W1_BASELINE_OP_LATENCY_NS = 1_000_000_000.0 / W1_BASELINE_INSERTS_PER_SEC  # ~175.44 ns

# Node lock hold time approximations (derived from Callgrind instruction counts:
# sync_map_insert ~862 instructions total; brief parent/leaf lock hold ~15-50 ns)
T_HOLD_LEAF_MUTATION_NS = 15.0      # Brief leaf slot/in-place update
T_HOLD_BRANCH_CASCADE_NS = 50.0     # Structural branch expansion/realloc


# ---------------------------------------------------------------------------
# 2. Analytical Contention & Scalability Bounds
# ---------------------------------------------------------------------------

def contended_rmw_ceiling(k: int, t_line_ns: float, t_hold_ns: float) -> float:
    """Calculates the asymptotic throughput ceiling (ops/second) imposed by k contended
    RMWs/critical sections on shared cache lines.

    Reference:
      Gunther (2007) USL serialization bottleneck; Hennessy & Patterson (2017) §5.2.
      In any parallel system with W workers, if each transaction requires serialization
      over k shared-line transfers of duration t_line_ns plus critical section hold
      t_hold_ns, the maximum service rate of that bottleneck is:
          X_max = 1 / (k * t_line + t_hold)
    """
    if k < 1:
        raise ValueError(f"k (contended lines) must be >= 1, got {k}")
    if t_line_ns <= 0.0:
        raise ValueError(f"t_line_ns must be positive, got {t_line_ns}")
    if t_hold_ns < 0.0:
        raise ValueError(f"t_hold_ns cannot be negative, got {t_hold_ns}")

    t_crit_seconds = (k * t_line_ns + t_hold_ns) * 1e-9
    return 1.0 / t_crit_seconds


def derive_usl_sigma(t_crit_ns: float, t0_ns: float) -> float:
    """Derives Gunther's USL contention parameter sigma directly from the ratio of
    critical section serialization latency to total single-threaded operation latency.

    Reference:
      Gunther, N. J. (2007). Guerrilla Capacity Planning, Springer. §6.3.
      sigma = T_crit / T_0 represents the fraction of time spent in serial bottlenecks.
    """
    if t_crit_ns < 0.0:
        raise ValueError(f"t_crit_ns cannot be negative, got {t_crit_ns}")
    if t0_ns <= 0.0:
        raise ValueError(f"t0_ns must be positive, got {t0_ns}")
    sigma = t_crit_ns / t0_ns
    return min(1.0, sigma)


def usl_throughput(
    w: int,
    t0_ns: float = W1_BASELINE_OP_LATENCY_NS,
    sigma: float = 0.0,
    kappa: float = 0.0,
) -> float:
    """Calculates aggregate system throughput across W workers using Gunther's
    Universal Scalability Law (USL).

    Reference:
      Gunther, N. J. (2007). Guerrilla Capacity Planning, Springer.
      X(W) = (W * X(1)) / (1 + sigma * (W - 1) + kappa * W * (W - 1))
      where sigma represents contention/serialization share, and kappa represents
      cross-talk / coherence coherency penalty.
    """
    if w < 1:
        raise ValueError(f"w (workers) must be >= 1, got {w}")
    if t0_ns <= 0.0:
        raise ValueError(f"t0_ns must be positive, got {t0_ns}")
    if sigma < 0.0 or sigma > 1.0:
        raise ValueError(f"sigma must be in [0.0, 1.0], got {sigma}")
    if kappa < 0.0:
        raise ValueError(f"kappa cannot be negative, got {kappa}")

    x1 = 1e9 / t0_ns
    denom = 1.0 + sigma * (w - 1) + kappa * w * (w - 1)
    return (w * x1) / denom


def shape_contention_bound(shape: str, t_line_ns: float = T_LINE_SPIN_MEDIAN_NS) -> dict[str, float | bool]:
    """Evaluates the theoretical scaling headroom for specific benchmark workloads:
      - 'ffi_disjoint': Uniform 64-bit keys split into disjoint slices (OLC best case:
        writers access disjoint branches, contending primarily on root word k=1).
      - 'core_shared_prefix': rng % 2M (bytes 7..3 constant; every insert shares a chain
        to level 3, contending on k=5 shared ancestor lines).

    Returns a dictionary with parameters, ceiling in M ops/s, and a boolean indicating
    whether the shape can clear the W=1 single-writer baseline (5.7 M ops/s).
    """
    if shape == "ffi_disjoint":
        k = 1
        t_hold = T_HOLD_LEAF_MUTATION_NS
    elif shape == "core_shared_prefix":
        k = 5
        t_hold = T_HOLD_BRANCH_CASCADE_NS
    else:
        raise ValueError(f"Unknown shape '{shape}'. Valid shapes: 'ffi_disjoint', 'core_shared_prefix'")

    ceiling_ops = contended_rmw_ceiling(k=k, t_line_ns=t_line_ns, t_hold_ns=t_hold)
    ceiling_mops = ceiling_ops / 1e6
    clears_w1 = ceiling_ops > W1_BASELINE_INSERTS_PER_SEC

    return {
        "k": k,
        "t_line_ns": t_line_ns,
        "t_hold_ns": t_hold,
        "ceiling_mops": ceiling_mops,
        "clears_w1": clears_w1,
    }


def allocator_counter_ceiling(t_line_ns: float = T_LINE_SPIN_MEDIAN_NS) -> float:
    """Calculates the theoretical allocation ceiling (allocs/second) when all concurrent
    writers contend on a single shared NodeAlloc counter cache line (cost 4).

    Reference:
      Hennessy & Patterson §5.2. An atomic RMW (fetch_add/sub) bouncing a cache line
      among W cores cannot complete faster than one line transfer per allocation:
          Alloc_max = 1 / t_line
    """
    if t_line_ns <= 0.0:
        raise ValueError(f"t_line_ns must be positive, got {t_line_ns}")
    return 1e9 / t_line_ns


def restart_storm_probability(w: int, t_hold_ns: float, t_op_ns: float) -> float:
    """Calculates the probability that at least one other writer attempts to acquire
    a node lock while it is held by an active writer.

    Reference:
      Leis et al. (DaMoN '16) §3.2 (Optimistic Lock Coupling).
      Assuming independent Poisson or uniform arrival across W-1 competing threads:
          P_collision = 1 - (1 - (t_hold / t_op))^(W - 1)
    """
    if w < 1:
        raise ValueError(f"w must be >= 1, got {w}")
    if t_hold_ns < 0.0:
        raise ValueError(f"t_hold_ns cannot be negative, got {t_hold_ns}")
    if t_op_ns <= 0.0:
        raise ValueError(f"t_op_ns must be positive, got {t_op_ns}")
    if t_hold_ns >= t_op_ns:
        return 1.0

    p_free = 1.0 - (t_hold_ns / t_op_ns)
    return 1.0 - math.pow(p_free, w - 1)


def expected_restarts_per_op(w: int, t_hold_ns: float, t_op_ns: float) -> float:
    """Calculates the expected number of CAS restart retries per operation under contention.

    Reference:
      Geometric retry distribution under contention probability P:
          E[Restarts] = P / (1 - P)
      Diverges as P -> 1.0 (restart storm threshold).
    """
    p = restart_storm_probability(w=w, t_hold_ns=t_hold_ns, t_op_ns=t_op_ns)
    if p >= 0.999999:
        return float("inf")
    return p / (1.0 - p)


def restart_ratio_ceiling(
    w: int,
    t_hold_ns: float = T_HOLD_LEAF_MUTATION_NS,
    t_op_ns: float = W1_BASELINE_OP_LATENCY_NS,
    safety_factor: float = 2.0,
) -> float:
    """Calculates the pre-registered ceiling for Stat::LockRestarts / write_ops at W writers,
    used to gate PR 5 health cells against livelock and restart storms.
    """
    if safety_factor <= 0.0:
        raise ValueError(f"safety_factor must be positive, got {safety_factor}")

    e_restarts = expected_restarts_per_op(w=w, t_hold_ns=t_hold_ns, t_op_ns=t_op_ns)
    return e_restarts * safety_factor


# ---------------------------------------------------------------------------
# 3. Unit Tests & Reference Invariants (Rule 12 / GEMINI.md §1.3)
# ---------------------------------------------------------------------------

class TestOlcBounds(unittest.TestCase):
    def test_contended_rmw_ceiling_reference_values(self):
        # Case 1: k=1, t_line=33.486 ns, t_hold=15 ns (total crit = 48.486 ns)
        # Expected: 1e9 / 48.485714... = ~20,624,630 ops/s = ~20.62 M ops/s
        ceil1 = contended_rmw_ceiling(1, T_LINE_SPIN_MEDIAN_NS, 15.0)
        self.assertAlmostEqual(ceil1 / 1e6, 20.6246, places=3)

        # Case 2: k=5, t_line=33.486 ns, t_hold=50 ns (total crit = 217.428 ns)
        # Expected: 1e9 / 217.42857... = ~4,599,211 ops/s = ~4.60 M ops/s
        ceil5 = contended_rmw_ceiling(5, T_LINE_SPIN_MEDIAN_NS, 50.0)
        self.assertAlmostEqual(ceil5 / 1e6, 4.5992, places=3)

    def test_shape_contention_bound_conclusions(self):
        # FFI disjoint shape has k=1, clearing W=1 (20.62 M > 5.7 M)
        res_ffi = shape_contention_bound("ffi_disjoint")
        self.assertTrue(res_ffi["clears_w1"])
        self.assertGreater(float(res_ffi["ceiling_mops"]), 5.7)

        # Shared-prefix shape has k=5, failing to clear W=1 (4.60 M < 5.7 M)
        res_shared = shape_contention_bound("core_shared_prefix")
        self.assertFalse(res_shared["clears_w1"])
        self.assertLess(float(res_shared["ceiling_mops"]), 5.7)

    def test_allocator_counter_ceiling(self):
        # When all writers contend on one cache line, alloc ceiling is ~29.86 M allocs/s
        alloc_ceil = allocator_counter_ceiling(T_LINE_SPIN_MEDIAN_NS)
        self.assertAlmostEqual(alloc_ceil / 1e6, 29.8635, places=3)

    def test_derive_usl_sigma(self):
        # T_crit = 17.544 ns, T0 = 175.44 ns -> sigma = 0.10
        sigma1 = derive_usl_sigma(17.544, 175.44)
        self.assertAlmostEqual(sigma1, 0.10, places=3)

        # T_crit exceeding T0 clamps to 1.0
        sigma_clamped = derive_usl_sigma(200.0, 100.0)
        self.assertEqual(sigma_clamped, 1.0)

    def test_usl_throughput(self):
        # At w=1, throughput is exactly 1 / t0
        w1_ops = usl_throughput(w=1, t0_ns=W1_BASELINE_OP_LATENCY_NS, sigma=0.05)
        self.assertAlmostEqual(w1_ops, W1_BASELINE_INSERTS_PER_SEC, delta=1.0)

        # With sigma=0.05 and no kappa, at w=8 throughput scales but sublinearly:
        # denom = 1 + 0.05 * 7 = 1.35; w * X1 / 1.35 = 8 * 5.7M / 1.35 = 33.78M
        w8_ops = usl_throughput(w=8, t0_ns=W1_BASELINE_OP_LATENCY_NS, sigma=0.05)
        self.assertAlmostEqual(w8_ops / 1e6, 33.7777, places=3)

    def test_restart_probability_and_storms(self):
        # At w=1, collision probability is strictly 0.0
        p1 = restart_storm_probability(w=1, t_hold_ns=15.0, t_op_ns=175.44)
        self.assertEqual(p1, 0.0)
        self.assertEqual(expected_restarts_per_op(w=1, t_hold_ns=15.0, t_op_ns=175.44), 0.0)

        # At w=16 on disjoint slices (t_hold=15 ns, t_op=175.44 ns):
        # ratio = 15/175.44 = 0.085500456; 1 - (1 - 0.085500456)^15 = 1 - 0.261674 = 0.738326
        p16_disjoint = restart_storm_probability(w=16, t_hold_ns=15.0, t_op_ns=175.44)
        self.assertAlmostEqual(p16_disjoint, 0.7383, places=3)
        e16_disjoint = expected_restarts_per_op(w=16, t_hold_ns=15.0, t_op_ns=175.44)
        # E = 0.738326 / (1 - 0.738326) = 2.8215 restarts per op
        self.assertAlmostEqual(e16_disjoint, 2.822, delta=0.01)

        # At w=16 on shared prefix (t_hold=50 ns, t_op=175.44 ns):
        # ratio = 50/175.44 = 0.285; 1 - (1 - 0.285)^15 = 1 - 0.0063 = 0.9937
        p16_shared = restart_storm_probability(w=16, t_hold_ns=50.0, t_op_ns=175.44)
        self.assertGreater(p16_shared, 0.99)
        e16_shared = expected_restarts_per_op(w=16, t_hold_ns=50.0, t_op_ns=175.44)
        # Expected restarts exceed 100 per op -> catastrophic restart storm
        self.assertGreater(e16_shared, 100.0)

    def test_invalid_arguments_raise_value_error(self):
        with self.assertRaises(ValueError):
            contended_rmw_ceiling(0, 33.0, 15.0)
        with self.assertRaises(ValueError):
            contended_rmw_ceiling(1, -5.0, 15.0)
        with self.assertRaises(ValueError):
            contended_rmw_ceiling(1, 33.0, -1.0)
        with self.assertRaises(ValueError):
            usl_throughput(0)
        with self.assertRaises(ValueError):
            usl_throughput(1, sigma=-0.1)
        with self.assertRaises(ValueError):
            shape_contention_bound("nonexistent")


if __name__ == "__main__":
    print("=== Expanse OLC Contention & Scalability Bounds (Issue #568 PR 4) ===")
    print()
    for shape in ("ffi_disjoint", "core_shared_prefix"):
        res = shape_contention_bound(shape)
        verdict = "PASS (clears W=1)" if res["clears_w1"] else "FAIL (cannot clear W=1 without per-writer partitioning)"
        print(f"Workload Shape: {shape}")
        print(f"  Contended lines (k):           {res['k']}")
        print(f"  Line transfer latency (t_line): {res['t_line_ns']:.2f} ns")
        print(f"  Critical hold latency (t_hold): {res['t_hold_ns']:.2f} ns")
        print(f"  Throughput ceiling:            {res['ceiling_mops']:.2f} M ops/s")
        print(f"  Evaluation vs W=1 (5.7 M/s):   {verdict}")
        print()

    print(f"NodeAlloc Shared Counter Line Contention Ceiling: {allocator_counter_ceiling() / 1e6:.2f} M allocs/s")
    print(f"W=16 Disjoint Restarts Ceiling (2x safety):        {restart_ratio_ceiling(w=16, t_hold_ns=15.0):.2f} restarts/op")
    print()
    unittest.main()
