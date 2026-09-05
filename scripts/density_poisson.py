#!/usr/bin/env python3
"""
scripts/density_poisson.py — the Poisson occupancy model behind the density
sawtooth (docs/ARCHITECTURE.md §3.5), as committed, unit-tested code.

For uniform random keys the top two key bytes saturate, and the population of a
2-byte-prefix expanse is Poisson-distributed with mean λ = N / 2^(w−48) for a
w-bit keyspace. A linear leaf holds at most `LEAF_CAP` keys; the 33rd cascades
the expanse into a branch of single-key immediates. Two quantities of that
model decide what the memory curve should look like at a given λ:

  * `cascade_share(lam, cap)`     — P(X > cap): the fraction of expanses that
                                    have cascaded.
  * `cascade_key_share(lam, cap)` — Σ_{k>cap} k·P(X = k) / λ: the fraction of
                                    *keys* that sit in cascaded expanses, which
                                    is what bytes/key is weighted by.

`predicted_cascaded(lam, expanses, cap)` turns the first into an expected
count, which `crates/expanse/examples/keyspace_density.rs --json` lets the
engine's own node census contradict or confirm (`branch_depth_histogram[6]`).

The Poisson law is the standard occupancy result for uniform hashing into a
fixed number of bins when the bin count is large and the per-bin probability
small (Feller, *An Introduction to Probability Theory and Its Applications*
vol. 1, ch. VI §5, binomial → Poisson); no other assumption enters. The
reference values pinned in `test_pins` are those the density sections of
`docs/benchmarks/hot_comparison/METHODOLOGY.md` quote, so a change to either
side fails here.

Usage:
  python3 scripts/density_poisson.py            # run the pinned tests, then print the table
  python3 scripts/density_poisson.py --self-test
"""

from __future__ import annotations

import math
import sys

LEAF_CAP = 32  # crates/expanse/src/types.rs — the tooth of the sawtooth
EXPANSES_64 = 1 << 16  # populated 2-byte-prefix expanses at 64 bits


def poisson_pmf(k: int, lam: float) -> float:
    """P(X = k) for X ~ Poisson(lam), computed in log space so large λ is exact
    to double precision rather than overflowing the factorial."""
    if k < 0:
        raise ValueError("k must be non-negative")
    if lam <= 0.0:
        raise ValueError("lam must be positive")
    return math.exp(-lam + k * math.log(lam) - math.lgamma(k + 1))


def poisson_sf(cap: int, lam: float) -> float:
    """P(X > cap) for X ~ Poisson(lam)."""
    if cap < 0:
        raise ValueError("cap must be non-negative")
    return max(0.0, 1.0 - sum(poisson_pmf(k, lam) for k in range(cap + 1)))


def cascade_share(lam: float, cap: int = LEAF_CAP) -> float:
    """Fraction of expanses whose population exceeds `cap` and so have
    cascaded out of a linear leaf: P(X > cap | λ)."""
    return poisson_sf(cap, lam)


def cascade_key_share(lam: float, cap: int = LEAF_CAP) -> float:
    """Fraction of keys that live in cascaded expanses: Σ_{k>cap} k·P(k) / λ.

    Uses the identity k·P(X = k) = λ·P(X = k−1), so the key-weighted tail is
    exactly P(X > cap − 1) — one call, no truncated sum."""
    if cap < 1:
        raise ValueError("cap must be at least 1")
    return poisson_sf(cap - 1, lam)


def predicted_cascaded(lam: float, expanses: int = EXPANSES_64, cap: int = LEAF_CAP) -> float:
    """Expected number of cascaded expanses out of `expanses`."""
    if expanses <= 0:
        raise ValueError("expanses must be positive")
    return cascade_share(lam, cap) * expanses


def lambda_at_share(share: float, cap: int = LEAF_CAP) -> float:
    """The λ at which P(X > cap) equals `share`, by bisection: P(X > cap) is
    monotone increasing in λ, so the root is unique."""
    if not 0.0 < share < 1.0:
        raise ValueError("share must lie strictly between 0 and 1")
    lo, hi = 1e-9, 4.0 * (cap + 1) + 64.0
    for _ in range(200):
        mid = 0.5 * (lo + hi)
        if cascade_share(mid, cap) < share:
            lo = mid
        else:
            hi = mid
    return 0.5 * (lo + hi)


