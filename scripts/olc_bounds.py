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
import tempfile
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
OLC_ARTIFACTS = {
    "hot_comparison": [REPO_ROOT / "docs" / "benchmarks" / "hot_comparison" / "results" / "multi_writer_olc" / f
                       for f in ("baseline_concurrent_ab.json", "baseline_concurrent_ab_run2.json")],
    "masstree_comparison": [REPO_ROOT / "docs" / "benchmarks" / "masstree_comparison" / "results" / "multi_writer_olc" / f
                            for f in ("baseline_concurrent_ab.json", "baseline_concurrent_ab_run2.json")],
}
BASELINE_WRITER_SCALING = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "baseline_writer_scaling.json"

# The W=1 per-insert fallback causes the Phase 4D prediction was derived from:
# `baseline_writer_scaling.json` as #839 committed it (engine ff3e0e06, CI run
# 34562877044), before #840 landed. That file has since been replaced by sweeps
# of the post-4D engine, so the inputs are frozen here: deriving the prediction
# from the live file would evaluate it against its own outcome (§8.7).
PRE_4D_W1_CAUSES: dict[str, dict[str, float]] = {
    "set": {"cap_expansion": 0.019883, "immediate_conversion": 0.015281, "branch_split": 0.751745},
    "map": {"cap_expansion": 0.133741, "immediate_conversion": 0.01374, "branch_split": 0.097611},
}

# The W=1 per-insert fallback causes the Phase 4C prediction is derived from:
# Measured on post-4D engine with granular CapExpansion subset partitioning (#568):
# 1,048,576 fresh inserts per arm after 1,048,576 prefill.
# set: cap_expansion=20849 (class=2960, leaf_full=17889, bm_near_full=0, sub=0, rem=0),
#      immediate_conversion=16023, branch_split=0. Total fallbacks: 36872 (3.52%).
# map: cap_expansion=140238 (class=110459, leaf_full=29779, bm_near_full=0, sub=0, rem=0),
#      immediate_conversion=14407, branch_split=0. Total fallbacks: 154645 (14.75%).
PRE_4C_W1_CAUSES: dict[str, dict[str, float]] = {
    "set": {
        "cap_expansion_class": 2960 / 1048576,
        "cap_expansion_leaf_full": 17889 / 1048576,
        "cap_expansion_bitmap_near_full": 0.0,
        "cap_expansion_map_bitmap_sub": 0.0,
        "cap_expansion_remove": 0.0,
        "immediate_conversion": 16023 / 1048576,
        "branch_split": 0.0,
        "root_growth": 0.0,
        "contention": 0.0,
        "unknown_tag": 0.0,
    },
    "map": {
        "cap_expansion_class": 110459 / 1048576,
        "cap_expansion_leaf_full": 29779 / 1048576,
        "cap_expansion_bitmap_near_full": 0.0,
        "cap_expansion_map_bitmap_sub": 0.0,
        "cap_expansion_remove": 0.0,
        "immediate_conversion": 14407 / 1048576,
        "branch_split": 0.0,
        "root_growth": 0.0,
        "contention": 0.0,
        "unknown_tag": 0.0,
    },
}

# ---------------------------------------------------------------------------
# Hypotheses: inputs no artifact measures. Each is a parameter of the bound
# functions below, never a hidden constant; METHODOLOGY §10 says which counter
# instantiates it before PR 5's cells run.
# ---------------------------------------------------------------------------

# Hold time of one per-node lock: leaf and branch rebuilds.
# Where measured health rows exist (HOT set ≈ 14.0 ns, HOT map ≈ 44.2–45.8 ns,
# Masstree map ≈ 42.6–44.6 ns), lock_hold_ns() is used.
# T_HOLD_CASCADE_HYPOTHESIS_NS remains a hypothesis for deep multi-level cascades.
T_HOLD_LEAF_HYPOTHESIS_NS = 15.0
T_HOLD_CASCADE_HYPOTHESIS_NS = 50.0

# Lines a writer must own per insert, per workload shape:
# - `ffi_disjoint` / `ffi_disjoint_padded`: Phase 1.5A under `feature = "lock-padded"`.
#   The mark_digit read-before-write guard means zero write-shared lines on the trie
#   in steady state (one read-shared line; one RFO per digit per fold epoch).
#   However, Collector::op_count is still RMW'd by every writer on every insert (occ.rs:1314);
#   wrapping it in Line<AtomicUsize> prevents false sharing with neighboring struct fields
#   but retains 1 true-shared cache line across writers (k = 1).
# - `ffi_disjoint_default`: Phase 1.5A default build (`Line<X> = X`).
#   In addition to Collector::op_count, the 64 atomic slots pack 8 per 64-byte line,
#   introducing residual false sharing across concurrent writer slots, bounded by k = 2.
# - `baseline_measured`: Pre-1.5A measured baseline on reference host (`b49835ad`
#   `masstree_conc_map_w4_r0`: `l2_rqsts.rfo_miss` = 12.208 [11.864, 12.441],
#   `xsnp_hitm` = 9.196 [8.952, 9.375]). Every insert locked the root node's version
#   word and contended on global words, transferring 9-12 cache lines per insert (k = 10).
# - `core_shared_prefix`: `core_concurrency`'s `rng % 2M` keys share bytes 7..3,
#   so every insert walks one chain to level 3 — about five shared lines (k = 5).
K_LINES = {
    "ffi_disjoint": 1,
    "ffi_disjoint_padded": 1,
    "ffi_disjoint_default": 2,
    "baseline_measured": 10,
    "core_shared_prefix": 5,
}


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


def lock_hold_ns(suite: str, arm: str, paths: dict[str, list[Path]] = OLC_ARTIFACTS) -> dict[str, float]:
    """The measured lock holding time per write op (in nanoseconds) extracted
    from the health rows of the committed baseline artifacts:
    (lock_hold_cycles / cycles_hz * 1e9) / write_ops.
    Measured on the reference host: HOT set ≈ 14.0 ns, HOT map ≈ 44.2–45.8 ns,
    Masstree map ≈ 42.6–44.6 ns."""
    vals = []
    for p in paths[suite]:
        d = json.loads(p.read_text())
        for h in d.get("health", []):
            if h.get("arm") == arm:
                lhc = h.get("lock_hold_cycles", {}).get("median", 0)
                hz = h.get("cycles_hz", {}).get("median", 1)
                w_ops = h.get("write_ops", {}).get("median", 0)
                if w_ops > 0 and hz > 0:
                    vals.append(float((lhc / hz * 1e9) / w_ops))
    if not vals:
        raise ValueError(f"{suite}/{arm}: lock hold health rows not found in artifacts")
    return {"median": statistics.median(vals), "min": min(vals), "max": max(vals), "runs": len(vals)}


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
    denom = (k * t_line_ns + t_hold_ns) * 1e-9
    if denom <= 0.0:
        raise ValueError("Sum of line transfer and lock hold times must be positive")
    return 1.0 / denom


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


