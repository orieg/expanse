#!/usr/bin/env python3
"""Bounds for the host guard's on-pin void boundary (#1270, METHODOLOGY.md §25).

    python3 scripts/host_guard_bounds.py            # the table §25.5 quotes
    python3 scripts/host_guard_bounds.py --self-test

`docs/benchmarks/concurrency/METHODOLOGY.md` §25 pre-registers a controlled
experiment that injects known foreign load on the pinned CPUs and measures
what it does to the `concurrency` suite's cells. These are the quantities it
is sized with, computed rather than narrated (AGENTS.md §8.8, commit 1):

- `fair_share_loss`: the throughput a cell loses to a continuous foreign load
  if throughput is proportional to the CPU time its threads get and the
  scheduler puts the load on an idle pinned CPU whenever one exists. A
  hypothesis with two stated assumptions, not a prediction of magnitude:
  lock-holder preemption can make the loss larger, SMT sharing can make it
  appear at W below the CPU count.
- `skew_residual`: how far the guard's on-pin reading moves when the own tree
  is read `lag` seconds later at one end of a window than at the other
  (`bench_host_guard.sample`).
- `quantization_sd` / `quantization_max`: the reading's resolution from
  `/proc/stat`'s tick counters alone.
- `ratio_halfwidth`, `bonferroni_confidence`, `min_detectable_ratio`: the
  resolution of a two-sample ratio of means, the interval the experiment's
  decision rule uses, and what it can detect at 80% power.

Sources: the normal-approximation interval and power formula for a
difference of two means (e.g. Wasserman, *All of Statistics*, Springer 2004,
§10.1 and §10.2), applied to a ratio by the delta method with equal means;
Bonferroni's inequality for the family-wise level; `proc(5)` for the
`/proc/stat` tick counters, which are printed in USER_HZ units.
"""

from __future__ import annotations

import argparse
import math
import sys
from statistics import NormalDist

USER_HZ = 100


def _positive(name: str, value: float) -> None:
    if not (isinstance(value, (int, float)) and math.isfinite(value) and value > 0):
        raise ValueError(f"{name} must be a positive finite number, got {value!r}")


def fair_share_loss(level: float, threads: int, cpus: int) -> float:
    """Fractional throughput loss from `level` CPUs of continuous foreign load.

    The cell's threads get `min(threads, cpus)` CPUs uninjected and
    `min(threads, cpus - level)` injected, if the load goes to an idle CPU
    when there is one and otherwise takes its time from the threads.
    """
    if level < 0 or not math.isfinite(level):
        raise ValueError(f"level must be a finite non-negative number, got {level!r}")
    if threads < 1 or cpus < 1 or level > cpus:
        raise ValueError(f"need threads >= 1, cpus >= 1 and level <= cpus, got {threads}, {cpus}, {level}")
    base = min(threads, cpus)
    return (base - min(threads, cpus - level)) / base


def skew_residual(own_cpus: float, lag_delta_s: float, window_s: float) -> float:
    """On-pin reading error when the own-tree read lags by `lag_delta_s` more at one end."""
    _positive("window_s", window_s)
    if own_cpus < 0 or lag_delta_s < 0:
        raise ValueError("own_cpus and lag_delta_s must be non-negative")
    return own_cpus * lag_delta_s / window_s


def quantization_max(cpus: int, window_s: float, hz: int = USER_HZ) -> float:
    """Largest error from truncating each CPU's counter to whole ticks at both ends."""
    _positive("window_s", window_s)
    if cpus < 1:
        raise ValueError("cpus must be >= 1")
    return cpus / (hz * window_s)


def quantization_sd(cpus: int, window_s: float, hz: int = USER_HZ) -> float:
    """Standard deviation of that error: each CPU's is the difference of two
    independent uniform [0, 1) truncations, variance 1/6 tick^2."""
    _positive("window_s", window_s)
    if cpus < 1:
        raise ValueError("cpus must be >= 1")
    return math.sqrt(cpus / 6.0) / (hz * window_s)


def ratio_halfwidth(cv: float, n1: int, n2: int, confidence: float = 0.95) -> float:
    """Half-width of the normal-approximation interval of mean(a) / mean(b)."""
    _positive("cv", cv)
    if n1 < 2 or n2 < 2 or not 0 < confidence < 1:
        raise ValueError("need n1, n2 >= 2 and 0 < confidence < 1")
    z = NormalDist().inv_cdf(0.5 + confidence / 2)
    return z * cv * math.sqrt(1.0 / n1 + 1.0 / n2)


def bonferroni_confidence(k: int, family: float = 0.95) -> float:
    """Per-comparison confidence that holds the family-wise level over k tests."""
    if k < 1 or not 0 < family < 1:
        raise ValueError("need k >= 1 and 0 < family < 1")
    return 1.0 - (1.0 - family) / k


