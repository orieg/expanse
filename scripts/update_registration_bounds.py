#!/usr/bin/env python3
"""Bounds for the contended `update` cells (#1417, AGENTS.md §8.8 commit 1).

The arithmetic `docs/benchmarks/concurrency/METHODOLOGY.md` §39 rests on,
computed here rather than narrated, and unit-tested against pinned values:

1. an upper bound on the W = 1 ratio `T_held / T_update` from the committed
   Callgrind figures (a HYPOTHESIS: it assumes time per instruction is the
   same on both arms);
2. the per-round spread of a paired, one-process-per-window ratio on the
   reference host, borrowed from the #568 Step 2 gate artifacts;
3. the smallest departure of the paired ratio from 1 that the registered round
   count resolves, the round count a given margin needs, and the chance that
   the falsifier can be reached when the true ratio is 1;
4. for §40 (the engine change), the per-window spread of the §39.9 base
   pair's gated cells, the smallest change-over-base gain of the `update`
   arm that spread resolves, and the base pair's own between-run drift,
   which the held-arm control of §40 bounds.

What this module does not do: predict a wall-clock level under contention,
attribute a mechanism, or evaluate the §39 statistic. The driver
(`docs/benchmarks/concurrency/scripts/mixed_concurrency.py --update-cells`)
evaluates it.

Inputs, and where each comes from
---------------------------------
`rmw_instructions` (`crates/expanse/benches/rmw_instructions.rs`), 50,000
operations per arm, pull request #1411, job "Perf / Callgrind Deterministic
Instructions", run https://github.com/orieg/expanse/actions/runs/37539678414 at
`56a8f741e` (workload: core_rmw_instructions; measured: GitHub-hosted x86_64
runner, 56a8f741e). Per operation: `sync_map_cas` 526.5 and
`sync_map_update_rmw` 1,113.9; `sync_strmap_cas` 1,571.4 and
`sync_strmap_update_rmw` 2,852.7; `sync_bytesmap_cas` 1,239.7 and
`sync_bytesmap_update_rmw` 2,131.6. The `_cas` arms pass the current value
without reading it, so `update - cas` is one validated read plus one reader
registration and drop. The held arm of §39 still makes that read, so
`update / cas` bounds `T_held / T_update` from above at W = 1.

`docs/benchmarks/concurrency/results/baseline_concurrent_step2_gate.json` and
`..._run2.json` (#568, METHODOLOGY.md §13, commit `5228fc3a`): 48 rounds of
per-round paired ratios between two builds of `benches/concurrency.rs`, one
bench process per 500 ms window, on the reference host. That is the instrument
shape §39 uses (paired adjacent windows, a process per window), on a different
workload (50% read on map and set) and pin (0-15), so the spread is a borrowed
sizing input, never a prediction.

Sources for the estimators
--------------------------
Efron, "Better Bootstrap Confidence Intervals", JASA 82 (1987): the BCa
  interval of a mean converges to the normal-theory interval
  `mean +/- z_{1-alpha/2} * s / sqrt(n)` when the sample is symmetric and the
  acceleration is near zero, which is the approximation `half_width` states.
  On skewed or small samples the BCa interval is wider or asymmetric; the
  approximation is a sizing aid, never the interval the driver reports.
Cohen, Statistical Power Analysis for the Behavioral Sciences, 2nd ed. (1988),
  ch. 2: the minimum detectable difference of a one-sample (paired) test,
  `(z_{1-alpha/2} + z_{1-beta}) * sigma / sqrt(n)`, and the sample size that
  inverts it.
The delta method for a ratio of two independent positive variables with
  coefficients of variation c1 and c2: CV(X/Y) ~= sqrt(c1^2 + c2^2) to first
  order. `kappa = rho(W) / rho(1)` is such a ratio.

Usage:
    python3 scripts/update_registration_bounds.py              # the table, then the unit tests
    python3 scripts/update_registration_bounds.py --self-test  # the unit tests only
    python3 scripts/update_registration_bounds.py --table      # the table only
"""

from __future__ import annotations

import json
import math
import statistics
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
RESULTS = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results"

# Instructions per operation, #1411 (see the module docstring).
UPDATE_PER_OP = {"map": 1113.9, "str": 2852.7, "bytes": 2131.6}
CAS_PER_OP = {"map": 526.5, "str": 1571.4, "bytes": 1239.7}