def phase4d_predicted_fallback_rate(
    arm: str,
    branchb_up: int = 192,
    leaf_cap: int = 32,
    artifact_path: Path | None = None,
) -> tuple[float, float]:
    """Predicted per-insert fallback rate interval [lower, upper] after Phase 4D
    concurrent BranchB subarray insertion on the W=1 uniform random workload.

    Sources:
      Leis, Scheibner, Kemper & Neumann, DaMoN 2016 §3 (optimistic lock coupling).
      The pre-4D W=1 fallback cause partitions in `PRE_4D_W1_CAUSES` (Refs #568,
      #839); pass `artifact_path` to derive from another artifact's W=1 row.

    Derivation (§8.14):
      At W=1, all fallbacks are deterministic structural transitions.
      Phase 4D handles subarray allocation and in-place expansion concurrently
      under the node's local version lock without global lock fallback.
      The only BranchB subarray-site insertion that remains as a fallback is the
      upgrade to BranchU when population exceeds BRANCHB_UP.
      A BranchB born from a leaf split already holds k0 <= LEAF_CAP + 1 digits,
      so the node accommodates at least (BRANCHB_UP + 1 - k0) subsequent subarray
      insertions before upgrading.
      Thus at most 1 / (BRANCHB_UP + 1 - (leaf_cap + 1)) of all subarray insertions
      trigger this upgrade (e.g. 1 / (193 - 33) = 1 / 160).

      Residual lower bound (zero upgrades):
        residual_lower = cap_expansion + immediate_conversion
      Residual upper bound (maximum upgrade rate):
        residual_upper = cap_expansion + immediate_conversion + branch_split / (BRANCHB_UP + 1 - (leaf_cap + 1))

    Returns:
      (residual_lower, residual_upper) as per-insert fallback rates.
    """
    if arm not in ("set", "map"):
        raise ValueError(f"arm must be 'set' or 'map', got '{arm}'")
    if branchb_up < 1:
        raise ValueError(f"branchb_up must be >= 1, got {branchb_up}")
    if leaf_cap < 1:
        raise ValueError(f"leaf_cap must be >= 1, got {leaf_cap}")
    k0 = leaf_cap + 1
    if branchb_up + 1 <= k0:
        raise ValueError(f"branchb_up + 1 ({branchb_up + 1}) must be > k0 ({k0})")
    if artifact_path is None:
        per_ins = PRE_4D_W1_CAUSES[arm]
    else:
        d = json.loads(artifact_path.read_text())
        row = next(
            (r for r in d.get("throughput", []) if r.get("arm") == arm and r.get("writers") == 1),
            None,
        )
        if not row:
            raise ValueError(f"{artifact_path}: no W=1 throughput row found for arm '{arm}'")
        per_ins = row["fallback_causes_per_insert"]
    f_bs = per_ins["branch_split"]
    f_ce = per_ins["cap_expansion"]
    f_ic = per_ins["immediate_conversion"]

    res_lower = f_ce + f_ic
    max_upgrade_fraction = 1.0 / (branchb_up + 1 - k0)
    res_upper = res_lower + f_bs * max_upgrade_fraction
    return (res_lower, res_upper)


def phase4c_predicted_fallback_rate(
    arm: str,
    branchb_up: int = 192,
    leaf_cap: int = 32,
    artifact_path: Path | None = None,
) -> tuple[float, float]:
    """Predicted per-insert fallback rate interval [lower, upper] after Phase 4C
    concurrent leaf capacity class expansion under parent expected-version coupling
    on the W=1 uniform random workload.

    Sources:
      Leis, Scheibner, Kemper & Neumann, DaMoN 2016 §3 (optimistic lock coupling).
      The pre-4C W=1 fallback cause partitions in `PRE_4C_W1_CAUSES` (Refs #568);
      pass `artifact_path` to derive from another artifact's W=1 row.

    Derivation (§8.14):
      At W=1, all fallbacks are deterministic structural transitions.
      Phase 4C eliminates insert-side capacity class growth within linear leaves
      (`cap_expansion_class`) and map bitmap-leaf sub-expanses (`cap_expansion_map_bitmap_sub`)
      by executing reallocations concurrently under parent expected-version coupling.
      The capacity transitions that remain serialized are:
      1. Linear leaf full (`cap_expansion_leaf_full`): level 1 converts to LeafB1 at 25,
         level >= 2 splits into BranchL3+ at 32.
      2. Bitmap leaf near-full (`cap_expansion_bitmap_near_full`): set converts to FullExpanse
         at 256, map near-full guard at 254.
      3. Immediate-to-leaf conversions (`immediate_conversion`).
      4. Residual BranchB to BranchU upgrades bounded by Phase 4D's upgrade ceiling.

      Residual lower bound (zero BranchU upgrades):
        residual_lower = cap_expansion_leaf_full + cap_expansion_bitmap_near_full + immediate_conversion
      Residual upper bound (maximum BranchU upgrade rate):
        residual_upper = residual_lower + branch_split / (BRANCHB_UP + 1 - (leaf_cap + 1))

    Returns:
      (residual_lower, residual_upper) as per-insert fallback rates.
    """
    if arm not in ("set", "map"):
        raise ValueError(f"arm must be 'set' or 'map', got '{arm}'")
    if branchb_up < 1:
        raise ValueError(f"branchb_up must be >= 1, got {branchb_up}")
    if leaf_cap < 1:
        raise ValueError(f"leaf_cap must be >= 1, got {leaf_cap}")
    k0 = leaf_cap + 1
    if branchb_up + 1 <= k0:
        raise ValueError(f"branchb_up + 1 ({branchb_up + 1}) must be > k0 ({k0})")
    if artifact_path is None:
        causes = PRE_4C_W1_CAUSES[arm]
        f_leaf_full = causes["cap_expansion_leaf_full"]
        f_bm_near_full = causes["cap_expansion_bitmap_near_full"]
        f_ic = causes["immediate_conversion"]
        f_bs = causes["branch_split"]
    else:
        d = json.loads(artifact_path.read_text())
        row = next(
            (r for r in d.get("throughput", []) if r.get("arm") == arm and r.get("writers") == 1),
            None,
        )
        if not row:
            raise ValueError(f"{artifact_path}: no W=1 throughput row found for arm '{arm}'")
        per_ins = row["fallback_causes_per_insert"]
        total_ops = row.get("write_ops", 1048576)
        ce_subsets = row.get("cap_expansion_subsets_total", {})
        f_leaf_full = ce_subsets.get("leaf_full", 0) / total_ops
        f_bm_near_full = ce_subsets.get("bitmap_near_full", 0) / total_ops
        f_ic = per_ins["immediate_conversion"]
        f_bs = per_ins.get("branch_split", 0.0)

    res_lower = f_leaf_full + f_bm_near_full + f_ic
    max_upgrade_fraction = 1.0 / (branchb_up + 1 - k0)
    res_upper = res_lower + f_bs * max_upgrade_fraction
    return (res_lower, res_upper)


