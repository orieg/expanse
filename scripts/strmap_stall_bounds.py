#!/usr/bin/env python3
"""Bounds behind #1586's stall measurement (concurrency METHODOLOGY §44.2, AGENTS.md §8.8 commit 1).

`SyncExpanseStrMap::mem_used` (`crates/expanse/src/sync.rs`) reads through
`Shared::read_locked`: it takes the fallback mutex, closes the writer gate
(`quiesce_writers`), takes the writer mutex, and walks every node shell and
suffix leaf (`StrNode::shell_bytes`, `crates/expanse/src/strmap.rs`). An insert
arriving while the gate is closed spins in `enter_writer_blocking` until the
walk ends. The three functions below are the arithmetic §44 reads:

1. `stalled_fraction(walk_s, period_s)` — the share of wall time during which
   no insert completes, for a poller that reads once per `period_s`;
2. `spin_core_seconds(walk_s, writers)` — CPU time the writers can burn
   spinning per reading;
3. `walk_projection(ir_per_key, keys, ir_per_s)` — the walk's duration
   projected from its instruction count. §44 uses it only to size the run.
   It is a projection, never a prediction: the walk misses cache once the
   tree outgrows the last-level cache, and an instruction count cannot see
   that.

`arm_ir_per_key` turns the #1587 Callgrind arm's total into instructions per
key, and the self-test reads the arm's read count and population from
`crates/expanse/benches/instructions.rs`, so a change there fails it.

Sources
-------
The arm figures are the PR-vs-main pass of CI run 39394551086 (PR #1587, head
5b7ae9bf): `sync_strmap_mem_used/routes` 15,539,041 instructions and
`sync_strmap_mem_used/short` 16,688,555, each over `MEM_USED_READS` (4) reads
of a 50,000-key map (`POP`). The rest is arithmetic, stated per function.

Usage:
    python3 scripts/strmap_stall_bounds.py             # the figures, then the self-test
    python3 scripts/strmap_stall_bounds.py --self-test # the self-test only
"""

from __future__ import annotations

import argparse
import math
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
INSTRUCTIONS_RS = REPO_ROOT / "crates" / "expanse" / "benches" / "instructions.rs"

# The #1587 arm (CI run 39394551086, PR-vs-main pass): total instructions per
# variant, the reads it makes and the keys each read walks.
ARM_TOTAL_IR = {"routes": 15_539_041, "short": 16_688_555}
ARM_READS = 4
ARM_KEYS = 50_000
ARM_RUN = "https://github.com/orieg/expanse/actions/runs/39394551086"

# §44's populations, the ones #1586 names.
POPULATIONS = (2_000_000, 20_000_000)


def _finite(name: str, x: float) -> float:
    if isinstance(x, bool) or not isinstance(x, (int, float)) or not math.isfinite(x):
        raise ValueError(f"{name} must be a finite number, got {x!r}")
    return float(x)


def stalled_fraction(walk_s: float, period_s: float) -> float:
    """The share of wall time with the writer gate closed, for one poller.

    One poller makes one reading per `period_s`, and every reading holds the
    gate closed for `walk_s`. While it is closed no writer enters, so no insert
    completes, whatever the writer count: the fraction is per wall second and
    W does not divide it. For `walk_s <= period_s` it is `walk_s / period_s`
    (METHODOLOGY §44.2). The poller is one thread and its readings do not
    overlap, so a walk longer than the period leaves the next reading due on
    the walk's return: the readings run back to back and the gate is closed
    for the whole window. The fraction is therefore `walk_s / max(walk_s,
    period_s)`, which is 1 there rather than a ratio above 1.
    """
    walk = _finite("walk_s", walk_s)
    period = _finite("period_s", period_s)
    if walk < 0:
        raise ValueError(f"walk_s must be >= 0, got {walk_s!r}")
    if period <= 0:
        raise ValueError(f"period_s must be > 0, got {period_s!r}")
    return walk / max(walk, period)


def spin_core_seconds(walk_s: float, writers: int) -> float:
    """CPU seconds the writers can spend spinning during one reading: `walk_s × writers`.

    An arriving writer spins in `enter_writer_blocking` (`core::hint::spin_loop`,
    no yield) until the gate reopens, so each of W writers can burn at most the
    walk's duration per reading. It is an upper bound: a writer that is not
    trying to enter during the walk burns nothing.
    """
    walk = _finite("walk_s", walk_s)
    if walk < 0:
        raise ValueError(f"walk_s must be >= 0, got {walk_s!r}")
    if isinstance(writers, bool) or not isinstance(writers, int) or writers < 0:
        raise ValueError(f"writers must be a non-negative integer, got {writers!r}")
    return walk * writers


def arm_ir_per_key(total_ir: int, reads: int, keys: int) -> float:
    """Instructions per key of one walk: `total_ir / (reads × keys)`."""
    for name, v in (("total_ir", total_ir), ("reads", reads), ("keys", keys)):
        if isinstance(v, bool) or not isinstance(v, int) or v <= 0:
            raise ValueError(f"{name} must be a positive integer, got {v!r}")
    return total_ir / (reads * keys)


