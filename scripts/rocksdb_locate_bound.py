#!/usr/bin/env python3
"""What a single mutex over the locate phase permits, before the host is spent (#802).

`FindLeafBlockForSeek` (`integrations/rocksdb/src/expanse_memtable.cc:136`) opens
with `lock_guard(mutex_)`, and it is the locate path for `Contains`, `Get` and
`IteratorImpl::Seek`. That is the same mutex `Insert` holds for its entire body.
So read throughput under a concurrent writer is bounded by arithmetic, not by
how many reader threads are started -- and the bound is derivable from numbers
already committed, which is why it is written down before the concurrent arm is
built rather than after it returns a surprise (AGENTS.md 8.8 commit 1).

## The model

One mutex serialises two kinds of work:

  * every read's locate phase, `locked_fraction` of a read's `read_ns`;
  * every insert's whole body, `insert_ns`.

Let `d = writer_ops_per_s * insert_ns * 1e-9` be the writer's lock duty cycle
-- the share of wall time the writer holds the lock, with `insert_ns` converted
to seconds. The readers get what is left, so
the aggregate read ceiling is

    R_max = (1 - d) / (locked_fraction * read_ns)      ops/s

with no `readers` term in it: that absence is the whole claim. Writing
`K = (1 - d) / locked_fraction` for that ceiling expressed in units of one
reader's throughput, aggregate read throughput and its scaling ratio are

    T(W) = min(W, K) / read_ns
    S(W) = T(W) / T(1) = clamp(K, 1, W)

`S` saturates at `K`. A shared lock on the locate phase -- readers concurrent
with each other, still excluded by the writer -- removes the reader-vs-reader
term but not the reader-vs-writer one. Each reader is then capped individually
rather than collectively, and the caps add:

    T_shared(W) = W * min(1, K) / read_ns,   S_shared(W) = W

so the two designs are separated by the *shape* of the curve, not by a
constant, and one sweep distinguishes them.

Note what that does and does not promise. `S_shared = W` always, so the shape
claim holds at every `locked_fraction`; the *size* of the win does not. The
throughput gain is `W * min(1, K)` over `min(W, K)`, which is `W` below `K = 1`,
falls as `W / K` through `1 <= K <= W`, and is exactly 1 once `K >= W`. A shared
lock is worth most where the lock covers most of a read and worth nothing where
it covers little -- so the same `implied_locked_fraction` that answers "is the
mutex binding?" also answers "would splitting it help?", and the arm does not
need a second design built before it can say.

## Relation to `scripts/fit_usl.py`

The repo already fits Gunther's Universal Scalability Law to measured scaling
curves, and its `alpha` is the Amdahl serialisation fraction. That instrument
needs data; this one exists because the pre-registration comes before the data
(AGENTS.md 8.8). They meet at the asymptote: USL's ratio ceiling with `beta = 0`
is `1 / alpha`, and this model's is `max(K, 1)`, so

    alpha = 1 / max(K, 1) = min(1, locked_fraction / (1 - d))

which `predicted_usl_alpha` computes. That is what makes the two checkable
against each other rather than merely adjacent: Unit C fits USL to the measured
curve, and the fitted `alpha` either lands near the `alpha` the locked fraction
predicts or it does not.

The models are not the same shape. This one is a hard work-conservation ceiling
-- a knee -- while USL's is a smooth saturation, so the fitted curve should bend
earlier than the knee rather than tracking it. And USL's `beta`, the coherency
term, has no counterpart here: this bound has no crosstalk in it at all, so a
fitted `beta > 0` is cost this derivation does not account for, and belongs on
the unexplained line rather than being folded into `alpha` after the fact
(AGENTS.md 8.20.4).

## What this is and is not

These are ceilings on an idealised handoff: zero lock transfer cost, no
convoying, no cache-line traffic on the mutex word, and a reader's unlocked
remainder perfectly overlapped. Every one of those is optimistic, so a measured
curve should sit at or below what this predicts. A measured curve *above* it
means an input is wrong -- most likely `locked_fraction`, which is the one
quantity here that no committed artifact yet pins.

That is also what makes the relation useful in reverse: `implied_locked_fraction`
turns a measured `S(W)` back into the fraction of a read that the lock covers,
which is the quantity the design decision in #802 actually turns on.

Inputs come from the committed suite artifact
(`docs/benchmarks/rocksdb_memtable/results/baseline_rocksdb.json`) rather than
being copied into this file, so a re-measurement re-derives the prediction
instead of silently leaving it stale (AGENTS.md 8.7).

## The idle curve's shape

The single-mutex ceiling above says nothing about how a curve bends below it,
and USL with `alpha` fixed at the profiled locked fraction does not describe
the idle curve: the `beta` each reader count needs is not one number
(`unexplained_term`, rendered below). `MODEL_CANDIDATES` fixes six shapes
before any held-out cell exists. `check_candidate` fits one on `S(2)`, `S(4)`
and `S(7)` and checks it at `S(3)`, `S(5)` and `S(6)` too, and `model_verdicts`
applies METHODOLOGY section 5.12's acceptance rule over two runs. Nothing here
chooses a shape after seeing the held-out cells; a shape that fails is
reported, and the section says what follows.

## Sizing the optimistic-seek arm

METHODOLOGY section 5.16 gates `S_opt(7) / S_full(7)` on a direction, like
section 5.14, and reports `S_opt(7) / S_trie(7)` without gating it.
`optimistic_gate_sizing` fixes the rounds before any optimistic cell exists: the
per-round spread of every round of the four section 5.15 runs
(`optimistic_sizing_inputs`), a Student-t planning half-width
(`sizing_relative_halfwidth`), and the fewest rounds at which a true gated
ratio of `OPTIMISTIC_MDR_TARGET` would clear 1 (`rounds_for_detectable_ratio`).

Usage:
    python3 scripts/rocksdb_locate_bound.py
    python3 scripts/rocksdb_locate_bound.py --writer-ops 1e6
    python3 scripts/rocksdb_locate_bound.py --self-test
"""

from __future__ import annotations

import argparse
import itertools
import json
import math
import statistics
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
ARTIFACT = REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable" / "results" / "baseline_rocksdb.json"

#: The arms this derivation reads, by their exact `id` in the artifact. Exact
#: ids, never a substring or a prefix scan: `readrandom/ExpanseMemTable` and
#: `prefixscan/ExpanseMemTable (Batch)` both start with the benchmark name, and
#: a loose match would silently pick up whichever the harvester emitted first.
INSERT_ARM = "fillrandom/ExpanseMemTable"
READ_ARM = "readrandom/ExpanseMemTable"


# ---------------------------------------------------------------------------
# Unit conversion
# ---------------------------------------------------------------------------

def ns_per_op(mops_per_s: float) -> float:
    """Nanoseconds per operation from a throughput in Mops/s."""
    if not (mops_per_s > 0.0) or math.isinf(mops_per_s):
        raise ValueError(f"mops_per_s must be finite and positive, got {mops_per_s!r}")
    return 1000.0 / mops_per_s


# ---------------------------------------------------------------------------
# The bound
# ---------------------------------------------------------------------------

def writer_duty_cycle(writer_ops_per_s: float, insert_ns: float) -> float:
    """Share of wall time the writer holds `mutex_`.

    `Insert` takes the lock for its whole body, so a writer offering
    `writer_ops_per_s` inserts per second holds it for that fraction of every
    second. A duty at or above 1.0 is a writer that saturates the lock on its
    own, leaving readers nothing; that is refused rather than returned as a
    negative ceiling (AGENTS.md 8.1), because it is a benchmark design error --
    the arm must not pre-register a writer rate at or above `1e9 / insert_ns`.
    """
    if writer_ops_per_s < 0.0 or math.isinf(writer_ops_per_s):
        raise ValueError(f"writer_ops_per_s must be finite and >= 0, got {writer_ops_per_s!r}")
    if not (insert_ns > 0.0) or math.isinf(insert_ns):
        raise ValueError(f"insert_ns must be finite and positive, got {insert_ns!r}")
    duty = writer_ops_per_s * insert_ns * 1e-9
    if duty >= 1.0:
        raise ValueError(
            f"writer at {writer_ops_per_s:.3g} ops/s x {insert_ns:.3f} ns saturates the lock "
            f"(duty {duty:.3f} >= 1.0); readers get no lock time at all. "
            f"Keep the offered write rate below {1e9 / insert_ns:.3g} ops/s."
        )
    return duty


def _validate(locked_fraction: float, readers: int) -> None:
    if not (0.0 < locked_fraction <= 1.0):
        raise ValueError(f"locked_fraction must be in (0, 1], got {locked_fraction!r}")
    if not isinstance(readers, int) or isinstance(readers, bool) or readers < 1:
        raise ValueError(f"readers must be an int >= 1, got {readers!r}")


def knee(locked_fraction: float, duty: float) -> float:
    """`K` -- the aggregate read ceiling in units of one reader's throughput.

    Also the reader count past which adding readers adds nothing: `S(W)`
    saturates here.
    """
    _validate(locked_fraction, 1)
    if not (0.0 <= duty < 1.0):
        raise ValueError(f"duty must be in [0, 1), got {duty!r}")
    return (1.0 - duty) / locked_fraction


def exclusive_read_ceiling_ops(read_ns: float, locked_fraction: float, duty: float) -> float:
    """Aggregate read ops/s the mutex permits, at any reader count."""
    if not (read_ns > 0.0) or math.isinf(read_ns):
        raise ValueError(f"read_ns must be finite and positive, got {read_ns!r}")
    return knee(locked_fraction, duty) / (read_ns * 1e-9)


def exclusive_read_throughput_ops(readers: int, read_ns: float,
                                  locked_fraction: float, duty: float) -> float:
    """Aggregate read ops/s at `readers`, under the mutex as it stands."""
    _validate(locked_fraction, readers)
    return min(float(readers), knee(locked_fraction, duty)) / (read_ns * 1e-9)


def exclusive_scaling(readers: int, locked_fraction: float, duty: float) -> float:
    """`S(W) = T(W)/T(1)` under the mutex: `clamp(K, 1, W)`.

    `read_ns` divides out of the ratio, which is why the prediction survives a
    re-measurement of the single-threaded cells unchanged.
    """
    _validate(locked_fraction, readers)
    return min(max(knee(locked_fraction, duty), 1.0), float(readers))


def shared_read_throughput_ops(readers: int, read_ns: float,
                               locked_fraction: float, duty: float) -> float:
    """Aggregate read ops/s with a *shared* lock over the locate phase.

    Readers no longer exclude each other, so the ceiling applies to each reader
    separately rather than to all of them together: `min(1, K)` per reader
    instead of `min(W, K)` across them. It takes `locked_fraction` for the same
    reason the exclusive twin does -- a reader whose locked share is already
    below what the writer leaves free (`locked_fraction <= 1 - duty`) is not
    capped at all, and scaling the whole curve by `(1 - duty)` regardless would
    understate the design it is meant to represent.
    """
    _validate(locked_fraction, readers)
    if not (read_ns > 0.0) or math.isinf(read_ns):
        raise ValueError(f"read_ns must be finite and positive, got {read_ns!r}")
    return readers * min(1.0, knee(locked_fraction, duty)) / (read_ns * 1e-9)


def shared_scaling(readers: int, locked_fraction: float, duty: float) -> float:
    """`S_shared(W) = W`, at every `locked_fraction`.

    The per-reader cap divides out of the ratio, which is the shape claim: a
    shared lock is linear where the exclusive one saturates. Stated as a
    function rather than as the literal `W` so both designs are compared
    through the same call shape and validate the same arguments.
    """
    _validate(locked_fraction, readers)
    if not (0.0 <= duty < 1.0):
        raise ValueError(f"duty must be in [0, 1), got {duty!r}")
    return float(readers)


def implied_locked_fraction(observed_scaling: float, readers: int,
                            duty: float) -> tuple[float, float]:
    """Invert `S(W)` to bounds on the locked fraction of a read.

    Returns `(lo, hi)`. The interval is a point only where the measurement is
    informative: `S = clamp(K, 1, W)` loses information at both ends, so a
    saturated curve bounds the fraction from below and a linear one from above,
    and reporting either as a point estimate would be inventing precision the
    measurement does not carry.
    """
    # `_validate` also checks a locked fraction; this function computes one
    # rather than taking it, so pass the identity value and check `readers`.
    _validate(1.0, readers)
    if not (0.0 <= duty < 1.0):
        raise ValueError(f"duty must be in [0, 1), got {duty!r}")
    if not (observed_scaling > 0.0) or math.isinf(observed_scaling):
        raise ValueError(f"observed_scaling must be finite and positive, got {observed_scaling!r}")
    if observed_scaling > readers * (1.0 + 1e-9):
        raise ValueError(
            f"observed scaling {observed_scaling} exceeds the reader count {readers}; "
            "no lock-serialisation model produces superlinear scaling, so an input is wrong."
        )
    residual = 1.0 - duty
    if observed_scaling <= 1.0:
        # Saturated: K <= 1. The lock covers at least `residual` of a read.
        return (min(residual, 1.0), 1.0)
    if observed_scaling >= readers * (1.0 - 1e-9):
        # Linear to the edge of the sweep: K >= W. Only an upper bound.
        return (0.0, residual / readers)
    return (residual / observed_scaling, residual / observed_scaling)