def immediate_conversion_predicted_fallback_rate(
    arm: str,
    branchb_up: int = 192,
    leaf_cap: int = 32,
    artifact_path: Path | None = None,
) -> tuple[float, float]:
    """Predicted per-insert fallback rate interval [lower, upper] after
    concurrent immediate-to-leaf conversion and immediate expansion under parent expected-version
    coupling on the W=1 uniform random workload.

    Sources:
      Leis, Scheibner, Kemper & Neumann, DaMoN 2016 §3 (optimistic lock coupling).
      The pre-4C W=1 fallback cause partitions in `PRE_4C_W1_CAUSES` (Refs #568);
      pass `artifact_path` to derive from another artifact's W=1 row.

    Derivation (§8.14):
      At W=1, all fallbacks are deterministic structural transitions.
      Concurrent leaf capacity expansion eliminated insert-side capacity class growth
      within linear leaves (`cap_expansion_class`) and map bitmap-leaf sub-expanses
      (`cap_expansion_map_bitmap_sub`).
      Concurrent immediate conversion eliminates insert-side immediate expansions and
      immediate-to-leaf conversions (`immediate_conversion`).
      The capacity transitions that remain serialized are:
      1. Linear leaf full (`cap_expansion_leaf_full`): level 1 converts to LeafB1 at 25,
         level >= 2 splits into BranchL3+ at 32 (addressed in Phase 4E).
      2. Bitmap leaf near-full (`cap_expansion_bitmap_near_full`): set converts to FullExpanse
         at 256, map near-full guard at 254.
      3. Residual BranchB to BranchU upgrades bounded by Phase 4D's upgrade ceiling.

      Residual lower bound (zero BranchU upgrades):
        residual_lower = cap_expansion_leaf_full + cap_expansion_bitmap_near_full
      Residual upper bound (maximum BranchU upgrade rate):
        residual_upper = residual_lower + branch_split / (BRANCHB_UP + 1 - (leaf_cap + 1))

    Returns:
      (residual_lower, residual_upper) as per-insert fallback rates.
    """
    if arm not in ("set", "map"):
        raise ValueError(f"arm must be 'set' or 'map', got '{arm}'")
    if branchb_up < 1:
        raise ValueError(f"branchb_up must be >= 1, got {branchb_up}")
    if leaf_cap < 1:
        raise ValueError(f"leaf_cap must be >= 1, got {leaf_cap}")
    k0 = leaf_cap + 1
    if branchb_up + 1 <= k0:
        raise ValueError(f"branchb_up + 1 ({branchb_up + 1}) must be > k0 ({k0})")
    if artifact_path is None:
        causes = PRE_4C_W1_CAUSES[arm]
        f_leaf_full = causes["cap_expansion_leaf_full"]
        f_bm_near_full = causes["cap_expansion_bitmap_near_full"]
        f_bs = causes["branch_split"]
    else:
        d = json.loads(artifact_path.read_text())
        row = next(
            (r for r in d.get("throughput", []) if r.get("arm") == arm and r.get("writers") == 1),
            None,
        )
        if not row:
            raise ValueError(f"{artifact_path}: no W=1 throughput row found for arm '{arm}'")
        per_ins = row["fallback_causes_per_insert"]
        total_ops = row.get("write_ops", 1048576)
        ce_subsets = row.get("cap_expansion_subsets_total", {})
        f_leaf_full = ce_subsets.get("leaf_full", 0) / total_ops
        f_bm_near_full = ce_subsets.get("bitmap_near_full", 0) / total_ops
        f_bs = per_ins.get("branch_split", 0.0)

    res_lower = f_leaf_full + f_bm_near_full
    max_upgrade_fraction = 1.0 / (branchb_up + 1 - k0)
    res_upper = res_lower + f_bs * max_upgrade_fraction
    return (res_lower, res_upper)


def leaf_split_predicted_fallback_rate(
    arm: str,
    branchb_up: int = 192,
    leaf_cap: int = 32,
    artifact_path: Path | None = None,
) -> tuple[float, float]:
    """Predicted per-insert fallback rate interval [lower, upper] after
    concurrent linear leaf split and branch conversion under parent expected-version
    coupling on the W=1 uniform random workload.

    Sources:
      Leis, Scheibner, Kemper & Neumann, DaMoN 2016 §3 (optimistic lock coupling).
      The pre-4C W=1 fallback cause partitions in `PRE_4C_W1_CAUSES` (Refs #568);
      pass `artifact_path` to derive from another artifact's W=1 row.

    Derivation (§8.14):
      At W=1, all fallbacks are deterministic structural transitions.
      Concurrent leaf capacity expansion eliminated insert-side capacity class growth
      within linear leaves (`cap_expansion_class`) and map bitmap-leaf sub-expanses
      (`cap_expansion_map_bitmap_sub`).
      Concurrent immediate conversion eliminated insert-side immediate expansions and
      immediate-to-leaf conversions (`immediate_conversion`).
      Concurrent leaf split eliminates linear leaf full conversions to bitmap leaves and
      branch cascades (`cap_expansion_leaf_full`).
      The capacity transitions that remain serialized on insert are:
      1. Bitmap leaf near-full (`cap_expansion_bitmap_near_full`): set converts to FullExpanse
         at 256, map near-full guard at 254. On uniform random workloads, this is 0.000%.
      2. Residual BranchB to BranchU upgrades bounded by Phase 4D's upgrade ceiling.

      Residual lower bound (zero BranchU upgrades):
        residual_lower = cap_expansion_bitmap_near_full
      Residual upper bound (maximum BranchU upgrade rate):
        residual_upper = residual_lower + branch_split / (BRANCHB_UP + 1 - (leaf_cap + 1))

    Returns:
      (residual_lower, residual_upper) as per-insert fallback rates.
    """
    if arm not in ("set", "map"):
        raise ValueError(f"arm must be 'set' or 'map', got '{arm}'")
    if branchb_up < 1:
        raise ValueError(f"branchb_up must be >= 1, got {branchb_up}")
    if leaf_cap < 1:
        raise ValueError(f"leaf_cap must be >= 1, got {leaf_cap}")
    k0 = leaf_cap + 1
    if branchb_up + 1 <= k0:
        raise ValueError(f"branchb_up + 1 ({branchb_up + 1}) must be > k0 ({k0})")
    if artifact_path is None:
        causes = PRE_4C_W1_CAUSES[arm]
        f_bm_near_full = causes["cap_expansion_bitmap_near_full"]
        f_bs = causes["branch_split"]
    else:
        d = json.loads(artifact_path.read_text())
        row = next(
            (r for r in d.get("throughput", []) if r.get("arm") == arm and r.get("writers") == 1),
            None,
        )
        if not row:
            raise ValueError(f"{artifact_path}: no W=1 throughput row found for arm '{arm}'")
        per_ins = row["fallback_causes_per_insert"]
        total_ops = row.get("write_ops", 1048576)
        ce_subsets = row.get("cap_expansion_subsets_total", {})
        f_bm_near_full = ce_subsets.get("bitmap_near_full", 0) / total_ops
        f_bs = per_ins.get("branch_split", 0.0)

    res_lower = f_bm_near_full
    max_upgrade_fraction = 1.0 / (branchb_up + 1 - k0)
    res_upper = res_lower + f_bs * max_upgrade_fraction
    return (res_lower, res_upper)