def min_detectable_ratio(cv: float, n1: int, n2: int, confidence: float, power: float = 0.8) -> float:
    """Smallest true |ratio - 1| the interval excludes 1 for with probability `power`."""
    _positive("cv", cv)
    if not 0 < power < 1:
        raise ValueError("0 < power < 1")
    z_power = NormalDist().inv_cdf(power)
    z = NormalDist().inv_cdf(0.5 + confidence / 2)
    return (z + z_power) * cv * math.sqrt(1.0 / n1 + 1.0 / n2)


# The experiment's shape (METHODOLOGY.md §25.3): 16 pinned CPUs, W in {1, 4, 16},
# four blocks of 18 rounds per arm, the three injected levels, 11 gated cells.
PIN_CPUS = 16
THREADS = (1, 4, 16)
LEVELS = (0.10, 0.25, 0.50)
BLOCKS = 4
ROUNDS = 18
GATED_CELLS = 11
WATCH_WINDOW_S = 2.0


def table() -> list[str]:
    n = BLOCKS * ROUNDS
    conf = bonferroni_confidence(GATED_CELLS)
    out = [f"windows per arm per cell n = {n}; per-cell confidence {conf:.5f} (Bonferroni over {GATED_CELLS})"]
    for level in LEVELS:
        losses = ", ".join(f"W={w}: {fair_share_loss(level, w, PIN_CPUS):.5f}" for w in THREADS)
        out.append(f"fair-share loss at {level:.2f} CPUs: {losses}")
    for cv in (0.005, 0.01, 0.02, 0.03, 0.04):
        out.append(f"cv {cv:.3f}: half-width {ratio_halfwidth(cv, n, n, conf):.5f}, "
                   f"min detectable |ratio - 1| {min_detectable_ratio(cv, n, n, conf):.5f}")
    out.append(f"skew residual, 8 busy CPUs, 40 ms more lag at one end, 2 s window: "
               f"{skew_residual(8, 0.040, WATCH_WINDOW_S):.3f}")
    for w in (1.0, WATCH_WINDOW_S):
        out.append(f"quantization over {PIN_CPUS} CPUs, {w:g} s window: sd {quantization_sd(PIN_CPUS, w):.4f}, "
                   f"max {quantization_max(PIN_CPUS, w):.3f}")
    return out


def self_test() -> int:
    def close(a, b, tol=1e-9):
        return abs(a - b) <= tol
    # Fair share: a load that fits in idle CPUs costs nothing; on a full pin it
    # takes level / cpus.
    assert close(fair_share_loss(0.25, 16, 16), 0.015625)
    assert close(fair_share_loss(0.10, 16, 16), 0.00625)
    assert close(fair_share_loss(0.50, 16, 16), 0.03125)
    assert fair_share_loss(0.5, 4, 16) == 0.0 and fair_share_loss(0.5, 1, 16) == 0.0
    assert close(fair_share_loss(1.0, 16, 16), 0.0625)
    assert close(fair_share_loss(2.0, 15, 16), 1 / 15)  # one idle CPU absorbs half of it
    # Skew: the example in bench_host_guard.sample's docstring.
    assert close(skew_residual(8, 0.040, 2.0), 0.16)
    # Quantization: 16 CPUs, 1 s at USER_HZ 100 -> max 0.16, sd sqrt(16/6)/100.
    assert close(quantization_max(16, 1.0), 0.16) and close(quantization_max(16, 2.0), 0.08)
    assert close(quantization_sd(16, 1.0), math.sqrt(16 / 6) / 100)
    assert close(quantization_sd(16, 2.0), 0.0081649658, 1e-9)
    # Two-sample half-width: 1.959964 * 0.01 * sqrt(2/72) = 0.0032666.
    assert close(ratio_halfwidth(0.01, 72, 72), 0.0032666, 1e-6)
    assert close(bonferroni_confidence(11), 1 - 0.05 / 11)
    # z(1 - 0.05/22) = 2.8376 (scipy.stats.norm.ppf(1 - 0.05/22)); + z(0.8) = 0.8416.
    z = NormalDist().inv_cdf(1 - 0.05 / 22)
    assert close(z, 2.8376, 1e-4), z
    assert close(min_detectable_ratio(0.02, 72, 72, bonferroni_confidence(11)), (z + 0.841621) * 0.02 * math.sqrt(2 / 72), 1e-6)
    for bad in (lambda: fair_share_loss(-0.1, 1, 16), lambda: fair_share_loss(17, 16, 16),
                lambda: skew_residual(1, 0.1, 0), lambda: ratio_halfwidth(0, 10, 10),
                lambda: bonferroni_confidence(0), lambda: quantization_sd(0, 1.0)):
        try:
            bad()
        except ValueError:
            continue
        raise AssertionError("invalid input accepted")
    assert len(table()) == 1 + len(LEVELS) + 5 + 1 + 2
    print("host_guard_bounds.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    print("\n".join(table()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
