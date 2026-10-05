#!/usr/bin/env python3
"""Mathematical bounds, USL model fitting, and instruction ceilings for #1280.

Covers two open items from #1280 (METHODOLOGY §34 & §35, AGENTS.md §8.8 commit 1, §1.3):
1. Item 1 (METHODOLOGY §34): Gunther's Universal Scalability Law (USL) model fitting, peak concurrency
   derivation, and candidate bounds for the four-thread throughput peak on SyncExpanseBlobMap.
2. Item 2 (METHODOLOGY §35): Pre-registered Callgrind instruction ceilings vs merge base and attribution
   model for the Shared::enter_writer hot/cold split across all five Sync* wrappers.

Sources:
--------
Gunther, Neil J. "Guerrilla Capacity Planning: A Tactical Approach to Planning for
  Highly Scalable Applications and Services", Springer, 2007.
Gunther, Neil J. "A General Theory of Computational Scalability Based on Rational Functions",
  Computing Research Repository (CoRR), arXiv:0808.1931, 2008.
Denning, P. J., Buzen, J. P. "The Operational Analysis of Queueing Network Models",
  ACM Computing Surveys 10(3), 1978.

Usage:
    python3 scripts/blob_peak_bounds.py              # Run derivations and display ceilings
    python3 scripts/blob_peak_bounds.py --self-test  # Run unit tests pinning reference values
"""

from __future__ import annotations

import argparse
import math
import sys
import unittest
from pathlib import Path
from typing import Any, Dict, List, Sequence, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))


# Reference baseline CI run for main instruction counts:
# Commit: 5ee1e8e399b78ed5fb42d62547f3529eee331b3f (push to main)
# CI Run ID: 37218971129, Job: 111486038808 (Perf / Callgrind Deterministic Instructions)
BASELINE_CI_RUN_ID = 37218971129
BASELINE_COMMIT_SHA = "5ee1e8e399b78ed5fb42d62547f3529eee331b3f"

# Historical constants from #1280 / #1289 attribution
ENTER_WRITER_OUTLINE_COST_IR = 2_350_000
BLOB_INSERT_INLINE_SAVE_IR = 1_399_996
NET_OVERWRITE_REGRESSION_IR = 950_004


# ---------------------------------------------------------------------------
# 1. Gunther's Universal Scalability Law (USL) Derivations
# ---------------------------------------------------------------------------