# Aliases for engine-condition naming discipline (AGENTS.md / GEMINI.md):
# Concrete engine conditions instead of ephemeral plan identifiers.
branch_upgrade_predicted_fallback_rate = phase4d_predicted_fallback_rate
leaf_expansion_predicted_fallback_rate = phase4c_predicted_fallback_rate
phase4b_predicted_fallback_rate = immediate_conversion_predicted_fallback_rate
leaf_split_predicted_fallback_rate_alias = leaf_split_predicted_fallback_rate
phase4e_predicted_fallback_rate = leaf_split_predicted_fallback_rate



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
        # k = 1, measured t_line = 33.37 ns, set t_hold = 14.04 ns: 1 / 47.41 ns ≈ 21.09 M ops/s.
        self.assertAlmostEqual(contended_rmw_ceiling(1, 33.37, 14.04) / 1e6, 1000.0 / 47.41, places=9)
        # k = 10, measured t_line = 33.37 ns, set t_hold = 14.04 ns: 1 / 347.74 ns ≈ 2.876 M ops/s.
        self.assertAlmostEqual(contended_rmw_ceiling(10, 33.37, 14.04) / 1e6, 1000.0 / 347.74, places=9)

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
        p = shape_bound("ffi_disjoint_padded", 30.0, 20.0)
        d = shape_bound("ffi_disjoint_default", 30.0, 20.0)
        b = shape_bound("baseline_measured", 30.0, 20.0)
        c = shape_bound("core_shared_prefix", 30.0, 50.0)
        self.assertEqual((p["k"], d["k"], b["k"], c["k"]), (1, 2, 10, 5))
        self.assertAlmostEqual(p["ceiling_mops"], 20.0, places=9)
        self.assertAlmostEqual(d["ceiling_mops"], 12.5, places=9)
        self.assertAlmostEqual(b["ceiling_mops"], 1000.0 / 320.0, places=9)
        self.assertAlmostEqual(c["ceiling_mops"], 5.0, places=9)

    def test_invalid_arguments_raise(self):
        for bad in ((0, 30.0, 15.0), (-1, 30.0, 15.0), (1, 0.0, 15.0), (1, 30.0, -1.0)):
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
            lh = lock_hold_ns(suite, arm)
            self.assertGreater(lh["median"], 0.0)
            self.assertLessEqual(lh["min"], lh["median"])
            self.assertLessEqual(lh["median"], lh["max"])

    def test_phase4d_predicted_fallback_rate(self):
        set_lo, set_hi = phase4d_predicted_fallback_rate("set")
        # set: 78.69% -> 3.52–3.99%
        self.assertAlmostEqual(set_lo * 100, 3.52, delta=0.01)
        self.assertAlmostEqual(set_hi * 100, 3.99, delta=0.01)
        self.assertLess(set_lo, set_hi)

        map_lo, map_hi = phase4d_predicted_fallback_rate("map")
        # map: 24.51% -> 14.75–14.81%
        self.assertAlmostEqual(map_lo * 100, 14.75, delta=0.01)
        self.assertAlmostEqual(map_hi * 100, 14.81, delta=0.01)
        self.assertLess(map_lo, map_hi)

        # The artifact path reads a W=1 row the same way: an artifact carrying
        # the frozen inputs reproduces the frozen prediction.
        import tempfile
        rows = [{"arm": arm, "writers": 1, "fallback_causes_per_insert": causes}
                for arm, causes in PRE_4D_W1_CAUSES.items()]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "pre4d.json"
            path.write_text(json.dumps({"throughput": rows}))
            for arm in ("set", "map"):
                self.assertEqual(phase4d_predicted_fallback_rate(arm, artifact_path=path),
                                 phase4d_predicted_fallback_rate(arm))

        with self.assertRaises(ValueError):
            phase4d_predicted_fallback_rate("str")
        with self.assertRaises(ValueError):
            phase4d_predicted_fallback_rate("set", branchb_up=0)
        with self.assertRaises(ValueError):
            phase4d_predicted_fallback_rate("set", leaf_cap=0)
        with self.assertRaises(ValueError):
            phase4d_predicted_fallback_rate("set", branchb_up=10, leaf_cap=20)

    def test_phase4c_predicted_fallback_rate(self):
        set_lo, set_hi = phase4c_predicted_fallback_rate("set")
        # set: 3.52% -> 3.23% (lower bound with CapExpansionClass eliminated)
        self.assertAlmostEqual(set_lo * 100, 3.234, delta=0.02)
        self.assertAlmostEqual(set_hi * 100, 3.234, delta=0.02)
        self.assertLessEqual(set_lo, set_hi)

        map_lo, map_hi = phase4c_predicted_fallback_rate("map")
        # map: 14.75% -> 4.21% (eliminating 10.53% of all inserts that were CapExpansionClass)
        self.assertAlmostEqual(map_lo * 100, 4.214, delta=0.02)
        self.assertAlmostEqual(map_hi * 100, 4.214, delta=0.02)
        self.assertLessEqual(map_lo, map_hi)

        # Artifact path reader test
        import tempfile
        rows = [
            {
                "arm": arm,
                "writers": 1,
                "write_ops": 1048576,
                "fallback_causes_per_insert": {
                    "immediate_conversion": causes["immediate_conversion"],
                    "branch_split": causes["branch_split"],
                },
                "cap_expansion_subsets_total": {
                    "class": int(causes["cap_expansion_class"] * 1048576),
                    "leaf_full": int(causes["cap_expansion_leaf_full"] * 1048576),
                    "bitmap_near_full": int(causes["cap_expansion_bitmap_near_full"] * 1048576),
                    "map_bitmap_sub": int(causes["cap_expansion_map_bitmap_sub"] * 1048576),
                    "remove": int(causes["cap_expansion_remove"] * 1048576),
                },
            }
            for arm, causes in PRE_4C_W1_CAUSES.items()
        ]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "pre4c.json"
            path.write_text(json.dumps({"throughput": rows}))
            for arm in ("set", "map"):
                self.assertAlmostEqual(
                    phase4c_predicted_fallback_rate(arm, artifact_path=path)[0],
                    phase4c_predicted_fallback_rate(arm)[0],
                    places=7,
                )

        with self.assertRaises(ValueError):
            phase4c_predicted_fallback_rate("str")
        with self.assertRaises(ValueError):
            phase4c_predicted_fallback_rate("set", branchb_up=0)
        with self.assertRaises(ValueError):
            phase4c_predicted_fallback_rate("set", leaf_cap=0)
        with self.assertRaises(ValueError):
            phase4c_predicted_fallback_rate("set", branchb_up=10, leaf_cap=20)

    def test_immediate_conversion_predicted_fallback_rate(self):
        set_lo, set_hi = immediate_conversion_predicted_fallback_rate("set")
        self.assertAlmostEqual(set_lo, 17889 / 1048576, places=9)
        self.assertAlmostEqual(set_hi, 17889 / 1048576, places=9)
        self.assertAlmostEqual(set_lo * 100, 1.706028, places=5)

        map_lo, map_hi = immediate_conversion_predicted_fallback_rate("map")
        self.assertAlmostEqual(map_lo, 29779 / 1048576, places=9)
        self.assertAlmostEqual(map_hi, 29779 / 1048576, places=9)
        self.assertAlmostEqual(map_lo * 100, 2.839947, places=5)

        # Backward compatibility alias test
        self.assertEqual(phase4b_predicted_fallback_rate("set"), (set_lo, set_hi))
        self.assertEqual(phase4b_predicted_fallback_rate("map"), (map_lo, map_hi))

        # Derivation from synthetic artifact
        rows = [
            {
                "arm": arm,
                "writers": 1,
                "write_ops": 1048576,
                "fallback_causes_per_insert": {
                    "immediate_conversion": causes["immediate_conversion"],
                    "branch_split": causes["branch_split"],
                },
                "cap_expansion_subsets_total": {
                    "class": int(causes["cap_expansion_class"] * 1048576),
                    "leaf_full": int(causes["cap_expansion_leaf_full"] * 1048576),
                    "bitmap_near_full": int(causes["cap_expansion_bitmap_near_full"] * 1048576),
                    "map_bitmap_sub": int(causes["cap_expansion_map_bitmap_sub"] * 1048576),
                    "remove": int(causes["cap_expansion_remove"] * 1048576),
                },
            }
            for arm, causes in PRE_4C_W1_CAUSES.items()
        ]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "pre4c.json"
            path.write_text(json.dumps({"throughput": rows}))
            for arm in ("set", "map"):
                self.assertAlmostEqual(
                    immediate_conversion_predicted_fallback_rate(arm, artifact_path=path)[0],
                    immediate_conversion_predicted_fallback_rate(arm)[0],
                    places=7,
                )

        with self.assertRaises(ValueError):
            immediate_conversion_predicted_fallback_rate("str")
        with self.assertRaises(ValueError):
            immediate_conversion_predicted_fallback_rate("set", branchb_up=0)
        with self.assertRaises(ValueError):
            immediate_conversion_predicted_fallback_rate("set", leaf_cap=0)
        with self.assertRaises(ValueError):
            immediate_conversion_predicted_fallback_rate("set", branchb_up=10, leaf_cap=20)

    def test_leaf_split_predicted_fallback_rate(self):
        set_lo, set_hi = leaf_split_predicted_fallback_rate("set")
        self.assertAlmostEqual(set_lo, 0.0, places=9)
        self.assertAlmostEqual(set_hi, 0.0, places=9)
        self.assertAlmostEqual(set_lo * 100, 0.0, places=5)

        map_lo, map_hi = leaf_split_predicted_fallback_rate("map")
        self.assertAlmostEqual(map_lo, 0.0, places=9)
        self.assertAlmostEqual(map_hi, 0.0, places=9)
        self.assertAlmostEqual(map_lo * 100, 0.0, places=5)

        # Backward compatibility alias test
        self.assertEqual(phase4e_predicted_fallback_rate("set"), (set_lo, set_hi))
        self.assertEqual(phase4e_predicted_fallback_rate("map"), (map_lo, map_hi))

        # Derivation from synthetic artifact
        rows = [
            {
                "arm": arm,
                "writers": 1,
                "write_ops": 1048576,
                "fallback_causes_per_insert": {
                    "immediate_conversion": causes["immediate_conversion"],
                    "branch_split": causes["branch_split"],
                },
                "cap_expansion_subsets_total": {
                    "class": int(causes["cap_expansion_class"] * 1048576),
                    "leaf_full": int(causes["cap_expansion_leaf_full"] * 1048576),
                    "bitmap_near_full": int(causes["cap_expansion_bitmap_near_full"] * 1048576),
                    "map_bitmap_sub": int(causes["cap_expansion_map_bitmap_sub"] * 1048576),
                    "remove": int(causes["cap_expansion_remove"] * 1048576),
                },
            }
            for arm, causes in PRE_4C_W1_CAUSES.items()
        ]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "pre4c.json"
            path.write_text(json.dumps({"throughput": rows}))
            for arm in ("set", "map"):
                self.assertAlmostEqual(
                    leaf_split_predicted_fallback_rate(arm, artifact_path=path)[0],
                    leaf_split_predicted_fallback_rate(arm)[0],
                    places=7,
                )

        with self.assertRaises(ValueError):
            leaf_split_predicted_fallback_rate("str")
        with self.assertRaises(ValueError):
            leaf_split_predicted_fallback_rate("set", branchb_up=0)
        with self.assertRaises(ValueError):
            leaf_split_predicted_fallback_rate("set", leaf_cap=0)
        with self.assertRaises(ValueError):
            leaf_split_predicted_fallback_rate("set", branchb_up=10, leaf_cap=20)


