#!/usr/bin/env python3
"""Bounds for the shared `BranchU` band cells (#1208, AGENTS.md §8.8 commit 1).

The arithmetic `docs/benchmarks/concurrency/METHODOLOGY.md` §24 rests on,
computed here rather than narrated, and unit-tested against pinned values:

1. what share of a band cycle's instructions the two U <-> B crossings are
   expected to take at W = 1, from the committed Callgrind figures;
2. the smallest per-round paired ratio `t_band / t_twin` that 8 interleaved
   rounds can resolve, for a given per-round coefficient of variation;
3. whether an expected effect clears that bound.

What this module does not do: predict a wall-clock level, attribute a
mechanism, or evaluate the §24 statistic. The crossing share is a
deterministic instruction share, and turning it into a time ratio assumes time
per instruction is the same on both cycles; that is a hypothesis, labelled as
one wherever it is used.

Inputs, and where each comes from
---------------------------------
`sync_map_branchu_band/top` (`crates/expanse/benches/instructions.rs`): 100
cycles of 33 removals and 33 reinsertions carrying a top `BranchU` from 193
digits to its demotion floor (160) and back, two crossings per cycle. Main CI
run https://github.com/orieg/expanse/actions/runs/36326465116 at `5fdb77f3`,
job "Perf / Callgrind Deterministic Instructions": 10,116,194 instructions at
the default x86-64 target and 8,977,550 at `-C target-cpu=x86-64-v3`.

`sync_map_branchu_thrash/top`: 100 cycles of two removals and two reinsertions
carrying a top `BranchU` across the one-digit band (191 <-> 193) that preceded
#1221, two crossings per cycle. Pull-request CI run
https://github.com/orieg/expanse/actions/runs/36256715324 at `e7d57aa3` (#1204),
same job: 3,278,033 instructions at the default target and 2,747,239 at
x86-64-v3, i.e. 16,390 and 13,736 per crossing. Those per-crossing figures
include the four operations of the cycle, so they are an upper bound on the
exclusive crossing alone. They were taken on #1204's engine with the one-digit
band, where a demotion rebuilds a 191-digit `BranchU` rather than a 160-digit
one; carrying them onto the widened band is the projection's second
assumption.

Sources for the estimators
--------------------------
Efron, "Better Bootstrap Confidence Intervals", JASA 82 (1987): the BCa
  interval of a mean converges to the normal-theory interval
  `mean +/- z_{1-alpha/2} * s / sqrt(n)` when the sample is symmetric and the
  acceleration is near zero, which is the approximation `bca_half_width`
  states. On skewed or small samples the BCa interval is wider or asymmetric;
  the approximation is a sizing aid, never the interval the driver reports.
Cohen, Statistical Power Analysis for the Behavioral Sciences, 2nd ed. (1988),
  ch. 2: the minimum detectable difference of a one-sample (paired) test,
  `(z_{1-alpha/2} + z_{1-beta}) * sigma / sqrt(n)`.
The delta method for a ratio of two independent positive variables with
  coefficients of variation c1 and c2: CV(X/Y) ~= sqrt(c1^2 + c2^2) to first
  order; with c1 = c2 = c that is sqrt(2) * c. Pairing within a round removes
  the part of the variation the two cells share, so this is conservative.

Usage:
    python3 scripts/branchu_band_bounds.py              # the table, then the unit tests
    python3 scripts/branchu_band_bounds.py --self-test  # the unit tests only
    python3 scripts/branchu_band_bounds.py --table      # the table only
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

# The Callgrind figures above, by target. Instructions over the whole arm.
BAND_CYCLES = 100
BAND_CROSSINGS_PER_CYCLE = 2
BAND_ARM_INSTRUCTIONS = {"x86-64": 10_116_194, "x86-64-v3": 8_977_550}
THRASH_CROSSINGS = 200
THRASH_ARM_INSTRUCTIONS = {"x86-64": 3_278_033, "x86-64-v3": 2_747_239}

# The §24 schedule: rounds per run.
ROUNDS = 8

# Writer-sweep artifacts at one commit under the §24 pin, read only for the
# per-round spread of a map writer cell; the band cells are a different
# workload, so this is a borrowed sizing input, never a prediction.
SPREAD_ARTIFACTS = (
    RESULTS / "baseline_writer_scaling_170a4bc3_percore.json",
    RESULTS / "baseline_writer_scaling_170a4bc3_percore_run2.json",
)

ALPHA = 0.05
POWER = 0.80


def z_quantile(p: float) -> float:
    """The standard normal quantile at `p`, 0 < p < 1."""
    if not 0.0 < p < 1.0:
        raise ValueError(f"p must lie in (0, 1), got {p}")
    return statistics.NormalDist().inv_cdf(p)


def per_crossing_instructions(arm_instructions: int, crossings: int) -> float:
    """Instructions per crossing of an arm that makes `crossings` crossings.

    Everything the arm executes is charged to its crossings, so for an arm that
    does other work too this is an upper bound on the crossing alone.
    """
    if arm_instructions <= 0:
        raise ValueError(f"arm_instructions must be positive, got {arm_instructions}")
    if crossings <= 0:
        raise ValueError(f"crossings must be positive, got {crossings}")
    return arm_instructions / crossings


def crossing_share(
    cycle_instructions: float, crossing_instructions: float, crossings_per_cycle: int = 2
) -> float:
    """Expected share of one band cycle's instructions taken by its crossings at W = 1.

    `cycle_instructions` is one band cycle (the band arm over its cycles);
    `crossing_instructions` is one crossing's cost from another arm. A share
    above 1 means the inputs are inconsistent, and is refused.
    """
    if cycle_instructions <= 0 or crossing_instructions <= 0:
        raise ValueError("instruction counts must be positive")
    if crossings_per_cycle <= 0:
        raise ValueError(f"crossings_per_cycle must be positive, got {crossings_per_cycle}")
    share = crossings_per_cycle * crossing_instructions / cycle_instructions
    if share >= 1.0:
        raise ValueError(f"crossings take {share:.3f} of the cycle: the inputs describe different work")
    return share


def projected_ratio(share: float) -> float:
    """`t_band / t_twin` at W = 1 if time is proportional to instructions (HYPOTHESIS).

    The twin cycle is taken to be the band cycle less its crossings, so the
    ratio is `1 / (1 - share)`.
    """
    if not 0.0 <= share < 1.0:
        raise ValueError(f"share must lie in [0, 1), got {share}")
    return 1.0 / (1.0 - share)


def ratio_cv(cv_numerator: float, cv_denominator: float) -> float:
    """First-order CV of a ratio of independent variables (delta method)."""
    if cv_numerator < 0 or cv_denominator < 0:
        raise ValueError("coefficients of variation must be non-negative")
    return math.hypot(cv_numerator, cv_denominator)


def bca_half_width(cv: float, rounds: int, alpha: float = ALPHA) -> float:
    """Relative half-width of a 95% interval of a mean over `rounds` samples.

    The normal-theory interval the BCa interval approaches on a symmetric
    sample (Efron 1987): `z_{1-alpha/2} * cv / sqrt(rounds)`, relative to the
    mean.
    """
    if cv < 0:
        raise ValueError(f"cv must be non-negative, got {cv}")
    if rounds < 3:
        raise ValueError(f"a BCa interval needs at least 3 rounds, got {rounds}")
    return z_quantile(1.0 - alpha / 2.0) * cv / math.sqrt(rounds)


def ratio_mde(cv_per_round: float, rounds: int = ROUNDS, alpha: float = ALPHA,
              power: float = POWER) -> float:
    """Smallest relative departure of the paired ratio from 1 that `rounds` rounds resolve.

    `cv_per_round` is the per-round CV of each cell's time. The per-round ratio's
    CV is taken as `ratio_cv(cv, cv)` (conservative: pairing removes the shared
    part), and the one-sample minimum detectable difference (Cohen 1988) is
    `(z_{1-alpha/2} + z_{power}) * cv_ratio / sqrt(rounds)`.
    """
    if not 0.0 < power < 1.0:
        raise ValueError(f"power must lie in (0, 1), got {power}")
    if rounds < 3:
        raise ValueError(f"a BCa interval needs at least 3 rounds, got {rounds}")
    cv_r = ratio_cv(cv_per_round, cv_per_round)
    return (z_quantile(1.0 - alpha / 2.0) + z_quantile(power)) * cv_r / math.sqrt(rounds)


def detectability(expected_ratio: float, mde: float) -> str:
    """`DETECTABLE` when the expected ratio departs from 1 by at least `mde`."""
    if expected_ratio <= 0:
        raise ValueError(f"expected_ratio must be positive, got {expected_ratio}")
    if mde < 0:
        raise ValueError(f"mde must be non-negative, got {mde}")
    return "DETECTABLE" if abs(expected_ratio - 1.0) >= mde else "UNDETECTABLE"


def observed_writer_cv(paths: tuple[Path, ...] = SPREAD_ARTIFACTS, arm: str = "map") -> float:
    """The largest per-round CV of `writer_mops` over the `arm` cells of `paths`."""
    worst = 0.0
    seen = 0
    for path in paths:
        artifact = json.loads(path.read_text())
        for cell in artifact["throughput"]:
            if cell.get("arm") != arm:
                continue
            xs = [float(r["writer_mops"]) for r in cell["rounds_raw"]]
            if len(xs) < 2:
                raise ValueError(f"{path.name} W={cell['writers']}: {len(xs)} rounds")
            worst = max(worst, statistics.stdev(xs) / statistics.fmean(xs))
            seen += 1
    if seen == 0:
        raise ValueError(f"no {arm} cells in {[p.name for p in paths]}")
    return worst


def render_table() -> str:
    lines = ["| target | per cycle | per crossing (thrash, upper bound) | crossing share | projected t_band / t_twin at W = 1 (hypothesis) |",
             "|---|---:|---:|---:|---:|"]
    for target in BAND_ARM_INSTRUCTIONS:
        cycle = BAND_ARM_INSTRUCTIONS[target] / BAND_CYCLES
        cross = per_crossing_instructions(THRASH_ARM_INSTRUCTIONS[target], THRASH_CROSSINGS)
        share = crossing_share(cycle, cross, BAND_CROSSINGS_PER_CYCLE)
        lines.append(f"| {target} | {cycle:,.0f} | {cross:,.0f} | {share:.4f} | {projected_ratio(share):.4f} |")
    cv = observed_writer_cv()
    mde = ratio_mde(cv)
    lines.append("")
    lines.append(f"Largest per-round writer CV in the 170a4bc3 per-core map cells: {cv:.4f} (borrowed).")
    lines.append(f"Ratio MDE at {ROUNDS} rounds, two-sided 5%, 80% power: {mde:.4f}; "
                 f"95% half-width of the ratio mean: {bca_half_width(ratio_cv(cv, cv), ROUNDS):.4f}.")
    for target in BAND_ARM_INSTRUCTIONS:
        cycle = BAND_ARM_INSTRUCTIONS[target] / BAND_CYCLES
        cross = per_crossing_instructions(THRASH_ARM_INSTRUCTIONS[target], THRASH_CROSSINGS)
        ratio = projected_ratio(crossing_share(cycle, cross))
        lines.append(f"W = 1, {target}: {detectability(ratio, mde)}")
    return "\n".join(lines)


class QuantileTests(unittest.TestCase):
    def test_normal_quantiles_match_the_table(self):
        # Standard normal table values.
        self.assertAlmostEqual(z_quantile(0.975), 1.959964, places=6)
        self.assertAlmostEqual(z_quantile(0.80), 0.841621, places=6)


class CallgrindShareTests(unittest.TestCase):
    def test_per_crossing_figures(self):
        self.assertAlmostEqual(per_crossing_instructions(3_278_033, 200), 16_390.165)
        self.assertAlmostEqual(per_crossing_instructions(2_747_239, 200), 13_736.195)

    def test_crossing_share_pinned(self):
        # 2 * 16,390.165 / 101,161.94 and 2 * 13,736.195 / 89,775.50.
        self.assertAlmostEqual(crossing_share(101_161.94, 16_390.165), 0.324038, places=6)
        self.assertAlmostEqual(crossing_share(89_775.50, 13_736.195), 0.306012, places=6)

    def test_projected_ratio_pinned(self):
        self.assertAlmostEqual(projected_ratio(0.324038), 1.479373, places=5)
        self.assertAlmostEqual(projected_ratio(0.0), 1.0)

    def test_inconsistent_inputs_are_refused(self):
        with self.assertRaises(ValueError):
            crossing_share(10_000.0, 6_000.0)


class DetectabilityTests(unittest.TestCase):
    def test_ratio_cv_is_first_order(self):
        self.assertAlmostEqual(ratio_cv(0.03, 0.04), 0.05)
        self.assertAlmostEqual(ratio_cv(0.02, 0.02), 0.02 * math.sqrt(2.0))

    def test_half_width_pinned(self):
        # 1.959964 * 0.02 / sqrt(8)
        self.assertAlmostEqual(bca_half_width(0.02, 8), 0.013859, places=6)

    def test_mde_pinned(self):
        # (1.959964 + 0.841621) * 0.02 * sqrt(2) / sqrt(8) = 2.801585 * 0.01
        self.assertAlmostEqual(ratio_mde(0.02, 8), 0.028016, places=6)
        self.assertAlmostEqual(ratio_mde(0.05, 8), 0.070040, places=6)

    def test_mde_falls_with_rounds(self):
        self.assertLess(ratio_mde(0.02, 16), ratio_mde(0.02, 8))

    def test_verdicts(self):
        self.assertEqual(detectability(1.48, 0.03), "DETECTABLE")
        self.assertEqual(detectability(1.01, 0.03), "UNDETECTABLE")
        self.assertEqual(detectability(0.95, 0.03), "DETECTABLE")


class ArtifactTests(unittest.TestCase):
    def test_borrowed_spread_is_read_from_the_committed_cells(self):
        cv = observed_writer_cv()
        self.assertGreater(cv, 0.0)
        self.assertLess(cv, 0.10)

    def test_w1_projection_is_detectable_at_the_borrowed_spread(self):
        mde = ratio_mde(observed_writer_cv())
        for target in BAND_ARM_INSTRUCTIONS:
            cycle = BAND_ARM_INSTRUCTIONS[target] / BAND_CYCLES
            cross = per_crossing_instructions(THRASH_ARM_INSTRUCTIONS[target], THRASH_CROSSINGS)
            self.assertEqual(detectability(projected_ratio(crossing_share(cycle, cross)), mde),
                             "DETECTABLE", target)


class ArgumentTests(unittest.TestCase):
    def test_invalid_arguments_raise(self):
        for call in (
            lambda: z_quantile(0.0),
            lambda: per_crossing_instructions(0, 2),
            lambda: per_crossing_instructions(10, 0),
            lambda: crossing_share(0.0, 1.0),
            lambda: crossing_share(10.0, 1.0, 0),
            lambda: projected_ratio(1.0),
            lambda: ratio_cv(-0.1, 0.1),
            lambda: bca_half_width(0.02, 2),
            lambda: ratio_mde(0.02, 2),
            lambda: ratio_mde(0.02, 8, power=1.0),
            lambda: detectability(0.0, 0.1),
            lambda: detectability(1.0, -0.1),
            lambda: observed_writer_cv(paths=()),
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