def usl_throughput(n: float, gamma: float, alpha: float, beta: float) -> float:
    """Computes modeled throughput X(N) under Gunther's USL:

        X(N) = (gamma * N) / (1 + alpha * (N - 1) + beta * N * (N - 1))

    Args:
        n: Concurrency level (N >= 1.0).
        gamma: Uncontended single-worker throughput (gamma > 0).
        alpha: Contention parameter (serialization fraction, 0 <= alpha <= 1).
        beta: Coherency parameter (pairwise crosstalk penalty, beta >= 0).
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
      - If beta == 0: returns float('inf') (Amdahl asymptote without retrograde fall).
      - If alpha >= 1 or (1 - alpha) / beta <= 1: returns 1.0 (strictly non-increasing).
      - Otherwise: returns sqrt((1 - alpha) / beta).
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


def fit_usl_ols(n_vals: Sequence[float], x_vals: Sequence[float]) -> Tuple[float, float, float]:
    """Fits USL parameters (gamma, alpha, beta) via Gunther's linear transformation:

        Y(N) = N / X(N) = c0 + c1 * (N - 1) + c2 * N * (N - 1)
        where: gamma = 1 / c0, alpha = c1 / c0, beta = c2 / c0.
    """
    if len(n_vals) != len(x_vals) or len(n_vals) < 3:
        raise ValueError(f"Need >= 3 data points for USL fitting, got {len(n_vals)}")

    # Set up normal equations Z^T * Z * c = Z^T * Y
    # z = [1, n - 1, n * (n - 1)]
    a = [[0.0] * 3 for _ in range(3)]
    b = [0.0] * 3
    for ni, xi in zip(n_vals, x_vals):
        if xi <= 0.0:
            raise ValueError(f"Throughput must be positive, got {xi} at N={ni}")
        yi = float(ni) / float(xi)
        z = [1.0, float(ni - 1.0), float(ni * (ni - 1.0))]
        for r in range(3):
            b[r] += z[r] * yi
            for c in range(3):
                a[r][c] += z[r] * z[c]

    # Cramer's rule determinant solver for 3x3
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

    c0, c1, c2 = res
    if c0 <= 0.0:
        raise ValueError(f"Inadmissible OLS fit: c0={c0} <= 0 (gamma must be positive)")

    gamma = 1.0 / c0
    alpha = max(0.0, min(1.0, c1 / c0))
    beta = max(0.0, c2 / c0)
    return gamma, alpha, beta


def usl_goodness_of_fit(
    n_vals: Sequence[float],
    x_vals: Sequence[float],
    gamma: float,
    alpha: float,
    beta: float,
) -> Dict[str, float]:
    """Computes R^2, RMSE, and NRMSE for a fitted USL model against data."""
    k = len(n_vals)
    if k != len(x_vals) or k < 3:
        raise ValueError(f"Need >= 3 matching data points, got {len(n_vals)} and {len(x_vals)}")

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


def classify_contention_mechanism(
    alpha: float,
    beta: float,
    alpha_ci: Tuple[float, float] | None = None,
    beta_ci: Tuple[float, float] | None = None,
    beta_floor: float = 0.0033,
) -> Dict[str, Any]:
    """Interprets USL alpha vs beta per AGENTS.md §8.20.5 Step 2 and METHODOLOGY §34.

    Classification rule:
      - Pairwise coherency crosstalk (beta): interval excludes 0 (beta_ci[0] > 0 or beta > beta_floor if CI unavailable).
      - Serialization (alpha): beta interval overlaps 0 and alpha interval excludes 0 (alpha_ci[0] > 0 or alpha > 0.15).
      - Inconclusive: neither interval excludes 0 or fit fails.
    """
    if beta_ci is not None:
        beta_excludes_zero = beta_ci[0] > 0.0
    else:
        beta_excludes_zero = beta > beta_floor

    if alpha_ci is not None:
        alpha_excludes_zero = alpha_ci[0] > 0.0
    else:
        alpha_excludes_zero = alpha > 0.15

    if beta_excludes_zero:
        diagnosis = "COHERENCY_CROSSTALK_DOMINATES"
        mechanism = "Pairwise coherency traffic (beta * N * (N-1)) sets retrograde curve; investigate cache lines"
    elif alpha_excludes_zero:
        diagnosis = "SERIALIZATION_DOMINATES"
        mechanism = "Serialization fraction alpha sets scaling limit; cache-line fixes cannot cure serial lock/section"
    else:
        diagnosis = "INCONCLUSIVE_OR_BALANCED"
        mechanism = "Neither parameter excludes zero with statistical significance; or both are within scalable thresholds"

    n_max = usl_n_max(alpha, beta)
    return {
        "diagnosis": diagnosis,
        "mechanism": mechanism,
        "alpha": alpha,
        "beta": beta,
        "alpha_ci": alpha_ci,
        "beta_ci": beta_ci,
        "n_max": n_max,
        "is_retrograde": not math.isinf(n_max) and n_max < 16.0,
    }


# ---------------------------------------------------------------------------
# 2. Named Hypotheses Analytical Models
# ---------------------------------------------------------------------------

def candidate_a_line_sharing_traffic(
    ops_per_sec: float,
    write_fraction: float = 0.5,
    cacheline_bytes: int = 64,
) -> Dict[str, float]:
    """Candidate A: has_writer_deltas AtomicBool release-store on every insert/removal.

    In BlobWriterArenas (crates/expanse/src/sync.rs:10280), `has_writer_deltas: AtomicBool`
    is stored true on every write. Under W threads, every write generates an L1 invalidation
    and cross-core coherence transaction for that cache line.
    """
    writes_per_sec = ops_per_sec * write_fraction
    invalidation_traffic_bytes_per_sec = writes_per_sec * cacheline_bytes
    return {
        "writes_per_sec": writes_per_sec,
        "invalidations_per_sec": writes_per_sec,
        "traffic_mb_per_sec": invalidation_traffic_bytes_per_sec / (1024 * 1024),
    }


def candidate_b_chunk_page_faults(
    appends_per_sec: float,
    record_bytes: int = 144,
    chunk_size: int = 2 * 1024 * 1024,
    page_size: int = 4096,
) -> Dict[str, float]:
    """Candidate B: minor page faults on fresh 2 MiB chunks allocated by private arenas.

    Each 2 MiB chunk granted (crates/expanse/src/blobmap.rs:1724) holds:
    chunk_size / record_bytes records. Touching fresh anonymous memory incurs
    chunk_size / page_size = 512 page faults per chunk under 4 KiB paging.
    """
    records_per_chunk = chunk_size / record_bytes
    chunks_per_sec = appends_per_sec / records_per_chunk
    pages_per_chunk = chunk_size / page_size
    page_faults_per_sec = chunks_per_sec * pages_per_chunk
    return {
        "chunks_per_sec": chunks_per_sec,
        "page_faults_per_sec": page_faults_per_sec,
        "pages_per_chunk": pages_per_chunk,
    }


def candidate_c_header_reads(
    removes_per_sec: float,
    overwrites_per_sec: float,
) -> Dict[str, float]:
    """Candidate C: charge_dead header reads resolving lengths in disparate chunks.

    In charge_dead (crates/expanse/src/sync.rs:10631), resolve_meta_in_table reads
    the 8-byte record header across published chunks to determine payload length.
    """
    total_header_reads_per_sec = removes_per_sec + overwrites_per_sec
    return {
        "header_reads_per_sec": total_header_reads_per_sec,
    }


# ---------------------------------------------------------------------------
# 3. Item 2: Pre-Registered Instruction Ceilings & Attribution Table
# ---------------------------------------------------------------------------

# Current Callgrind instruction counts extracted from CI Run 37218971129 (main)
CURRENT_SYNC_BENCHMARKS: Dict[str, int] = {
    "sync_blobmap_churn/random": 143_334_832,
    "sync_blobmap_compact/random": 19_436_930,
    "sync_blobmap_compare_exchange/random": 52_661_355,
    "sync_blobmap_get/random": 19_682_875,
    "sync_blobmap_insert/random": 58_997_021,
    "sync_blobmap_insert_reclaiming/random": 812_952,
    "sync_blobmap_overwrite/random": 34_411_863,
    "sync_blobmap_remove/random": 67_503_310,
    "sync_blobmap_remove_miss/random": 19_221_107,
    "sync_bytesmap_churn/routes": 304_005_440,
    "sync_bytesmap_compare_exchange/routes": 130_533_586,
    "sync_bytesmap_get/routes": 32_270_096,
    "sync_bytesmap_insert/routes": 114_150_712,
    "sync_bytesmap_overwrite/routes": 58_672_690,
    "sync_bytesmap_remove/routes": 185_224_369,
    "sync_bytesmap_update/routes": 102_419_586,
    "sync_map_branchu_band/top": 10_065_997,
    "sync_map_churn/leaf": 124_997_852,
    "sync_map_churn/random": 123_607_195,
    "sync_map_compact_baseline/random60": 46_094_675,
    "sync_map_compact_drained/random60": 20_549_879,
    "sync_map_compare_exchange/random": 41_764_777,
    "sync_map_count_after_write/one_top_byte": 1_709_974_464,
    "sync_map_count_after_write/random": 32_139_090,
    "sync_map_count_after_write/sequential": 14_501_811,
    "sync_map_count_locked/one_top_byte": 11_110_655,
    "sync_map_count_locked/random": 11_050_125,
    "sync_map_count_locked/sequential": 4_426_901,
    "sync_map_drain_floor/top": 248_231,
    "sync_map_get/leaf": 7_554_521,
    "sync_map_get/random": 14_970_295,
    "sync_map_insert/random": 51_129_429,
    "sync_map_next_after_scan/clustered": 26_221_848,
    "sync_map_next_after_scan/random": 33_051_471,
    "sync_map_next_after_scan/sequential": 35_051_479,
    "sync_map_prev/random": 34_114_722,
    "sync_map_prev_locked/random": 57_226_942,
    "sync_map_remove/random": 64_379_580,
    "sync_map_scan/clustered": 4_595_253,
    "sync_map_scan/random": 10_331_730,
    "sync_map_scan/sequential": 4_593_400,
    "sync_map_update/random": 41_764_777,
    "sync_map_write_twin/one_top_byte": 2_163_979,
    "sync_map_write_twin/random": 1_926_727,
    "sync_map_write_twin/sequential": 2_179_054,
    "sync_set_churn/leaf": 97_839_488,
    "sync_set_churn/random": 98_495_626,
    "sync_set_compact_baseline/random60": 22_884_629,
    "sync_set_compact_drained/random60": 19_101_885,
    "sync_set_contains/leaf": 7_104_526,
    "sync_set_contains/random": 14_804_794,
    "sync_set_drain_floor/top": 251_841,
    "sync_set_insert/random": 47_271_375,
    "sync_set_remove/random": 55_982_089,
    "sync_strmap_churn/routes": 192_580_541,
    "sync_strmap_churn_short/short": 237_502_750,
    "sync_strmap_compare_exchange/routes": 123_741_610,
    "sync_strmap_get_short/short": 29_832_322,
    "sync_strmap_insert/routes": 79_106_223,
    "sync_strmap_insert_short/short": 99_490_464,
    "sync_strmap_insert_sorted/uuid": 46_912_663,
    "sync_strmap_remove/routes": 89_281_391,
    "sync_strmap_update/routes": 74_308_922,
}


def pre_registered_sync_ceilings() -> Dict[str, Dict[str, Any]]:
    """Derives and locks the Callgrind instruction ceilings for the enter_writer split PR.

    Per METHODOLOGY §35 and review item 5, ceilings are expressed as delta % vs merge base
    in the stage-2 PR's own instruction-counts run, with absolute counts from run 37218971129
    retained as context:
      - `sync_blobmap_overwrite/random`: ceiling <= +0.0% vs merge base;
        expected delta -6.8% (projected) (-2,350,000 Ir context from rust:1.98/Valgrind 3.24.0).
      - `sync_blobmap_insert/random`: ceiling <= +0.0% vs merge base.
      - `sync_blobmap_remove/random`: ceiling <= +0.0% vs merge base.
      - `sync_blobmap_compare_exchange/random`: ceiling <= +0.0% vs merge base.
      - All other `sync_*` arms: ceiling <= +0.1% vs merge base (noise floor per AGENTS.md §6).
      - Plain-tree arms: strictly <= +0.1% vs merge base (AGENTS.md §2.1 invariant 5).
    """
    ceilings = {}
    for arm, base_ir in CURRENT_SYNC_BENCHMARKS.items():
        if arm == "sync_blobmap_overwrite/random":
            max_pct = 0.0
            expected_delta = -ENTER_WRITER_OUTLINE_COST_IR
            expected_delta_pct = -6.8  # (projected)
            reason = "enter_writer inlined back; drops out-of-line call overhead"
        elif arm in ("sync_blobmap_insert/random", "sync_blobmap_remove/random", "sync_blobmap_compare_exchange/random"):
            max_pct = 0.0
            expected_delta = 0
            expected_delta_pct = 0.0
            reason = "hot path inlined; no regression permitted vs merge base"
        else:
            max_pct = 0.1
            expected_delta = 0
            expected_delta_pct = 0.0
            reason = "AGENTS.md §6 review noise ceiling (<= +0.1% vs merge base)"

        ceilings[arm] = {
            "base_ir": base_ir,
            "max_regression_pct": max_pct,
            "max_allowed_ir": int(base_ir * (1.0 + max_pct / 100.0)),
            "expected_delta_ir": expected_delta,
            "expected_delta_pct_projected": expected_delta_pct,
            "rationale": reason,
        }
    return ceilings


# ---------------------------------------------------------------------------
# 3b. Build A vs Build C Wall-Clock Speedup Evaluation (METHODOLOGY §35.6)
# ---------------------------------------------------------------------------

BUILD_A_VS_C_NAMED_CELL = "SyncExpanseBlobMap, 50% read, 16 threads, pin 0-15"
BUILD_A_VS_C_HISTORICAL_RUN1_CI = (1.217, 1.247)  # Context only from README §26 R2
BUILD_A_VS_C_HISTORICAL_RUN2_CI = (1.234, 1.260)  # Context only from README §26 R2
BUILD_A_VS_C_TARGET_FLOOR = 1.15  # Pre-registered floor (target)


def derive_build_a_speedup_floor(
    historical_min_lower: float = 1.217,
    dispersion_margin: float = 0.067,
) -> float:
    """Derives the pre-registered floor for Build A ÷ Build C wall-clock evaluation.

    Args:
        historical_min_lower: Lowest observed BCa 95% CI lower bound from README §26 R2 (context only).
        dispersion_margin: Margin to absorb between-run spread per BENCHMARKING rule 18.

    Returns:
        Floor value (1.15, labeled (target)), requiring a >15% speedup with lower bound >= floor.
    """
    if historical_min_lower <= 1.0:
        raise ValueError(f"Historical min lower must exceed 1.0, got {historical_min_lower}")
    if dispersion_margin < 0.0:
        raise ValueError(f"Dispersion margin must be non-negative, got {dispersion_margin}")
    floor = historical_min_lower - dispersion_margin
    if floor <= 1.0:
        raise ValueError(f"Derived floor must exceed 1.0, got {floor}")
    return round(floor, 4)


def evaluate_build_a_cross_run_verdict(
    run1_bca_ci: Tuple[float, float],
    run2_bca_ci: Tuple[float, float],
    floor: float = BUILD_A_VS_C_TARGET_FLOOR,
) -> Dict[str, Any]:
    """Evaluates cross-run verdict for Build A ÷ Build C per METHODOLOGY §35.6 and rule 18.

    Enforces the cross-run rule:
      - PASS: lower bound of BCa 95% CI >= floor in BOTH runs.
      - REFUTED: upper bound of BCa 95% CI < floor in BOTH runs.
      - INCONCLUSIVE: runs disagree, or either CI spans the floor.

    Args:
        run1_bca_ci: (lower, upper) BCa 95% CI for run 1.
        run2_bca_ci: (lower, upper) BCa 95% CI for run 2.
        floor: Pre-registered floor (default 1.15 (target)).

    Returns:
        Dict with per-run verdicts, cross-run consistency, and overall verdict.
    """
    for name, ci in [("run1", run1_bca_ci), ("run2", run2_bca_ci)]:
        if len(ci) != 2 or ci[0] > ci[1]:
            raise ValueError(f"Malformed CI for {name}: {ci}")

    l1, u1 = run1_bca_ci
    l2, u2 = run2_bca_ci

    run1_pass = l1 >= floor
    run1_refuted = u1 < floor
    run2_pass = l2 >= floor
    run2_refuted = u2 < floor

    # Cross-run rule: claim only when both runs move the same way
    moves_same_way = (run1_pass and run2_pass) or (run1_refuted and run2_refuted)

    if run1_pass and run2_pass:
        verdict = "PASS"
    elif run1_refuted and run2_refuted:
        verdict = "REFUTED"
    elif run1_pass != run2_pass and not (run1_refuted or run2_refuted):
        verdict = "INCONCLUSIVE"
    else:
        verdict = "INCONCLUSIVE"

    return {
        "floor": floor,
        "run1_ci": run1_bca_ci,
        "run2_ci": run2_bca_ci,
        "run1_passes": run1_pass,
        "run2_passes": run2_pass,
        "moves_same_way": moves_same_way,
        "verdict": verdict,
    }


class TestBlobPeakBounds(unittest.TestCase):
    """Pins mathematical derivations, USL fits, and ceiling integrity."""

    def test_analytical_primitives(self):
        """Pins standard Gunther USL equations."""
        self.assertAlmostEqual(usl_throughput(1.0, 1000.0, 0.05, 0.001), 1000.0)
        self.assertAlmostEqual(usl_throughput(4.0, 1000.0, 0.0, 0.0), 4000.0)
        self.assertEqual(usl_n_max(0.10, 0.0), float("inf"))
        self.assertAlmostEqual(usl_n_max(0.05, 0.002), math.sqrt(0.95 / 0.002), places=5)

    def test_usl_synthetic_recovery(self):
        """Verifies exact recovery of known parameters under OLS."""
        true_gamma, true_alpha, true_beta = 10000.0, 0.04, 0.002
        n_vals = [1.0, 2.0, 4.0, 8.0, 16.0]
        x_vals = [usl_throughput(ni, true_gamma, true_alpha, true_beta) for ni in n_vals]
        gamma, alpha, beta = fit_usl_ols(n_vals, x_vals)
        self.assertAlmostEqual(gamma, true_gamma, places=3)
        self.assertAlmostEqual(alpha, true_alpha, places=5)
        self.assertAlmostEqual(beta, true_beta, places=6)
        gof = usl_goodness_of_fit(n_vals, x_vals, gamma, alpha, beta)
        self.assertAlmostEqual(gof["r_squared"], 1.0, places=5)
        self.assertLess(gof["nrmse"], 1e-4)

    def test_blob_observed_scaling_fit(self):
        """Pins USL fit on observed mixed blob data: (1, 11869112), (4, 20369263), (16, 13198147)."""
        n_vals = [1.0, 4.0, 16.0]
        x_vals = [11869112.0, 20369263.0, 13198147.0]
        gamma, alpha, beta = fit_usl_ols(n_vals, x_vals)

        self.assertAlmostEqual(gamma, 11869112.0, places=1)
        self.assertAlmostEqual(alpha, 0.293932, places=4)
        self.assertAlmostEqual(beta, 0.037416, places=4)

        n_max = usl_n_max(alpha, beta)
        # Peak concurrency point is at ~4.34 threads
        self.assertAlmostEqual(n_max, 4.3438, places=3)

        peak_tp = usl_peak_throughput(gamma, alpha, beta)
        self.assertAlmostEqual(peak_tp, 20408041.59, places=1)

        diag = classify_contention_mechanism(alpha, beta)
        self.assertEqual(diag["diagnosis"], "COHERENCY_CROSSTALK_DOMINATES")
        self.assertTrue(diag["is_retrograde"])

    def test_invalid_parameters_raise(self):
        """Confirms fail-loud behavior on invalid USL parameters."""
        with self.assertRaises(ValueError):
            usl_throughput(0.5, 100.0, 0.1, 0.01)
        with self.assertRaises(ValueError):
            usl_throughput(2.0, -50.0, 0.1, 0.01)
        with self.assertRaises(ValueError):
            usl_throughput(2.0, 100.0, 1.5, 0.01)
        with self.assertRaises(ValueError):
            usl_throughput(2.0, 100.0, 0.1, -0.01)

    def test_candidate_models(self):
        """Verifies candidate hypothesis bounding functions."""
        # Candidate A
        a_res = candidate_a_line_sharing_traffic(ops_per_sec=20_000_000.0, write_fraction=0.5)
        self.assertEqual(a_res["writes_per_sec"], 10_000_000.0)
        self.assertEqual(a_res["invalidations_per_sec"], 10_000_000.0)
        self.assertAlmostEqual(a_res["traffic_mb_per_sec"], (10_000_000.0 * 64) / (1024 * 1024), places=3)

        # Candidate B
        b_res = candidate_b_chunk_page_faults(appends_per_sec=5_000_000.0, record_bytes=144)
        self.assertGreater(b_res["page_faults_per_sec"], 0.0)
        self.assertEqual(b_res["pages_per_chunk"], 512.0)

        # Candidate C
        c_res = candidate_c_header_reads(removes_per_sec=2_500_000.0, overwrites_per_sec=2_500_000.0)
        self.assertEqual(c_res["header_reads_per_sec"], 5_000_000.0)

    def test_ceilings_structure(self):
        """Verifies integrity of pre-registered instruction ceilings."""
        ceilings = pre_registered_sync_ceilings()
        self.assertEqual(len(ceilings), 63)
        self.assertIn("sync_blobmap_overwrite/random", ceilings)

        ow = ceilings["sync_blobmap_overwrite/random"]
        self.assertEqual(ow["base_ir"], 34_411_863)
        self.assertEqual(ow["max_regression_pct"], 0.0)
        self.assertEqual(ow["max_allowed_ir"], 34_411_863)

        for arm, entry in ceilings.items():
            self.assertGreater(entry["base_ir"], 0)
            self.assertIn(entry["max_regression_pct"], (0.0, 0.1))
            self.assertGreaterEqual(entry["max_allowed_ir"], entry["base_ir"])

    def test_classify_with_ci(self):
        """Verifies classification rules require CI to exclude zero."""
        # Beta CI excludes zero -> COHERENCY_CROSSTALK_DOMINATES
        res1 = classify_contention_mechanism(0.20, 0.03, alpha_ci=(0.10, 0.30), beta_ci=(0.01, 0.05))
        self.assertEqual(res1["diagnosis"], "COHERENCY_CROSSTALK_DOMINATES")

        # Beta CI overlaps zero, alpha CI excludes zero -> SERIALIZATION_DOMINATES
        res2 = classify_contention_mechanism(0.25, 0.001, alpha_ci=(0.15, 0.35), beta_ci=(-0.002, 0.004))
        self.assertEqual(res2["diagnosis"], "SERIALIZATION_DOMINATES")

        # Both CI overlap zero -> INCONCLUSIVE_OR_BALANCED
        res3 = classify_contention_mechanism(0.05, 0.001, alpha_ci=(-0.02, 0.12), beta_ci=(-0.001, 0.003))
        self.assertEqual(res3["diagnosis"], "INCONCLUSIVE_OR_BALANCED")

    def test_build_a_cross_run_evaluation(self):
        """Verifies Build A ÷ Build C floor derivation and cross-run pass rule."""
        floor = derive_build_a_speedup_floor(1.217, 0.067)
        self.assertEqual(floor, 1.15)

        # Historical run 1 & 2 pass against 1.15 floor
        res_hist = evaluate_build_a_cross_run_verdict(
            BUILD_A_VS_C_HISTORICAL_RUN1_CI,
            BUILD_A_VS_C_HISTORICAL_RUN2_CI,
            floor=floor,
        )
        self.assertEqual(res_hist["verdict"], "PASS")
        self.assertTrue(res_hist["moves_same_way"])

        # Diverging runs -> INCONCLUSIVE
        res_div = evaluate_build_a_cross_run_verdict((1.16, 1.22), (1.10, 1.18), floor=floor)
        self.assertEqual(res_div["verdict"], "INCONCLUSIVE")
        self.assertFalse(res_div["moves_same_way"])

        # Both runs below floor -> REFUTED
        res_ref = evaluate_build_a_cross_run_verdict((1.02, 1.12), (1.05, 1.14), floor=floor)
        self.assertEqual(res_ref["verdict"], "REFUTED")
        self.assertTrue(res_ref["moves_same_way"])

        # Invalid intervals raise
        with self.assertRaises(ValueError):
            evaluate_build_a_cross_run_verdict((1.20, 1.10), (1.16, 1.20))


def main() -> None:
    parser = argparse.ArgumentParser(description="Blob four-thread peak and enter_writer bounds.")
    parser.add_argument("--self-test", action="store_true", help="Run reference-pinned unit tests")
    args = parser.parse_args()

    if args.self_test:
        sys.argv = [sys.argv[0]]
        unittest.main()
        return

    print("=== Expanse #1280: Blob Peak & Shared::enter_writer Bounds ===")
    print(f"Reference CI Run: {BASELINE_CI_RUN_ID} (commit {BASELINE_COMMIT_SHA[:10]})")
    print("\n1. USL Fit on Observed Blob Mixed Cells (50% read):")
    n_vals = [1.0, 4.0, 16.0]
    x_vals = [11869112.0, 20369263.0, 13198147.0]
    gamma, alpha, beta = fit_usl_ols(n_vals, x_vals)
    n_max = usl_n_max(alpha, beta)
    peak = usl_peak_throughput(gamma, alpha, beta)
    print(f"  gamma (T=1):  {gamma:,.2f} ops/s")
    print(f"  alpha (cont): {alpha:.6f}")
    print(f"  beta (coher): {beta:.6f}")
    print(f"  N_max (peak): {n_max:.2f} threads")
    print(f"  Peak Ops/s:   {peak:,.2f} ops/s")

    print("\n2. Shared::enter_writer Hot/Cold Split Ceilings (Sample):")
    ceilings = pre_registered_sync_ceilings()
    for arm in [
        "sync_blobmap_overwrite/random",
        "sync_blobmap_insert/random",
        "sync_blobmap_remove/random",
        "sync_blobmap_compare_exchange/random",
        "sync_map_insert/random",
    ]:
        c = ceilings[arm]
        print(f"  {arm:40}: base={c['base_ir']:,} Ir, ceiling={c['max_regression_pct']}% (max={c['max_allowed_ir']:,} Ir)")

    print("\n3. Build A vs Build C Speedup Evaluation (Named Cell):")
    floor = derive_build_a_speedup_floor()
    print(f"  Named cell:   {BUILD_A_VS_C_NAMED_CELL}")
    print(f"  Pre-reg floor: BCa 95% CI lower bound >= {floor} (target)")
    hist_eval = evaluate_build_a_cross_run_verdict(
        BUILD_A_VS_C_HISTORICAL_RUN1_CI,
        BUILD_A_VS_C_HISTORICAL_RUN2_CI,
        floor=floor,
    )
    print(f"  Historical run 1 CI: {BUILD_A_VS_C_HISTORICAL_RUN1_CI}")
    print(f"  Historical run 2 CI: {BUILD_A_VS_C_HISTORICAL_RUN2_CI}")
    print(f"  Cross-run verdict on historical: {hist_eval['verdict']} (moves_same_way={hist_eval['moves_same_way']})")


if __name__ == "__main__":
    main()