# ---------------------------------------------------------------------------
# Ordered reads on the concurrent map (#900)
# ---------------------------------------------------------------------------
#
# What an optimistic predecessor or successor search must validate, and what
# that costs, derived from the single-threaded walks it is built on.
#
# `nav::prev` descends the child for the probe's digit with the probe's low
# bytes, and when that child yields nothing it scans the branch's lower live
# digits, searching each with the maximum remainder (crates/expanse/src/nav.rs,
# the BranchL3/L7, BranchB and BranchU arms of `prev`). Every node form returns
# a key for the maximum remainder when it is non-empty (the leaf, LeafB1,
# immediate and prefix-skipping branch arms of the same function), so the
# first live lower sibling always answers. A search therefore makes at most one
# sibling descent, from the deepest branch on its path that has a lower live
# digit. `nav::next` is the mirror image, with remainder 0. Branches sit at
# levels 8 down to 2 (`debug_assert!(level >= 2)` at every branch site in
# crates/expanse/src/mutate.rs; level 1 holds leaves).

BRANCH_TOP_LEVEL = 8
BRANCH_MIN_LEVEL = 2
#: The retry budget of every `Sync*` optimistic read (crates/expanse/src/sync.rs, `MAX_RETRIES`).
MAX_READ_RETRIES = 64