def lam(n: int, bits: int) -> float:
    """λ = N / 2^(bits − 48): mean population of a 2-byte-prefix expanse."""
    if bits < 49 or bits > 64:
        raise ValueError("bits must lie in 49..=64")
    return n / float(1 << (bits - 48))


def test_pins() -> None:
    """Reference values quoted in the density sections; a drift on either side
    fails here rather than in a reader's arithmetic."""
    # A pmf that must sum to one, and the standard Poisson mean.
    assert abs(sum(poisson_pmf(k, 7.5) for k in range(200)) - 1.0) < 1e-12
    assert abs(sum(k * poisson_pmf(k, 7.5) for k in range(200)) - 7.5) < 1e-9
    # The λ of the three census cells.
    assert abs(lam(1_000_000, 64) - 15.2588) < 5e-5
    assert abs(lam(2_000_000, 64) - 30.5176) < 5e-5
    assert abs(lam(800_000, 62) - 48.8281) < 5e-5
    # The pinned shares at the two first-cliff cells (4 d.p.).
    assert round(cascade_share(30.52), 4) == 0.3503, cascade_share(30.52)
    assert round(cascade_key_share(30.52), 4) == 0.4182, cascade_key_share(30.52)
    assert round(cascade_share(15.26), 4) == 0.0001, cascade_share(15.26)
    # The key-weighted identity against the explicit truncated sum.
    explicit = sum(k * poisson_pmf(k, 30.52) for k in range(33, 400)) / 30.52
    assert abs(cascade_key_share(30.52) - explicit) < 1e-12
    # A cap-48 control: at λ = 30.52 the 49th key is essentially never reached.
    assert round(cascade_share(30.52, 48), 5) == 0.00125, cascade_share(30.52, 48)
    # Where the cascade is 10% and 90% on: the width of the tooth's rise.
    assert round(lambda_at_share(0.10), 1) == 25.9, lambda_at_share(0.10)
    assert round(lambda_at_share(0.90), 1) == 40.5, lambda_at_share(0.90)
    # The cap-48 control's ramp, and the second tooth's ramp one byte level
    # down (sub-expanse occupancy λ / 256 crossing the cap).
    assert round(lambda_at_share(0.10, 48), 1) == 40.3, lambda_at_share(0.10, 48)
    assert round(lambda_at_share(0.90, 48), 1) == 58.2, lambda_at_share(0.90, 48)
    assert round(256 * lambda_at_share(0.10)) == 6627, 256 * lambda_at_share(0.10)
    assert round(256 * lambda_at_share(0.90)) == 10379, 256 * lambda_at_share(0.90)
    # Expected cascaded count at the 2M @64 census cell.
    assert round(predicted_cascaded(30.52)) == 22955, predicted_cascaded(30.52)
    # Input validation is a ValueError, never a silent number.
    for bad in (lambda: poisson_pmf(-1, 1.0), lambda: poisson_pmf(1, 0.0),
                lambda: lambda_at_share(1.0), lambda: lam(10, 40),
                lambda: predicted_cascaded(1.0, 0), lambda: cascade_key_share(1.0, 0)):
        try:
            bad()
        except ValueError:
            pass
        else:
            raise AssertionError("invalid input must raise")


def main(argv: list[str]) -> int:
    test_pins()
    if "--self-test" in argv:
        print("density_poisson: self-test OK")
        return 0
    print(f"Poisson occupancy model, LEAF_CAP = {LEAF_CAP}")
    print(f"{'λ':>8} {'P(X>cap)':>10} {'key share':>10} {'cascaded/65536':>15}")
    for lam_ in (6.10, 9.16, 12.21, 15.26, 18.31, 24.41, 25.9, 30.52, 36.62, 40.5, 48.83, 61.04):
        print(
            f"{lam_:>8.2f} {cascade_share(lam_):>10.4f} {cascade_key_share(lam_):>10.4f} "
            f"{predicted_cascaded(lam_):>15.0f}"
        )
    print(f"P(X > 32) = 0.10 at λ = {lambda_at_share(0.10):.1f}; = 0.90 at λ = {lambda_at_share(0.90):.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
