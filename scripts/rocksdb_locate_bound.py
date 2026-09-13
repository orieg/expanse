#!/usr/bin/env python3
"""What a single mutex over the locate phase permits, before the host is spent (#802).

`FindLeafBlockForSeek` (`integrations/rocksdb/src/expanse_memtable.cc:137`) opens
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
    return "\n".join(lines)


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
