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

Usage:
    python3 scripts/rocksdb_locate_bound.py
    python3 scripts/rocksdb_locate_bound.py --writer-ops 1e6
    python3 scripts/rocksdb_locate_bound.py --self-test
"""

from __future__ import annotations

import argparse
import json
import math
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


def narrowed_scaling_bracket(readers: int, alpha_now: float, alpha_narrowed: float,
                             term: float) -> tuple[float, float]:
    """`(pessimistic, optimistic)` `S(W)` for a lock that covers less of a read.

    The narrowed lock changes `alpha` from `alpha_now` to `alpha_narrowed`, the
    share of a read it still covers. What it does to the unexplained term is
    not derivable here, so two bounding assumptions stand in for it:

    - pessimistic: the term does not depend on how long the lock is held, as a
      fixed cost per acquisition would not;
    - optimistic: it scales with the hold time, `term * alpha_narrowed / alpha_now`.

    Neither assumption is measured. The bracket is what can be predicted without
    naming the term, and an outcome outside it says the term behaves like
    neither. A negative `term` is refused: `alpha_now` already over-explains
    the curve, and no bracket follows from a model that does.
    """
    if term < 0.0:
        raise ValueError(f"the unexplained term is {term!r}: alpha alone already over-explains "
                         f"S, so this model does not describe the curve")
    if not (0.0 < alpha_now <= 1.0):
        raise ValueError(f"alpha_now must be in (0, 1], got {alpha_now!r}")
    if not (0.0 <= alpha_narrowed <= alpha_now):
        raise ValueError(f"alpha_narrowed must be in [0, alpha_now], got {alpha_narrowed!r}")
    pessimistic = usl_scaling(readers, alpha_narrowed, term)
    optimistic = usl_scaling(readers, alpha_narrowed, term * alpha_narrowed / alpha_now)
    return (pessimistic, optimistic)


def min_detectable_ratio(relative_halfwidth: float) -> float:
    """Smallest scaling ratio whose BCa lower bound clears 1.0 (AGENTS.md 8.4).

    A claim passes on its CI lower bound, not its point estimate, so a ratio
    `S` measured with relative half-width `h` is distinguishable from "no
    scaling" only when `S * (1 - h) > 1`.
    """
    if not (0.0 <= relative_halfwidth < 1.0):
        raise ValueError(f"relative_halfwidth must be in [0, 1), got {relative_halfwidth!r}")
    return 1.0 / (1.0 - relative_halfwidth)


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
IDLE_CURVES = (
    REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable" / "results" / "baseline_concurrent_reads_amended_h1.json",
    REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable" / "results" / "baseline_concurrent_reads_amended_h1_run2.json",
)


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
    lines.extend(render_narrowed())
    return "\n".join(lines)


def render_narrowed() -> list[str]:
    """The narrowed-mutex arm's bracket, where a locate profile is committed."""
    import glob  # noqa: PLC0415
    profiles = sorted(glob.glob(str(REPO_ROOT / PROFILE_GLOB)))
    out = ["", "  Narrowed-mutex arm (idle writer, one-sibling pin), (projected):"]
    if not profiles:
        out.append("    no locate profile is committed yet; dispatch the `rocksdb_locate_profile` suite")
        return out
    curves = [json.loads(p.read_text()) for p in IDLE_CURVES if p.is_file()]
    for path in profiles:
        prof = load_locate_profile(Path(path))
        lf, tf = prof["locked_fraction"], prof["trie_fraction"]
        out.append(f"    profile {Path(path).parent.name} ({prof['commit'][:8]}, pin {prof['pin']}): "
                   f"locked_fraction {lf[0]:.4f} [{lf[1]:.4f}, {lf[2]:.4f}], "
                   f"trie_fraction {tf[0]:.4f} [{tf[1]:.4f}, {tf[2]:.4f}]")
        for curve in curves:
            idle = curve["scaling"]["idle"]
            for w in (2, 4, 7):
                cell = idle[f"S({w})"]
                lo, hi = unexplained_term_interval(tuple(cell["ci"]), w, (lf[1], lf[2]))
                term = unexplained_term(cell["point"], w, lf[0])
                if term < 0.0:
                    out.append(f"      S({w}) {cell['point']:.3f}: term {term:.4f} < 0, no bracket")
                    continue
                pess, opt = narrowed_scaling_bracket(w, lf[0], tf[0], term)
                out.append(f"      S({w}) {cell['point']:.3f} -> unexplained term {term:.4f} "
                           f"[{lo:.4f}, {hi:.4f}]; narrowed S({w}) in [{pess:.3f}, {opt:.3f}]")
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
    # Bracket: alpha 0.3 -> 0.05, term 0.2, hold ratio 1/6, W = 7.
    #   pessimistic 7 / (1 + 0.3 + 8.4) = 7 / 9.7   = 0.7216495
    #   optimistic  7 / (1 + 0.3 + 1.4) = 7 / 2.7   = 2.5925926
    pess, opt = narrowed_scaling_bracket(7, 0.3, 0.05, 0.2)
    check("narrowed bracket pessimistic", pess, 0.7216495, tol=1e-6)
    check("narrowed bracket optimistic", opt, 2.5925926, tol=1e-6)
    check("no narrowing leaves both ends at today's curve",
          narrowed_scaling_bracket(7, 0.3, 0.3, 0.2), (0.625, 0.625))
    # The negative-term refusal is checked by its reason: usl_scaling also
    # rejects a negative beta, so any ValueError would pass without the
    # bracket's own refusal existing.
    try:
        narrowed_scaling_bracket(7, 0.3, 0.05, -0.01)
    except ValueError as exc:
        if "over-explains" not in str(exc):
            fails.append(f"negative term refused for the wrong reason: {exc}")
    else:
        fails.append("negative term: did not raise")
    for name, call in (
        ("narrowed alpha above today's", lambda: narrowed_scaling_bracket(7, 0.3, 0.4, 0.2)),
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
