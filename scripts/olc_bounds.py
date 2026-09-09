#!/usr/bin/env python3
"""Contention bounds for Stage B multi-writer optimistic lock coupling (#568, plan PR 4).

The bound functions live here, unit-tested against pinned reference values,
so the pre-registration in `docs/benchmarks/concurrency/METHODOLOGY.md` §10
invokes them rather than restating arithmetic (AGENTS.md §8.8 commit 1,
§8.14). Every empirical input is read from a committed artifact at run time
and named with its estimator; the one input no artifact carries — the hold
time of a per-node lock — is a stated hypothesis, and every conclusion that
depends on it says so.

Sources:
  Leis, Scheibner, Kemper & Neumann, "The ART of Practical Synchronization",
    DaMoN 2016 (optimistic lock coupling; restart on a failed upgrade).
  Hennessy & Patterson, Computer Architecture: A Quantitative Approach,
    6th ed., §5.2 (a line an RMW bounces between cores costs one transfer
    per acquisition).
  `docs/benchmarks/concurrency/results/line_transfer.json` (the reference
    host's pairwise line-transfer matrix, #568 PR 0) and the two FFI suites'
    `results/baseline_concurrent_ab*.json` (the two-commit sweeps of #568
    PR 3, head halves at the merged engine).

Usage:
    python3 scripts/olc_bounds.py             # the bounds, then the unit tests
    python3 scripts/olc_bounds.py --self-test # the unit tests only
"""

from __future__ import annotations

import json
import math
import statistics
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
LINE_TRANSFER = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "line_transfer.json"
AB_ARTIFACTS = {
    "hot_comparison": [REPO_ROOT / "docs" / "benchmarks" / "hot_comparison" / "results" / f
                       for f in ("baseline_concurrent_ab.json", "baseline_concurrent_ab_run2.json")],
    "masstree_comparison": [REPO_ROOT / "docs" / "benchmarks" / "masstree_comparison" / "results" / f
                            for f in ("baseline_concurrent_ab.json", "baseline_concurrent_ab_run2.json")],
}

# ---------------------------------------------------------------------------
# Hypotheses: inputs no artifact measures. Each is a parameter of the bound
# functions below, never a hidden constant; METHODOLOGY §10 says which counter
# instantiates it before PR 5's cells run.
# ---------------------------------------------------------------------------

# Hold time of one per-node lock (hypothesis): a brief in-place leaf or slot
# store, and a structural rebuild that allocates and publishes a node. The
# sync_* Callgrind arms give instructions per whole operation (about 750–2,150
# on the merged engine), not the nanoseconds a lock is held; these two values
# are the shape of the argument, to be replaced by the measured hold before
# the gate is evaluated.
T_HOLD_LEAF_HYPOTHESIS_NS = 15.0
T_HOLD_CASCADE_HYPOTHESIS_NS = 50.0

# Lines a writer must own per insert, per workload shape (from the plan's
# reading of the engine: the FFI cells insert uniform 64-bit keys in disjoint
# slices, so writers meet only at the root word; `core_concurrency`'s
# `rng % 2M` keys share bytes 7..3, so every insert walks one chain to level
# 3 — about five shared lines).
K_LINES = {"ffi_disjoint": 1, "core_shared_prefix": 5}


# ---------------------------------------------------------------------------
# Measured inputs, read from the artifacts
# ---------------------------------------------------------------------------

def line_transfer_ns(path: Path = LINE_TRANSFER) -> dict[str, float]:
    """The reference host's one-way line transfer, spinning, between physical
    P-cores: the median over the pairwise cells of each cell's mean (the
    artifact's own estimator, BCa over 7 repeats per cell), plus the min and
    max cell so a reader sees the spread (SMT siblings are the min)."""
    d = json.loads(path.read_text())
    cells = [c for c in d["cells"] if c.get("mode") == "spin" and c.get("cpu_b") is not None]
    if not cells:
        raise ValueError(f"{path}: no spinning pair cells")
    means = [c["ns_per_transfer"]["mean"] for c in cells]
    return {"median": statistics.median(means), "min": min(means), "max": max(means), "cells": len(cells)}


def w1_insert_rate(suite: str, arm: str, paths: dict[str, list[Path]] = AB_ARTIFACTS) -> dict[str, float]:
    """The merged engine's single-writer insert rate on one FFI arm: the union
    over the two two-commit runs of the head half's C1 W=1 median (M inserts/s),
    which is the level a multi-writer ceiling has to clear."""
    vals = []
    for p in paths[suite]:
        d = json.loads(p.read_text())
        for r in d["throughput"]:
            if r["arm"] == arm and r["writers"] == 1 and r["readers"] == 0:
                vals.append(r["head"]["expanse_writer_mops_median"])
    if len(vals) != 2:
        raise ValueError(f"{suite}/{arm}: expected the C1 W=1 cell in both runs, found {len(vals)}")
    return {"union_lower": min(vals), "union_upper": max(vals)}