# §39 registration. The margin is the smallest cost an engine change is worth
# pre-registering for; the rounds are sized against it below. The driver
# carries the same two values and its self-test asserts they agree.
MARGIN = 0.05
ROUNDS = 36
# One full balance cycle of the §39 order: 3 engine rotations x 4 Williams rows
# x 2 arm orders, least common multiple 12.
DESIGN_PERIOD = 12

SPREAD_ARTIFACTS = (
    RESULTS / "baseline_concurrent_step2_gate.json",
    RESULTS / "baseline_concurrent_step2_gate_run2.json",
)
# The map cells only: §39 measures map, str and bytes, and the Step 2 set cell
# at 16 threads under the SMT pin is bimodal (a set-specific shape, §13), which
# would size the run for a distribution §39's cells are not expected to have.
SPREAD_ENGINE = "map"

ALPHA = 0.05
POWER = 0.80


def z_quantile(p: float) -> float:
    """The standard normal quantile at `p`, 0 < p < 1."""
    if not 0.0 < p < 1.0:
        raise ValueError(f"p must lie in (0, 1), got {p}")
    return statistics.NormalDist().inv_cdf(p)


def w1_ratio_upper(update_per_op: float, cas_per_op: float) -> float:
    """Upper bound on `T_held / T_update` at W = 1 if time is proportional to instructions (HYPOTHESIS).

    `update` is a read, a registration and an exchange; `cas` is the exchange
    alone. The held arm is the read and the exchange, so its cost lies in
    `[cas, update]` and the ratio of rates in `[1, update / cas]`.
    """
    if update_per_op <= 0 or cas_per_op <= 0:
        raise ValueError("instruction counts must be positive")
    if update_per_op < cas_per_op:
        raise ValueError(f"update ({update_per_op}) below cas ({cas_per_op}): the inputs describe different work")
    return update_per_op / cas_per_op


def ratio_cv(cv_numerator: float, cv_denominator: float) -> float:
    """First-order CV of a ratio of independent variables (delta method)."""
    if cv_numerator < 0 or cv_denominator < 0:
        raise ValueError("coefficients of variation must be non-negative")
    return math.hypot(cv_numerator, cv_denominator)


def half_width(cv: float, rounds: int, alpha: float = ALPHA) -> float:
    """Relative half-width of a 95% interval of a mean over `rounds` per-round values of CV `cv`."""
    if cv < 0:
        raise ValueError(f"cv must be non-negative, got {cv}")
    if rounds < 3:
        raise ValueError(f"a BCa interval needs at least 3 rounds, got {rounds}")
    return z_quantile(1.0 - alpha / 2.0) * cv / math.sqrt(rounds)


def mde(cv: float, rounds: int, alpha: float = ALPHA, power: float = POWER) -> float:
    """Smallest departure from 1 of a per-round ratio of CV `cv` that `rounds` rounds resolve (Cohen 1988)."""
    if not 0.0 < power < 1.0:
        raise ValueError(f"power must lie in (0, 1), got {power}")
    if cv < 0:
        raise ValueError(f"cv must be non-negative, got {cv}")
    if rounds < 3:
        raise ValueError(f"a BCa interval needs at least 3 rounds, got {rounds}")
    return (z_quantile(1.0 - alpha / 2.0) + z_quantile(power)) * cv / math.sqrt(rounds)