def walk_projection(ir_per_key: float, keys: int, ir_per_s: float) -> float:
    """Seconds one walk would take at `ir_per_s`: `ir_per_key × keys / ir_per_s`.

    Labelled a projection: `ir_per_key` is a Callgrind count at 50,000 keys,
    where the tree fits in cache, and `ir_per_s` is an assumed instruction
    rate, not a measurement. Past the last-level cache the walk's dependent
    loads miss and the rate falls, which an instruction count does not show.
    METHODOLOGY §44 uses it to size the run and never compares it with a
    result.
    """
    ipk = _finite("ir_per_key", ir_per_key)
    rate = _finite("ir_per_s", ir_per_s)
    if ipk <= 0:
        raise ValueError(f"ir_per_key must be > 0, got {ir_per_key!r}")
    if isinstance(keys, bool) or not isinstance(keys, int) or keys <= 0:
        raise ValueError(f"keys must be a positive integer, got {keys!r}")
    if rate <= 0:
        raise ValueError(f"ir_per_s must be > 0, got {ir_per_s!r}")
    return ipk * keys / rate


def ir_per_key(shape: str) -> float:
    """The #1587 arm's instructions per key for `shape` (`routes` or `short`)."""
    if shape not in ARM_TOTAL_IR:
        raise ValueError(f"unknown shape {shape!r}; expected one of {sorted(ARM_TOTAL_IR)}")
    return arm_ir_per_key(ARM_TOTAL_IR[shape], ARM_READS, ARM_KEYS)


def _read_const(name: str) -> int:
    m = re.search(rf"^const {name}: usize = ([0-9_]+);", INSTRUCTIONS_RS.read_text(), re.M)
    if not m:
        raise RuntimeError(f"`const {name}` not found in {INSTRUCTIONS_RS}")
    return int(m.group(1).replace("_", ""))


def _raises(fn, *args) -> None:
    try:
        fn(*args)
    except ValueError:
        return
    raise AssertionError(f"{fn.__name__}{args} did not raise ValueError")


def self_test() -> int:
    # The arm's read count and population are the harness's own constants.
    assert _read_const("MEM_USED_READS") == ARM_READS
    assert _read_const("POP") == ARM_KEYS
    # Per-key counts: 15,539,041 / 200,000 and 16,688,555 / 200,000.
    assert abs(ir_per_key("routes") - 77.695205) < 1e-9, ir_per_key("routes")
    assert abs(ir_per_key("short") - 83.442775) < 1e-9, ir_per_key("short")
    # Per read, as the perf report prints it: 3,884,760.25 and 4,172,138.75.
    assert ARM_TOTAL_IR["routes"] / ARM_READS == 3_884_760.25
    assert ARM_TOTAL_IR["short"] / ARM_READS == 4_172_138.75
    _raises(ir_per_key, "uuid")

    # stalled_fraction: walk / period below the period, 1 at and above it.
    assert stalled_fraction(0.015, 1.0) == 0.015
    assert stalled_fraction(0.015, 0.1) == 0.15
    assert stalled_fraction(0.0, 1.0) == 0.0
    assert stalled_fraction(1.0, 1.0) == 1.0
    assert stalled_fraction(0.25, 0.1) == 1.0
    # §44.4's M1 threshold, 1 % of wall time, is a 10 ms walk at T = 1 s.
    assert stalled_fraction(0.010, 1.0) == 0.01
    for bad in ((-0.1, 1.0), (0.1, 0.0), (0.1, -1.0), (float("nan"), 1.0),
                (0.1, float("inf")), ("0.1", 1.0), (True, 1.0)):
        _raises(stalled_fraction, *bad)

    # spin_core_seconds: walk × writers.
    assert spin_core_seconds(0.015, 4) == 0.06
    assert spin_core_seconds(0.2, 8) == 1.6
    assert spin_core_seconds(0.2, 0) == 0.0
    for bad in ((0.1, -1), (0.1, 1.5), (0.1, True), (-0.1, 4), (float("nan"), 4)):
        _raises(spin_core_seconds, *bad)

    # walk_projection: 77.695205 Ir/key × 2e7 keys = 1,553,904,100 Ir.
    assert abs(walk_projection(ir_per_key("routes"), 20_000_000, 1e9) - 1.5539041) < 1e-9
    assert abs(walk_projection(ir_per_key("short"), 2_000_000, 1e9) - 0.16688555) < 1e-9
    assert abs(walk_projection(80.0, 50_000, 4e9) - 0.001) < 1e-15
    for bad in ((0.0, 10, 1e9), (-1.0, 10, 1e9), (80.0, 0, 1e9), (80.0, 10.0, 1e9),
                (80.0, 10, 0.0), (80.0, 10, float("nan")), (80.0, True, 1e9)):
        _raises(walk_projection, *bad)
    for bad in ((0, 4, 50_000), (100, 0, 50_000), (100, 4, -1), (100.0, 4, 50_000)):
        _raises(arm_ir_per_key, *bad)
    print("strmap_stall_bounds.py self-test PASSED")
    return 0


def report() -> None:
    print(f"#1587 arm ({ARM_RUN}, PR-vs-main pass):")
    for shape in ("routes", "short"):
        print(f"  {shape}: {ARM_TOTAL_IR[shape]:,} Ir / ({ARM_READS} reads x {ARM_KEYS:,} keys)"
              f" = {ir_per_key(shape):.4f} Ir/key")
    print("Walk projections (assumed instruction rates; sizing only, never a prediction):")
    for shape in ("routes", "short"):
        for keys in POPULATIONS:
            lo = walk_projection(ir_per_key(shape), keys, 4e9)
            hi = walk_projection(ir_per_key(shape), keys, 1e9)
            print(f"  {shape} N={keys:,}: {lo:.3f} s at 4e9 Ir/s .. {hi:.3f} s at 1e9 Ir/s")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if not args.self_test:
        report()
    return self_test()


if __name__ == "__main__":
    sys.exit(main())