def predicted_usl_alpha(locked_fraction: float, duty: float) -> float:
    """The USL contention parameter this bound predicts (`scripts/fit_usl.py`).

    Matched at the asymptote: USL's scaling ratio with `beta = 0` tends to
    `1 / alpha`, and this model's tends to `max(K, 1)`. Clamped into `(0, 1]`
    by construction, so it is always an admissible fit target -- `fit_usl.py`
    treats `alpha > 1` as REFUTED, and an inadmissible *prediction* would be a
    defect in this derivation rather than a finding about the code.

    `beta` is deliberately absent: this bound models work conservation on one
    lock and contains no coherency term, so it predicts nothing about crosstalk.
    """
    _validate(locked_fraction, 1)
    if not (0.0 <= duty < 1.0):
        raise ValueError(f"duty must be in [0, 1), got {duty!r}")
    return 1.0 / max(knee(locked_fraction, duty), 1.0)


def gate_boundary_locked_fraction(threshold: float, duty: float) -> float:
    """The locked fraction a scaling threshold decides at, at writer duty `duty`.

    `S(W) = clamp(K, 1, W)` with `K = (1 - duty) / locked_fraction`, so on the
    unclamped stretch a gate of the form "`S` below `threshold`" is the same
    decision as "`locked_fraction` above `(1 - duty) / threshold`". METHODOLOGY
    section 5.3 states H1's threshold of 2.0 in `S`; this is what it means in
    the quantity the design decision turns on.
    """
    if not (threshold > 1.0) or math.isinf(threshold):
        raise ValueError(f"threshold must be finite and > 1, got {threshold!r}")
    if not (0.0 <= duty < 1.0):
        raise ValueError(f"duty must be in [0, 1), got {duty!r}")
    return (1.0 - duty) / threshold


def max_gate_boundary_shift(threshold: float, offered_duty: float) -> float:
    """The most a writer that runs short can move a gate's locked-fraction boundary.

    A paced writer sleeps to an absolute schedule, so its achieved duty lies in
    `[0, offered_duty]`. The boundary `(1 - d) / threshold` is linear and
    decreasing in `d`, so over that interval it moves by at most
    `offered_duty / threshold`, reached only by a writer that stalls outright.
    A shortfall of any size therefore moves the decision by no more than this.
    """
    return (gate_boundary_locked_fraction(threshold, 0.0)
            - gate_boundary_locked_fraction(threshold, offered_duty))


def predicted_alpha_from_scaling(observed_scaling: float, readers: int) -> float:
    """The USL `alpha` this bound predicts from a measured `S(W)` -- with no duty in it.

    Composing `implied_locked_fraction` with `predicted_usl_alpha` gives
    `1 / clamp(S, 1, W)`: the duty enters the implied locked fraction as
    `(1 - d)` and leaves the knee as `(1 - d)` again, so it cancels. A writer
    that misses its offered rate changes the locked fraction inferred from a
    cell, but not the `alpha` that fraction predicts. The self-test pins the
    cancellation by composing the two existing functions across a grid of
    duties, not by restating this closed form.
    """
    _validate(1.0, readers)
    if not (observed_scaling > 0.0) or math.isinf(observed_scaling):
        raise ValueError(f"observed_scaling must be finite and positive, got {observed_scaling!r}")
    if observed_scaling > readers * (1.0 + 1e-9):
        raise ValueError(f"observed scaling {observed_scaling} exceeds the reader count {readers}")
    return 1.0 / min(max(observed_scaling, 1.0), float(readers))


def usl_scaling(readers: int, alpha: float, beta: float) -> float:
    """`S(W) = W / (1 + alpha (W - 1) + beta W (W - 1))`: Gunther's Universal
    Scalability Law as a speedup over one reader, the form `scripts/fit_usl.py`
    fits (its `X(N) = gamma N / (1 + alpha (N - 1) + beta N (N - 1))`, divided
    by `X(1) = gamma`).

    `alpha` is the serial fraction and `beta` the pairwise term. `beta` may be
    any finite non-negative number; `alpha` is a fraction, in [0, 1].
    """
    if not isinstance(readers, int) or readers < 1:
        raise ValueError(f"readers must be an int >= 1, got {readers!r}")
    if not (0.0 <= alpha <= 1.0):
        raise ValueError(f"alpha must be in [0, 1], got {alpha!r}")
    if not (beta >= 0.0) or math.isinf(beta):
        raise ValueError(f"beta must be finite and non-negative, got {beta!r}")
    w = float(readers)
    return w / (1.0 + alpha * (w - 1.0) + beta * w * (w - 1.0))


def unexplained_term(observed_scaling: float, readers: int, alpha: float) -> float:
    """The second USL coefficient a measured `S(W)` needs once `alpha` is fixed from outside.

    Solving `S = W / (1 + alpha (W - 1) + b W (W - 1))` for `b` gives

        b = (W / S - 1 - alpha (W - 1)) / (W (W - 1))

    It is whatever the measured curve needs beyond the fixed `alpha`, and
    nothing in this file says what it is. AGENTS.md 8.20.4 forbids naming a
    remainder by subtraction, so it is reported as unexplained rather than as
    coherency. It is negative when `alpha` alone already takes `S` below what
    was measured, which says the fixed `alpha` is too large for this curve.
    """
    if not isinstance(readers, int) or readers < 2:
        raise ValueError(f"readers must be an int >= 2 (W = 1 carries no term), got {readers!r}")
    if not (observed_scaling > 0.0) or math.isinf(observed_scaling):
        raise ValueError(f"observed_scaling must be finite and positive, got {observed_scaling!r}")
    if not (0.0 <= alpha <= 1.0):
        raise ValueError(f"alpha must be in [0, 1], got {alpha!r}")
    w = float(readers)
    return (w / observed_scaling - 1.0 - alpha * (w - 1.0)) / (w * (w - 1.0))


def unexplained_term_interval(scaling_ci: tuple[float, float], readers: int,
                              alpha_ci: tuple[float, float]) -> tuple[float, float]:
    """The unexplained term's range over the intervals of `S` and of `alpha`.

    The term falls as `S` rises and as `alpha` rises, and is monotone in each,
    so its extremes sit at opposite corners of the two intervals: the lowest
    `S` with the lowest `alpha`, and the highest with the highest. That is exact
    for a box of inputs, not a bootstrap interval.
    """
    s_lo, s_hi = scaling_ci
    a_lo, a_hi = alpha_ci
    if not (s_lo <= s_hi and a_lo <= a_hi):
        raise ValueError(f"intervals must be ordered, got S {scaling_ci!r} and alpha {alpha_ci!r}")
    return (unexplained_term(s_hi, readers, a_hi), unexplained_term(s_lo, readers, a_lo))


def min_detectable_ratio(relative_halfwidth: float) -> float:
    """Smallest scaling ratio whose BCa lower bound clears 1.0 (AGENTS.md 8.4).

    A claim passes on its CI lower bound, not its point estimate, so a ratio
    `S` measured with relative half-width `h` is distinguishable from "no
    scaling" only when `S * (1 - h) > 1`.
    """
    if not (0.0 <= relative_halfwidth < 1.0):
        raise ValueError(f"relative_halfwidth must be in [0, 1), got {relative_halfwidth!r}")
    return 1.0 / (1.0 - relative_halfwidth)


def paired_ratio_relative_halfwidth(halfwidth_a: float, halfwidth_b: float) -> float:
    """Relative half-width of `A / B` from the relative half-widths of `A` and `B`.

    First-order propagation of error for a quotient of two independent
    estimates adds relative uncertainties in quadrature (H. H. Ku, "Notes on the
    Use of Propagation of Error Formulas", J. Res. NBS 70C(4), 1966, 263-273):

        h(A / B) = sqrt(h(A)^2 + h(B)^2)

    Independence is the conservative case for an interleaved paired design.
    Round-to-round drift that moves both arms together is positively
    correlated, and a positive covariance subtracts from the quotient's
    variance, so the measured paired interval is expected to be no wider. It is
    a projection for sizing a gate, never a substitute for the interval the
    rounds produce.
    """
    for name, h in (("halfwidth_a", halfwidth_a), ("halfwidth_b", halfwidth_b)):
        if not (0.0 <= h < 1.0):
            raise ValueError(f"{name} must be in [0, 1), got {h!r}")
    return math.hypot(halfwidth_a, halfwidth_b)


def _betacf(a: float, b: float, x: float) -> float:
    """Continued fraction for the regularized incomplete beta function.

    Modified Lentz evaluation, as in Press, Teukolsky, Vetterling & Flannery,
    *Numerical Recipes*, 3rd ed. (2007), section 6.4 (`betacf`). Raises rather
    than returning a partial sum when it does not converge (AGENTS.md 8.1).
    """
    fpmin = 1e-300
    qab, qap, qam = a + b, a + 1.0, a - 1.0
    c = 1.0
    d = 1.0 - qab * x / qap
    d = 1.0 / (d if abs(d) > fpmin else fpmin)
    h = d
    for m in range(1, 1001):
        m2 = 2 * m
        aa = m * (b - m) * x / ((qam + m2) * (a + m2))
        d = 1.0 + aa * d
        d = 1.0 / (d if abs(d) > fpmin else fpmin)
        c = 1.0 + aa / c
        c = c if abs(c) > fpmin else fpmin
        h *= d * c
        aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2))
        d = 1.0 + aa * d
        d = 1.0 / (d if abs(d) > fpmin else fpmin)
        c = 1.0 + aa / c
        c = c if abs(c) > fpmin else fpmin
        delta = d * c
        h *= delta
        if abs(delta - 1.0) < 1e-15:
            return h
    raise ValueError(f"incomplete beta continued fraction did not converge (a={a}, b={b}, x={x})")


def _regularized_incomplete_beta(a: float, b: float, x: float) -> float:
    """`I_x(a, b)`, by the symmetry switch of *Numerical Recipes* 3rd ed. section 6.4."""
    if not (a > 0 and b > 0):
        raise ValueError(f"a and b must be positive, got {a!r}, {b!r}")
    if x <= 0.0:
        return 0.0
    if x >= 1.0:
        return 1.0
    front = math.exp(math.lgamma(a + b) - math.lgamma(a) - math.lgamma(b)
                     + a * math.log(x) + b * math.log1p(-x))
    if x < (a + 1.0) / (a + b + 2.0):
        return front * _betacf(a, b, x) / a
    return 1.0 - front * _betacf(b, a, 1.0 - x) / b


def student_t_cdf(t: float, df: int) -> float:
    """CDF of Student's t with `df` degrees of freedom.

    `P(T <= t) = 1 - I_{df / (df + t^2)}(df / 2, 1 / 2) / 2` for `t >= 0`
    (Abramowitz & Stegun, *Handbook of Mathematical Functions*, 26.7.1 with
    26.5.27), and the reflection `1 - P(T <= -t)` below zero.
    """
    if df < 1:
        raise ValueError(f"df must be >= 1, got {df!r}")
    tail = 0.5 * _regularized_incomplete_beta(df / 2.0, 0.5, df / (df + t * t))
    return 1.0 - tail if t >= 0 else tail


def student_t_quantile(p: float, df: int) -> float:
    """The `p` quantile of Student's t, by bisection on `student_t_cdf`.

    Planning only: the gates are read on BCa intervals (AGENTS.md 8.4), and a
    t interval over few rounds is wider than BCa's, so sizing with it errs on
    the side of more rounds.
    """
    if not 0.5 <= p < 1.0:
        raise ValueError(f"p must be in [0.5, 1), got {p!r}")
    lo, hi = 0.0, 1.0
    while student_t_cdf(hi, df) < p:
        hi *= 2.0
        if hi > 1e12:
            raise ValueError(f"no bracket for the {p} quantile at df={df}")
    for _ in range(200):
        mid = 0.5 * (lo + hi)
        if student_t_cdf(mid, df) < p:
            lo = mid
        else:
            hi = mid
    return 0.5 * (lo + hi)


def per_round_cv(values: list[float]) -> float:
    """Sample coefficient of variation (`n - 1` standard deviation over the mean) of per-round values."""
    if len(values) < 2:
        raise ValueError(f"a spread needs at least two rounds, got {len(values)}")
    mean = statistics.fmean(values)
    if not mean > 0:
        raise ValueError(f"per-round values must have a positive mean, got {mean!r}")
    return statistics.stdev(values) / mean


def sizing_relative_halfwidth(cv: float, rounds: int) -> float:
    """Planning relative half-width of a mean of `rounds` per-round ratios with spread `cv`.

    `t(0.975, rounds - 1) * cv / sqrt(rounds)`: the Student-t interval on a
    mean, relative to the mean. The paired ratio is a mean over rounds, so its
    relative half-width shrinks as `1 / sqrt(rounds)` at a fixed spread.
    """
    if cv < 0:
        raise ValueError(f"cv must be non-negative, got {cv!r}")
    if rounds < 3:
        raise ValueError(f"BCa needs at least 3 rounds, got {rounds!r}")
    return student_t_quantile(0.975, rounds - 1) * cv / math.sqrt(rounds)