# ---------------------------------------------------------------------------
# Bounds
# ---------------------------------------------------------------------------

def contended_rmw_ceiling(k: int, t_line_ns: float, t_hold_ns: float) -> float:
    """Aggregate ceiling, in ops/s, when every operation must own `k` shared
    lines (one transfer each, Hennessy & Patterson §5.2) and then hold a lock
    for `t_hold_ns`: 1 / (k·t_line + t_hold). Independent of the writer count —
    it is the service rate of the serial part, the term the writers queue on."""
    if k < 1:
        raise ValueError(f"k must be >= 1, got {k}")
    if t_line_ns <= 0.0:
        raise ValueError(f"t_line_ns must be positive, got {t_line_ns}")
    if t_hold_ns < 0.0:
        raise ValueError(f"t_hold_ns cannot be negative, got {t_hold_ns}")
    return 1.0 / ((k * t_line_ns + t_hold_ns) * 1e-9)


def allocator_counter_ceiling(t_line_ns: float) -> float:
    """Allocations per second when every writer's allocation does one RMW on
    one shared counter line: one transfer per allocation, 1 / t_line. Binds
    only if allocations per insert are known; the merged engine allocates on a
    fraction of inserts (leaf and branch rebuilds), so this is the ceiling on
    the allocating inserts, not on inserts."""
    if t_line_ns <= 0.0:
        raise ValueError(f"t_line_ns must be positive, got {t_line_ns}")
    return 1e9 / t_line_ns


def collision_probability(w: int, t_hold_ns: float, t_op_ns: float) -> float:
    """Probability that at least one of the other W−1 writers holds the lock a
    writer needs at the moment it asks, with each writer holding for
    t_hold of every t_op and arrivals independent: 1 − (1 − t_hold/t_op)^(W−1).
    Leis et al. restart on a failed upgrade, so this is the restart
    probability per acquisition on a shared node."""
    if w < 1:
        raise ValueError(f"w must be >= 1, got {w}")
    if t_hold_ns < 0.0:
        raise ValueError(f"t_hold_ns cannot be negative, got {t_hold_ns}")
    if t_op_ns <= 0.0:
        raise ValueError(f"t_op_ns must be positive, got {t_op_ns}")
    if t_hold_ns >= t_op_ns:
        return 1.0
    return 1.0 - math.pow(1.0 - t_hold_ns / t_op_ns, w - 1)


def expected_restarts_per_op(w: int, t_hold_ns: float, t_op_ns: float) -> float:
    """Restarts per operation when every collision restarts and retries are
    independent: geometric, p / (1 − p). Diverges as p → 1 (a restart storm)."""
    p = collision_probability(w, t_hold_ns, t_op_ns)
    if p >= 0.999_999:
        return math.inf
    return p / (1.0 - p)


def restart_ceiling(w: int, t_hold_ns: float, t_op_ns: float, safety_factor: float = 2.0) -> float:
    """The ceiling METHODOLOGY §10 registers for `Stat::LockRestarts ÷ write_ops`
    at W writers: the expected restarts per operation times a safety factor.
    The factor is a pre-registration choice, not a derivation; it is written
    down there before the cells run."""
    if safety_factor <= 0.0:
        raise ValueError(f"safety_factor must be positive, got {safety_factor}")
    return expected_restarts_per_op(w, t_hold_ns, t_op_ns) * safety_factor


def shape_bound(shape: str, t_line_ns: float, t_hold_ns: float) -> dict[str, float]:
    """The ceiling for one workload shape (its `k` from `K_LINES`)."""
    if shape not in K_LINES:
        raise ValueError(f"unknown shape {shape!r}; known: {', '.join(K_LINES)}")
    k = K_LINES[shape]
    ceiling = contended_rmw_ceiling(k, t_line_ns, t_hold_ns)
    return {"k": k, "t_line_ns": t_line_ns, "t_hold_ns": t_hold_ns, "ceiling_mops": ceiling / 1e6}


# ---------------------------------------------------------------------------
# Unit tests: reference values pinned by hand-checkable arithmetic, plus the
# artifact readers against the committed files.
# ---------------------------------------------------------------------------