def get_read_set_branches() -> int:
    """Branch versions a point lookup validates at most: one per level from
    `BRANCH_TOP_LEVEL` down to `BRANCH_MIN_LEVEL`, plus none for the terminal,
    which its parent's version covers."""
    return BRANCH_TOP_LEVEL - BRANCH_MIN_LEVEL + 1


def ordered_read_set_branches(backtrack_level: int) -> int:
    """Branch versions an ordered search validates at most when it leaves the
    probe's path at the branch at `backtrack_level`.

    The path from the root down to that branch is `BRANCH_TOP_LEVEL -
    backtrack_level + 1` branches. Below it, the failed descent along the probe
    and the one sibling descent each cross at most `backtrack_level -
    BRANCH_MIN_LEVEL` more. Every one of them decided the answer, so all of
    them are in the read set. The tree version is one more, and is not counted
    here.
    """
    if not BRANCH_MIN_LEVEL <= backtrack_level <= BRANCH_TOP_LEVEL:
        raise ValueError(f"backtrack_level must be in [{BRANCH_MIN_LEVEL}, {BRANCH_TOP_LEVEL}], got {backtrack_level}")
    below = backtrack_level - BRANCH_MIN_LEVEL
    return (BRANCH_TOP_LEVEL - backtrack_level + 1) + 2 * below


def max_ordered_read_set_branches() -> int:
    """The largest `ordered_read_set_branches` over every backtrack level."""
    return max(ordered_read_set_branches(level)
               for level in range(BRANCH_MIN_LEVEL, BRANCH_TOP_LEVEL + 1))


def version_word_cost(nodes: int, retained: bool) -> tuple[int, int]:
    """`(version loads, fences)` to validate `nodes` branch versions.

    A hand-over-hand walk (`get`) samples each node once (`node_sample`: one
    load) and validates it once before dereferencing a pointer read from it
    (`node_validate`: one fence and one load), crates/expanse/src/occ.rs. A
    retained read set does the same per step, because a racily loaded child
    pointer still has to be validated before it is followed, and then
    re-validates every node once more after the last load, behind one shared
    fence. This is the cost model of that design, not a count taken from
    code that exists; Callgrind decides once it does.
    """
    if nodes < 0:
        raise ValueError(f"nodes must be >= 0, got {nodes}")
    if not retained:
        return 2 * nodes, nodes
    return 3 * nodes, nodes + (1 if nodes else 0)


def failed_attempt_share(read_ops: int, read_attempts: int, read_fallbacks: int) -> float:
    """Share of optimistic attempts that failed validation, from the reader counters.

    An operation that succeeded optimistically made exactly one successful
    attempt; one that fell back made only failed attempts. So the successful
    attempts number `read_ops - read_fallbacks`, and the rest failed.
    """
    if min(read_ops, read_attempts, read_fallbacks) < 0:
        raise ValueError("counters cannot be negative")
    if read_fallbacks > read_ops:
        raise ValueError(f"read_fallbacks ({read_fallbacks}) cannot exceed read_ops ({read_ops})")
    succeeded = read_ops - read_fallbacks
    if read_attempts < succeeded:
        raise ValueError(f"read_attempts ({read_attempts}) cannot be below the successful attempts ({succeeded})")
    if read_attempts == 0:
        raise ValueError("no attempts: the share is undefined")
    return (read_attempts - succeeded) / read_attempts


def per_node_failure(attempt_failure: float, nodes: int) -> float:
    """Per-node validation failure probability implied by an attempt failure
    probability over `nodes` validated nodes, if nodes fail independently:
    `1 - (1 - p)^(1/nodes)`. The independence is a hypothesis; writes that
    concentrate near the probe break it."""
    if not 0.0 <= attempt_failure < 1.0:
        raise ValueError(f"attempt_failure must be in [0, 1), got {attempt_failure}")
    if nodes < 1:
        raise ValueError(f"nodes must be >= 1, got {nodes}")
    return 1.0 - math.pow(1.0 - attempt_failure, 1.0 / nodes)


def attempt_failure(node_failure: float, nodes: int) -> float:
    """Probability an attempt fails when each of `nodes` validated nodes fails
    independently with `node_failure`: `1 - (1 - q)^nodes`."""
    if not 0.0 <= node_failure <= 1.0:
        raise ValueError(f"node_failure must be in [0, 1], got {node_failure}")
    if nodes < 0:
        raise ValueError(f"nodes must be >= 0, got {nodes}")
    return 1.0 - math.pow(1.0 - node_failure, nodes)


def fallback_share(attempt_failure_p: float, max_retries: int = MAX_READ_RETRIES) -> float:
    """Share of operations that exhaust every attempt and take the writer
    mutex, if attempts fail independently: `p^max_retries`."""
    if not 0.0 <= attempt_failure_p <= 1.0:
        raise ValueError(f"attempt_failure_p must be in [0, 1], got {attempt_failure_p}")
    if max_retries < 1:
        raise ValueError(f"max_retries must be >= 1, got {max_retries}")
    return math.pow(attempt_failure_p, max_retries)


def expected_attempts(attempt_failure_p: float, max_retries: int = MAX_READ_RETRIES) -> float:
    """Expected optimistic attempts per operation, capped at `max_retries`:
    the truncated geometric sum `(1 - p^M) / (1 - p)`, which is `M` at `p = 1`."""
    if not 0.0 <= attempt_failure_p <= 1.0:
        raise ValueError(f"attempt_failure_p must be in [0, 1], got {attempt_failure_p}")
    if max_retries < 1:
        raise ValueError(f"max_retries must be >= 1, got {max_retries}")
    if attempt_failure_p == 1.0:
        return float(max_retries)
    return (1.0 - math.pow(attempt_failure_p, max_retries)) / (1.0 - attempt_failure_p)