def rounds_for_detectable_ratio(cv: float, target_ratio: float, min_rounds: int = 3,
                                max_rounds: int = 1000) -> int:
    """Fewest rounds, at least `min_rounds`, whose projected lower bound clears 1 at `target_ratio`.

    That is the smallest `n` with `min_detectable_ratio(sizing_relative_halfwidth(cv, n))`
    at or below `target_ratio`. Raises when `max_rounds` does not reach it,
    rather than returning the cap as if it did (AGENTS.md 8.1).
    """
    if not target_ratio > 1.0:
        raise ValueError(f"target_ratio must be above 1, got {target_ratio!r}")
    if min_rounds < 3:
        raise ValueError(f"min_rounds must be >= 3, got {min_rounds!r}")
    for n in range(min_rounds, max_rounds + 1):
        h = sizing_relative_halfwidth(cv, n)
        if h < 1.0 and min_detectable_ratio(h) <= target_ratio:
            return n
    raise ValueError(f"cv {cv!r} needs more than {max_rounds} rounds to resolve {target_ratio!r}")


def queue_scaling(readers: int, locked_fraction: float, handoff: float,
                  growth: float = 0.0) -> float:
    """`S(W)` for readers cycling through one lock, by mean-value analysis.

    A closed network of `W` readers and two stations, in units of one
    uncontended read. At a delay station a reader does the unlocked
    `1 - locked_fraction` of its read. At one first-come-first-served lock the
    service time is `locked_fraction` while one reader is present, and
    `locked_fraction + handoff + growth * (n - 2)` once `n >= 2` are. The
    recursion is Reiser and Lavenberg's mean-value analysis ("Mean-Value
    Analysis of Closed Multichain Queuing Networks", J. ACM 27(2), 1980):

        R(n) = s(n) (1 + Q(n - 1)),   X(n) = n / (Z + R(n)),   Q(n) = X(n) R(n)

    with `Z = 1 - locked_fraction` and `S(W) = X(W) / X(1)`. It is exact for a
    load-independent service time, which is `handoff = growth = 0`. With `s(n)`
    depending on `n` it is the same recursion used as an approximation, not a
    product-form result.

    `handoff` stands for a cost an acquisition pays once a second reader exists
    (a futex wake, a migration), and `growth` for how that cost rises per
    further reader. Nothing in this file measures either: they are fitted, and
    the names say what they would have to be, not what they are.
    """
    if not isinstance(readers, int) or readers < 1:
        raise ValueError(f"readers must be an int >= 1, got {readers!r}")
    if not (0.0 < locked_fraction <= 1.0):
        raise ValueError(f"locked_fraction must be in (0, 1], got {locked_fraction!r}")
    for name, value in (("handoff", handoff), ("growth", growth)):
        if not (value >= 0.0) or math.isinf(value):
            raise ValueError(f"{name} must be finite and non-negative, got {value!r}")
    think = 1.0 - locked_fraction
    queue = 0.0
    x1 = x = 0.0
    for n in range(1, readers + 1):
        service = locked_fraction if n == 1 else locked_fraction + handoff + growth * (n - 2)
        response = service * (1.0 + queue)
        x = n / (think + response)
        queue = x * response
        if n == 1:
            x1 = x
    return x / x1


#: METHODOLOGY section 5.12. Each shape is fitted on the reader counts every
#: committed idle curve has, and checked as well on three no curve had when the
#: shapes were fixed.
TRAINING_READERS = (2, 4, 7)
HELD_OUT_READERS = (3, 5, 6)

#: The candidates, fixed before the held-out cells were measured. `mechanistic`
#: shapes take the profiled `locked_fraction`, which is what would let one carry
#: over to a lock that covers less of a read; the others describe the curve and
#: name no lock parameter. Each parameter is `(name, low, high, low_is_physical,
#: high_is_physical)`. A fit may rest on a physical limit (a cost of zero, a
#: serial fraction of one). One that ends on a search limit has not converged
#: inside the box it was given, and is refused rather than read.
MODEL_CANDIDATES: dict[str, dict] = {
    "usl_alpha_profiled": {
        "mechanistic": True,
        "params": (("beta", 0.0, 5.0, True, False),),
        "shape": lambda w, f, p: usl_scaling(w, f, p[0]),
    },
    "queue_handoff": {
        "mechanistic": True,
        "params": (("handoff", 0.0, 20.0, True, False),),
        "shape": lambda w, f, p: queue_scaling(w, f, p[0]),
    },
    "queue_handoff_growing": {
        "mechanistic": True,
        "params": (("handoff", 0.0, 20.0, True, False), ("growth", 0.0, 5.0, True, False)),
        "shape": lambda w, f, p: queue_scaling(w, f, p[0], p[1]),
    },
    "reciprocal_linear": {
        "mechanistic": False,
        "params": (("a", -5.0, 5.0, False, False), ("b", -1.0, 1.0, False, False)),
        "shape": lambda w, f, p: 1.0 / (p[0] + p[1] * w),
    },
    "step_power": {
        "mechanistic": False,
        "params": (("s2", 0.1, 2.0, False, False), ("gamma", -1.0, 2.0, False, False)),
        "shape": lambda w, f, p: p[0] * (w / 2.0) ** (-p[1]),
    },
    "usl_free": {
        "mechanistic": False,
        "params": (("alpha", 0.0, 1.0, True, True), ("beta", 0.0, 5.0, True, False)),
        "shape": lambda w, f, p: usl_scaling(w, p[0], p[1]),
    },
}


def _minimise(objective, bounds: list[tuple[float, float]]) -> tuple[tuple[float, ...], float]:
    """A deterministic bounded minimiser: a grid, then a shrinking pattern search.

    No scipy. The CI lint job has none, and an estimator that changes with the
    host's installed packages has already changed one reading here (METHODOLOGY
    section 5.10). The grid is 401 points for one parameter and 61 x 61 for two.
    The pattern search starts at the grid spacing, shrinks it by 0.7 over 80
    passes, and clamps every step to the bounds. A point where the shape is
    undefined scores infinity rather than raising.
    """
    k = len(bounds)
    n = 400 if k == 1 else 60

    def score(p):
        try:
            v = objective(p)
        except (ZeroDivisionError, OverflowError, ValueError):
            return math.inf
        return v if math.isfinite(v) else math.inf

    best_v, best_p = math.inf, None
    for idx in itertools.product(range(n + 1), repeat=k):
        p = tuple(lo + (hi - lo) * i / n for (lo, hi), i in zip(bounds, idx))
        v = score(p)
        if v < best_v:
            best_v, best_p = v, p
    if best_p is None:
        raise ValueError("the objective is not finite anywhere on the grid")
    step = [(hi - lo) / n for lo, hi in bounds]
    for _ in range(80):
        improved = True
        while improved:
            improved = False
            for i in range(k):
                for d in (-1.0, 1.0):
                    q = list(best_p)
                    q[i] = min(max(q[i] + d * step[i], bounds[i][0]), bounds[i][1])
                    v = score(tuple(q))
                    if v < best_v:
                        best_v, best_p, improved = v, tuple(q), True
        step = [s * 0.7 for s in step]
    return best_p, best_v


def fit_candidate(name: str, curve: dict, locked_fraction: float | None) -> dict:
    """Fit one candidate to a curve's training points, weighted by their intervals.

    `curve` maps `W` to `(point, ci_lower, ci_upper)` for `S(W)`. The fit
    minimises `sum(((shape(W) - point) / half_width) ** 2)` over
    `TRAINING_READERS`, with `half_width = (ci_upper - ci_lower) / 2`, so a
    point measured tightly counts for more than one measured loosely.
    """
    spec = MODEL_CANDIDATES[name]
    if spec["mechanistic"] and locked_fraction is None:
        raise ValueError(f"{name} takes the profiled locked_fraction, and none was given")
    f = locked_fraction if spec["mechanistic"] else None
    points = []
    for w in TRAINING_READERS:
        if w not in curve:
            raise ValueError(f"the curve has no S({w}), a training point")
        point, lo, hi = curve[w]
        half = (hi - lo) / 2.0
        if not half > 0.0 or not (lo <= point <= hi):
            raise ValueError(f"S({w}) = {point!r} with interval [{lo!r}, {hi!r}] cannot weight a fit")
        points.append((w, point, half))
    shape = spec["shape"]
    params, chi2 = _minimise(
        lambda p: sum(((shape(w, f, p) - s) / h) ** 2 for w, s, h in points),
        [(lo, hi) for _, lo, hi, _, _ in spec["params"]],
    )
    at_limit = []
    for (pname, lo, hi, lo_physical, hi_physical), v in zip(spec["params"], params):
        tol = 1e-9 * (hi - lo)
        if (not lo_physical and v <= lo + tol) or (not hi_physical and v >= hi - tol):
            at_limit.append(pname)
    names = [p[0] for p in spec["params"]]
    return {"params": dict(zip(names, params)), "chi2_training": chi2, "search_limit": at_limit}


def check_candidate(name: str, curve: dict, locked_fraction: float | None,
                    readers: tuple[int, ...] = TRAINING_READERS + HELD_OUT_READERS) -> dict:
    """METHODOLOGY section 5.12's rule for one candidate on one curve.

    Fitted on `TRAINING_READERS` only. The candidate passes on the curve when
    its fit converged and its prediction lies inside the measured interval at
    every `W` in `readers`: the training points and, by default, the held-out
    ones. A held-out point is never used to fit.
    """
    spec = MODEL_CANDIDATES[name]
    fit = fit_candidate(name, curve, locked_fraction)
    f = locked_fraction if spec["mechanistic"] else None
    p = tuple(fit["params"].values())
    rows = {}
    for w in readers:
        if w not in curve:
            raise ValueError(f"the curve has no S({w})")
        point, lo, hi = curve[w]
        pred = spec["shape"](w, f, p)
        rows[w] = {"predicted": pred, "point": point, "ci": (lo, hi), "inside": lo <= pred <= hi,
                   "role": "training" if w in TRAINING_READERS else "held_out"}
    misses = [w for w, r in rows.items() if not r["inside"]]
    return {**fit, "readers": rows, "misses": misses,
            "accepted": not fit["search_limit"] and not misses}


def held_out_chi2(check: dict) -> float:
    """`sum(((predicted - point) / half_width) ** 2)` over a check's held-out rows."""
    total = 0.0
    for r in check["readers"].values():
        if r["role"] == "held_out":
            lo, hi = r["ci"]
            total += ((r["predicted"] - r["point"]) / ((hi - lo) / 2.0)) ** 2
    return total


