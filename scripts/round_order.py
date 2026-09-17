#!/usr/bin/env python3
"""Round-order split of an interleaved two-arm concurrent cell (AGENTS.md §8.2).

The FFI concurrent harnesses (`hot_concurrent`, `masstree_concurrent`) time two
arms in every round and alternate which arm is timed first by round parity: the
competitor is first on even rounds and Expanse on odd rounds
(`let mt_first = round % 2 == 0` / `let rowex_first = round % 2 == 0`). The
runners keep `round` in `rounds_raw` and drop the harness's `first` field, so
position is recovered here from the parity, and position and parity are one
variable in these artifacts: nothing below can tell "timed first" from "odd
round".

A published median pools both positions. Where an arm's two positions sit at
different levels, that median lies between two modes and describes neither, and
a before/after comparison of two such medians can report a change that neither
mode shows. This module computes the split so a README can print it beside the
median; it names no cause.

    split(cell, role, side, other) -> Split
    flagged(cell, role, side, other) -> bool

A side of a cell is *flagged* when its two position medians differ by more than
the cell's own BCa interval is wide, both taken relative to their centre:

    |median(first) - median(second)| / median(all)  >  (ci_upper - ci_lower) / ratio

The interval is the Expanse ÷ competitor ratio's, the only interval a cell
carries, so the yardstick is shared by both sides of a cell.
"""

from __future__ import annotations

import statistics
import sys
from typing import NamedTuple


class Split(NamedTuple):
    """One side of one cell, split by which arm was timed first."""

    median: float        # over every round, recomputed from rounds_raw
    first: float         # median over the rounds this side was timed first
    second: float        # median over the rounds it was timed second
    gap: float           # |first - second| / median
    interval: float      # (ci_upper - ci_lower) / ratio, the cell's own interval
    low: float
    high: float


def expanse_first(round_no: int) -> bool:
    """Both harnesses time the competitor first on even rounds."""
    return round_no % 2 == 1


def split(cell: dict, role: str, side: str, other: str) -> Split:
    """`side` is "expanse" or the competitor's key prefix (`other`)."""
    if side not in ("expanse", other):
        raise ValueError(f"side must be 'expanse' or {other!r}, got {side!r}")
    ratio_key = f"{role}_expanse_over_{other}"
    if cell.get(ratio_key) is None:
        raise ValueError(f"cell carries no {role} ratio")
    field = f"{side}_{role}_mops"
    rows = cell["rounds_raw"]
    if len(rows) < 4:
        raise ValueError("a position split needs at least two rounds in each position")
    mine_first = [r[field] for r in rows if expanse_first(r["round"]) == (side == "expanse")]
    mine_second = [r[field] for r in rows if expanse_first(r["round"]) != (side == "expanse")]
    if len(mine_first) < 2 or len(mine_second) < 2:
        raise ValueError("a position split needs at least two rounds in each position")
    every = [r[field] for r in rows]
    med = statistics.median(every)
    first, second = statistics.median(mine_first), statistics.median(mine_second)
    width = (cell[f"{role}_ci_upper"] - cell[f"{role}_ci_lower"]) / cell[ratio_key]
    return Split(med, first, second, abs(first - second) / med, width, min(every), max(every))


def flagged(cell: dict, role: str, side: str, other: str) -> bool:
    s = split(cell, role, side, other)
    return s.gap > s.interval


def _self_test() -> int:
    failures = []

    def cell(exp, comp, lo, hi):
        rows = [{"round": i, "expanse_writer_mops": e, "x_writer_mops": c} for i, (e, c) in enumerate(zip(exp, comp))]
        return {"rounds_raw": rows, "writer_expanse_over_x": 1.0, "writer_ci_lower": lo, "writer_ci_upper": hi}

    # Expanse is first on odd rounds: 3.0 first, 4.0 second; the competitor is flat.
    c = cell([4.0, 3.0, 4.0, 3.0, 4.0, 3.0], [5.0] * 6, 0.9, 1.1)
    s = split(c, "writer", "expanse", "x")
    if (s.first, s.second) != (3.0, 4.0):
        failures.append(f"position medians: {s}")
    if abs(s.gap - 1.0 / 3.5) > 1e-12 or abs(s.interval - 0.2) > 1e-12:
        failures.append(f"gap or interval: {s}")
    if not flagged(c, "writer", "expanse", "x") or flagged(c, "writer", "x", "x"):
        failures.append("a bimodal side must flag and a flat side must not")
    # The competitor is first on even rounds.
    c2 = cell([1.0] * 6, [7.0, 6.0, 7.0, 6.0, 7.0, 6.0], 0.9, 1.1)
    s2 = split(c2, "writer", "x", "x")
    if (s2.first, s2.second) != (7.0, 6.0):
        failures.append(f"competitor position medians: {s2}")
    # A gap inside the interval is not flagged.
    if flagged(cell([4.0, 3.0] * 3, [5.0] * 6, 0.5, 1.5), "writer", "expanse", "x"):
        failures.append("a gap narrower than the interval flagged")
    for bad in (lambda: split(c, "reader", "expanse", "x"), lambda: split(c, "writer", "y", "x"),
                lambda: split(cell([1.0] * 3, [1.0] * 3, 0.9, 1.1), "writer", "expanse", "x")):
        try:
            bad()
        except ValueError:
            continue
        failures.append("an invalid input did not raise")
    for f in failures:
        print(f"  FAIL {f}")
    print(f"round_order.py --self-test: {'all checks passed' if not failures else f'{len(failures)} failure(s)'}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(_self_test() if "--self-test" in sys.argv else 0)