#: Committed reader health cells for the concurrent map: 8 optimistic readers
#: against 1-8 writers, HOT and Masstree FFI suites, one-thread-per-core pin `0-15`.
#: The HOT cell is the frozen `a1982ff2` pair under `results/step0/`, which the
#: projection in `docs/benchmarks/concurrency/METHODOLOGY.md` §12.1 is dated to;
#: the suite's top-level `baseline_concurrent.json` is its current publication.
ORDERED_HEALTH_ARTIFACTS = (
    REPO_ROOT / "docs" / "benchmarks" / "hot_comparison" / "results" / "step0" / "baseline_concurrent.json",
    REPO_ROOT / "docs" / "benchmarks" / "hot_comparison" / "results" / "multi_writer_olc" / "baseline_concurrent_ab.json",
    REPO_ROOT / "docs" / "benchmarks" / "masstree_comparison" / "results" / "multi_writer_olc" / "baseline_concurrent_ab.json",
)


def map_read_health(paths: tuple[Path, ...] = ORDERED_HEALTH_ARTIFACTS) -> list[dict]:
    """`get` attempt-failure shares from the committed `map` reader health cells.

    One row per cell: its artifact, engine commit, writers, readers and the
    `failed_attempt_share` of the summed `rounds_raw` counters. A cell without
    reader counters is skipped, not read as zero (AGENTS.md 8.1). The engine
    has changed since every one of these commits, so a projection built on
    them is dated to its commit and is not a measurement of `main`.
    """
    rows = []
    for path in paths:
        obj = json.loads(Path(path).read_text())
        commit = str(obj.get("provenance", {}).get("commit", "unknown"))[:8]
        for cell in obj.get("health", []):
            if cell.get("arm") != "map":
                continue
            rr = cell.get("rounds_raw") or []
            ops = sum(r.get("read_ops", 0) for r in rr)
            att = sum(r.get("read_attempts", 0) for r in rr)
            fb = sum(r.get("read_fallbacks", 0) for r in rr)
            if not att:
                continue
            rows.append({
                "artifact": Path(path).relative_to(REPO_ROOT).as_posix(),
                "commit": commit,
                "writers": cell.get("writers"),
                "readers": cell.get("readers"),
                "attempt_failure": failed_attempt_share(ops, att, fb),
            })
    return rows


def ordered_projection(get_attempt_failure: float, get_nodes: int, ordered_nodes: int) -> dict[str, float]:
    """An ordered read's attempt failure, expected attempts and fallback share,
    projected from a `get` attempt failure over `get_nodes` validated nodes to
    `ordered_nodes`, under the per-node independence hypothesis."""
    q = per_node_failure(get_attempt_failure, get_nodes)
    p = attempt_failure(q, ordered_nodes)
    return {
        "node_failure": q,
        "attempt_failure": p,
        "expected_attempts": expected_attempts(p),
        "fallback_share": fallback_share(p),
    }


class TestOrderedReadBounds(unittest.TestCase):
    def test_read_set_sizes(self):
        self.assertEqual(get_read_set_branches(), 7)
        # Backtracking at the root: 1 path branch, then 6 + 6 below it.
        self.assertEqual(ordered_read_set_branches(8), 13)
        # At the lowest branch the sibling is a terminal: the read set is get's.
        self.assertEqual(ordered_read_set_branches(2), 7)
        self.assertEqual(ordered_read_set_branches(5), 10)
        self.assertEqual(max_ordered_read_set_branches(), 13)
        for level in range(BRANCH_MIN_LEVEL, BRANCH_TOP_LEVEL + 1):
            self.assertEqual(ordered_read_set_branches(level), level + 5)
            self.assertGreaterEqual(ordered_read_set_branches(level), get_read_set_branches())

    def test_version_word_cost(self):
        self.assertEqual(version_word_cost(7, retained=False), (14, 7))
        self.assertEqual(version_word_cost(13, retained=True), (39, 14))
        self.assertEqual(version_word_cost(0, retained=True), (0, 0))

    def test_failed_attempt_share(self):
        self.assertAlmostEqual(failed_attempt_share(100, 125, 0), 0.2)
        # Two fallbacks: 98 successful attempts of 225, the rest failed.
        self.assertAlmostEqual(failed_attempt_share(100, 225, 2), 127 / 225)
        self.assertEqual(failed_attempt_share(10, 10, 0), 0.0)

    def test_independence_round_trip(self):
        q = per_node_failure(0.2, 4)
        self.assertAlmostEqual(q, 1.0 - 0.8 ** 0.25, places=12)
        self.assertAlmostEqual(attempt_failure(q, 4), 0.2, places=12)
        self.assertAlmostEqual(attempt_failure(0.1, 2), 0.19)
        self.assertEqual(attempt_failure(0.3, 0), 0.0)

    def test_fallback_and_attempts(self):
        self.assertAlmostEqual(fallback_share(0.5, 3), 0.125)
        self.assertAlmostEqual(expected_attempts(0.5, 3), 1.75)
        self.assertEqual(expected_attempts(0.0), 1.0)
        self.assertEqual(expected_attempts(1.0, 64), 64.0)
        self.assertLess(fallback_share(0.2), 1e-40)

    def test_ordered_projection(self):
        # get fails 10% of attempts over 4 nodes; the same per-node rate over 13 nodes.
        proj = ordered_projection(0.1, 4, 13)
        q = 1.0 - 0.9 ** 0.25
        self.assertAlmostEqual(proj["node_failure"], q, places=12)
        self.assertAlmostEqual(proj["attempt_failure"], 1.0 - (1.0 - q) ** 13, places=12)
        self.assertAlmostEqual(proj["expected_attempts"],
                               (1.0 - proj["attempt_failure"] ** 64) / (1.0 - proj["attempt_failure"]), places=12)
        # Same node count: the projection returns get's own share.
        self.assertAlmostEqual(ordered_projection(0.1, 7, 7)["attempt_failure"], 0.1, places=12)

    def test_map_read_health_reads_counters(self):
        with tempfile.TemporaryDirectory() as td:
            p = Path(td) / "health.json"
            p.write_text(json.dumps({
                "provenance": {"commit": "abcdef1234"},
                "health": [
                    {"arm": "map", "writers": 2, "readers": 8,
                     "rounds_raw": [{"read_ops": 100, "read_attempts": 120, "read_fallbacks": 0},
                                    {"read_ops": 100, "read_attempts": 105, "read_fallbacks": 1}]},
                    {"arm": "set", "writers": 2, "readers": 8,
                     "rounds_raw": [{"read_ops": 1, "read_attempts": 9, "read_fallbacks": 0}]},
                    {"arm": "map", "writers": 4, "readers": 8, "rounds_raw": []},
                ],
            }))
            global REPO_ROOT
            saved = REPO_ROOT
            try:
                REPO_ROOT = Path(td)
                rows = map_read_health((p,))
            finally:
                REPO_ROOT = saved
        self.assertEqual(len(rows), 1)
        self.assertEqual((rows[0]["commit"], rows[0]["writers"]), ("abcdef12", 2))
        # 199 successful attempts of 225.
        self.assertAlmostEqual(rows[0]["attempt_failure"], 26 / 225)

    def test_invalid_arguments_raise(self):
        for call in (
            lambda: ordered_read_set_branches(1),
            lambda: ordered_read_set_branches(9),
            lambda: version_word_cost(-1, retained=True),
            lambda: failed_attempt_share(10, 5, 0),
            lambda: failed_attempt_share(10, 20, 11),
            lambda: failed_attempt_share(0, 0, 0),
            lambda: per_node_failure(1.0, 3),
            lambda: per_node_failure(0.1, 0),
            lambda: attempt_failure(1.5, 3),
            lambda: fallback_share(0.5, 0),
            lambda: expected_attempts(-0.1),
        ):
            with self.assertRaises(ValueError):
                call()