def model_verdicts(curves: list[dict], fraction_span: tuple[float, float, float]) -> dict:
    """METHODOLOGY section 5.12's acceptance rule over the held-out runs.

    A candidate is `ACCEPTED` only if `check_candidate` passes it on every run.
    A mechanistic one must also pass at each of the three locked fractions in
    `fraction_span` (the lowest interval end, the mean point, the highest end of
    the two profile runs), refitted at each: a shape that holds at one fraction
    and not at a neighbour the profile cannot exclude is not carried over.
    """
    if len(curves) < 2:
        raise ValueError(f"the rule reads two runs, got {len(curves)}")
    lo_f, mid_f, hi_f = fraction_span
    if not (0.0 < lo_f <= mid_f <= hi_f <= 1.0):
        raise ValueError(f"fraction_span must be ordered inside (0, 1], got {fraction_span!r}")
    out = {}
    for name, spec in MODEL_CANDIDATES.items():
        fractions = (lo_f, mid_f, hi_f) if spec["mechanistic"] else (None,)
        checks = [[check_candidate(name, c, f) for f in fractions] for c in curves]
        accepted = all(ch["accepted"] for run in checks for ch in run)
        out[name] = {
            "verdict": "ACCEPTED" if accepted else "REJECTED",
            "mechanistic": spec["mechanistic"],
            "n_params": len(spec["params"]),
            "held_out_chi2": sum(held_out_chi2(run[len(run) // 2]) for run in checks),
            "checks": checks,
        }
    return out


def select_model(verdicts: dict) -> str | None:
    """The accepted candidate section 5.12 carries forward, or `None`.

    Mechanistic before descriptive, since only a mechanistic shape names a lock
    parameter a narrower lock could change. Then fewer parameters, then the
    lower held-out chi-square summed over runs.
    """
    ranked = sorted((not v["mechanistic"], v["n_params"], v["held_out_chi2"], name)
                    for name, v in verdicts.items() if v["verdict"] == "ACCEPTED")
    return ranked[0][3] if ranked else None


def model_consequence(verdicts: dict) -> str:
    """What section 5.12 says follows from its verdicts."""
    chosen = select_model(verdicts)
    if chosen is None:
        return ("no candidate passed: the narrowed-mutex arm is pre-registered on the "
                "directional gate, and every candidate is reported with its misses")
    if not verdicts[chosen]["mechanistic"]:
        return (f"only a descriptive shape passed ({chosen}): it names no lock parameter, so "
                f"the narrowed-mutex arm is pre-registered on the directional gate")
    return (f"{chosen} passed: the narrowed-mutex arm's prediction is derived from it, in a "
            f"pre-registration written before that arm's rounds")


def directional_verdict(intervals: list[tuple[float, float]], floor: float = 1.0) -> str:
    """METHODOLOGY section 5.14's gate over one pin's runs of a paired ratio.

    `PASS` when every run's BCa lower bound is above `floor`, `REFUTED` when
    every run's upper bound is below it, `BOUNDARY_RESULT` otherwise -- an
    interval that contains `floor`, or runs that disagree (AGENTS.md 8.4, and
    `docs/BENCHMARKING.md` rule 18 for the both-runs part).
    """
    if len(intervals) < 2:
        raise ValueError(f"the gate reads two runs, got {len(intervals)}")
    for lo, hi in intervals:
        if not lo <= hi:
            raise ValueError(f"interval [{lo!r}, {hi!r}] is not ordered")
    if all(lo > floor for lo, _ in intervals):
        return "PASS"
    if all(hi < floor for _, hi in intervals):
        return "REFUTED"
    return "BOUNDARY_RESULT"


def held_out_separation(curve: dict, locked_fraction: float) -> list[tuple[str, str, float]]:
    """How far apart the candidates' held-out predictions are, before any held-out cell.

    For every pair of candidates whose training fit on `curve` lies inside all
    three training intervals, the largest gap between their predictions over
    `HELD_OUT_READERS`, in units of the curve's median training half-width.
    A gap below one says a held-out interval of that width cannot tell the
    two apart, whatever the cells return.
    """
    half = statistics.median((curve[w][2] - curve[w][1]) / 2.0 for w in TRAINING_READERS)
    fits = {}
    for name, spec in MODEL_CANDIDATES.items():
        f = locked_fraction if spec["mechanistic"] else None
        ch = check_candidate(name, curve, f, readers=TRAINING_READERS)
        if ch["accepted"]:
            p = tuple(ch["params"].values())
            fits[name] = [spec["shape"](w, f, p) for w in HELD_OUT_READERS]
    names = sorted(fits)
    return [(a, b, max(abs(x - y) for x, y in zip(fits[a], fits[b])) / half)
            for i, a in enumerate(names) for b in names[i + 1:]]


# ---------------------------------------------------------------------------
# Inputs, from the committed artifact
# ---------------------------------------------------------------------------

def load_arms(path: Path = ARTIFACT) -> dict:
    """Reads `insert_ns` and `read_ns` from the committed suite artifact.

    Fails loudly on a missing file, a missing arm or an unexpected unit rather
    than falling back to a literal, so a moved or re-shaped artifact stops this
    derivation instead of silently pinning it to numbers nobody re-checked
    (AGENTS.md 8.1).
    """
    if not path.is_file():
        raise FileNotFoundError(f"suite artifact not found: {path}")
    obj = json.loads(path.read_text())
    # `cells` first, `arms` second. The suite's shell-loop harvester wrote the
    # published arms under `arms`; `scripts/check_bench_provenance.py` requires a
    # key from its own CELL_KEYS list, so the #868 driver writes `cells`. Reading
    # both is what keeps this derivation working across the re-measurement
    # instead of breaking at it -- and it still fails loudly when neither key
    # holds the arm it needs, rather than falling back to a literal (8.1).
    cells = obj.get("cells")
    if not isinstance(cells, list) or not cells:
        cells = obj.get("arms") or []
    by_id = {a["id"]: a for a in cells}
    out = {}
    for label, arm_id in (("insert", INSERT_ARM), ("read", READ_ARM)):
        arm = by_id.get(arm_id)
        if arm is None:
            raise KeyError(f"{path.name} has no arm `{arm_id}`; present: {sorted(by_id)}")
        if arm.get("unit") != "Mops_per_second":
            raise ValueError(f"arm `{arm_id}` has unit {arm.get('unit')!r}, expected Mops_per_second")
        out[f"{label}_ns"] = ns_per_op(float(arm["point"]))
        out[f"{label}_mops"] = float(arm["point"])
        out[f"{label}_ci"] = [float(arm["ci_lower"]), float(arm["ci_upper"])]
        out[f"{label}_n"] = int(arm["n"])
    prov = obj.get("provenance", {})
    out["commit"] = prov.get("commit", "unknown")
    out["host"] = prov.get("host_description", "unknown")
    out["run_id"] = prov.get("run_id", "unknown")
    return out


PROFILE_GLOB = "docs/benchmarks/rocksdb_memtable/results/locate_profile/pin_one_sibling/*/profile_rocksdb_conc_idle_r1.json"
_RESULTS = REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable" / "results"
#: The committed idle curves under the one-sibling pin: sections 5.8 and 5.10.
COMMITTED_IDLE_CURVES = tuple(_RESULTS / f"baseline_concurrent_reads_{n}.json" for n in (
    "pin_one_sibling", "pin_one_sibling_run2", "amended_h1", "amended_h1_run2"))
#: The two held-out runs section 5.12 fixes, once committed.
HELD_OUT_CURVES = (_RESULTS / "baseline_concurrent_reads_heldout.json",
                   _RESULTS / "baseline_concurrent_reads_heldout_run2.json")
HELD_OUT_PIN = "0,2,4,6,8,10,12,14"
#: METHODOLOGY section 5.14's runs: two per pin, once committed.
NARROWED_PINS = {"pin_one_sibling": "0,2,4,6,8,10,12,14", "pin_0-15": "0-15"}
NARROWED_CURVES = {label: (_RESULTS / f"baseline_concurrent_reads_narrowed_{label}.json",
                           _RESULTS / f"baseline_concurrent_reads_narrowed_{label}_run2.json")
                   for label in NARROWED_PINS}
NARROWED_RATIOS = ("T(1)", "S(2)", "S(4)", "S(7)")

#: METHODOLOGY section 5.16 sizes the optimistic-seek arm from the four
#: section 5.15 runs: the only committed artifacts that carry a scope other than
#: `full`, and paired per-round ratios, for the gate cell (idle `R = 7`).
OPTIMISTIC_SIZING_SOURCES = tuple(p for label in NARROWED_PINS for p in NARROWED_CURVES[label])
#: The smallest true paired ratio section 5.16's gated ratio must be able to
#: tell from 1. A choice, fixed before the sizing was computed, not a derivation.
OPTIMISTIC_MDR_TARGET = 1.05
#: The one ratio section 5.16 gates (O1). `opt/trie` (O2) is reported only and
#: does not set the rounds.
OPTIMISTIC_GATED_RATIO = "opt/full"
#: Section 5.14's rounds per cell; section 5.16 never runs fewer.
OPTIMISTIC_ROUNDS_FLOOR = 5


def optimistic_sizing_inputs(path: Path) -> dict:
    """Per-round spreads of the idle `R = 7` cell in one section 5.15 artifact, every round, no filtering.

    `cv_full` and `cv_trie` are the per-round spreads of each scope's `S(7)`;
    `cv_trie_over_full` is the spread of the paired per-round `S_trie(7) / S_full(7)`.
    """
    obj = json.loads(Path(path).read_text())
    try:
        full = obj["scaling_by_lock_scope"]["full"]["idle"]["S(7)"]["rounds_raw"]
        trie = obj["scaling_by_lock_scope"]["trie"]["idle"]["S(7)"]["rounds_raw"]
        ratio = obj["lock_scope_ratio"]["idle"]["S(7)"]["rounds_raw"]
    except (KeyError, TypeError) as exc:
        raise KeyError(f"{Path(path).name}: no idle S(7) per-round data for both scopes ({exc})") from exc
    if not len(full) == len(trie) == len(ratio):
        raise ValueError(f"{Path(path).name}: round counts differ (full {len(full)}, "
                         f"trie {len(trie)}, ratio {len(ratio)})")
    return {"name": Path(path).name, "rounds": len(ratio), "cv_full": per_round_cv(full),
            "cv_trie": per_round_cv(trie), "cv_trie_over_full": per_round_cv(ratio)}


def optimistic_gate_sizing(runs: list[dict], target: float = OPTIMISTIC_MDR_TARGET,
                           floor: int = OPTIMISTIC_ROUNDS_FLOOR) -> dict:
    """Planning spread, detectable ratio and rounds for section 5.16's gate and its reported ratio.

    No run of the optimistic scope exists, so its per-round spread is stood in
    for by the `trie` scope's, the other scope that does not hold `mutex_` over
    the whole locate phase. That is an assumption, stated in section 5.16.

    - `opt/full` (O1, the gated ratio): the larger, over every run, of the
      measured paired spread of `trie/full` and the independent projection
      `hypot(cv_trie, cv_full)`. It alone sets `rounds`.
    - `opt/trie` (O2, reported and never gated): no paired analogue exists, so
      the independent projection `hypot(cv_trie, cv_trie)`, the larger over
      every run. Its detectable ratio at `rounds` is reported so that the
      section can say what the reported interval can and cannot resolve.

    `paired_ratio_relative_halfwidth` is the quadrature both projections use.
    Independence is conservative for interleaved arms (its docstring), and the
    largest run is taken, so the rounds are sized on the widest observed spread.
    """
    if not runs:
        raise ValueError("sizing needs at least one run")
    cvs = {
        "opt/full": max(max(r["cv_trie_over_full"], paired_ratio_relative_halfwidth(r["cv_trie"], r["cv_full"]))
                        for r in runs),
        "opt/trie": max(paired_ratio_relative_halfwidth(r["cv_trie"], r["cv_trie"]) for r in runs),
    }
    out = {}
    for name, cv in cvs.items():
        h_floor = sizing_relative_halfwidth(cv, floor)
        out[name] = {"cv": cv, "halfwidth_at_floor": h_floor,
                     "mdr_at_floor": min_detectable_ratio(h_floor),
                     "rounds_for_target": rounds_for_detectable_ratio(cv, target, min_rounds=floor)}
    rounds = out[OPTIMISTIC_GATED_RATIO]["rounds_for_target"]
    for v in out.values():
        v["halfwidth_at_rounds"] = sizing_relative_halfwidth(v["cv"], rounds)
        v["mdr_at_rounds"] = min_detectable_ratio(v["halfwidth_at_rounds"])
    return {"target": target, "floor": floor, "rounds": rounds, "ratios": out}


def load_locate_profile(path: Path) -> dict:
    """`locked_fraction` and `trie_fraction`, point and interval, from a locate profile.

    `docs/benchmarks/rocksdb_memtable/scripts/locate_profile.py` writes the
    artifact. A share with no interval is refused, because the bracket reads
    the interval, not the point.
    """
    obj = json.loads(Path(path).read_text())
    out = {"commit": obj.get("provenance", {}).get("commit", "unknown"),
           "pin": (obj.get("provenance", {}).get("pin") or [None])[-1]}
    for name in ("locked_fraction", "trie_fraction"):
        v = obj.get("shares", {}).get(name)
        if not v or v.get("ci_lower") is None:
            raise ValueError(f"{Path(path).name}: `shares.{name}` carries no interval")
        out[name] = (float(v["point"]), float(v["ci_lower"]), float(v["ci_upper"]))
    return out



def profile_fraction_span(paths: list[Path]) -> tuple[float, float, float]:
    """`(lowest ci_lower, mean point, highest ci_upper)` of `locked_fraction`.

    Section 5.12 fixes the two committed profile runs as the input, both under
    the one-sibling pin; any other count, or another pin, is refused.
    """
    profiles = [load_locate_profile(p) for p in paths]
    if len(profiles) != 2:
        raise ValueError(f"section 5.12 reads the two committed profile runs, found {len(profiles)}")
    for path, prof in zip(paths, profiles):
        if prof["pin"] != HELD_OUT_PIN:
            raise ValueError(f"{Path(path).parent.name}: pin {prof['pin']!r}, expected {HELD_OUT_PIN}")
    lf = [p["locked_fraction"] for p in profiles]
    return (min(x[1] for x in lf), sum(x[0] for x in lf) / len(lf), max(x[2] for x in lf))


def idle_curve(obj: dict, readers: tuple[int, ...]) -> dict:
    """`{W: (point, ci_lower, ci_upper)}` for the idle `S(W)` of a driver artifact."""
    idle = obj.get("scaling", {}).get("idle", {})
    out = {}
    for w in readers:
        cell = idle.get(f"S({w})")
        if not cell or cell.get("ci") is None:
            raise ValueError(f"no idle S({w}) with an interval")
        out[w] = (float(cell["point"]), float(cell["ci"][0]), float(cell["ci"][1]))
    return out


def held_out_problems(obj: dict) -> list[str]:
    """Why a driver artifact is not one of the held-out runs section 5.12 fixes."""
    problems = []
    p = obj.get("provenance", {})
    if p.get("core_pin") != HELD_OUT_PIN:
        problems.append(f"core_pin {p.get('core_pin')!r}, expected {HELD_OUT_PIN}")
    if p.get("host", {}).get("scaling_governor_pin_source") != "EXPANSE_BENCH_PIN_APPLIED":
        problems.append("the pin was not verified as applied (EXPANSE_BENCH_PIN_APPLIED)")
    if obj.get("settings", {}).get("window_seconds") != 2.0:
        problems.append(f"window {obj.get('settings', {}).get('window_seconds')!r} s, expected 2.0")
    modes = sorted(obj.get("scaling", {}))
    if modes != ["idle"]:
        problems.append(f"modes {modes}, expected idle only")
    idle = obj.get("scaling", {}).get("idle", {})
    for w in (2, 3, 4, 5, 6, 7):
        cell = idle.get(f"S({w})")
        if not cell or cell.get("ci") is None:
            problems.append(f"no idle S({w}) with an interval")
        elif cell.get("n") != 5:
            problems.append(f"idle S({w}) pairs {cell.get('n')} rounds, expected 5")
    return problems


# ---------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------

FRACTIONS = (1.0, 0.75, 0.5, 0.25, 0.1, 0.05)
READER_COUNTS = (1, 2, 4, 8)


def render(arms: dict, writer_ops_per_s: float) -> str:
    insert_ns = arms["insert_ns"]
    read_ns = arms["read_ns"]
    duty = writer_duty_cycle(writer_ops_per_s, insert_ns)

    lines = []
    lines.append("#802 -- what one mutex over the locate phase permits")
    lines.append("")
    lines.append(f"  inputs (measured: {arms['host']}, {arms['commit'][:8]})")
    lines.append(f"    artifact  {ARTIFACT.relative_to(REPO_ROOT)}")
    lines.append(f"    run       {arms['run_id']}")
    lines.append(f"    insert  {arms['insert_mops']:.3f} Mops/s [{arms['insert_ci'][0]:.3f}, "
                 f"{arms['insert_ci'][1]:.3f}] n={arms['insert_n']}  ->  {insert_ns:.2f} ns/insert")
    lines.append(f"    read    {arms['read_mops']:.3f} Mops/s [{arms['read_ci'][0]:.3f}, "
                 f"{arms['read_ci'][1]:.3f}] n={arms['read_n']}  ->  {read_ns:.2f} ns/read")
    lines.append(f"    writer offered rate {writer_ops_per_s:,.0f} ops/s  ->  lock duty {duty:.3%}")
    lines.append(f"    writer saturates the lock alone at {1e9 / insert_ns:,.0f} ops/s")
    lines.append("")
    lines.append("  Predicted aggregate read throughput, Mops/s (S(W) in brackets).")
    lines.append("  `locked` is the share of a read spent inside `mutex_` -- the one input")
    lines.append("  no committed artifact pins yet, so the sweep is over it.")
    lines.append("")
    head = ("    locked | design |" + "".join(f"{f'W={w}':>18}" for w in READER_COUNTS)
            + "   K   alpha  D2 gain")
    lines.append(head)
    lines.append("    " + "-" * (len(head) - 4))
    for f in FRACTIONS:
        excl = [exclusive_read_throughput_ops(w, read_ns, f, duty) for w in READER_COUNTS]
        shrd = [shared_read_throughput_ops(w, read_ns, f, duty) for w in READER_COUNTS]
        lines.append(
            f"    {f:>6.2f} | mutex  |"
            + "".join(f"{t / 1e6:>10.2f} [{exclusive_scaling(w, f, duty):>4.2f}]"
                      for w, t in zip(READER_COUNTS, excl))
            + f"   {knee(f, duty):>5.2f}  {predicted_usl_alpha(f, duty):>5.3f}")
        lines.append(
            "           | shared |"
            + "".join(f"{t / 1e6:>10.2f} [{shared_scaling(w, f, duty):>4.2f}]"
                      for w, t in zip(READER_COUNTS, shrd))
            + f"                  {shrd[-1] / excl[-1]:>5.2f}x")
    lines.append("")
    lines.append("  `alpha` is the USL contention parameter this bound predicts, the fit target")
    lines.append("  for `scripts/fit_usl.py` once the arm has run. `D2 gain` is the shared")
    lines.append("  design's throughput over the mutex's at the sweep's")
    lines.append("  widest cell. It is largest exactly where the lock covers most of a read and")
    lines.append("  closes as `locked` falls -- so the measurement that answers whether the mutex")
    lines.append("  binds also answers whether splitting it would pay, with no second build.")
    lines.append("")
    # Derived, never stamped (AGENTS.md 8.2): at duty 0 the fully-locked ceiling
    # equals the single-threaded rate rather than falling below it, so the
    # sentence has to read the numbers it is describing.
    solo = 1e3 / read_ns
    full = exclusive_read_ceiling_ops(read_ns, 1.0, duty) / 1e6
    if math.isclose(full, solo, rel_tol=1e-9):
        verdict = "exactly on it -- an idle writer leaves the whole lock to the readers"
    elif full < solo:
        verdict = f"below it, by {solo / full:.2f}x"
    else:
        verdict = f"above it, by {full / solo:.2f}x"
    lines.append(f"  Single-threaded reference: {solo:.3f} Mops/s. The locked = 1.00 ceiling "
                 f"({full:.2f} Mops/s) sits {verdict}.")
    lines.append("")
    lines.append("  Detectability (AGENTS.md 8.4): a scaling ratio measured with relative")
    lines.append("  half-width h clears 1.0 only above 1/(1-h).")
    for h in (0.01, 0.05, 0.10):
        lines.append(f"    h = {h:.0%}  ->  smallest distinguishable S = {min_detectable_ratio(h):.3f}")
    lines.append("")
    lines.append("  These are ceilings on an idealised handoff (no transfer cost, no convoying,")
    lines.append("  reader remainder perfectly overlapped), so a measured curve should sit at or")
    lines.append("  below them. Nothing here is measured: the mechanism is arithmetic over two")
    lines.append("  committed single-threaded cells, and the concurrent arm is what tests it.")
    lines.extend(render_model_check())
    lines.extend(render_gate_detectability())
    lines.extend(render_narrowed_verdicts())
    lines.extend(render_optimistic_sizing())
    return "\n".join(lines)


def narrowed_problems(obj: dict, pin: str) -> list[str]:
    """Why a driver artifact is not one of the runs METHODOLOGY section 5.14 fixes for `pin`."""
    problems = []
    p = obj.get("provenance", {})
    if p.get("core_pin") != pin:
        problems.append(f"core_pin {p.get('core_pin')!r}, expected {pin}")
    if p.get("host", {}).get("scaling_governor_pin_source") != "EXPANSE_BENCH_PIN_APPLIED":
        problems.append("the pin was not verified as applied (EXPANSE_BENCH_PIN_APPLIED)")
    s = obj.get("settings", {})
    for key, want in (("window_seconds", 2.0), ("paced_rate_ops_per_s", 250000.0),
                      ("readers", [1, 2, 4, 7]), ("modes", ["idle", "paced"]),
                      ("lock_scopes", ["full", "trie"])):
        if s.get(key) != want:
            problems.append(f"settings.{key} {s.get(key)!r}, expected {want!r}")
    for mode in ("idle", "paced"):
        for key in NARROWED_RATIOS:
            cell = obj.get("lock_scope_ratio", {}).get(mode, {}).get(key)
            if not cell or cell.get("ci") is None:
                problems.append(f"no {mode} {key} trie/full ratio with an interval")
            elif cell.get("n") != 5:
                problems.append(f"{mode} {key} pairs {cell.get('n')} rounds, expected 5")
    return problems


def render_narrowed_verdicts() -> list[str]:
    """Section 5.14's verdicts per pin, once both of that pin's runs are committed."""
    out = ["", "  Narrowed-mutex arm, trie/full paired ratios (METHODOLOGY section 5.14):"]
    for label, pin in NARROWED_PINS.items():
        present = [p for p in NARROWED_CURVES[label] if p.is_file()]
        if len(present) < 2:
            out.append(f"    {label}: {len(present)} of 2 runs committed; dispatch "
                       f"`rocksdb_concurrent_narrowed` with cpu_pin={pin}")
            continue
        objs = [json.loads(p.read_text()) for p in present]
        bad = [(p.name, narrowed_problems(o, pin)) for p, o in zip(present, objs)]
        bad = [b for b in bad if b[1]]
        if bad:
            out.append(f"    {label}: {bad[0][0]} is not a section 5.14 run: " + "; ".join(bad[0][1]))
            continue
        for mode in ("idle", "paced"):
            for key in NARROWED_RATIOS:
                cells = [o["lock_scope_ratio"][mode][key] for o in objs]
                gated = mode == "idle" and key == "S(7)"
                verdict = directional_verdict([tuple(c["ci"]) for c in cells]) if gated else "reported, not gated"
                vals = "  ".join(f"{c['point']:.4f} [{c['ci'][0]:.4f}, {c['ci'][1]:.4f}]" for c in cells)
                out.append(f"    {label} {mode} {key}: {vals} -> {verdict}")
        for path, o in zip(present, objs):
            for scope, report in o.get("paced_rate_check_by_lock_scope", {}).items():
                e = report["per_readers"].get("7")
                if e:
                    out.append(f"    {path.name} [{scope}] paced R=7: {e['achieved_min_ops_per_s']:,.0f}-"
                               f"{e['achieved_max_ops_per_s']:,.0f} inserts/s, {len(report['flags'])} flagged cell(s)")
    return out


def render_gate_detectability() -> list[str]:
    """What paired trie/full ratio of `S(7)` a two-arm run could distinguish from 1.

    Projected from committed full-scope curves under the one-sibling pin: each
    curve's relative half-width of `S(7)` stands in for both arms, combined by
    `paired_ratio_relative_halfwidth` and read through `min_detectable_ratio`.
    """
    out = ["", "  Directional gate, trie/full S(7) (projected from committed full-scope curves):"]
    sources = [(p, mode) for p in COMMITTED_IDLE_CURVES[2:] for mode in ("idle", "paced")]
    sources += [(p, "idle") for p in HELD_OUT_CURVES if p.is_file()]
    for path, mode in sources:
        cell = json.loads(path.read_text())["scaling"][mode]["S(7)"]
        h = (cell["ci"][1] - cell["ci"][0]) / 2.0 / cell["point"]
        paired = paired_ratio_relative_halfwidth(h, h)
        out.append(f"    {path.name} {mode}: S(7) relative half-width {h:.2%} -> paired {paired:.2%}, "
                   f"lower bound clears 1 above a ratio of {min_detectable_ratio(paired):.4f}")
    return out


def render_optimistic_sizing() -> list[str]:
    """Section 5.16's sizing, from every round of the four section 5.15 runs."""
    out = ["", "  Optimistic-seek arm sizing (METHODOLOGY section 5.16), idle S(7), from section 5.15's runs:"]
    missing = [p.name for p in OPTIMISTIC_SIZING_SOURCES if not p.is_file()]
    if missing:
        out.append(f"    not evaluable: missing {', '.join(missing)}")
        return out
    runs = [optimistic_sizing_inputs(p) for p in OPTIMISTIC_SIZING_SOURCES]
    for r in runs:
        out.append(f"    {r['name']}: {r['rounds']} rounds; per-round CV full {r['cv_full']:.4f}, "
                   f"trie {r['cv_trie']:.4f}, paired trie/full {r['cv_trie_over_full']:.4f}; "
                   f"hypot(trie, full) {paired_ratio_relative_halfwidth(r['cv_trie'], r['cv_full']):.4f}, "
                   f"hypot(trie, trie) {paired_ratio_relative_halfwidth(r['cv_trie'], r['cv_trie']):.4f}")
    s = optimistic_gate_sizing(runs)
    for name, v in s["ratios"].items():
        role = "gated (O1)" if name == OPTIMISTIC_GATED_RATIO else "reported, not gated (O2)"
        out.append(f"    {name} [{role}]: planning CV {v['cv']:.4f}; at {s['floor']} rounds relative half-width "
                   f"{v['halfwidth_at_floor']:.4f}, lower bound clears 1 above {v['mdr_at_floor']:.4f}; "
                   f"{v['rounds_for_target']} rounds reach {s['target']:.2f}")
    out.append(f"    rounds per cell: {s['rounds']} (set by {OPTIMISTIC_GATED_RATIO} alone); at that count " + "; ".join(
        f"{name} clears 1 above {v['mdr_at_rounds']:.4f}" for name, v in s["ratios"].items()))
    out.append(f"    t(0.975, {s['rounds'] - 1}) = {student_t_quantile(0.975, s['rounds'] - 1):.4f}; "
               f"the gates themselves read BCa intervals")
    return out


def render_model_check() -> list[str]:
    """Section 5.12's candidates on the committed curves, and its verdicts once run."""
    import glob  # noqa: PLC0415
    out = ["", "  Idle-curve model check (METHODOLOGY section 5.12), one-sibling pin:"]
    profiles = [Path(p) for p in sorted(glob.glob(str(REPO_ROOT / PROFILE_GLOB)))]
    try:
        span = profile_fraction_span(profiles)
    except ValueError as exc:
        out.append(f"    not evaluable: {exc}")
        return out
    out.append(f"    locked_fraction {span[1]:.4f}, the mean of {len(profiles)} profile runs; "
               f"mechanistic shapes are also refitted at {span[0]:.4f} and {span[2]:.4f}")
    for path in COMMITTED_IDLE_CURVES:
        curve = idle_curve(json.loads(path.read_text()), TRAINING_READERS)
        out.append(f"    {path.name}: " + "  ".join(
            f"S({w}) {curve[w][0]:.4f} [{curve[w][1]:.4f}, {curve[w][2]:.4f}]" for w in TRAINING_READERS))
        out.append("      USL with alpha = locked_fraction needs beta " + ", ".join(
            f"{unexplained_term(curve[w][0], w, span[1]):.3f} at R={w}" for w in TRAINING_READERS)
            + "; one USL needs one beta")
        for name, spec in MODEL_CANDIDATES.items():
            f = span[1] if spec["mechanistic"] else None
            ch = check_candidate(name, curve, f, readers=TRAINING_READERS)
            p = tuple(ch["params"].values())
            if ch["search_limit"]:
                status = "search limit " + ",".join(ch["search_limit"])
            elif ch["misses"]:
                status = "misses R=" + ",".join(str(w) for w in ch["misses"])
            else:
                status = "inside"
            params = ", ".join(f"{k} {v:.4f}" for k, v in ch["params"].items())
            pred = "  ".join(f"S({w}) {spec['shape'](w, f, p):.4f}" for w in HELD_OUT_READERS)
            out.append(f"      {name:22s} {params:30s} chi2 {ch['chi2_training']:8.2f}  "
                       f"training {status:12s} -> {pred}")
        for a, b, gap in held_out_separation(curve, span[1]):
            out.append(f"      held-out gap, {a} vs {b}: {gap:.2f} training half-widths at most")
    present = [p for p in HELD_OUT_CURVES if p.is_file()]
    if len(present) < len(HELD_OUT_CURVES):
        out.append(f"    held-out runs: {len(present)} of {len(HELD_OUT_CURVES)} committed; "
                   f"dispatch `rocksdb_concurrent_heldout` with cpu_pin={HELD_OUT_PIN}")
        return out
    objs = [json.loads(p.read_text()) for p in present]
    for path, obj in zip(present, objs):
        problems = held_out_problems(obj)
        if problems:
            out.append(f"    {path.name} is not a section 5.12 run: " + "; ".join(problems))
            return out
    verdicts = model_verdicts([idle_curve(o, TRAINING_READERS + HELD_OUT_READERS) for o in objs], span)
    for name, v in verdicts.items():
        misses = sorted({w for run in v["checks"] for ch in run for w in ch["misses"]})
        limits = sorted({x for run in v["checks"] for ch in run for x in ch["search_limit"]})
        why = (f" (misses R={','.join(map(str, misses))})" if misses else "") + \
              (f" (search limit {','.join(limits)})" if limits else "")
        out.append(f"    {name:22s} {v['verdict']}{why}; held-out chi2 {v['held_out_chi2']:.2f}")
    out.append(f"    consequence: {model_consequence(verdicts)}")
    return out


# ---------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------

def _close(a: float, b: float, tol: float = 1e-6) -> bool:
    return abs(a - b) <= tol * max(1.0, abs(b))


def self_test() -> int:
    fails: list[str] = []

    def check(name: str, got, want, tol: float = 1e-6):
        ok = _close(got, want, tol) if isinstance(want, float) else got == want
        if not ok:
            fails.append(f"{name}: got {got!r}, want {want!r}")

    # --- unit conversion, against hand arithmetic -------------------------
    # 4.422 Mops/s -> 1000/4.422 ns.
    check("ns_per_op(4.422)", ns_per_op(4.422), 226.14201718)
    check("ns_per_op(3.788)", ns_per_op(3.788), 263.99155227)
    check("ns_per_op round trip", 1000.0 / ns_per_op(7.0), 7.0)

    # --- duty cycle -------------------------------------------------------
    # A 1 Mops/s writer against a 226.142 ns insert holds the lock 22.614% of
    # the time: 1e6 * 226.142e-9.
    duty = writer_duty_cycle(1e6, 226.14201718)
    check("writer_duty_cycle(1e6)", duty, 0.22614201718)
    check("writer_duty_cycle(0)", writer_duty_cycle(0.0, 226.142), 0.0)

    # --- the ceiling ------------------------------------------------------
    read_ns = 263.99155227
    # locked = 1.0: (1 - 0.226142) / 263.99155e-9 = 2.93137e6 ops/s, and that
    # is BELOW the single-threaded 3.788 Mops/s -- the prediction that makes
    # H1 worth measuring.
    ceiling = exclusive_read_ceiling_ops(read_ns, 1.0, duty)
    check("exclusive_read_ceiling_ops(locked=1)", ceiling, 2931372.0, tol=1e-5)
    if not ceiling < 3.788e6:
        fails.append("ceiling at locked=1 is not below the single-threaded rate")

    # --- the knee and the scaling shape -----------------------------------
    check("knee(1.0)", knee(1.0, duty), 0.77385798282)
    check("knee(0.1)", knee(0.1, duty), 7.7385798282)
    # Saturated below one reader: adding readers adds nothing at all.
    check("exclusive_scaling(8, locked=1)", exclusive_scaling(8, 1.0, duty), 1.0)
    check("exclusive_scaling(2, locked=1)", exclusive_scaling(2, 1.0, duty), 1.0)
    # Knee inside the sweep: S saturates at K.
    check("exclusive_scaling(8, locked=0.1)", exclusive_scaling(8, 0.1, duty), 7.7385798282)
    check("exclusive_scaling(4, locked=0.1)", exclusive_scaling(4, 0.1, duty), 4.0)
    # Knee past the sweep: linear.
    check("exclusive_scaling(8, locked=0.05)", exclusive_scaling(8, 0.05, duty), 8.0)
    check("exclusive_scaling(1, locked=0.5)", exclusive_scaling(1, 0.5, duty), 1.0)
    # Monotone non-decreasing in W, and never superlinear.
    for f in FRACTIONS:
        prev = 0.0
        for w in READER_COUNTS:
            s = exclusive_scaling(w, f, duty)
            if s < prev - 1e-12:
                fails.append(f"exclusive_scaling not monotone at locked={f}, W={w}")
            if s > w + 1e-9:
                fails.append(f"exclusive_scaling superlinear at locked={f}, W={w}")
            prev = s

    # --- the shared-lock twin --------------------------------------------
    # locked = 1: each reader is capped at K = 0.7739 of its solo rate, and the
    # caps add -- 8 * 0.773858 / 263.99155e-9 = 23.451e6 ops/s.
    check("shared_read_throughput_ops(8, locked=1)",
          shared_read_throughput_ops(8, read_ns, 1.0, duty), 23450976.0, tol=1e-5)
    # locked = 0.5: K = 1.548 > 1, so no reader is capped at all and the curve
    # is plain W/read_ns. This is the cell the earlier (1 - duty)-everywhere
    # spelling got wrong, understating the design it represents.
    check("shared_read_throughput_ops(8, locked=0.5)",
          shared_read_throughput_ops(8, read_ns, 0.5, duty), 8.0 / (read_ns * 1e-9), tol=1e-9)
    check("shared_scaling(8, locked=1)", shared_scaling(8, 1.0, duty), 8.0)
    check("shared_scaling(8, locked=0.05)", shared_scaling(8, 0.05, duty), 8.0)
    # Shared is never worse than exclusive at any cell: it relaxes a constraint.
    for f in FRACTIONS:
        for w in READER_COUNTS:
            sh = shared_read_throughput_ops(w, read_ns, f, duty)
            ex = exclusive_read_throughput_ops(w, read_ns, f, duty)
            if sh < ex - 1e-6:
                fails.append(f"shared below exclusive at locked={f}, W={w}: {sh} < {ex}")
    # The gap is large where the lock covers a read and small where it does not
    # -- that dependence is the point, and asserting it both ways stops the
    # twin collapsing back into a constant factor.
    wide = (shared_read_throughput_ops(8, read_ns, 1.0, duty)
            / exclusive_read_throughput_ops(8, read_ns, 1.0, duty))
    narrow = (shared_read_throughput_ops(8, read_ns, 0.05, duty)
              / exclusive_read_throughput_ops(8, read_ns, 0.05, duty))
    check("D2 gain at locked=1", wide, 8.0)
    check("D2 gain at locked=0.05", narrow, 1.0)
    # The middle regime the docstring names: `W / K` while `1 <= K <= W`.
    k_mid = knee(0.25, duty)
    mid = (shared_read_throughput_ops(8, read_ns, 0.25, duty)
           / exclusive_read_throughput_ops(8, read_ns, 0.25, duty))
    if not 1.0 <= k_mid <= 8.0:
        fails.append(f"locked=0.25 is not in the middle regime at this duty (K={k_mid})")
    check("D2 gain at locked=0.25 is W/K", mid, 8.0 / k_mid)

    # --- the idle-writer control cell -------------------------------------
    # With no writer at all, a fully-locked read path still does not scale:
    # K = 1, so S(W) = 1 at every W. That is reader-vs-reader serialisation
    # with nothing to blame it on, and it is why the sweep needs a writer-idle
    # control -- without it, a flat curve cannot be attributed to the writer
    # rather than to the lock itself, and D1 and D2 cannot be told apart.
    check("knee(1.0, duty=0)", knee(1.0, 0.0), 1.0)
    check("exclusive_scaling(8, locked=1, duty=0)", exclusive_scaling(8, 1.0, 0.0), 1.0)
    check("shared_scaling(8, locked=1, duty=0)", shared_scaling(8, 1.0, 0.0), 8.0)
    check("idle-writer ceiling equals the solo rate",
          exclusive_read_ceiling_ops(read_ns, 1.0, 0.0), 1e9 / read_ns, tol=1e-9)

    # --- the inverse ------------------------------------------------------
    lo, hi = implied_locked_fraction(1.0, 8, duty)
    check("implied(S=1) lower", lo, 0.77385798282)
    check("implied(S=1) upper", hi, 1.0)
    lo, hi = implied_locked_fraction(2.0, 8, duty)
    check("implied(S=2) point lower", lo, 0.38692899141)
    check("implied(S=2) point upper", hi, 0.38692899141)
    lo, hi = implied_locked_fraction(8.0, 8, duty)
    check("implied(S=W) lower", lo, 0.0)
    check("implied(S=W) upper", hi, 0.09673224785)
    # Round trip through the informative region.
    for s in (1.5, 2.0, 4.0, 7.0):
        f_lo, f_hi = implied_locked_fraction(s, 8, duty)
        check(f"round trip S={s}", exclusive_scaling(8, f_lo, duty), s)
        check(f"round trip bounds coincide S={s}", f_lo, f_hi)

    # --- the USL bridge ---------------------------------------------------
    # alpha = 1 / max(K, 1), and must stay admissible (0, 1] at every cell --
    # fit_usl.py treats alpha > 1 as REFUTED, so an inadmissible prediction
    # would be this derivation's bug, not a finding.
    check("predicted_usl_alpha(locked=1)", predicted_usl_alpha(1.0, duty), 1.0)
    check("predicted_usl_alpha(locked=0.25)", predicted_usl_alpha(0.25, duty),
          1.0 / knee(0.25, duty))
    check("predicted_usl_alpha(locked=0.1)", predicted_usl_alpha(0.1, duty),
          1.0 / knee(0.1, duty))
    for f in FRACTIONS:
        for d in (0.0, duty, 0.9):
            a = predicted_usl_alpha(f, d)
            if not (0.0 < a <= 1.0):
                fails.append(f"predicted_usl_alpha inadmissible at locked={f}, duty={d}: {a}")
    # It must agree with the asymptote of the scaling curve it is derived from:
    # at a reader count past the knee, S(W) is the ratio ceiling and alpha its
    # reciprocal. Checked at a locked fraction whose knee sits inside the sweep.
    big = 4096
    check("alpha matches the scaling asymptote",
          predicted_usl_alpha(0.25, duty), 1.0 / exclusive_scaling(big, 0.25, duty))
    # A fully serialised read path is alpha = 1 whatever the writer does.
    check("predicted_usl_alpha(locked=1, duty=0)", predicted_usl_alpha(1.0, 0.0), 1.0)

    # --- detectability ----------------------------------------------------
    check("min_detectable_ratio(0)", min_detectable_ratio(0.0), 1.0)
    check("min_detectable_ratio(0.06)", min_detectable_ratio(0.06), 1.06382978723)
    # The suite's tightest published arm is readrandom at
    # 3.788 [3.756, 3.812] -- a relative half-width of 0.74%. Even at ten times
    # that, the H1 threshold of 2.0 is far above the detection floor.
    if not min_detectable_ratio(0.10) < 2.0:
        fails.append("H1's threshold of 2.0 is not above the detection floor at h=10%")

    # --- fail-loud input validation ---------------------------------------
    for name, fn in (
        ("ns_per_op(0)", lambda: ns_per_op(0.0)),
        ("ns_per_op(-1)", lambda: ns_per_op(-1.0)),
        ("ns_per_op(inf)", lambda: ns_per_op(float("inf"))),
        ("duty >= 1", lambda: writer_duty_cycle(5e6, 226.142)),
        ("duty exactly 1", lambda: writer_duty_cycle(1e9 / 226.142, 226.142)),
        ("negative writer rate", lambda: writer_duty_cycle(-1.0, 226.142)),
        ("locked = 0", lambda: exclusive_scaling(8, 0.0, 0.2)),
        ("locked > 1", lambda: exclusive_scaling(8, 1.5, 0.2)),
        ("readers = 0", lambda: exclusive_scaling(0, 0.5, 0.2)),
        ("readers bool", lambda: exclusive_scaling(True, 0.5, 0.2)),
        ("superlinear observation", lambda: implied_locked_fraction(9.0, 8, 0.2)),
        ("halfwidth >= 1", lambda: min_detectable_ratio(1.0)),
        ("usl alpha, locked = 0", lambda: predicted_usl_alpha(0.0, 0.2)),
        ("usl alpha, duty = 1", lambda: predicted_usl_alpha(0.5, 1.0)),
    ):
        try:
            fn()
        except ValueError:
            pass
        except Exception as exc:  # noqa: BLE001
            fails.append(f"{name}: raised {type(exc).__name__}, expected ValueError")
        else:
            fails.append(f"{name}: did not raise")

    # --- gate boundaries under a writer that runs short (#802 amendment) ---
    # The paced writer's offered duty in METHODOLOGY section 5.2: 250,000
    # inserts/s x the section 2 insert cell (1000/4.422 ns).
    offered = writer_duty_cycle(250_000.0, 226.14201718)
    check("offered duty at 250k/s", offered, 0.0565355043)
    check("H1 boundary at offered duty", gate_boundary_locked_fraction(2.0, offered), 0.4717322479)
    check("H1 boundary, writer stalled", gate_boundary_locked_fraction(2.0, 0.0), 0.5)
    check("H1 max boundary shift", max_gate_boundary_shift(2.0, offered), 0.0282677521)
    # The shift is the difference of the two ends, and every duty in between
    # lands inside them: monotone, so no interior shortfall exceeds the bound.
    for k in range(0, 101):
        d = offered * k / 100
        b = gate_boundary_locked_fraction(2.0, d)
        if not (gate_boundary_locked_fraction(2.0, offered) - 1e-12 <= b <= 0.5 + 1e-12):
            fails.append(f"H1 boundary at duty {d!r} left [boundary(offered), 0.5]: {b!r}")
    # The boundary is exactly the point `implied_locked_fraction` returns for
    # an `S` equal to the threshold, so the two functions describe one decision.
    for d in (0.0, 0.0319, 0.0484, offered):
        lo, hi = implied_locked_fraction(2.0, 7, d)
        check(f"boundary == implied_locked_fraction(2.0, 7, {d})", gate_boundary_locked_fraction(2.0, d), lo)
    for bad in ((1.0, 0.0), (0.5, 0.0), (float("inf"), 0.0), (2.0, 1.0), (2.0, -0.1)):
        try:
            gate_boundary_locked_fraction(*bad)
        except ValueError:
            pass
        else:
            fails.append(f"gate_boundary_locked_fraction{bad}: did not raise")

    # alpha from a measured S carries no duty: pinned by composing the two
    # existing functions, not by restating the closed form.
    for s in (0.62, 1.0, 1.5, 2.0, 6.0, 7.0):
        want = predicted_alpha_from_scaling(s, 7)
        for d in (0.0, 0.0319, 0.0484, offered, 0.5):
            lo, hi = implied_locked_fraction(s, 7, d)
            lf = lo if s < 7 else hi
            check(f"alpha(S={s}) at duty {d} via the composition", predicted_usl_alpha(lf, d), want)
    check("alpha from a saturated S", predicted_alpha_from_scaling(0.62, 7), 1.0)
    check("alpha from S = 2.0", predicted_alpha_from_scaling(2.0, 7), 0.5)
    check("alpha from a linear S", predicted_alpha_from_scaling(7.0, 7), 1.0 / 7.0)
    for bad in ((0.0, 7), (8.0, 7), (float("inf"), 7), (2.0, 0)):
        try:
            predicted_alpha_from_scaling(*bad)
        except ValueError:
            pass
        else:
            fails.append(f"predicted_alpha_from_scaling{bad}: did not raise")

    # --- the loader, structurally -----------------------------------------
    # Pinned by shape, not by value: a re-measurement must re-derive the
    # prediction, and must not break this test into meaninglessness.
    try:
        arms = load_arms()
    except Exception as exc:  # noqa: BLE001
        fails.append(f"load_arms(): raised {type(exc).__name__}: {exc}")
    else:
        for key in ("insert_ns", "read_ns", "insert_mops", "read_mops", "commit", "host"):
            if key not in arms:
                fails.append(f"load_arms(): missing key {key}")
        if arms.get("insert_ns", 0) <= 0 or arms.get("read_ns", 0) <= 0:
            fails.append("load_arms(): non-positive latency derived from the artifact")
        # The render path must not raise on the artifact as committed.
        try:
            text = render(arms, 1e6)
        except Exception as exc:  # noqa: BLE001
            fails.append(f"render(): raised {type(exc).__name__}: {exc}")
        else:
            if "locked" not in text or "shared" not in text:
                fails.append("render(): output is missing the locked sweep or the shared twin")

    try:
        load_arms(Path("/nonexistent/baseline.json"))
    except FileNotFoundError:
        pass
    else:
        fails.append("load_arms() on a missing file: did not raise FileNotFoundError")

    # --- both artifact shapes, and neither ---------------------------------
    # The suite's artifact moves its published arms from `arms` to `cells` when
    # it is re-measured through the #868 driver, because the provenance gate
    # requires one of its own cell keys. Reading exactly one of the two spellings
    # would break this derivation at the re-measurement, silently on whichever
    # side was not covered -- so both are pinned here, against identical
    # contents, and an artifact carrying neither must still raise.
    import tempfile  # noqa: PLC0415 - self-test only

    _arm_rows = [
        {"id": INSERT_ARM, "unit": "Mops_per_second", "n": 5,
         "point": 4.0, "ci_lower": 3.9, "ci_upper": 4.1},
        {"id": READ_ARM, "unit": "Mops_per_second", "n": 5,
         "point": 2.0, "ci_lower": 1.9, "ci_upper": 2.1},
    ]
    with tempfile.TemporaryDirectory() as td:
        shapes = {}
        for key in ("arms", "cells"):
            p = Path(td) / f"{key}.json"
            p.write_text(json.dumps({key: _arm_rows,
                                     "provenance": {"commit": "abc1234"}}))
            shapes[key] = load_arms(p)
        if shapes["arms"] != shapes["cells"]:
            fails.append(f"load_arms() reads `arms` and `cells` differently: "
                         f"{shapes['arms']} vs {shapes['cells']}")
        check("load_arms reads the cells shape", shapes["cells"]["insert_ns"], 250.0)
        empty = Path(td) / "neither.json"
        empty.write_text(json.dumps({"provenance": {}}))
        try:
            load_arms(empty)
        except KeyError:
            pass
        else:
            fails.append("load_arms() on an artifact with neither key: did not raise")

    # --- the narrowed-mutex arm: USL with an alpha fixed from outside ------
    check("usl_scaling(1, any)", usl_scaling(1, 0.7, 3.0), 1.0)
    check("usl_scaling(7, 0, 0) is linear", usl_scaling(7, 0.0, 0.0), 7.0)
    check("usl_scaling(7, 1, 0) is flat", usl_scaling(7, 1.0, 0.0), 1.0)
    # 2 / (1 + 0.5 * 1 + 0.1 * 2 * 1) = 2 / 1.7
    check("usl_scaling(2, 0.5, 0.1)", usl_scaling(2, 0.5, 0.1), 1.1764705882)
    # alpha 0.3, beta 0.2 at W = 7: 7 / (1 + 1.8 + 8.4) = 7 / 11.2 = 0.625.
    check("usl_scaling(7, 0.3, 0.2)", usl_scaling(7, 0.3, 0.2), 0.625)
    # ...and the term that curve needs beyond alpha = 0.3 is beta again:
    # (7 / 0.625 - 1 - 1.8) / 42 = 8.4 / 42 = 0.2.
    check("unexplained_term inverts usl_scaling", unexplained_term(0.625, 7, 0.3), 0.2)
    for w, a, b in ((2, 0.1, 0.05), (4, 0.6, 0.0), (7, 0.25, 1.3)):
        check(f"round trip W={w} alpha={a} beta={b}",
              unexplained_term(usl_scaling(w, a, b), w, a), b, tol=1e-9)
    # A linear curve against alpha = 0.5 needs a negative term.
    if not unexplained_term(7.0, 7, 0.5) < 0.0:
        fails.append("unexplained_term: alpha over-explaining S must give a negative term")
    # Interval over S in [0.611, 0.631] at alpha 0.3:
    # (7/0.631 - 2.8)/42 = 0.1974643 and (7/0.611 - 2.8)/42 = 0.2061102.
    lo, hi = unexplained_term_interval((0.611, 0.631), 7, (0.3, 0.3))
    check("unexplained_term_interval lower", lo, 0.1974643, tol=1e-6)
    check("unexplained_term_interval upper", hi, 0.2061102, tol=1e-6)
    lo2, hi2 = unexplained_term_interval((0.611, 0.631), 7, (0.25, 0.35))
    if not (lo2 < lo and hi2 > hi):
        fails.append("a wider alpha interval must widen the term interval at both ends")
    # --- the directional gate's detectability --------------------------------
    check("paired half-width 3-4-5", paired_ratio_relative_halfwidth(0.03, 0.04), 0.05)
    check("paired half-width of one exact arm", paired_ratio_relative_halfwidth(0.02, 0.0), 0.02)
    # composed with the lower-bound rule: h = 0.05 clears 1 only above 1 / 0.95.
    check("detectable paired ratio at h = 0.05",
          min_detectable_ratio(paired_ratio_relative_halfwidth(0.03, 0.04)), 1.0526315789)
    for name, call in (("negative half-width", lambda: paired_ratio_relative_halfwidth(-0.01, 0.02)),
                       ("half-width of 1", lambda: paired_ratio_relative_halfwidth(0.02, 1.0))):
        try:
            call()
        except ValueError:
            pass
        else:
            fails.append(f"{name}: did not raise")

    # --- section 5.14: the directional gate and its admissibility check ----------
    check("both lower bounds above 1", directional_verdict([(1.01, 1.05), (1.002, 1.2)]), "PASS")
    check("both upper bounds below 1", directional_verdict([(0.9, 0.99), (0.8, 0.999)]), "REFUTED")
    check("one run contains 1", directional_verdict([(1.01, 1.05), (0.99, 1.03)]), "BOUNDARY_RESULT")
    check("runs on opposite sides", directional_verdict([(1.01, 1.05), (0.9, 0.95)]), "BOUNDARY_RESULT")
    check("a bound exactly at 1 does not pass", directional_verdict([(1.0, 1.05), (1.01, 1.05)]), "BOUNDARY_RESULT")
    for name, call in (("one run", lambda: directional_verdict([(1.01, 1.05)])),
                       ("unordered interval", lambda: directional_verdict([(1.05, 1.01), (1.01, 1.05)]))):
        try:
            call()
        except ValueError:
            pass
        else:
            fails.append(f"{name}: did not raise")
    good_run = {
        "provenance": {"core_pin": "0-15", "host": {"scaling_governor_pin_source": "EXPANSE_BENCH_PIN_APPLIED"}},
        "settings": {"window_seconds": 2.0, "paced_rate_ops_per_s": 250000.0, "readers": [1, 2, 4, 7],
                     "modes": ["idle", "paced"], "lock_scopes": ["full", "trie"]},
        "lock_scope_ratio": {m: {k: {"point": 1.1, "ci": [1.05, 1.15], "n": 5} for k in NARROWED_RATIOS}
                             for m in ("idle", "paced")},
    }
    check("an admissible section 5.14 run", narrowed_problems(good_run, "0-15"), [])
    got = narrowed_problems(good_run, "0,2,4,6,8,10,12,14")
    if len(got) != 1 or "core_pin" not in got[0]:
        fails.append(f"narrowed_problems on the wrong pin: {got!r}")
    one_scope = json.loads(json.dumps(good_run))
    one_scope["settings"]["lock_scopes"] = ["full"]
    del one_scope["lock_scope_ratio"]["paced"]["S(7)"]
    got = narrowed_problems(one_scope, "0-15")
    if not any("lock_scopes" in g for g in got) or not any("paced S(7)" in g for g in got):
        fails.append(f"narrowed_problems must name the missing scope and ratio: {got!r}")

    # --- section 5.12: the queue, the candidates and the acceptance rule ----
    check("queue_scaling(1) is 1", queue_scaling(1, 0.4, 3.0, 1.0), 1.0)
    # A lock that is the whole read, with no handoff, serialises exactly: S = 1.
    check("queue_scaling saturated", queue_scaling(7, 1.0, 0.0, 0.0), 1.0)
    # f = 0.5, W = 2, no handoff: R(1) = 0.5, Q(1) = 0.5; R(2) = 0.5 * 1.5 = 0.75,
    # X(2) = 2 / (0.5 + 0.75) = 1.6.
    check("queue_scaling(2, 0.5, 0)", queue_scaling(2, 0.5, 0.0), 1.6)
    # handoff 0.5: s(2) = 1.0, R(2) = 1.5, X(2) = 2 / 2.0 = 1.0.
    check("queue_scaling(2, 0.5, 0.5)", queue_scaling(2, 0.5, 0.5), 1.0)
    # growth 0.25 at W = 3: Q(2) = 1.0 * 1.5 = 1.5; s(3) = 1.25, R(3) = 3.125,
    # X(3) = 3 / 3.625.
    check("queue_scaling(3, 0.5, 0.5, 0.25)", queue_scaling(3, 0.5, 0.5, 0.25), 0.8275862069)
    for name, call in (
        ("queue readers 0", lambda: queue_scaling(0, 0.5, 0.0)),
        ("queue locked_fraction 0", lambda: queue_scaling(3, 0.0, 0.0)),
        ("queue negative handoff", lambda: queue_scaling(3, 0.5, -0.1)),
        ("queue infinite growth", lambda: queue_scaling(3, 0.5, 0.1, math.inf)),
    ):
        try:
            call()
        except ValueError:
            pass
        else:
            fails.append(f"{name}: did not raise")

    def synthetic(shape, hw=0.005, shift=None):
        c = {w: (shape(w), shape(w) - hw, shape(w) + hw) for w in range(2, 8)}
        for w, d in (shift or {}).items():
            s = c[w][0] + d
            c[w] = (s, s - hw, s + hw)
        return c

    truth = synthetic(lambda w: queue_scaling(w, 0.5, 0.9, 0.04))
    ch = check_candidate("queue_handoff_growing", truth, 0.5)
    check("fit recovers handoff", round(ch["params"]["handoff"], 3), 0.9)
    check("fit recovers growth", round(ch["params"]["growth"], 3), 0.04)
    check("a shape checked on its own curve is accepted", ch["accepted"], True)
    check("every reader count is checked", sorted(ch["readers"]), [2, 3, 4, 5, 6, 7])
    check("held-out rows are labelled", sorted(w for w, r in ch["readers"].items()
                                               if r["role"] == "held_out"), [3, 5, 6])
    # The rule reads the held-out points: shift only S(5) out of reach, leave the
    # training points exact, and the same fit must be refused on R = 5 alone.
    moved = check_candidate("queue_handoff_growing",
                            synthetic(lambda w: queue_scaling(w, 0.5, 0.9, 0.04), shift={5: 0.03}), 0.5)
    check("a held-out miss refuses the shape", (moved["accepted"], moved["misses"]), (False, [5]))
    # A one-parameter queue cannot follow a power-law step (hand check: its
    # S(5) and S(6) differ by under 0.001, the step's by 0.017).
    step = synthetic(lambda w: 0.76 * (w / 2.0) ** -0.15)
    check("queue_handoff refused on a power-law step",
          check_candidate("queue_handoff", step, 0.5)["accepted"], False)
    check("step_power accepted on its own curve", check_candidate("step_power", step, None)["accepted"], True)
    # A curve whose 1/S climbs faster than b = 1 allows pins b at its search limit.
    steep = {2: (0.3, 0.29, 0.31), 4: (0.1, 0.095, 0.105), 7: (0.05, 0.045, 0.055)}
    lim = check_candidate("reciprocal_linear", steep, None, readers=TRAINING_READERS)
    check("a search-limit fit is named", lim["search_limit"], ["b"])
    check("a search-limit fit is refused", lim["accepted"], False)
    # usl_free may rest on alpha = 1, a physical limit, and is not refused for it.
    flat = synthetic(lambda w: usl_scaling(w, 1.0, 0.02))
    check("usl_free at alpha = 1 is not a search limit",
          check_candidate("usl_free", flat, None)["search_limit"], [])
    for name, call in (
        ("mechanistic without a fraction", lambda: fit_candidate("queue_handoff", truth, None)),
        ("missing training point", lambda: fit_candidate("step_power", {2: truth[2], 4: truth[4]}, None)),
        ("zero-width interval", lambda: fit_candidate("step_power", {**truth, 4: (0.7, 0.7, 0.7)}, None)),
        ("missing held-out point", lambda: check_candidate("step_power", {w: truth[w] for w in (2, 4, 7)}, None)),
        ("one run", lambda: model_verdicts([truth], (0.49, 0.5, 0.51))),
        ("unordered span", lambda: model_verdicts([truth, truth], (0.52, 0.5, 0.51))),
    ):
        try:
            call()
        except ValueError:
            pass
        else:
            fails.append(f"{name}: did not raise")
    # Both runs: a shape accepted on one run and refused on the other is REJECTED.
    v = model_verdicts([truth, synthetic(lambda w: queue_scaling(w, 0.5, 0.9, 0.04), shift={3: 0.03})],
                       (0.5, 0.5, 0.5))
    check("accepted on one run of two is rejected", v["queue_handoff_growing"]["verdict"], "REJECTED")
    v = model_verdicts([truth, truth], (0.5, 0.5, 0.5))
    check("accepted on both runs", v["queue_handoff_growing"]["verdict"], "ACCEPTED")
    # A mechanistic shape must hold across the profile's span: at f = 0.9 the
    # queue's S(2) cannot reach a curve drawn at f = 0.5 with no handoff.
    v = model_verdicts([truth, truth], (0.5, 0.5, 0.9))
    check("a shape refused at one end of the span is rejected",
          v["queue_handoff_growing"]["verdict"], "REJECTED")

    def fake(mech, k, chi2, verdict="ACCEPTED"):
        return {"verdict": verdict, "mechanistic": mech, "n_params": k, "held_out_chi2": chi2}
    check("mechanistic before descriptive, whatever the parameter count",
          select_model({"d": fake(False, 1, 0.1), "m": fake(True, 2, 9.0)}), "m")
    check("fewer parameters next", select_model({"a": fake(True, 2, 0.1), "b": fake(True, 1, 5.0)}), "b")
    check("then the lower held-out chi-square",
          select_model({"a": fake(True, 1, 3.0), "b": fake(True, 1, 2.0)}), "b")
    check("nothing accepted selects nothing",
          select_model({"a": fake(True, 1, 0.0, "REJECTED")}), None)
    if "directional gate" not in model_consequence({"a": fake(False, 2, 1.0)}):
        fails.append("a descriptive-only pass must fall back to the directional gate")
    if "directional gate" not in model_consequence({"a": fake(True, 2, 1.0, "REJECTED")}):
        fails.append("no pass must fall back to the directional gate")
    if "derived from it" not in model_consequence({"a": fake(True, 2, 1.0)}):
        fails.append("a mechanistic pass must carry the prediction forward")
    # Separation pairs exactly the shapes whose training fit is inside, and the
    # gap is recomputed here from check_candidate's own held-out predictions.
    wide = synthetic(lambda w: 0.76 * (w / 2.0) ** -0.15, hw=0.03)
    fitted = {}
    for name, spec in MODEL_CANDIDATES.items():
        c = check_candidate(name, wide, 0.5 if spec["mechanistic"] else None)
        if not c["search_limit"] and all(c["readers"][w]["inside"] for w in TRAINING_READERS):
            fitted[name] = [c["readers"][w]["predicted"] for w in HELD_OUT_READERS]
    pairs = held_out_separation(wide, 0.5)
    names = sorted(fitted)
    check("separation pairs exactly the shapes that fit", sorted((a, b) for a, b, _ in pairs),
          [(a, b) for i, a in enumerate(names) for b in names[i + 1:]])
    if not pairs:
        fails.append("the separation check needs at least one pair of fitted shapes")
    for a, b, gap in (p for p in pairs if p[0] in fitted and p[1] in fitted):
        check(f"separation {a} vs {b}", gap,
              max(abs(x - y) for x, y in zip(fitted[a], fitted[b])) / 0.03, tol=1e-9)
    # held_out_problems reads the keys the driver writes: a section 5.10 artifact
    # carries the verified pin, and is refused for its modes and its missing R.
    amended = json.loads(COMMITTED_IDLE_CURVES[2].read_text())
    got = held_out_problems(amended)
    if any("pin" in g for g in got) or not any("modes" in g for g in got) \
            or not any("S(3)" in g for g in got):
        fails.append(f"held_out_problems on a section 5.10 artifact: {got!r}")
    for name, call in (
        ("W = 1 carries no term", lambda: unexplained_term(1.0, 1, 0.3)),
        ("alpha above 1", lambda: usl_scaling(7, 1.2, 0.0)),
        ("unordered interval", lambda: unexplained_term_interval((0.7, 0.6), 7, (0.3, 0.3))),
    ):
        try:
            call()
        except ValueError:
            pass
        else:
            fails.append(f"{name}: did not raise")
    # --- section 5.16 sizing ----------------------------------------------
    # Student t 0.975 quantiles, against the standard table (Abramowitz &
    # Stegun Table 26.10): df 1, 4, 9, 29.
    check("t(0.975, 1)", student_t_quantile(0.975, 1), 12.7062047, tol=1e-6)
    check("t(0.975, 4)", student_t_quantile(0.975, 4), 2.7764451, tol=1e-6)
    check("t(0.975, 9)", student_t_quantile(0.975, 9), 2.2621572, tol=1e-6)
    check("t(0.975, 29)", student_t_quantile(0.975, 29), 2.0452296, tol=1e-6)
    check("t cdf is symmetric", student_t_cdf(-1.5, 7), 1.0 - student_t_cdf(1.5, 7), tol=1e-12)
    check("t cdf at 0", student_t_cdf(0.0, 3), 0.5, tol=1e-12)
    # per_round_cv by hand: [1, 2, 3] has mean 2 and n-1 sd 1.
    check("per_round_cv([1, 2, 3])", per_round_cv([1.0, 2.0, 3.0]), 0.5, tol=1e-12)
    # 2.7764451 * 0.05 / sqrt(5) = 0.0620832...
    check("sizing_relative_halfwidth(0.05, 5)", sizing_relative_halfwidth(0.05, 5),
          2.7764451 * 0.05 / math.sqrt(5.0), tol=1e-6)
    n = rounds_for_detectable_ratio(0.05, 1.05, min_rounds=5)
    if not (min_detectable_ratio(sizing_relative_halfwidth(0.05, n)) <= 1.05
            < min_detectable_ratio(sizing_relative_halfwidth(0.05, n - 1))):
        fails.append(f"rounds_for_detectable_ratio(0.05, 1.05) = {n} is not the fewest")
    check("rounds floor holds", rounds_for_detectable_ratio(0.001, 1.05, min_rounds=5), 5)
    for name, call in (
        ("per_round_cv of one round", lambda: per_round_cv([1.0])),
        ("sizing below 3 rounds", lambda: sizing_relative_halfwidth(0.05, 2)),
        ("target at 1", lambda: rounds_for_detectable_ratio(0.05, 1.0)),
        ("unreachable target", lambda: rounds_for_detectable_ratio(5.0, 1.0001, max_rounds=10)),
    ):
        try:
            call()
        except ValueError:
            pass
        else:
            fails.append(f"{name}: did not raise")
    # optimistic_gate_sizing takes the larger measured-or-projected spread for
    # opt/full and the trie-trie projection for opt/trie, over every run.
    runs = [{"name": "a", "rounds": 5, "cv_full": 0.03, "cv_trie": 0.04, "cv_trie_over_full": 0.06},
            {"name": "b", "rounds": 5, "cv_full": 0.01, "cv_trie": 0.05, "cv_trie_over_full": 0.02}]
    sz = optimistic_gate_sizing(runs)
    check("opt/full planning cv", sz["ratios"]["opt/full"]["cv"], 0.06, tol=1e-12)
    check("opt/trie planning cv", sz["ratios"]["opt/trie"]["cv"], math.hypot(0.05, 0.05), tol=1e-12)
    # O1 alone sets the rounds; O2's wider spread must not.
    check("rounds is the gated ratio's", sz["rounds"], sz["ratios"]["opt/full"]["rounds_for_target"])
    if not sz["ratios"]["opt/trie"]["rounds_for_target"] > sz["rounds"]:
        fails.append("the synthetic runs must make O2 need more rounds than O1, or the check above pins nothing")
    check("O2 reported at O1's rounds", sz["ratios"]["opt/trie"]["halfwidth_at_rounds"],
          sizing_relative_halfwidth(math.hypot(0.05, 0.05), sz["rounds"]), tol=1e-12)
    # The committed sizing section 5.16 cites: 24 rounds for O1 at the 1.05 target.
    committed_sizing = optimistic_gate_sizing([optimistic_sizing_inputs(p) for p in OPTIMISTIC_SIZING_SOURCES])
    check("section 5.16 rounds", committed_sizing["rounds"], 24)
    # The committed section 5.15 artifacts: every round is read.
    committed = [optimistic_sizing_inputs(p) for p in OPTIMISTIC_SIZING_SOURCES]
    check("section 5.15 runs read", len(committed), 4)
    for r, p in zip(committed, OPTIMISTIC_SIZING_SOURCES):
        raw = json.loads(p.read_text())["lock_scope_ratio"]["idle"]["S(7)"]["rounds_raw"]
        check(f"{r['name']} rounds, unfiltered", r["rounds"], len(raw))
        check(f"{r['name']} paired cv", r["cv_trie_over_full"], per_round_cv(raw), tol=1e-12)
    import tempfile as _tempfile  # noqa: PLC0415 - self-test only
    with _tempfile.TemporaryDirectory() as td:
        bad = {"scaling_by_lock_scope": {"full": {"idle": {"S(7)": {"rounds_raw": [0.6, 0.61, 0.62]}}},
                                         "trie": {"idle": {"S(7)": {"rounds_raw": [1.1, 1.0]}}}},
               "lock_scope_ratio": {"idle": {"S(7)": {"rounds_raw": [1.8, 1.7, 1.9]}}}}
        bp = Path(td) / "short.json"
        bp.write_text(json.dumps(bad))
        try:
            optimistic_sizing_inputs(bp)
        except ValueError:
            pass
        else:
            fails.append("optimistic_sizing_inputs accepted scopes with different round counts")

    # load_locate_profile reads points and intervals, and refuses a share
    # without an interval.
    with tempfile.TemporaryDirectory() as td:
        good = {"provenance": {"commit": "abc12345", "pin": ["taskset", "-c", "0,2,4"]},
                "shares": {"locked_fraction": {"point": 0.3, "ci_lower": 0.29, "ci_upper": 0.31},
                           "trie_fraction": {"point": 0.05, "ci_lower": 0.04, "ci_upper": 0.06}}}
        gp = Path(td) / "profile.json"
        gp.write_text(json.dumps(good))
        got = load_locate_profile(gp)
        check("load_locate_profile locked_fraction", got["locked_fraction"], (0.3, 0.29, 0.31))
        check("load_locate_profile pin", got["pin"], "0,2,4")
        bad = json.loads(json.dumps(good))
        bad["shares"]["trie_fraction"]["ci_lower"] = None
        bp = Path(td) / "bad.json"
        bp.write_text(json.dumps(bad))
        try:
            load_locate_profile(bp)
        except ValueError:
            pass
        else:
            fails.append("load_locate_profile: a share without an interval was accepted")

    if fails:
        print("rocksdb_locate_bound.py --self-test: FAILED")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("rocksdb_locate_bound.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--writer-ops", type=float, default=1e6,
                    help="writer's offered insert rate, ops/s (default 1e6)")
    ap.add_argument("--artifact", type=Path, default=ARTIFACT,
                    help="suite artifact to read insert/read latency from")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    print(render(load_arms(args.artifact), args.writer_ops))
    return 0


if __name__ == "__main__":
    sys.exit(main())