def rounds_needed(cv: float, margin: float, period: int = DESIGN_PERIOD,
                  alpha: float = ALPHA, power: float = POWER) -> int:
    """The fewest whole design periods of rounds whose MDE is at most `margin`."""
    if margin <= 0:
        raise ValueError(f"margin must be positive, got {margin}")
    if period < 1:
        raise ValueError(f"period must be positive, got {period}")
    if cv < 0:
        raise ValueError(f"cv must be non-negative, got {cv}")
    n = math.ceil(((z_quantile(1.0 - alpha / 2.0) + z_quantile(power)) * cv / margin) ** 2)
    n = max(n, 3)
    return period * -(-n // period)


def falsify_power(cv: float, rounds: int, margin: float, alpha: float = ALPHA) -> float:
    """Chance that the upper bound falls below `1 + margin` when the true ratio is 1.

    Normal approximation: the upper bound is `mean + z * s`, with
    `s = cv / sqrt(rounds)` and the mean distributed `N(1, s)`, so the chance is
    `Phi(margin / s - z)`. This is the falsifier's reachability; a design where
    it is small could only ever read INCONCLUSIVE on a null effect.
    """
    if margin <= 0:
        raise ValueError(f"margin must be positive, got {margin}")
    if cv <= 0:
        raise ValueError(f"cv must be positive, got {cv}")
    if rounds < 3:
        raise ValueError(f"a BCa interval needs at least 3 rounds, got {rounds}")
    s = cv / math.sqrt(rounds)
    return statistics.NormalDist().cdf(margin / s - z_quantile(1.0 - alpha / 2.0))


def borrowed_ratio_cv(contended: bool, paths: tuple[Path, ...] = SPREAD_ARTIFACTS,
                      engine: str = SPREAD_ENGINE) -> float:
    """The largest per-round CV of the paired ratio over the `engine` cells of `paths`.

    `contended` selects the cells above one thread, otherwise the one-thread cells.
    """
    worst = 0.0
    seen = 0
    for path in paths:
        artifact = json.loads(path.read_text())
        for cell in artifact["ratio"]:
            if cell.get("engine_key") != engine or (int(cell["threads"]) > 1) != contended:
                continue
            xs = [float(r["ratio"]) for r in cell["rounds_raw"]]
            if len(xs) < 2:
                raise ValueError(f"{path.name} {engine} T={cell['threads']}: {len(xs)} rounds")
            worst = max(worst, statistics.stdev(xs) / statistics.fmean(xs))
            seen += 1
    if seen == 0:
        raise ValueError(f"no {engine} cells (contended={contended}) in {[p.name for p in paths]}")
    return worst


# ---------------------------------------------------------------------------
# §40: the engine change's cross-build comparison (#1417)
# ---------------------------------------------------------------------------
#
# §40 compares the `update` arm of the change build's §39 cells with the base
# build's, run against run. The base is the §39.9 pair, whose engine is the
# change's base engine; the two builds run in different dispatches, so the
# comparison is between runs, not within a round. These functions size it
# from the base pair's own windows: the per-window spread of the gated cells,
# the smallest gain that spread resolves, and how far one arm moved between
# the two base runs, which is the drift the held-arm control of §40 bounds.

BASE_ARTIFACTS = (
    RESULTS / "baseline_concurrent_update_cells.json",
    RESULTS / "baseline_concurrent_update_cells_run2.json",
)
GAIN_THREADS = 8
ENGINES = ("map", "str", "bytes")
ARMS = ("update", "held")


def window_rates(path: Path, engine: str, arm: str, threads: int = GAIN_THREADS) -> list[float]:
    """Updates per second of every window of one §39 throughput cell, in round order."""
    artifact = json.loads(path.read_text())
    cells = [c for c in artifact["throughput"]
             if c["engine"] == engine and c["arm"] == arm and int(c["threads"]) == threads]
    if len(cells) != 1:
        raise ValueError(f"{path.name}: {len(cells)} cells for {engine}/{arm} at {threads} threads, not one")
    rates = [float(w["updates_s"]) for w in sorted(cells[0]["rounds_raw"], key=lambda w: w["round"])]
    if len(rates) < 3:
        raise ValueError(f"{path.name}: {engine}/{arm} at {threads} threads has {len(rates)} windows")
    return rates


def cv(values: list[float]) -> float:
    """Sample coefficient of variation."""
    if len(values) < 2:
        raise ValueError(f"a spread needs at least 2 values, got {len(values)}")
    mean = statistics.fmean(values)
    if mean <= 0:
        raise ValueError(f"a coefficient of variation needs a positive mean, got {mean}")
    return statistics.stdev(values) / mean


def two_sample_ratio_mde(cv_num: float, n_num: int, cv_den: float, n_den: int,
                         alpha: float = ALPHA, power: float = POWER) -> float:
    """Smallest departure from 1 of a ratio of two independent means that the samples resolve.

    First-order (delta method) CV of a ratio of means, `sqrt(c1^2/n1 + c2^2/n2)`,
    times `z_{1-alpha/2} + z_{power}` (Cohen 1988, ch. 2). A sizing aid only:
    the interval §40 reads is the two-sample BCa of `scripts/bca_bootstrap.py`.
    """
    if not 0.0 < power < 1.0:
        raise ValueError(f"power must lie in (0, 1), got {power}")
    if cv_num < 0 or cv_den < 0:
        raise ValueError("coefficients of variation must be non-negative")
    if n_num < 3 or n_den < 3:
        raise ValueError(f"a two-sample BCa interval needs at least 3 values per arm, got {n_num} and {n_den}")
    se = math.sqrt(cv_num ** 2 / n_num + cv_den ** 2 / n_den)
    return (z_quantile(1.0 - alpha / 2.0) + z_quantile(power)) * se


def between_run_ratio(engine: str, arm: str, paths: tuple[Path, ...] = BASE_ARTIFACTS,
                      threads: int = GAIN_THREADS) -> float:
    """Mean updates/s of the second run's cell over the first's: the base pair's between-run drift."""
    if len(paths) != 2:
        raise ValueError(f"a between-run ratio needs two runs, got {len(paths)}")
    first, second = (statistics.fmean(window_rates(p, engine, arm, threads)) for p in paths)
    return second / first


def worst_base_cv(engine: str, paths: tuple[Path, ...] = BASE_ARTIFACTS,
                  threads: int = GAIN_THREADS) -> float:
    """The largest per-window CV of `engine`'s gated cells over both arms and both base runs."""
    return max(cv(window_rates(p, engine, a, threads)) for p in paths for a in ARMS)


def render_table() -> str:
    lines = ["| engine | update / op | cas / op | T_held / T_update at W = 1, upper bound (hypothesis) |",
             "|---|---:|---:|---:|"]
    for e in UPDATE_PER_OP:
        lines.append(f"| {e} | {UPDATE_PER_OP[e]:,.1f} | {CAS_PER_OP[e]:,.1f} | "
                     f"{w1_ratio_upper(UPDATE_PER_OP[e], CAS_PER_OP[e]):.4f} |")
    cv_w = borrowed_ratio_cv(True)
    cv_1 = borrowed_ratio_cv(False)
    cv_k = ratio_cv(cv_w, cv_1)
    lines.append("")
    lines.append(f"Borrowed per-round ratio CV (Step 2 map cells): contended {cv_w:.4f}, one thread {cv_1:.4f}; "
                 f"kappa {cv_k:.4f}.")
    lines.append(f"Registered: margin {MARGIN}, {ROUNDS} rounds (period {DESIGN_PERIOD}).")
    for label, cv in (("rho(W >= 2)", cv_w), ("rho(1)", cv_1), ("kappa", cv_k)):
        lines.append(f"  {label}: MDE {mde(cv, ROUNDS):.4f}, 95% half-width {half_width(cv, ROUNDS):.4f}, "
                     f"rounds needed for MDE <= margin {rounds_needed(cv, MARGIN)}, "
                     f"falsifier reachable at ratio 1 with probability {falsify_power(cv, ROUNDS, MARGIN):.3f}")
    lines.append("")
    lines.append(f"§40, from the §39.9 base pair at W = {GAIN_THREADS} (per-window CV; between-run ratio run 2 / run 1):")
    lines.append("| engine | worst CV | update run 2 / run 1 | held run 2 / run 1 | MDE of T_update(change) / T_update(base) |")
    lines.append("|---|---:|---:|---:|---:|")
    for e in ENGINES:
        worst = worst_base_cv(e)
        lines.append(f"| {e} | {worst:.4f} | {between_run_ratio(e, 'update'):.4f} | "
                     f"{between_run_ratio(e, 'held'):.4f} | "
                     f"{two_sample_ratio_mde(worst, ROUNDS, worst, ROUNDS):.4f} |")
    return "\n".join(lines)


class QuantileTests(unittest.TestCase):
    def test_normal_quantiles_match_the_table(self):
        self.assertAlmostEqual(z_quantile(0.975), 1.959964, places=6)
        self.assertAlmostEqual(z_quantile(0.80), 0.841621, places=6)


class InstructionBoundTests(unittest.TestCase):
    def test_w1_upper_bounds_pinned(self):
        # 1113.9 / 526.5, 2852.7 / 1571.4, 2131.6 / 1239.7
        self.assertAlmostEqual(w1_ratio_upper(1113.9, 526.5), 2.115670, places=6)
        self.assertAlmostEqual(w1_ratio_upper(2852.7, 1571.4), 1.815388, places=6)
        self.assertAlmostEqual(w1_ratio_upper(2131.6, 1239.7), 1.719448, places=6)

    def test_every_engine_has_both_counts(self):
        self.assertEqual(set(UPDATE_PER_OP), set(CAS_PER_OP))
        for e in UPDATE_PER_OP:
            self.assertGreater(w1_ratio_upper(UPDATE_PER_OP[e], CAS_PER_OP[e]), 1.0, e)


class DetectabilityTests(unittest.TestCase):
    def test_ratio_cv_is_first_order(self):
        self.assertAlmostEqual(ratio_cv(0.03, 0.04), 0.05)

    def test_half_width_pinned(self):
        # 1.959964 * 0.1 / 6
        self.assertAlmostEqual(half_width(0.1, 36), 0.032666, places=6)

    def test_mde_pinned(self):
        # (1.959964 + 0.841621) * 0.1 / 6 = 2.801585 / 60
        self.assertAlmostEqual(mde(0.1, 36), 0.046693, places=6)
        self.assertAlmostEqual(mde(0.02, 8), 0.019810, places=6)

    def test_rounds_needed_pinned(self):
        # (2.801585 * 0.1 / 0.05)^2 = 31.40 -> 32 -> 36 in periods of 12
        self.assertEqual(rounds_needed(0.1, 0.05), 36)
        # (2.801585 * 0.05 / 0.05)^2 = 7.85 -> 8 -> 12
        self.assertEqual(rounds_needed(0.05, 0.05), 12)
        self.assertEqual(rounds_needed(0.1, 0.05, period=1), 32)
        # A zero spread still needs a BCa interval's 3 rounds.
        self.assertEqual(rounds_needed(0.0, 0.05, period=1), 3)

    def test_falsify_power_pinned(self):
        # Phi(0.05 / (0.1 / 6) - 1.959964) = Phi(1.040036)
        self.assertAlmostEqual(falsify_power(0.1, 36, 0.05), 0.850836, places=5)
        self.assertLess(falsify_power(0.1, 12, 0.05), falsify_power(0.1, 36, 0.05))

    def test_mde_falls_with_rounds(self):
        self.assertLess(mde(0.1, 48), mde(0.1, 36))


class ArtifactTests(unittest.TestCase):
    def test_borrowed_spread_is_read_from_the_committed_cells(self):
        cv_w = borrowed_ratio_cv(True)
        cv_1 = borrowed_ratio_cv(False)
        # The Step 2 map cells: 16 threads 0.0928 and 0.1015, 1 thread 0.0068 and 0.0084.
        self.assertAlmostEqual(cv_w, 0.1015, places=4)
        self.assertAlmostEqual(cv_1, 0.0084, places=4)

    def test_registered_rounds_resolve_the_margin(self):
        cv_k = ratio_cv(borrowed_ratio_cv(True), borrowed_ratio_cv(False))
        self.assertLessEqual(mde(cv_k, ROUNDS), MARGIN)
        self.assertLessEqual(rounds_needed(cv_k, MARGIN), ROUNDS)
        self.assertEqual(ROUNDS % DESIGN_PERIOD, 0)
        self.assertGreaterEqual(falsify_power(cv_k, ROUNDS, MARGIN), POWER)

    def test_the_margin_is_inside_every_w1_bound(self):
        # A margin above the largest W = 1 bound could not be met even by the
        # whole uncontended difference; it is far inside all three.
        for e in UPDATE_PER_OP:
            self.assertLess(1.0 + MARGIN, w1_ratio_upper(UPDATE_PER_OP[e], CAS_PER_OP[e]), e)


class CrossBuildTests(unittest.TestCase):
    """§40: sizing the change-versus-base comparison from the committed §39.9 pair."""

    def test_two_sample_mde_pinned(self):
        # 2.801585 * sqrt(0.02^2 / 36 + 0.02^2 / 36) = 2.801585 * 0.0047140
        self.assertAlmostEqual(two_sample_ratio_mde(0.02, 36, 0.02, 36), 0.013207, places=6)
        # Unequal arms: 2.801585 * sqrt(0.01^2 / 36 + 0.03^2 / 12) = 2.801585 * 0.0088192
        self.assertAlmostEqual(two_sample_ratio_mde(0.01, 36, 0.03, 12), 0.024708, places=6)
        self.assertLess(two_sample_ratio_mde(0.02, 72, 0.02, 72), two_sample_ratio_mde(0.02, 36, 0.02, 36))

    def test_cv_pinned(self):
        # mean 2, sample stdev 1
        self.assertAlmostEqual(cv([1.0, 2.0, 3.0]), 0.5)

    def test_base_windows_are_read_from_the_committed_cells(self):
        for path in BASE_ARTIFACTS:
            for e in ENGINES:
                for a in ARMS:
                    self.assertEqual(len(window_rates(path, e, a)), ROUNDS, (path.name, e, a))
        # Worst per-window CV over both arms and runs at W = 8: map held run 1,
        # str held run 2, bytes held run 1.
        self.assertAlmostEqual(worst_base_cv("map"), 0.012034, places=6)
        self.assertAlmostEqual(worst_base_cv("str"), 0.021795, places=6)
        self.assertAlmostEqual(worst_base_cv("bytes"), 0.018033, places=6)

    def test_base_between_run_drift_pinned(self):
        self.assertAlmostEqual(between_run_ratio("map", "update"), 0.998759, places=6)
        self.assertAlmostEqual(between_run_ratio("map", "held"), 1.002117, places=6)
        self.assertAlmostEqual(between_run_ratio("str", "update"), 1.001901, places=6)
        self.assertAlmostEqual(between_run_ratio("str", "held"), 0.997522, places=6)
        self.assertAlmostEqual(between_run_ratio("bytes", "update"), 0.996200, places=6)
        self.assertAlmostEqual(between_run_ratio("bytes", "held"), 1.001644, places=6)

    def test_the_cross_build_design_resolves_the_margin(self):
        # The drift between the two base runs sits far inside the held-arm
        # control's band, and 36 windows a side resolve a gain well below the
        # margin; the expected gain (the base rho) is two orders above it.
        for e in ENGINES:
            for a in ARMS:
                self.assertLess(abs(between_run_ratio(e, a) - 1.0), MARGIN / 10, (e, a))
            worst = worst_base_cv(e)
            self.assertLess(two_sample_ratio_mde(worst, ROUNDS, worst, ROUNDS), MARGIN / 2, e)


class ArgumentTests(unittest.TestCase):
    def test_invalid_arguments_raise(self):
        for call in (
            lambda: z_quantile(1.0),
            lambda: w1_ratio_upper(0.0, 1.0),
            lambda: w1_ratio_upper(1.0, 2.0),
            lambda: ratio_cv(-0.1, 0.1),
            lambda: half_width(-0.1, 36),
            lambda: half_width(0.1, 2),
            lambda: mde(0.1, 2),
            lambda: mde(0.1, 36, power=0.0),
            lambda: rounds_needed(0.1, 0.0),
            lambda: rounds_needed(0.1, 0.05, period=0),
            lambda: falsify_power(0.0, 36, 0.05),
            lambda: falsify_power(0.1, 36, 0.0),
            lambda: borrowed_ratio_cv(True, paths=()),
            lambda: cv([1.0]),
            lambda: cv([0.0, 0.0]),
            lambda: two_sample_ratio_mde(-0.1, 36, 0.1, 36),
            lambda: two_sample_ratio_mde(0.1, 2, 0.1, 36),
            lambda: two_sample_ratio_mde(0.1, 36, 0.1, 36, power=1.0),
            lambda: between_run_ratio("map", "update", paths=BASE_ARTIFACTS[:1]),
            lambda: window_rates(BASE_ARTIFACTS[0], "set", "update"),
        ):
            with self.assertRaises(ValueError):
                call()


if __name__ == "__main__":
    if "--table" in sys.argv:
        print(render_table())
        sys.exit(0)
    if "--self-test" not in sys.argv:
        print(render_table())
        print()
    sys.argv = [sys.argv[0]]
    unittest.main()