def report() -> None:
    lt = line_transfer_ns()
    t_line = lt["median"]
    print("Stage B contention bounds (#568 plan PR 4) — inputs from the committed artifacts")
    print(f"  t_line (spinning, P-core pairs, median of {lt['cells']} cell means): {t_line:.2f} ns "
          f"[cells span {lt['min']:.2f}–{lt['max']:.2f}]")
    holds = {}
    for suite, arm in (("hot_comparison", "set"), ("hot_comparison", "map"), ("masstree_comparison", "map")):
        u = w1_insert_rate(suite, arm)
        lh = lock_hold_ns(suite, arm)
        holds[(suite, arm)] = lh["median"]
        print(f"  W=1 insert rate, merged engine, {suite}/{arm}: [{u['union_lower']:.2f}, {u['union_upper']:.2f}] M/s "
              f"(measured t_hold: {lh['median']:.1f} ns [span {lh['min']:.1f}–{lh['max']:.1f}])")
    print()
    print("Predicted Fallback Rates after Leaf Expansion (W=1 uniform random, Refs #568):")
    for arm in ("set", "map"):
        c_lo, c_hi = leaf_expansion_predicted_fallback_rate(arm)
        d_lo, d_hi = branch_upgrade_predicted_fallback_rate(arm)
        print(f"  {arm}: post-branch-upgrade {d_lo*100:.2f}% -> predicted post-leaf-expansion [{c_lo*100:.2f}%, {c_hi*100:.2f}%] "
              f"(eliminating CapExpansionClass: {PRE_4C_W1_CAUSES[arm]['cap_expansion_class']*100:.2f}%)")
    print()
    print("Predicted Fallback Rates after Immediate Conversion (W=1 uniform random, Refs #568):")
    for arm in ("set", "map"):
        b_lo, b_hi = immediate_conversion_predicted_fallback_rate(arm)
        c_lo, c_hi = leaf_expansion_predicted_fallback_rate(arm)
        print(f"  {arm}: post-leaf-expansion {c_lo*100:.2f}% -> predicted post-immediate-conversion [{b_lo*100:.2f}%, {b_hi*100:.2f}%] "
              f"(eliminating ImmediateConversion: {PRE_4C_W1_CAUSES[arm]['immediate_conversion']*100:.2f}%)")
    print()
    print("Predicted Fallback Rates after Leaf Split (W=1 uniform random, Refs #568):")
    for arm in ("set", "map"):
        e_lo, e_hi = leaf_split_predicted_fallback_rate(arm)
        b_lo, b_hi = immediate_conversion_predicted_fallback_rate(arm)
        print(f"  {arm}: post-immediate-conversion {b_lo*100:.2f}% -> predicted post-leaf-split [{e_lo*100:.2f}%, {e_hi*100:.2f}%] "
              f"(eliminating CapExpansionLeafFull: {PRE_4C_W1_CAUSES[arm]['cap_expansion_leaf_full']*100:.2f}%)")
    print()
    print("Ordered reads on the concurrent map (#900), derived from nav.rs:")
    print(f"  branch versions validated: get <= {get_read_set_branches()}; ordered read <= {max_ordered_read_set_branches()} "
          f"(backtracking at level l: l + 5); the tree version is one more")
    for nodes, retained in ((get_read_set_branches(), False), (max_ordered_read_set_branches(), True)):
        loads, fences = version_word_cost(nodes, retained)
        print(f"  {'retained read set' if retained else 'hand-over-hand'} over {nodes} nodes: {loads} version loads, {fences} fences (cost model)")
    print("  projected from committed get attempt-failure shares (independence hypothesis; dated to each artifact's commit):")
    for row in map_read_health():
        for get_nodes in (3, 5, 7):
            pr = ordered_projection(row["attempt_failure"], get_nodes, max_ordered_read_set_branches())
            print(f"    {row['artifact']} @{row['commit']} W={row['writers']} R={row['readers']}: get fails "
                  f"{row['attempt_failure']:.2%} of attempts; over {get_nodes} -> 13 nodes an ordered read fails "
                  f"{pr['attempt_failure']:.2%}, {pr['expected_attempts']:.2f} attempts/op, fallback share {pr['fallback_share']:.1e}")
    print()
    print("Contention ceilings by workload shape (evaluated against measured t_line = 33.37 ns):")
    t_hold_set = holds[("hot_comparison", "set")]
    t_hold_map = holds[("hot_comparison", "map")]
    for shape in ("ffi_disjoint_padded", "ffi_disjoint_default", "baseline_measured"):
        k = K_LINES[shape]
        c_set = contended_rmw_ceiling(k, t_line, t_hold_set) / 1e6
        c_map = contended_rmw_ceiling(k, t_line, t_hold_map) / 1e6
        print(f"  {shape}: k = {k} -> ceiling {c_set:.2f} M ops/s (set, t_hold={t_hold_set:.1f} ns) | "
              f"{c_map:.2f} M ops/s (map, t_hold={t_hold_map:.1f} ns)")
    c_prefix = contended_rmw_ceiling(K_LINES["core_shared_prefix"], t_line, T_HOLD_CASCADE_HYPOTHESIS_NS) / 1e6
    print(f"  core_shared_prefix: k = {K_LINES['core_shared_prefix']}, t_hold = {T_HOLD_CASCADE_HYPOTHESIS_NS:.0f} ns (hypothesis) -> ceiling {c_prefix:.2f} M ops/s")
    print(f"  allocator counter line: {allocator_counter_ceiling(t_line) / 1e6:.2f} M allocs/s on the allocating inserts")
    for w in (2, 4, 8, 16):
        r = restart_ceiling(w, t_hold_set, 1e9 / 5.4e6)
        print(f"  restart ceiling at W={w} (measured set t_hold {t_hold_set:.1f} ns, t_op from 5.4 M/s, safety 2x): {r:.2f} restarts/op")
    print()


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.argv = [sys.argv[0]]
        unittest.main()
    report()
    unittest.main(argv=[sys.argv[0]])