class TestOlcBounds(unittest.TestCase):
    def test_contended_rmw_ceiling_reference_values(self):
        # 1 / (1 × 30 ns + 20 ns) = 20 M ops/s exactly.
        self.assertAlmostEqual(contended_rmw_ceiling(1, 30.0, 20.0) / 1e6, 20.0, places=9)
        # 1 / (5 × 30 ns + 50 ns) = 5 M ops/s exactly.
        self.assertAlmostEqual(contended_rmw_ceiling(5, 30.0, 50.0) / 1e6, 5.0, places=9)

    def test_allocator_counter_ceiling(self):
        self.assertAlmostEqual(allocator_counter_ceiling(40.0) / 1e6, 25.0, places=9)

    def test_collision_and_restarts(self):
        self.assertEqual(collision_probability(1, 15.0, 175.0), 0.0)
        self.assertEqual(expected_restarts_per_op(1, 15.0, 175.0), 0.0)
        # W=2, hold 1/4 of the op: p = 1 − 3/4 = 0.25, restarts = 1/3.
        self.assertAlmostEqual(collision_probability(2, 25.0, 100.0), 0.25, places=9)
        self.assertAlmostEqual(expected_restarts_per_op(2, 25.0, 100.0), 1.0 / 3.0, places=9)
        # Hold ≥ op: certain collision, a storm.
        self.assertEqual(collision_probability(3, 100.0, 100.0), 1.0)
        self.assertTrue(math.isinf(expected_restarts_per_op(3, 100.0, 100.0)))
        self.assertAlmostEqual(restart_ceiling(2, 25.0, 100.0, safety_factor=2.0), 2.0 / 3.0, places=9)

    def test_shape_bound_uses_k(self):
        a = shape_bound("ffi_disjoint", 30.0, 20.0)
        b = shape_bound("core_shared_prefix", 30.0, 50.0)
        self.assertEqual((a["k"], b["k"]), (1, 5))
        self.assertAlmostEqual(a["ceiling_mops"], 20.0, places=9)
        self.assertAlmostEqual(b["ceiling_mops"], 5.0, places=9)

    def test_invalid_arguments_raise(self):
        for bad in ((0, 30.0, 15.0), (1, 0.0, 15.0), (1, 30.0, -1.0)):
            with self.assertRaises(ValueError):
                contended_rmw_ceiling(*bad)
        with self.assertRaises(ValueError):
            collision_probability(0, 15.0, 100.0)
        with self.assertRaises(ValueError):
            shape_bound("nonexistent", 30.0, 15.0)
        with self.assertRaises(ValueError):
            restart_ceiling(2, 25.0, 100.0, safety_factor=0.0)

    def test_artifact_readers(self):
        lt = line_transfer_ns()
        self.assertGreater(lt["cells"], 0)
        self.assertLess(lt["min"], lt["median"])
        self.assertLessEqual(lt["median"], lt["max"])
        for suite, arm in (("hot_comparison", "set"), ("hot_comparison", "map"), ("masstree_comparison", "map")):
            u = w1_insert_rate(suite, arm)
            self.assertLessEqual(u["union_lower"], u["union_upper"])
            self.assertGreater(u["union_lower"], 0.0)


def report() -> None:
    lt = line_transfer_ns()
    t_line = lt["median"]
    print("Stage B contention bounds (#568 plan PR 4) — inputs from the committed artifacts")
    print(f"  t_line (spinning, P-core pairs, median of {lt['cells']} cell means): {t_line:.2f} ns "
          f"[cells span {lt['min']:.2f}–{lt['max']:.2f}]")
    for suite, arm in (("hot_comparison", "set"), ("hot_comparison", "map"), ("masstree_comparison", "map")):
        u = w1_insert_rate(suite, arm)
        print(f"  W=1 insert rate, merged engine, {suite}/{arm}: [{u['union_lower']:.2f}, {u['union_upper']:.2f}] M/s")
    print("  t_hold: HYPOTHESIS — no artifact measures a lock hold; the values below are the argument's shape")
    print()
    for shape, t_hold in (("ffi_disjoint", T_HOLD_LEAF_HYPOTHESIS_NS), ("core_shared_prefix", T_HOLD_CASCADE_HYPOTHESIS_NS)):
        r = shape_bound(shape, t_line, t_hold)
        print(f"  {shape}: k = {r['k']}, t_hold = {t_hold:.0f} ns (hypothesis) -> ceiling {r['ceiling_mops']:.2f} M ops/s")
    print(f"  allocator counter line: {allocator_counter_ceiling(t_line) / 1e6:.2f} M allocs/s on the allocating inserts")
    for w in (2, 4, 8, 16):
        r = restart_ceiling(w, T_HOLD_LEAF_HYPOTHESIS_NS, 1e9 / 5.4e6)
        print(f"  restart ceiling at W={w} (t_hold hypothesis 15 ns, t_op from 5.4 M/s, safety 2x): {r:.2f} restarts/op")
    print()


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.argv = [sys.argv[0]]
        unittest.main()
    report()
    unittest.main(argv=[sys.argv[0]])
