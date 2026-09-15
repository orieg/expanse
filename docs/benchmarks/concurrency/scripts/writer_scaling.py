#!/usr/bin/env python3
"""Expanse-native writer scaling driver across physical P-cores (Refs #568, Phase 1.5D).

Runs `crates/expanse/examples/writer_scaling.rs` across W in {1, 2, 4, 8} on physical
P-cores for map (64-bit), set (63-bit), and str arms.

Two builds, never one (AGENTS.md §6 / hot_concurrent.rs:36-42):
- Pass 1 (throughput): uninstrumented release build, interleaved across W within each round,
  balancing position and first-order carryover across rounds (Williams design).
  Emits elapsed_s and writer_mops. Refuses to run if occ-stats is enabled.
  Every timed cell runs in a harness process of its own (`--writers W --round r
  --position p`, METHODOLOGY.md §15): process-wide state carried from one cell into
  the next changed a W = 8 cell's throughput with the cell that ran before it.
- Pass 2 (counters): diagnostic build (--features occ-stats), captures exact lock_fallbacks
  and write_ops across all rounds. Refuses to emit elapsed_s or writer_mops. It times
  nothing, so it still runs every cell of an arm in one process.

A comparison (`--compare`, `--variants`, the ablation shorthands) runs the 2 x len(W)
(build, W) cells of each round in the order of that round's row of a Williams design
over those cells, one process per cell: over 2 x len(W) rounds every (build, W) cell
holds every position once and every ordered pair of cells is adjacent once.

Before writing, the driver checks every row against the schedule that asked for it
(build, round, position, W) and refuses an artifact that disagrees; the artifact
records `provenance.cell_isolation = "process"`.

Computes:
- expanse_writer_mops_mean as headline point estimate with BCa 95% bootstrap CI (AGENTS.md §8.4)
- expanse_writer_mops_median as auxiliary field for historical continuity
- scaling factor C(N) = T(W) / T(1) with paired bootstrap BCa 95% CI across interleaved rounds
- lock fallbacks and fallback rate from the diagnostic occ-stats pass
- str arm as the alpha=1 coarse-mutex reference curve (0 lock fallbacks by construction)

`--ordered-readers` (#900, `METHODOLOGY.md` §12.3-§12.5) is a separate sweep on
the map arm:
- cells probe in {uniform, hotspot} x (W, R) in {(0,1), (0,4), (1,4), (4,4)} x
  read_op in {prev_locked, prev}, 8 rounds by default;
- within each round the (read_op, W, R) cells of each probe block follow a
  Williams row, one harness process per cell, throughput pass then counters pass;
- it runs under the pin `0,2,4,6,8,10,12,14`, which it sets when unset and
  refuses to replace, except for a `--quick` smoke run written outside the
  committed results;
- it writes P12.4 (`read_fallbacks / read_ops` below 0.1% in every `prev` cell)
  and P12.5 (the per-round paired `prev` / `prev_locked` reader throughput ratio
  at W=1, R=4, uniform, with the W=0, R=1 control).

Usage:
    python3 docs/benchmarks/concurrency/scripts/writer_scaling.py --out docs/benchmarks/concurrency/results/baseline_writer_scaling.json
    python3 docs/benchmarks/concurrency/scripts/writer_scaling.py --ordered-readers
    python3 docs/benchmarks/concurrency/scripts/writer_scaling.py --self-test

`--self-test` is run by the `writer-scaling-selftest` CI job, gated on the
`concurrency-instrument` path filter: this file and the harness example it
builds and parses. `scripts/gate.sh` does not run it, because that mirrors CI's
`lint` and `test` jobs and this is neither -- run it by hand when changing
either end of the contract.
"""

from __future__ import annotations

import argparse
import collections
import contextlib
import datetime
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
import bca_bootstrap  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

# The construction labels the shared estimator can return, read from the module
# rather than re-listed here so a widened vocabulary cannot leave this file
# asserting an old one.
CI_METHODS = frozenset(
    v for k, v in vars(bca_bootstrap).items()
    if k.startswith("CI_METHOD_") and isinstance(v, str)
)
from bench_provenance import (  # noqa: E402
    MIN_WINDOW_S,
    begin_cell,
    end_cell,
    estimators,
    new_provenance,
)

THROUGHPUT_TARGET = REPO_ROOT / "target" / "throughput"
COUNTERS_TARGET = REPO_ROOT / "target" / "occ-stats"
# The six fallback causes the harness emits per counters row. They partition
# `lock_fallbacks` exactly; the harness refuses a row that breaks that, and the
# driver re-checks it so a schema drift cannot pass through silently (§8.1).
CAUSE_NAMES = (
    "cap_expansion",
    "immediate_conversion",
    "branch_split",
    "root_growth",
    "contention",
    "unknown_tag",
)

COMMITTED_RESULTS_PATH = (
    REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "baseline_writer_scaling.json"
)
DIAGNOSTIC_RESULTS_PATH = (
    REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "diagnostic_writer_scaling.json"
)
PADDED_RESULTS_PATH = (
    REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "padded_writer_scaling.json"
)
# Hypothesis D arms (METHODOLOGY.md §11): each shorthand flag compares one
# ablation feature against the default build.
ABLATION_ARMS = {
    "compare_ablation_alloc": "ablation-sharded-alloc",
    "compare_ablation_epoch": "ablation-striped-epoch",
    "compare_ablation_unstriped_freelist": "ablation-unstriped-freelist",
}
# Inverse ablations where the default build represents the promoted optimization
# and the ablation variant represents the unstriped/unoptimized baseline.
# For these, the reported scaling ratio is C_default(W) / C_variant(W) (§8.20.7).
INVERSE_ABLATIONS = {"ablation-unstriped-freelist"}

# METHODOLOGY.md §15: every timed writer-mode cell runs in a harness process of
# its own. Recorded in every artifact this driver writes, and required by
# `scripts/check_bench_provenance.py` of every writer-scaling artifact measured
# after the change.
CELL_ISOLATION = "process"
# The build label of the default (no-feature) build in a schedule and in
# `rounds_raw`. A variant is labelled by its feature list.
DEFAULT_BUILD = "default"
CELL_SCHEDULE_SWEEP = (
    "one harness process per timed (W, round) cell; within round r the W cells follow row r "
    "of a Williams design over the writer counts (position = index in that row); the counters "
    "pass times nothing and runs every cell of an arm in one process"
)
CELL_SCHEDULE_COMPARISON = (
    "one harness process per timed (build, W, round) cell; within round r the 2 x len(W) "
    "(build, W) cells follow row r of a Williams design over those cells, indexed "
    "(default, W1), (variant, W1), (default, W2), ... (position = index in that row); the "
    "counters passes time nothing and run every cell of an arm in one process per build"
)


def ratio_description(variant_list: list[str]) -> str:
    """The `provenance.estimators.ratio` text for a comparison run.

    Names the ratio each variant's comparison reports, in that comparison's own
    direction: an inverse ablation reports C_default(W) / C_variant(W).
    """
    clauses = [
        f"{v}: C_default(W) / C_variant(W), the default build over an inverse "
        "ablation that restores the replaced path"
        if v in INVERSE_ABLATIONS
        else f"{v}: C_variant(W) / C_default(W), the variant build over the default baseline"
        for v in variant_list
    ]
    return "Expanse paired scaling ratio per variant; " + "; ".join(clauses)

ABLATION_RESULTS_PATHS = tuple(
    REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / f"ablation_{arm}_writer_scaling.json"
    for arm in ("alloc", "epoch", "freelist", "unstriped_freelist")
)


def get_throughput_target(features: str | None = None) -> Path:
    if features:
        slug = features.replace(",", "_").replace(" ", "_").replace("-", "_")
        return REPO_ROOT / "target" / f"throughput-{slug}"
    return THROUGHPUT_TARGET


def get_counters_target(features: str | None = None) -> Path:
    if features:
        slug = features.replace(",", "_").replace(" ", "_").replace("-", "_")
        return REPO_ROOT / "target" / f"occ-stats-{slug}"
    return COUNTERS_TARGET


def get_binaries(features: str | None = None) -> tuple[Path, Path]:
    tp_target = get_throughput_target(features)
    cnt_target = get_counters_target(features)
    throughput_bin = tp_target / "release" / "examples" / "writer_scaling"
    counters_bin = cnt_target / "release" / "examples" / "writer_scaling"
    return throughput_bin, counters_bin


def _print_binary_info(label: str, path: Path) -> None:
    rel_path = (
        path.relative_to(REPO_ROOT) if path.is_relative_to(REPO_ROOT) else path.name
    )
    if path.exists():
        stat = path.stat()
        mtime_iso = datetime.datetime.fromtimestamp(
            stat.st_mtime, tz=datetime.timezone.utc
        ).strftime("%Y-%m-%d %H:%M:%SZ")
        print(
            f"  {label} binary: {rel_path} (mtime: {mtime_iso}, size: {stat.st_size} bytes)"
        )
    else:
        print(f"  {label} binary: {rel_path} (does not exist)")


def build_binaries(features: str | None = None, verbose: bool = True) -> tuple[Path, Path]:
    """Two builds, never one (AGENTS.md §6 / hot_concurrent.rs:36-42).

    Throughput comes from uninstrumented or targeted feature build (refuses occ-stats).
    Counters come from diagnostic build (--features occ-stats, refuses timing).
    Both builds use explicit, isolated target dirs so neither overwrites the other
    nor depends on external CARGO_TARGET_DIR environment settings.
    """
    throughput_bin, counters_bin = get_binaries(features)
    tp_target = get_throughput_target(features)
    feat_msg = f" (--features {features})" if features else " (default features, uninstrumented)"
    if verbose:
        print(f"building throughput binary{feat_msg} ...")
    tp_env = dict(os.environ)
    tp_env["CARGO_TARGET_DIR"] = str(tp_target)
    cmd = [
        "cargo",
        "build",
        "--release",
        "-p",
        "expanse-trie",
    ]
    if features:
        cmd.extend(["--features", features])
    cmd.extend(["--example", "writer_scaling"])
    subprocess.run(
        cmd,
        cwd=str(REPO_ROOT),
        env=tp_env,
        check=True,
    )
    if verbose:
        _print_binary_info("throughput", throughput_bin)

    cnt_target = get_counters_target(features)
    cnt_features = f"occ-stats,{features}" if features else "occ-stats"
    if verbose:
        print(f"building diagnostic counters binary (--features {cnt_features}) ...")
    cnt_env = dict(os.environ)
    cnt_env["CARGO_TARGET_DIR"] = str(cnt_target)
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "-p",
            "expanse-trie",
            "--features",
            cnt_features,
            "--example",
            "writer_scaling",
        ],
        cwd=str(REPO_ROOT),
        env=cnt_env,
        check=True,
    )
    if verbose:
        _print_binary_info("counters", counters_bin)

    return throughput_bin, counters_bin


def run_pass(
    binary: Path,
    role: str,
    arm: str,
    writers: list[int],
    rounds: int,
    round_opt: int | None = None,
    quick: bool = False,
) -> list[dict[str, Any]]:
    """Every cell of an arm in ONE harness process: the counters pass only.

    A timed cell never goes through here. Cells sharing a process share its
    allocator arenas and every other piece of process-wide state, and a W = 8
    cell's throughput moved with the cell that ran before it (METHODOLOGY.md
    §15). Timed cells go through `run_throughput_pass` / `run_comparison`, one
    process each. The counters pass reads exact counts and no clock.
    """
    cmd = [
        str(binary),
        "--role",
        role,
        "--arm",
        arm,
        "--writers",
        ",".join(str(w) for w in writers),
    ]
    if round_opt is not None:
        cmd.extend(["--round", str(round_opt)])
    else:
        cmd.extend(["--rounds", str(rounds)])
    if quick:
        cmd.append("--quick")

    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        sys.stderr.write(
            f"writer_scaling ({role}) failed (exit {proc.returncode}):\n{proc.stderr}\n"
        )
        sys.exit(proc.returncode)

    rows: list[dict[str, Any]] = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if line.startswith("{") and line.endswith("}"):
            try:
                row = json.loads(line)
                if row.get("role") == role and row.get("arm") == "expanse":
                    rows.append(row)
            except json.JSONDecodeError:
                continue

    expected_count = len(writers) * (1 if round_opt is not None else rounds)
    if len(rows) != expected_count:
        sys.stderr.write(
            f"Expected {expected_count} rows for role={role} arm={arm}, got {len(rows)}\n"
        )
        sys.exit(1)

    return rows


def writer_cell_schedule(
    writers_list: list[int], rounds: int, builds: tuple[str, ...] = (DEFAULT_BUILD,)
) -> list[dict[str, Any]]:
    """Every timed writer-mode harness invocation, in execution order.

    The treatments are the (build, W) cells, indexed (build_0, W_0), (build_1,
    W_0), ..., (build_0, W_1), .... Round r runs them in the order of row r of a
    Williams design (`williams_positions`), and `position` is the index in that
    row. With one build this is exactly the order the harness's own
    `williams_order` gave a multi-W invocation. For an even number of cells, as
    many rounds as cells put every cell in every position once and make every
    ordered pair of cells adjacent once.
    """
    if len(set(builds)) != len(builds):
        raise ValueError(f"build labels must be distinct, got {builds}")
    treatments = [(b, w) for w in writers_list for b in builds]
    out: list[dict[str, Any]] = []
    for r in range(rounds):
        for pos, idx in enumerate(williams_positions(len(treatments), r)):
            build, w = treatments[idx]
            out.append({"round": r, "position": pos, "writers": w, "build": build})
    return out


def writer_cell_argv(binary: Path, arm: str, cell: dict[str, Any], quick: bool) -> list[str]:
    """The harness command for ONE timed writer cell: one W, its round, its position."""
    cmd = [
        str(binary),
        "--role",
        "throughput",
        "--arm",
        arm,
        "--writers",
        str(cell["writers"]),
        "--round",
        str(cell["round"]),
        "--position",
        str(cell["position"]),
    ]
    if quick:
        cmd.append("--quick")
    return cmd


def writer_cell_row(stdout: str, arm: str, cell: dict[str, Any]) -> dict[str, Any]:
    """The one throughput row a single-cell process printed, checked against its schedule entry."""
    rows = []
    for line in stdout.splitlines():
        line = line.strip()
        if line.startswith("{") and line.endswith("}"):
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            if row.get("role") == "throughput" and row.get("arm") == "expanse":
                rows.append(row)
    if len(rows) != 1:
        raise RuntimeError(
            f"writer cell ({arm}, {cell}) emitted {len(rows)} throughput rows, expected exactly 1: "
            "a timed cell runs alone in its process (METHODOLOGY.md §15, AGENTS.md §8.1)"
        )
    row = rows[0]
    for key in ("round", "position", "writers"):
        if row.get(key) != cell[key]:
            raise RuntimeError(
                f"writer cell ({arm}, {cell}): the row carries {key}={row.get(key)!r}, "
                f"the schedule ran {key}={cell[key]!r} (METHODOLOGY.md §15)"
            )
    row["build"] = cell["build"]
    return row


def run_writer_cell(binary: Path, arm: str, cell: dict[str, Any], quick: bool) -> dict[str, Any]:
    """One timed writer cell in a harness process of its own."""
    cmd = writer_cell_argv(binary, arm, cell, quick)
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(
            f"writer_scaling cell ({arm}, {cell}) failed (exit {proc.returncode}):\n{proc.stderr}"
        )
    return writer_cell_row(proc.stdout, arm, cell)


def check_rows_against_schedule(
    rows: list[dict[str, Any]], schedule: list[dict[str, Any]], context: str
) -> None:
    """Rows and schedule hold the same (build, round, position, W) cells, each once."""
    def key(x: dict[str, Any]) -> tuple[Any, ...]:
        return (x.get("build"), x.get("round"), x.get("position"), x.get("writers"))

    want, got = collections.Counter(map(key, schedule)), collections.Counter(map(key, rows))
    if want != got:
        raise ValueError(
            f"{context}: rows disagree with the per-cell schedule "
            f"(build, round, position, W) -- missing {sorted((want - got).elements(), key=str)}, "
            f"unexpected {sorted((got - want).elements(), key=str)}; refusing to report them "
            "(METHODOLOGY.md §15, AGENTS.md §8.1)"
        )


def run_throughput_pass(
    binary: Path, arm: str, writers_list: list[int], rounds: int, quick: bool = False
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """The timed pass of a single-build sweep: one process per (W, round) cell.

    Returns the rows and the schedule they were run under.
    """
    schedule = writer_cell_schedule(writers_list, rounds)
    rows = [run_writer_cell(binary, arm, cell, quick) for cell in schedule]
    check_rows_against_schedule(rows, schedule, f"{arm} throughput pass")
    return rows, schedule


def expected_cells(
    arm: str, schedule: list[dict[str, Any]]
) -> dict[tuple[str, str, int], list[tuple[int, int]]]:
    """(arm, build, W) -> the sorted (round, position) cells the schedule ran."""
    out: dict[tuple[str, str, int], list[tuple[int, int]]] = {}
    for c in schedule:
        out.setdefault((arm, c["build"], int(c["writers"])), []).append((c["round"], c["position"]))
    return {k: sorted(v) for k, v in out.items()}


def check_artifact_against_schedule(
    artifact: dict[str, Any], expected: dict[tuple[str, str, int], list[tuple[int, int]]]
) -> None:
    """Refuse an artifact whose timed rows are not the cells the schedule ran.

    Every cell under `throughput` (the default build) and `throughput_variant`
    (its `variant` build) must carry in `rounds_raw` exactly the (round,
    position) cells the schedule ran for its (arm, build, W), each row labelled
    with that build; every scheduled (arm, build, W) must appear; and the
    provenance must say the cells ran one process each.
    """
    problems: list[str] = []
    isolation = artifact.get("provenance", {}).get("cell_isolation")
    if isolation != CELL_ISOLATION:
        problems.append(f"provenance.cell_isolation is {isolation!r}, not {CELL_ISOLATION!r}")
    seen: set[tuple[str, str, int]] = set()
    for list_key in ("throughput", "throughput_variant"):
        for cell in artifact.get(list_key, []):
            build = DEFAULT_BUILD if list_key == "throughput" else cell.get("variant")
            k = (cell.get("arm"), build, int(cell.get("writers", -1)))
            if k in seen:
                problems.append(f"{k}: summarised twice")
                continue
            seen.add(k)
            raw = cell.get("rounds_raw") or []
            wrong_build = sorted({str(r.get("build")) for r in raw if r.get("build") != build})
            if wrong_build:
                problems.append(f"{k}: rounds_raw rows labelled build {wrong_build}")
            got = sorted((r.get("round"), r.get("position")) for r in raw)
            if k not in expected:
                problems.append(f"{k}: a cell the schedule never ran")
            elif got != expected[k]:
                problems.append(f"{k}: rounds_raw (round, position) {got} != scheduled {expected[k]}")
    missing = sorted(set(expected) - seen)
    if missing:
        problems.append(f"scheduled cells absent from the artifact: {missing}")
    if problems:
        raise ValueError(
            "refusing to write an artifact whose rows disagree with the per-cell schedule "
            "(METHODOLOGY.md §15, AGENTS.md §8.1): " + "; ".join(problems)
        )


def summarize_arm(
    arm: str,
    writers_list: list[int],
    rounds: int,
    throughput_rows: list[dict[str, Any]],
    counters_rows: list[dict[str, Any]],
    load_attribution: dict[str, Any],
    throughput_target: Path = THROUGHPUT_TARGET,
) -> list[dict[str, Any]]:
    """Summarize cells for an arm across all writer counts.

    M1: Paired bootstrap C(N) across interleaved rounds.
    M2: Macro-mean as headline point estimate alongside BCa 95% CI (AGENTS.md §8.4).
    B1: Separate throughput (uninstrumented) and counters (occ-stats) provenance.
    AGENTS.md §7: Zero machine/home paths leaked in emitted fields.
    AGENTS.md §8.1: Fail-closed assertions on measurement invariants.
    """
    # Index throughput by (round, writers)
    t_by_round_w: dict[tuple[int, int], float] = {}
    for r in throughput_rows:
        t_by_round_w[(int(r["round"]), int(r["writers"]))] = float(r["writer_mops"])

    # Index counters by writers -> list of rows
    c_by_w: dict[int, list[dict[str, Any]]] = {w: [] for w in writers_list}
    for r in counters_rows:
        w = int(r["writers"])
        if w in c_by_w:
            c_by_w[w].append(r)

    cells: list[dict[str, Any]] = []

    for w in writers_list:
        t_rows_w = [r for r in throughput_rows if int(r["writers"]) == w]
        first_t = t_rows_w[0]

        # 1. Throughput statistics: mean is headline (AGENTS.md §8.4), median auxiliary
        mops_samples = [t_by_round_w[(round_idx, w)] for round_idx in range(rounds)]
        if len(mops_samples) < 3:
            raise ValueError(
                f"Need at least 3 rounds for BCa bootstrap CI (AGENTS.md §8.1, §8.4), got {len(mops_samples)}"
            )
        mean_mops, ci_lower, ci_upper, mops_ci_method = bca_bootstrap_ci_with_method(
            mops_samples, confidence=0.95
        )
        median_mops = sorted(mops_samples)[len(mops_samples) // 2]

        # 2. Scaling factor C(N) via paired bootstrap across interleaved rounds
        if w == 1:
            cn_mean = 1.0
            cn_ci_lower = 1.0
            cn_ci_upper = 1.0
            cn_median = 1.0
            # C(1) is 1.0 by definition, not by resampling — no construction ran,
            # so naming one would be a claim about an estimator that was never
            # invoked (AGENTS.md §8.1).
            cn_ci_method = None
        else:
            paired_ratios: list[float] = []
            for round_idx in range(rounds):
                t1 = t_by_round_w.get((round_idx, 1))
                if t1 is None or t1 <= 0.0:
                    raise ValueError(
                        f"Missing or non-positive W=1 throughput for round {round_idx} (got {t1})"
                    )
                tw = t_by_round_w.get((round_idx, w))
                if tw is None:
                    raise ValueError(f"Missing W={w} throughput for round {round_idx}")
                paired_ratios.append(tw / t1)

            if len(paired_ratios) < 3:
                raise ValueError(
                    f"Need at least 3 paired ratios for BCa bootstrap CI (AGENTS.md §8.1, §8.4), got {len(paired_ratios)}"
                )
            cn_mean, cn_ci_lower, cn_ci_upper, cn_ci_method = bca_bootstrap_ci_with_method(
                paired_ratios, confidence=0.95
            )
            cn_median = sorted(paired_ratios)[len(paired_ratios) // 2]

        # 3. Counters statistics (strictly from occ-stats diagnostic build across all rounds)
        c_rows_w = c_by_w.get(w, [])
        fallbacks_samples = [int(r.get("lock_fallbacks", 0)) for r in c_rows_w]
        median_fallbacks = (
            sorted(fallbacks_samples)[len(fallbacks_samples) // 2]
            if fallbacks_samples
            else 0
        )
        total_fallbacks = sum(fallbacks_samples)
        total_ops = sum(int(r.get("write_ops", 0)) for r in c_rows_w)
        fallback_rate = (total_fallbacks / total_ops) if total_ops > 0 else 0.0

        cause_totals = {name: 0 for name in CAUSE_NAMES}
        for r in c_rows_w:
            causes = r.get("fallback_causes")
            if not isinstance(causes, dict) or set(causes) != set(CAUSE_NAMES):
                raise ValueError(
                    f"{arm} W={w} round {r.get('round')}: fallback_causes must carry exactly "
                    f"{CAUSE_NAMES}, got {causes!r}"
                )
            if sum(int(v) for v in causes.values()) != int(r["lock_fallbacks"]):
                raise ValueError(
                    f"{arm} W={w} round {r['round']}: causes sum to "
                    f"{sum(int(v) for v in causes.values())}, lock_fallbacks = {r['lock_fallbacks']}"
                )
            if int(r["inserts"]) != int(r["write_ops"]):
                raise ValueError(
                    f"{arm} W={w} round {r['round']}: Stat::Inserts = {r['inserts']}, "
                    f"harness inserted {r['write_ops']}"
                )
            # Invariant 1: QuiesceCalls == LockFallbacks (on insert-only sweeps)
            if int(r.get("quiesce_calls", 0)) != int(r["lock_fallbacks"]):
                raise ValueError(
                    f"{arm} W={w} round {r['round']}: quiesce_calls ({r.get('quiesce_calls')}) != "
                    f"lock_fallbacks ({r['lock_fallbacks']})"
                )
            # Invariant 2: Contention exact partition
            c_closed = int(r.get("contention_gate_closed", 0))
            c_exhausted = int(r.get("contention_retry_exhausted", 0))
            if c_closed + c_exhausted != int(causes["contention"]):
                raise ValueError(
                    f"{arm} W={w} round {r['round']}: contention_gate_closed ({c_closed}) + "
                    f"contention_retry_exhausted ({c_exhausted}) = {c_closed + c_exhausted} != "
                    f"causes['contention'] ({causes['contention']})"
                )
            # Invariant 3: BranchSplit exact partition
            bs_sub = int(r.get("branch_split_subarray", 0))
            bs_lin = int(r.get("branch_split_linear", 0))
            bs_pfx = int(r.get("branch_split_prefix", 0))
            bs_rem = int(r.get("branch_split_remove", 0))
            bs_upg = int(r.get("branch_split_upgrade", 0))
            bs_sum = bs_sub + bs_lin + bs_pfx + bs_rem + bs_upg
            if bs_sum != int(causes["branch_split"]):
                raise ValueError(
                    f"{arm} W={w} round {r['round']}: branch_split partition ({bs_sub} + {bs_lin} + "
                    f"{bs_pfx} + {bs_rem} + {bs_upg} = {bs_sum}) != causes['branch_split'] ({causes['branch_split']})"
                )
            # Invariant 4: CapExpansion exact partition (#568)
            ce_cls = int(r.get("cap_expansion_class", 0))
            ce_full = int(r.get("cap_expansion_leaf_full", 0))
            ce_bm = int(r.get("cap_expansion_bitmap_near_full", 0))
            ce_sub = int(r.get("cap_expansion_map_bitmap_sub", 0))
            ce_rem = int(r.get("cap_expansion_remove", 0))
            ce_sum = ce_cls + ce_full + ce_bm + ce_sub + ce_rem
            if ce_sum != int(causes["cap_expansion"]):
                raise ValueError(
                    f"{arm} W={w} round {r['round']}: cap_expansion partition ({ce_cls} + {ce_full} + "
                    f"{ce_bm} + {ce_sub} + {ce_rem} = {ce_sum}) != causes['cap_expansion'] ({causes['cap_expansion']})"
                )
            for name in CAUSE_NAMES:
                cause_totals[name] += int(causes[name])
        # Shares of fallbacks say which Phase 4 rung a cell needs; per-insert
        # rates say how much of the workload that rung would move.
        cause_share = {
            name: (round(v / total_fallbacks, 6) if total_fallbacks > 0 else 0.0)
            for name, v in cause_totals.items()
        }
        cause_per_insert = {
            name: (round(v / total_ops, 6) if total_ops > 0 else 0.0)
            for name, v in cause_totals.items()
        }

        total_restarts = sum(int(r.get("lock_restarts", 0)) for r in c_rows_w)
        total_c_closed = sum(int(r.get("contention_gate_closed", 0)) for r in c_rows_w)
        total_c_exhausted = sum(int(r.get("contention_retry_exhausted", 0)) for r in c_rows_w)
        total_g_blocked = sum(int(r.get("gate_blocked_entries", 0)) for r in c_rows_w)
        total_g_wait = sum(int(r.get("gate_wait_cycles", 0)) for r in c_rows_w)
        total_q_drain = sum(int(r.get("quiesce_drain_cycles", 0)) for r in c_rows_w)
        total_bs_sub = sum(int(r.get("branch_split_subarray", 0)) for r in c_rows_w)
        total_bs_lin = sum(int(r.get("branch_split_linear", 0)) for r in c_rows_w)
        total_bs_pfx = sum(int(r.get("branch_split_prefix", 0)) for r in c_rows_w)
        total_bs_rem = sum(int(r.get("branch_split_remove", 0)) for r in c_rows_w)
        total_bs_upg = sum(int(r.get("branch_split_upgrade", 0)) for r in c_rows_w)
        total_ce_cls = sum(int(r.get("cap_expansion_class", 0)) for r in c_rows_w)
        total_ce_full = sum(int(r.get("cap_expansion_leaf_full", 0)) for r in c_rows_w)
        total_ce_bm = sum(int(r.get("cap_expansion_bitmap_near_full", 0)) for r in c_rows_w)
        total_ce_sub = sum(int(r.get("cap_expansion_map_bitmap_sub", 0)) for r in c_rows_w)
        total_ce_rem = sum(int(r.get("cap_expansion_remove", 0)) for r in c_rows_w)

        restarts_per_insert = round(total_restarts / total_ops, 6) if total_ops > 0 else 0.0
        gate_blocked_entries_per_insert = (
            round(total_g_blocked / total_ops, 6) if total_ops > 0 else 0.0
        )
        gate_wait_cycles_per_insert = (
            round(total_g_wait / total_ops, 6) if total_ops > 0 else 0.0
        )
        quiesce_drain_cycles_per_fallback = (
            round(total_q_drain / total_fallbacks, 6) if total_fallbacks > 0 else 0.0
        )
        contention_subset_totals = {
            "gate_closed": total_c_closed,
            "retry_exhausted": total_c_exhausted,
        }
        contention_subset_share = {
            "gate_closed": (
                round(total_c_closed / cause_totals["contention"], 6)
                if cause_totals["contention"] > 0
                else 0.0
            ),
            "retry_exhausted": (
                round(total_c_exhausted / cause_totals["contention"], 6)
                if cause_totals["contention"] > 0
                else 0.0
            ),
        }
        branch_split_subset_totals = {
            "subarray": total_bs_sub,
            "linear": total_bs_lin,
            "prefix": total_bs_pfx,
            "remove": total_bs_rem,
            "upgrade": total_bs_upg,
        }
        branch_split_subset_share = {
            "subarray": (
                round(total_bs_sub / cause_totals["branch_split"], 6)
                if cause_totals["branch_split"] > 0
                else 0.0
            ),
            "linear": (
                round(total_bs_lin / cause_totals["branch_split"], 6)
                if cause_totals["branch_split"] > 0
                else 0.0
            ),
            "prefix": (
                round(total_bs_pfx / cause_totals["branch_split"], 6)
                if cause_totals["branch_split"] > 0
                else 0.0
            ),
            "remove": (
                round(total_bs_rem / cause_totals["branch_split"], 6)
                if cause_totals["branch_split"] > 0
                else 0.0
            ),
            "upgrade": (
                round(total_bs_upg / cause_totals["branch_split"], 6)
                if cause_totals["branch_split"] > 0
                else 0.0
            ),
        }
        cap_expansion_subset_totals = {
            "class": total_ce_cls,
            "leaf_full": total_ce_full,
            "bitmap_near_full": total_ce_bm,
            "map_bitmap_sub": total_ce_sub,
            "remove": total_ce_rem,
        }
        cap_expansion_subset_share = {
            k: (
                round(v / cause_totals["cap_expansion"], 6)
                if cause_totals["cap_expansion"] > 0
                else 0.0
            )
            for k, v in cap_expansion_subset_totals.items()
        }

        total_retired = sum(int(r.get("retired", 0)) for r in c_rows_w)
        retired_per_insert = round(total_retired / total_ops, 6) if total_ops > 0 else 0.0
        has_allocs = any(r.get("total_allocs") is not None for r in c_rows_w)
        if has_allocs:
            total_node_allocs = sum(
                int(r.get("total_allocs", 0)) for r in c_rows_w if r.get("total_allocs") is not None
            )
            total_allocs_per_insert = (
                round(total_node_allocs / total_ops, 6) if total_ops > 0 else 0.0
            )
        else:
            total_node_allocs = None
            total_allocs_per_insert = None
        tsc_hz = int(first_t.get("tsc_hz", 0))

        tp_rel = throughput_target.relative_to(REPO_ROOT)
        cnt_rel = COUNTERS_TARGET.relative_to(REPO_ROOT)

        cell: dict[str, Any] = {
            "workload_id": first_t["workload_id"],
            "arm": arm,
            "writers": w,
            "readers": 0,
            "prefill": first_t["prefill"],
            "fresh_keys": first_t["fresh_keys"],
            "rounds": rounds,
            "tsc_hz": tsc_hz,
            "expanse_writer_mops_mean": round(mean_mops, 4),
            "writer_ci_lower": round(ci_lower, 4),
            "writer_ci_upper": round(ci_upper, 4),
            # Which construction produced each interval beside it, from
            # `bca_bootstrap.CI_METHOD_*` (#880). Two intervals, so two labels:
            # anything but `bca` means one of BCa's corrections degenerated on
            # that sample, and the cell says so (AGENTS.md §8.1). The C(W) label
            # is `null` at W=1, where C(1) is 1.0 by definition and nothing was
            # resampled.
            "writer_ci_method": mops_ci_method,
            "expanse_writer_mops_median": round(median_mops, 4),
            "scaling_factor_c_n": round(cn_mean, 4),
            "scaling_factor_c_n_mean": round(cn_mean, 4),
            "scaling_factor_c_n_ci_lower": round(cn_ci_lower, 4),
            "scaling_factor_c_n_ci_upper": round(cn_ci_upper, 4),
            "scaling_factor_c_n_ci_method": cn_ci_method,
            "scaling_factor_c_n_median": round(cn_median, 4),
            "lock_fallbacks": median_fallbacks,
            "lock_fallbacks_median": median_fallbacks,
            "fallback_rate": round(fallback_rate, 6),
            "fallback_causes_total": cause_totals,
            "fallback_cause_share": cause_share,
            "fallback_causes_per_insert": cause_per_insert,
            "contention_subsets_total": contention_subset_totals,
            "contention_subset_share": contention_subset_share,
            "branch_split_subsets_total": branch_split_subset_totals,
            "branch_split_subset_share": branch_split_subset_share,
            "cap_expansion_subsets_total": cap_expansion_subset_totals,
            "cap_expansion_subset_share": cap_expansion_subset_share,
            "gate_blocked_entries_per_insert": gate_blocked_entries_per_insert,
            "gate_wait_cycles_per_insert": gate_wait_cycles_per_insert,
            "quiesce_drain_cycles_per_fallback": quiesce_drain_cycles_per_fallback,
            "lock_restarts_per_insert": restarts_per_insert,
            "total_retired": total_retired,
            "retired_per_insert": retired_per_insert,
            "total_allocs": total_node_allocs,
            "total_allocs_per_insert": total_allocs_per_insert,
            "build_provenance": {
                "throughput": f"{tp_rel}/release/examples/writer_scaling",
                "counters": (
                    f"{cnt_rel}/release/examples/writer_scaling (--features occ-stats)"
                    if c_rows_w
                    else None
                ),
            },
            "rounds_raw": [
                {
                    "round": r["round"],
                    "position": r["position"],
                    "build": r.get("build"),
                    "writer_mops": r["writer_mops"],
                    "writer_elapsed_s": r["writer_elapsed_s"],
                    "write_ops": r["write_ops"],
                    "tsc_hz": r.get("tsc_hz", 0),
                }
                for r in t_rows_w
            ],
            "counters_raw": [
                {
                    "round": r["round"],
                    "position": r["position"],
                    "write_ops": r["write_ops"],
                    "lock_fallbacks": r["lock_fallbacks"],
                    "inserts": r["inserts"],
                    "lock_restarts": r["lock_restarts"],
                    "contention_gate_closed": r["contention_gate_closed"],
                    "contention_retry_exhausted": r["contention_retry_exhausted"],
                    "gate_blocked_entries": r["gate_blocked_entries"],
                    "gate_wait_cycles": r["gate_wait_cycles"],
                    "quiesce_calls": r["quiesce_calls"],
                    "quiesce_drain_cycles": r["quiesce_drain_cycles"],
                    "branch_split_subarray": r["branch_split_subarray"],
                    "branch_split_linear": r["branch_split_linear"],
                    "branch_split_prefix": r["branch_split_prefix"],
                    "branch_split_remove": r["branch_split_remove"],
                    "branch_split_upgrade": r.get("branch_split_upgrade", 0),
                    "cap_expansion_class": r.get("cap_expansion_class", 0),
                    "cap_expansion_leaf_full": r.get("cap_expansion_leaf_full", 0),
                    "cap_expansion_bitmap_near_full": r.get("cap_expansion_bitmap_near_full", 0),
                    "cap_expansion_map_bitmap_sub": r.get("cap_expansion_map_bitmap_sub", 0),
                    "cap_expansion_remove": r.get("cap_expansion_remove", 0),
                    "retired": r.get("retired", 0),
                    "total_allocs": r.get("total_allocs"),
                    "tsc_hz": r.get("tsc_hz", 0),
                    "fallback_causes": r["fallback_causes"],
                }
                for r in c_rows_w
            ],
            "load": load_attribution,
        }
        if "keyspace_bits" in first_t:
            cell["keyspace_bits"] = first_t["keyspace_bits"]
        if "dist" in first_t:
            cell["dist"] = first_t["dist"]

        cells.append(cell)

    return cells


def compute_paired_scaling_ratios(
    arm: str,
    variant_name: str,
    writers_list: list[int],
    rounds: int,
    all_t_rows_default: list[dict[str, Any]],
    all_t_rows_variant: list[dict[str, Any]],
    inverse: bool = False,
) -> dict[str, Any]:
    """Compute paired scaling ratios C_variant(w) / C_default(w) (or C_default(w) / C_variant(w) for inverse ablations) per round."""
    is_inverse = (variant_name in INVERSE_ABLATIONS) or inverse

    t_by_round_w_def: dict[tuple[int, int], float] = {
        (int(r["round"]), int(r["writers"])): float(r["writer_mops"])
        for r in all_t_rows_default
    }
    t_by_round_w_var: dict[tuple[int, int], float] = {
        (int(r["round"]), int(r["writers"])): float(r["writer_mops"])
        for r in all_t_rows_variant
    }

    comparison_stats: dict[str, Any] = {
        "arm": arm,
        "variant_name": variant_name,
        "rounds": rounds,
        "is_inverse": is_inverse,
        # Which ratio every `per_writer` entry holds. The mean is stored under
        # `ratio_c_variant_over_c_default_mean` in both directions, so this is
        # what says an inverse ablation's value is default over variant.
        "ratio_direction": (
            "c_default_over_c_variant" if is_inverse else "c_variant_over_c_default"
        ),
        "per_writer": {},
    }

    for w in writers_list:
        if w == 1:
            # Skip W=1: by definition C(1) == 1.0, so the ratio is identically 1.0,
            # bca_bootstrap_ci returns (1,1,1), and testing C(1) > 1.0 is not a concurrency decision.
            continue

        paired_ratios: list[float] = []
        for r in range(rounds):
            t1_d = t_by_round_w_def[(r, 1)]
            tw_d = t_by_round_w_def[(r, w)]
            if t1_d <= 0:
                raise RuntimeError(f"Round {r} W=1 default throughput <= 0 ({t1_d}) (AGENTS.md §8.1)")
            c_d = tw_d / t1_d
            if c_d <= 0:
                raise RuntimeError(f"Round {r} W={w} default scaling C(W) <= 0 ({c_d}) (AGENTS.md §8.1)")

            t1_v = t_by_round_w_var[(r, 1)]
            tw_v = t_by_round_w_var[(r, w)]
            if t1_v <= 0:
                raise RuntimeError(f"Round {r} W=1 variant throughput <= 0 ({t1_v}) (AGENTS.md §8.1)")
            c_v = tw_v / t1_v

            if is_inverse:
                paired_ratios.append(c_d / c_v)
            else:
                paired_ratios.append(c_v / c_d)

        mean_ratio, ci_lower, ci_upper, ratio_ci_method = bca_bootstrap_ci_with_method(
            paired_ratios, confidence=0.95
        )
        median_ratio = sorted(paired_ratios)[len(paired_ratios) // 2]
        # Verdict decision rule (§8.4, §8.20):
        # A single run cannot CONFIRM; confirmation requires a second independent run across two committed artifacts.
        # CI_lower > 1.0 -> SINGLE_RUN_PASS (candidate confirmed pending run 2)
        # CI_upper < 1.0 -> REJECTED
        # CI spans 1.0   -> INCONCLUSIVE (data cannot reject or confirm)
        if ci_lower > 1.0:
            verdict = "SINGLE_RUN_PASS"
        elif ci_upper < 1.0:
            verdict = "REJECTED"
        else:
            verdict = "INCONCLUSIVE"

        stat_entry = {
            "w": w,
            "ratio_c_variant_over_c_default_mean": round(mean_ratio, 4),
            "ratio_ci_lower": round(ci_lower, 4),
            "ratio_ci_upper": round(ci_upper, 4),
            # The construction behind this verdict's interval
            # (`bca_bootstrap.CI_METHOD_*`, #880): a SINGLE_RUN_PASS or REJECTED
            # read off a degenerate or clamped interval is a different claim
            # from one read off a BCa interval, so the artifact names which.
            "ratio_ci_method": ratio_ci_method,
            "ratio_median": round(median_ratio, 4),
            "verdict": verdict,
            "paired_ratios_raw": [round(x, 6) for x in paired_ratios],
        }
        comparison_stats["per_writer"][str(w)] = stat_entry

        ratio_label = (
            f"Ratio C_default({w}) / C_{variant_name}({w})"
            if is_inverse
            else f"Ratio C_{variant_name}({w}) / C_default({w})"
        )
        print(
            f"  [Comparison W={w:<2}] {ratio_label}: "
            f"Mean {mean_ratio:.4f} [{ci_lower:.4f}, {ci_upper:.4f}] | "
            f"Verdict: {verdict}"
        )

    return comparison_stats


def run_comparison(
    bin_default: Path,
    bin_variant: Path,
    variant_name: str,
    counters_bin_default: Path,
    counters_bin_variant: Path | None,
    arm: str,
    writers_list: list[int],
    rounds: int,
    prov: dict[str, Any],
    quick: bool = False,
    inverse: bool = False,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    """Runs interleaved (build × W) execution within each round, one process per cell.

    Within round r the 2 x len(W) (build, W) cells run in the order of row r of
    a Williams design over those cells (`writer_cell_schedule`), each in a
    harness process of its own (METHODOLOGY.md §15). Over 2 x len(W) rounds
    every cell holds every position once and every ordered pair of cells is
    adjacent once, so position, host drift and first-order carryover land on
    both builds and every W alike (AGENTS.md §8.20.2).
    Computes paired bootstrap BCa 95% CI on C_variant(w) / C_default(w).

    Returns the default cells, the variant cells, the comparison, and the
    schedule the timed cells ran under.
    """
    if variant_name == DEFAULT_BUILD:
        raise ValueError(f"a variant cannot be labelled {DEFAULT_BUILD!r}")
    print("\n========================================================================")
    print(f" Interleaved (build × W) Execution: default vs {variant_name}")
    print(f" Arm: {arm} | Writers: {writers_list} | Rounds: {rounds}")
    print("========================================================================")

    all_t_rows_default: list[dict[str, Any]] = []
    all_t_rows_variant: list[dict[str, Any]] = []

    # One load window for the arm, covering both builds. The builds alternate
    # cell by cell inside it, so a per-build phase window cannot separate them;
    # and a second `begin_cell` taken back to back with the first opens a
    # window of well under a millisecond, whose busy-CPU quotient is rounding
    # divided by almost nothing. Both builds' cells carry this one attribution.
    start_snap = begin_cell(prov, f"arm:{arm}:comparison")

    binaries = {DEFAULT_BUILD: bin_default, variant_name: bin_variant}
    schedule = writer_cell_schedule(writers_list, rounds, builds=(DEFAULT_BUILD, variant_name))
    for cell in schedule:
        row = run_writer_cell(binaries[cell["build"]], arm, cell, quick)
        (all_t_rows_default if cell["build"] == DEFAULT_BUILD else all_t_rows_variant).append(row)
    check_rows_against_schedule(
        all_t_rows_default + all_t_rows_variant, schedule, f"{arm} default vs {variant_name}"
    )

    load = end_cell(start_snap)

    print(f"  [Pass 2/2] Diagnostic counters (default) — {arm} arm across W ∈ {writers_list} (occ-stats build)")
    c_rows_def = run_pass(counters_bin_default, "counters", arm, writers_list, rounds=rounds, quick=quick)

    c_rows_var: list[dict[str, Any]] = []
    if counters_bin_variant is not None:
        print(
            f"  [Pass 2/2] Diagnostic counters ({variant_name}) — {arm} arm across W ∈ {writers_list} (occ-stats,{variant_name} build)"
        )
        c_rows_var = run_pass(counters_bin_variant, "counters", arm, writers_list, rounds=rounds, quick=quick)

    tp_target_def = bin_default.parent.parent
    tp_target_var = bin_variant.parent.parent

    cells_default = summarize_arm(
        arm, writers_list, rounds, all_t_rows_default, c_rows_def, dict(load), throughput_target=tp_target_def
    )
    cells_variant = summarize_arm(
        arm, writers_list, rounds, all_t_rows_variant, c_rows_var, dict(load), throughput_target=tp_target_var
    )
    for c in cells_variant:
        c["variant"] = variant_name

    comparison_stats = compute_paired_scaling_ratios(
        arm,
        variant_name,
        writers_list,
        rounds,
        all_t_rows_default,
        all_t_rows_variant,
        inverse=inverse,
    )

    return cells_default, cells_variant, comparison_stats, schedule


# ---------------------------------------------------------------------------
# Ordered readers (#900, docs/benchmarks/concurrency/METHODOLOGY.md §12.3–§12.5)
# ---------------------------------------------------------------------------

ORDERED_READERS_RESULTS_PATH = (
    REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "ordered_readers_writer_scaling.json"
)
# §12.4: one thread per physical P-core on the reference host. §12.5 voids a
# cell whose harness rows record any other pin.
ORDERED_READERS_PIN = "0,2,4,6,8,10,12,14"
ORDERED_READERS_WORKLOAD_ID = "concurrency_ordered_readers_map_64bit"
ORDERED_READERS_PROBES = ("uniform", "hotspot")
ORDERED_READERS_OPS = ("prev_locked", "prev")
ORDERED_READERS_WR = ((0, 1), (0, 4), (1, 4), (4, 4))
# The unit §12.4 interleaves within a round: (read_op, W, R).
ORDERED_READERS_BLOCK = tuple(
    (op, w, r) for (w, r) in ORDERED_READERS_WR for op in ORDERED_READERS_OPS
)
# P12.4's ceiling as an exact fraction: read_fallbacks / read_ops < 1 / 1000.
P124_CEILING = (1, 1000)
P125_GATE = ("uniform", 1, 4)
P125_CONTROL = ("uniform", 0, 1)
READER_COUNTER_FIELDS = ("read_ops", "read_attempts", "read_fallbacks", "locked_reads")
READER_TIMING_FIELDS = ("reader_elapsed_s", "reader_mops", "writer_elapsed_s", "writer_mops")


def committed_result_paths() -> tuple[Path, ...]:
    """The committed artifacts a `--quick` run must not overwrite."""
    return (
        COMMITTED_RESULTS_PATH.resolve(),
        DIAGNOSTIC_RESULTS_PATH.resolve(),
        PADDED_RESULTS_PATH.resolve(),
        *(p.resolve() for p in ABLATION_RESULTS_PATHS),
        ORDERED_READERS_RESULTS_PATH.resolve(),
    )


def williams_positions(n: int, round_idx: int) -> list[int]:
    """Row `round_idx` of a Williams design over `n` treatments.

    The construction `williams_order` in `writer_scaling.rs` uses: first row
    0, 1, n-1, 2, n-2, ...; row r adds r mod n. For even n, n rows put every
    treatment in every position once and make every ordered pair of distinct
    treatments adjacent once.
    """
    if n <= 1:
        return list(range(n))
    first = [0]
    lo, hi = 1, n - 1
    while len(first) < n:
        first.append(lo)
        lo += 1
        if len(first) < n:
            first.append(hi)
            hi -= 1
    return [(i + round_idx) % n for i in first]


def ordered_readers_schedule(rounds: int) -> list[dict[str, Any]]:
    """Every harness invocation of an ordered-reader sweep, in execution order.

    Each round runs both probe blocks, and which block goes first alternates by
    round. Within a block the eight (read_op, W, R) cells follow row `round` of
    a Williams design (§12.4's interleaving), so across 8 rounds each cell holds
    each position once and each ordered pair of cells is adjacent once. The
    `position` is the cell's index within its block.
    """
    out: list[dict[str, Any]] = []
    n = len(ORDERED_READERS_BLOCK)
    for r in range(rounds):
        probes = ORDERED_READERS_PROBES if r % 2 == 0 else ORDERED_READERS_PROBES[::-1]
        for block, probe in enumerate(probes):
            for pos, idx in enumerate(williams_positions(n, r)):
                op, w, rd = ORDERED_READERS_BLOCK[idx]
                out.append({
                    "round": r, "block": block, "probe": probe, "position": pos,
                    "read_op": op, "writers": w, "readers": rd,
                })
    return out


def _pin_list(value: str | None) -> list[int] | None:
    """A CPU list, expanded; `None` for an absent, `none`, `off` or unparsable value."""
    if not value or value.lower() in ("none", "off", "unset"):
        return None
    try:
        return bench_pin.expand(value)
    except ValueError:
        return None


def pins_equal(a: str | None, b: str | None) -> bool:
    """Two pins name the same CPUs, however each CPU list is spelled."""
    la, lb = _pin_list(a), _pin_list(b)
    if la is None or lb is None:
        return (a or "") == (b or "")
    return la == lb


def resolve_ordered_readers_pin(env: dict[str, str], smoke: bool) -> str | None:
    """Point `env` at the §12.4 pin before `bench_pin.apply` reads it.

    An unset `EXPANSE_BENCH_PIN` becomes `0,2,4,6,8,10,12,14`. A pin variable
    that names anything else, whether requested or inherited from a runner that
    sourced `bench_pin.sh`, is refused with `ValueError`, because §12.5 voids
    every cell measured under it. The one exception is a smoke run (`--quick`
    to a path outside the committed results), for which the departure is
    returned as a notice instead: a host without `sched_setaffinity` can only
    run it with `EXPANSE_BENCH_PIN=off`.
    """
    required = _pin_list(ORDERED_READERS_PIN)
    if not env.get("EXPANSE_BENCH_PIN"):
        env["EXPANSE_BENCH_PIN"] = ORDERED_READERS_PIN
    departures = [
        f"{name}={env[name]!r}"
        for name in ("EXPANSE_BENCH_PIN", "EXPANSE_BENCH_PIN_APPLIED")
        if env.get(name) and _pin_list(env[name]) != required
    ]
    if not departures:
        return None
    message = (
        f"--ordered-readers measures under the pin {ORDERED_READERS_PIN} "
        f"(METHODOLOGY.md §12.4), but {', '.join(departures)} names another, and "
        f"§12.5 voids every cell measured under it"
    )
    if not smoke:
        raise ValueError(message)
    return message


def check_row_pins(rows: list[dict[str, Any]], applied: str) -> None:
    """Every harness row records the pin the driver applied (§12.5)."""
    bad = sorted({str(r.get("cpu_pin")) for r in rows if not pins_equal(r.get("cpu_pin"), applied)})
    if bad:
        raise ValueError(
            f"harness rows record cpu_pin {bad}, but the driver applied {applied!r}; "
            f"a row whose pin is not the applied one is not a cell of this run (§12.5)"
        )


def run_reader_invocation(
    binary: Path, role: str, run: dict[str, Any], quick: bool
) -> dict[str, Any]:
    """One reader-mode cell from the harness, checked against the schedule that asked for it."""
    cmd = [
        str(binary), "--role", role, "--arm", "map",
        "--writers", str(run["writers"]), "--readers", str(run["readers"]),
        "--read-op", run["read_op"], "--probe", run["probe"],
        "--round", str(run["round"]), "--position", str(run["position"]),
    ]
    if quick:
        cmd.append("--quick")
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(
            f"writer_scaling reader cell ({role}, {run}) failed "
            f"(exit {proc.returncode}):\n{proc.stderr}"
        )
    rows = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if line.startswith("{") and line.endswith("}"):
            row = json.loads(line)
            if row.get("role") == role and row.get("workload_id") == ORDERED_READERS_WORKLOAD_ID:
                rows.append(row)
    if len(rows) != 1:
        raise RuntimeError(f"reader cell ({role}, {run}) emitted {len(rows)} rows, expected 1")
    row = rows[0]
    for key in ("round", "position", "read_op", "probe", "writers", "readers"):
        if row.get(key) != run[key]:
            raise RuntimeError(
                f"reader cell ({role}, {run}): the row carries {key}={row.get(key)!r}, "
                f"the schedule ran {run[key]!r}"
            )
    row["block"] = run["block"]
    return row


def check_reader_throughput_row(row: dict[str, Any]) -> None:
    """A throughput row carries reader timing and no counters (§12.5)."""
    ctx = f"{row.get('cell')} round {row.get('round')}"
    leaked = [k for k in (*READER_COUNTER_FIELDS, "lock_fallbacks", "inserts") if k in row]
    if leaked:
        raise ValueError(f"{ctx}: throughput row carries counter field(s) {leaked}; the two roles never share a binary (§12.5)")
    for k in ("reader_ops", "reader_elapsed_s", "reader_mops"):
        v = row.get(k)
        if not isinstance(v, (int, float)) or isinstance(v, bool) or v <= 0:
            raise ValueError(f"{ctx}: throughput row {k} = {v!r}, expected a positive number")


def check_reader_counters_row(row: dict[str, Any]) -> None:
    """The identities a reader-mode counters row owes, re-checked on the driver side (§8.1).

    `read_locked` and `with_locked` both quiesce the writers, so
    `quiesce_calls == lock_fallbacks + locked_reads`. An optimistic reader
    counts one `read_ops` per call and takes the writer mutex only by falling
    back; a `prev_locked` reader never enters the optimistic protocol.
    """
    ctx = f"{row.get('cell')} round {row.get('round')}"
    needed = (*READER_COUNTER_FIELDS, "reader_ops", "inserts", "write_ops", "lock_fallbacks",
              "quiesce_calls", "fallback_causes")
    missing = [k for k in needed if k not in row]
    if missing:
        raise ValueError(f"{ctx}: counters row lacks {missing}")
    leaked = [k for k in READER_TIMING_FIELDS if k in row]
    if leaked:
        raise ValueError(f"{ctx}: counters row carries timing field(s) {leaked}; the two roles never share a binary (§12.5)")
    causes = row["fallback_causes"]
    if not isinstance(causes, dict) or set(causes) != set(CAUSE_NAMES):
        raise ValueError(f"{ctx}: fallback_causes must carry exactly {CAUSE_NAMES}, got {causes!r}")
    if sum(int(v) for v in causes.values()) != int(row["lock_fallbacks"]):
        raise ValueError(f"{ctx}: causes sum to {sum(int(v) for v in causes.values())}, lock_fallbacks = {row['lock_fallbacks']}")
    if int(row["inserts"]) != int(row["write_ops"]):
        raise ValueError(f"{ctx}: Stat::Inserts = {row['inserts']}, the cell inserted {row['write_ops']}")
    if int(row["quiesce_calls"]) != int(row["lock_fallbacks"]) + int(row["locked_reads"]):
        raise ValueError(
            f"{ctx}: quiesce_calls ({row['quiesce_calls']}) != lock_fallbacks ({row['lock_fallbacks']}) "
            f"+ locked_reads ({row['locked_reads']})"
        )
    ops, reader_ops = int(row["read_ops"]), int(row["reader_ops"])
    if row["read_op"] == "prev_locked":
        if ops != 0 or int(row["locked_reads"]) != reader_ops:
            raise ValueError(
                f"{ctx}: a with_locked reader cell needs read_ops == 0 and locked_reads == reader_ops, "
                f"got read_ops {ops}, locked_reads {row['locked_reads']}, reader_ops {reader_ops}"
            )
    elif ops != reader_ops or int(row["locked_reads"]) != int(row["read_fallbacks"]):
        raise ValueError(
            f"{ctx}: an optimistic reader cell needs read_ops == reader_ops and locked_reads == "
            f"read_fallbacks, got read_ops {ops}, reader_ops {reader_ops}, locked_reads "
            f"{row['locked_reads']}, read_fallbacks {row['read_fallbacks']}"
        )


def summarize_ordered_readers(
    throughput_rows: list[dict[str, Any]],
    counters_rows: list[dict[str, Any]],
    rounds: int,
    load: dict[str, Any],
    throughput_target: Path = THROUGHPUT_TARGET,
) -> list[dict[str, Any]]:
    """One cell per (probe, read_op, W, R), each carrying every round of both roles.

    A cell missing a round in either role, or holding a round twice, is refused:
    its statistics would silently rest on fewer rounds than §12.4 fixes.
    """
    cells: list[dict[str, Any]] = []
    for probe in ORDERED_READERS_PROBES:
        for op, w, r in ORDERED_READERS_BLOCK:
            key = (probe, op, w, r)
            label = f"{probe} {op} W={w} R={r}"

            def matches(row: dict[str, Any]) -> bool:
                return (row["probe"], row["read_op"], int(row["writers"]), int(row["readers"])) == key

            t = sorted((x for x in throughput_rows if matches(x)), key=lambda x: int(x["round"]))
            c = sorted((x for x in counters_rows if matches(x)), key=lambda x: int(x["round"]))
            for role, got in (("throughput", t), ("counters", c)):
                seen = [int(x["round"]) for x in got]
                if seen != list(range(rounds)):
                    raise ValueError(
                        f"{label}: {role} rows cover rounds {seen}, expected each of 0..{rounds - 1} once"
                    )
            for x in t:
                check_reader_throughput_row(x)
            for x in c:
                check_reader_counters_row(x)
            if rounds < 3:
                raise ValueError(f"{label}: need at least 3 rounds for a BCa interval, got {rounds}")

            mops = [float(x["reader_mops"]) for x in t]
            mean, lo, hi, reader_ci_method = bca_bootstrap_ci_with_method(mops, confidence=0.95)
            totals = {k: sum(int(x[k]) for x in c) for k in (*READER_COUNTER_FIELDS, "reader_ops")}
            read_ops = totals["read_ops"]
            cells.append({
                "workload_id": t[0]["workload_id"],
                "arm": "map",
                "probe": probe,
                "read_op": op,
                "writers": w,
                "readers": r,
                "prefill": t[0]["prefill"],
                "hotspot_prefill": t[0]["hotspot_prefill"],
                "hotspot_base": t[0]["hotspot_base"],
                "fresh_keys": t[0]["fresh_keys"],
                "rounds": rounds,
                "cpu_pin": t[0]["cpu_pin"],
                "reader_mops_mean": round(mean, 6),
                "reader_ci_lower": round(lo, 6),
                "reader_ci_upper": round(hi, 6),
                "reader_ci_method": reader_ci_method,
                "reader_mops_median": round(sorted(mops)[len(mops) // 2], 6),
                "read_counters_total": totals,
                "attempts_per_op": round(totals["read_attempts"] / read_ops, 6) if read_ops else None,
                "fallback_rate": (totals["read_fallbacks"] / read_ops) if read_ops else None,
                "build_provenance": {
                    "throughput": f"{throughput_target.relative_to(REPO_ROOT)}/release/examples/writer_scaling",
                    "counters": f"{COUNTERS_TARGET.relative_to(REPO_ROOT)}/release/examples/writer_scaling (--features occ-stats)",
                },
                "rounds_raw": [
                    {k: x.get(k) for k in (
                        "round", "block", "position", "reader_ops", "reader_elapsed_s", "reader_mops",
                        "write_ops", "writer_elapsed_s", "writer_mops", "population_after", "cpu_pin", "tsc_hz",
                    )}
                    for x in t
                ],
                "counters_raw": [
                    {k: x.get(k) for k in (
                        "round", "block", "position", "reader_ops", "write_ops", "inserts",
                        *READER_COUNTER_FIELDS, "lock_fallbacks", "quiesce_calls", "fallback_causes",
                        "population_after", "cpu_pin",
                    )}
                    for x in c
                ],
                "load": load,
            })
    return cells


def p124_verdict(cells: list[dict[str, Any]]) -> dict[str, Any]:
    """P12.4 (§12.3): `read_fallbacks ÷ read_ops` below 0.1% in every `prev` cell.

    Read off each cell's summed counters, and compared in integers
    (`fallbacks × 1000 < read_ops`). A cell at exactly 0.1% is therefore not
    below the ceiling: the claim is "below", so equality does not meet it, and
    the cell is named with the cells above.
    """
    num, den = P124_CEILING
    per_cell: list[dict[str, Any]] = []
    refuted: list[str] = []
    for c in cells:
        if c["read_op"] != "prev":
            continue
        totals = c["read_counters_total"]
        ops, fb = int(totals["read_ops"]), int(totals["read_fallbacks"])
        label = f"{c['probe']} W={c['writers']} R={c['readers']}"
        if ops <= 0:
            raise ValueError(f"P12.4 {label}: read_ops = {ops}, so the cell made no optimistic reads to evaluate")
        holds = fb * den < ops * num
        per_cell.append({
            "cell": label, "probe": c["probe"], "writers": c["writers"], "readers": c["readers"],
            "read_ops": ops, "read_fallbacks": fb, "fallback_rate": fb / ops, "holds": holds,
        })
        if not holds:
            refuted.append(label)
    if not per_cell:
        raise ValueError("P12.4: no prev cells to evaluate")
    return {
        "claim": "read_fallbacks / read_ops < 0.001 in every prev cell, uniform and hotspot (METHODOLOGY.md §12.3 P12.4)",
        "ceiling": num / den,
        "boundary": "exactly 0.1% is not below the ceiling and counts against the claim",
        "per_cell": per_cell,
        "verdict": "HOLDS" if not refuted else "REFUTED",
        "refuted_cells": refuted,
    }


def p125_decision(ci_lower: float, ci_upper: float) -> str:
    """§12.3 P12.5's verdict per run; a claim needs the same verdict in two runs."""
    if ci_lower > 1.0:
        return "SINGLE_RUN_PASS"
    if ci_upper < 1.0:
        return "REJECTED"
    return "INCONCLUSIVE"


def p125_paired_ratio(
    throughput_rows: list[dict[str, Any]], probe: str, writers: int, readers: int, rounds: int
) -> dict[str, Any]:
    """`reader_mops(prev) / reader_mops(prev_locked)`, paired within each round, with a BCa 95% CI of the mean."""
    label = f"{probe} W={writers} R={readers}"
    by_round: dict[tuple[int, str], float] = {}
    for row in throughput_rows:
        if (row["probe"], int(row["writers"]), int(row["readers"])) != (probe, writers, readers):
            continue
        key = (int(row["round"]), row["read_op"])
        if key in by_round:
            raise ValueError(f"P12.5 {label}: round {key[0]} holds two {key[1]} rows")
        by_round[key] = float(row["reader_mops"])
    ratios: list[float] = []
    for r in range(rounds):
        optimistic = by_round.get((r, "prev"))
        locked = by_round.get((r, "prev_locked"))
        if optimistic is None or locked is None:
            raise ValueError(
                f"P12.5 {label}: round {r} is unpaired (prev {optimistic}, prev_locked {locked})"
            )
        if optimistic <= 0 or locked <= 0:
            raise ValueError(f"P12.5 {label}: round {r} has a non-positive throughput ({optimistic}, {locked})")
        ratios.append(optimistic / locked)
    if len(ratios) < 3:
        raise ValueError(f"P12.5 {label}: need at least 3 paired rounds for a BCa interval, got {len(ratios)}")
    mean, lo, hi, p125_ci_method = bca_bootstrap_ci_with_method(ratios, confidence=0.95)
    return {
        "cell": label, "probe": probe, "writers": writers, "readers": readers,
        "ratio_mean": round(mean, 6),
        "ratio_ci_lower": round(lo, 6),
        "ratio_ci_upper": round(hi, 6),
        "ratio_ci_method": p125_ci_method,
        "ratio_median": round(sorted(ratios)[len(ratios) // 2], 6),
        "paired_ratios_raw": [round(x, 6) for x in ratios],
        "verdict": p125_decision(lo, hi),
    }


def p125_report(throughput_rows: list[dict[str, Any]], rounds: int) -> dict[str, Any]:
    """The P12.5 gate cell, its control, and every other cell's ratio for reference."""
    reference = [
        {**p125_paired_ratio(throughput_rows, probe, w, r, rounds), "gated": False}
        for probe in ORDERED_READERS_PROBES
        for w, r in ORDERED_READERS_WR
        if (probe, w, r) not in (P125_GATE, P125_CONTROL)
    ]
    return {
        "statistic": "mean over rounds of reader_mops(prev) / reader_mops(prev_locked), each ratio paired "
                     "within one round of the throughput build, with a BCa 95% interval "
                     "(METHODOLOGY.md §12.3 P12.5)",
        "decision": "SINGLE_RUN_PASS when the lower bound is above 1.0, REJECTED when the upper bound is "
                    "below 1.0, INCONCLUSIVE otherwise; a claim needs the same verdict in two independent runs",
        "gate": {**p125_paired_ratio(throughput_rows, *P125_GATE, rounds), "gated": True},
        "control": {**p125_paired_ratio(throughput_rows, *P125_CONTROL, rounds), "gated": False},
        "reference": reference,
    }


def ordered_read_projection() -> dict[str, Any]:
    """§12.1's attempts-per-operation projection, which P12.4 reports beside. Not gated.

    Read from `scripts/olc_bounds.py`, under its per-node independence
    hypothesis and dated to the commits of the reader health artifacts it
    reads. A failure to compute it is recorded by name, never as zero (§8.1).
    """
    try:
        import olc_bounds  # noqa: PLC0415 -- optional: the projection is reported, not gated

        nodes = olc_bounds.max_ordered_read_set_branches()
        rows = olc_bounds.map_read_health()
        projected = [
            olc_bounds.ordered_projection(row["attempt_failure"], get_nodes, nodes)["expected_attempts"]
            for row in rows
            for get_nodes in (3, 5, 7)
        ]
    except (ImportError, OSError, ValueError, KeyError, json.JSONDecodeError) as exc:
        return {"available": False, "error": f"{type(exc).__name__}: {exc}", "gated": False}
    if not projected:
        return {"available": False, "error": "map_read_health() found no map reader cells", "gated": False}
    return {
        "available": True,
        "source": "scripts/olc_bounds.py: ordered_projection(attempt failure of each map_read_health() "
                  "cell, get_nodes in (3, 5, 7), max_ordered_read_set_branches())",
        "hypothesis": "per-node independence; writes concentrated near the probe break it",
        "dated_to_commits": sorted({row["commit"] for row in rows}),
        "ordered_nodes": nodes,
        "expected_attempts_min": round(min(projected), 4),
        "expected_attempts_max": round(max(projected), 4),
        "gated": False,
    }


def attempts_report(cells: list[dict[str, Any]]) -> dict[str, Any]:
    """Measured attempts per optimistic ordered read, per `prev` cell, beside the projection."""
    return {
        "per_cell": [
            {
                "cell": f"{c['probe']} W={c['writers']} R={c['readers']}",
                "read_ops": c["read_counters_total"]["read_ops"],
                "read_attempts": c["read_counters_total"]["read_attempts"],
                "attempts_per_op": c["attempts_per_op"],
            }
            for c in cells
            if c["read_op"] == "prev"
        ],
        "projection": ordered_read_projection(),
        "gated": False,
    }


def build_ordered_readers_artifact(
    prov: dict[str, Any],
    cells: list[dict[str, Any]],
    throughput_rows: list[dict[str, Any]],
    rounds: int,
    applied_pin: str,
    quick: bool,
) -> dict[str, Any]:
    """The committed shape: provenance, the cells, and both predictions' verdicts."""
    conforms = pins_equal(applied_pin, ORDERED_READERS_PIN)
    void: list[str] = []
    if not conforms:
        void.append(f"applied pin {applied_pin!r} is not {ORDERED_READERS_PIN} (METHODOLOGY.md §12.5)")
    if quick:
        void.append("--quick population: a smoke run of the instrument, not the §12.4 cells")
    return {
        # Every reader cell already runs in a process of its own
        # (`run_reader_invocation`); stated, as for the writer sweep (§15).
        "provenance": {**prov, "cell_isolation": CELL_ISOLATION},
        "throughput": cells,
        "ordered_readers": {
            "issue": 900,
            "preregistration": "docs/benchmarks/concurrency/METHODOLOGY.md §12.3-§12.5",
            "pin": {"required": ORDERED_READERS_PIN, "applied": applied_pin, "conforms": conforms},
            "rounds": rounds,
            "quick": quick,
            "schedule": "each round runs both probe blocks, alternating which goes first; within a block the "
                        "eight (read_op, W, R) cells follow that round's row of a Williams design; one harness "
                        "process per cell; the throughput pass runs every round before the counters pass",
            "void": void,
            "soundness_gates": "not evaluated by this driver: a cell read before every §12.2 gate passed on "
                               "the measured head is void (§12.5)",
            "p12_4": p124_verdict(cells),
            "attempts_per_op": attempts_report(cells),
            "p12_5": p125_report(throughput_rows, rounds),
        },
    }


def run_ordered_readers(args: argparse.Namespace) -> int:
    """`--ordered-readers`: the §12.4 cells, both roles, and the P12.4 / P12.5 verdicts."""
    out_path = Path(args.out) if args.out else ORDERED_READERS_RESULTS_PATH
    committed = out_path.resolve().is_relative_to((REPO_ROOT / "docs" / "benchmarks").resolve())
    smoke = bool(args.quick) and not committed
    try:
        notice = resolve_ordered_readers_pin(os.environ, smoke)
    except ValueError as exc:
        sys.stderr.write(f"refusing to start: {exc}\nNo benchmark was run and no numbers were produced.\n")
        return 1
    if notice:
        sys.stderr.write(f"::notice:: smoke run: {notice}; nothing this run produces is a §12.4 cell\n")
    applied = bench_pin.apply("writer_scaling.py --ordered-readers")
    if not pins_equal(applied, ORDERED_READERS_PIN) and not smoke:
        sys.stderr.write(
            f"refusing to start: the applied pin is {applied!r}, not {ORDERED_READERS_PIN} (§12.5)\n"
        )
        return 1

    throughput_bin, counters_bin = build_binaries(verbose=True)
    ratio = ("P12.5: mean over rounds of reader_mops(prev) / reader_mops(prev_locked), paired within each "
             "round, BCa 95% interval")
    prov = new_provenance(
        suite="concurrency",
        issue=900,
        ratio=ratio,
        repo_root=REPO_ROOT,
        core_pin=applied,
        estimators=estimators(
            ratio,
            columns="per-cell reader_mops_mean is the mean over rounds with a BCa 95% interval; "
                    "reader_mops_median is auxiliary; P12.4 reads summed counters over rounds",
        ),
    )
    schedule = ordered_readers_schedule(args.rounds)
    print("========================================================================")
    print(" Ordered readers on SyncExpanseMap (#900, METHODOLOGY.md §12.4)")
    print(f" Cells: {len(ORDERED_READERS_PROBES) * len(ORDERED_READERS_BLOCK)} | Rounds: {args.rounds} | "
          f"Pin: {applied} | Quick: {bool(args.quick)}")
    print("========================================================================")
    try:
        start = begin_cell(prov, "ordered_readers:throughput")
        t_rows = []
        for i, run in enumerate(schedule):
            t_rows.append(run_reader_invocation(throughput_bin, "throughput", run, args.quick))
            if (i + 1) % len(ORDERED_READERS_BLOCK) == 0:
                print(f"  [throughput] {i + 1}/{len(schedule)} cells")
        load = end_cell(start)
        c_rows = []
        for i, run in enumerate(schedule):
            c_rows.append(run_reader_invocation(counters_bin, "counters", run, args.quick))
            if (i + 1) % len(ORDERED_READERS_BLOCK) == 0:
                print(f"  [counters] {i + 1}/{len(schedule)} cells")
        check_row_pins(t_rows + c_rows, applied)
        cells = summarize_ordered_readers(t_rows, c_rows, args.rounds, load)
        artifact = build_ordered_readers_artifact(prov, cells, t_rows, args.rounds, applied, bool(args.quick))
    except (RuntimeError, ValueError) as exc:
        sys.stderr.write(f"ordered readers failed: {exc} (AGENTS.md §8.1)\n")
        return 1

    report = artifact["ordered_readers"]
    for c in cells:
        rate = "n/a" if c["fallback_rate"] is None else f"{c['fallback_rate'] * 100:.4f}%"
        print(
            f"  {c['probe']:>7} {c['read_op']:>11} W={c['writers']} R={c['readers']} | reader Mops/s "
            f"{c['reader_mops_mean']:.4f} [{c['reader_ci_lower']:.4f}, {c['reader_ci_upper']:.4f}] "
            f"| attempts/op {c['attempts_per_op']} | fallbacks {rate}"
        )
    p124 = report["p12_4"]
    print(f"  P12.4: {p124['verdict']}" + (f" in {', '.join(p124['refuted_cells'])}" if p124["refuted_cells"] else ""))
    for name in ("gate", "control"):
        e = report["p12_5"][name]
        print(f"  P12.5 {name} ({e['cell']}): ratio {e['ratio_mean']:.4f} "
              f"[{e['ratio_ci_lower']:.4f}, {e['ratio_ci_upper']:.4f}] {e['verdict']}")
    for reason in report["void"]:
        sys.stderr.write(f"::warning:: this run is void as a §12.4 measurement: {reason}\n")

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(artifact, indent=2) + "\n")
    print(f"\nWrote artifact to {out_path}")
    return 0


def probe_pmu_events() -> list[str]:
    events: list[str] = []
    proc = subprocess.run(["perf", "list"], capture_output=True, text=True, check=False)
    out = proc.stdout if proc.returncode == 0 else ""

    if "cpu_core/cycles/" in out:
        events.append("cpu_core/cycles/")
    elif "cycles" in out:
        events.append("cycles")

    if "cpu_core/ref-cycles/" in out:
        events.append("cpu_core/ref-cycles/")
    elif "ref-cycles" in out:
        events.append("ref-cycles")

    snoop_candidates = [
        "cpu_core/mem_load_l3_hit_retired.xsnp_fwd/",
        "mem_load_l3_hit_retired.xsnp_fwd",
        "cpu_core/mem_load_l3_hit_retired.xsnp_hitm/",
        "mem_load_l3_hit_retired.xsnp_hitm",
        "mem_load_retired.fb_hit",
    ]
    for cand in snoop_candidates:
        if cand in out:
            events.append(cand)
            break

    return events


def _resolve_event_key(
    observed: "set[str] | frozenset[str]",
    requested: list[str],
    *,
    want_ref: bool,
) -> str | None:
    """Pick the counts key for cycles (or ref-cycles) out of what perf reported.

    `perf` may report an event under a PMU-qualified name that does not match
    the string we asked for (`cycles` -> `cpu_core/cycles/`), so the request
    list is not a reliable index into the counts. Match on the observed keys and
    fall back to the request only when the observed set is empty.
    """
    def matches(k: str) -> bool:
        has_ref = "ref" in k
        return ("cycles" in k) and (has_ref if want_ref else not has_ref)

    for key in sorted(observed):
        if matches(key):
            return key
    return next((k for k in requested if matches(k)), None)


def parse_perf_stat_csv(stderr_text: str) -> dict[str, int]:
    result: dict[str, int] = {}
    for line in stderr_text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(",")
        if len(parts) >= 3:
            raw_val = parts[0].strip()
            if raw_val in ("<not counted>", "<not supported>"):
                continue
            try:
                count = int(raw_val)
            except ValueError:
                continue
            event_name = parts[2].strip()
            result[event_name] = count
    return result


def frequency_droop_by_writers(
    round_data: dict[int, dict[int, dict[str, float]]],
    cyc_key: str | None,
    ref_key: str | None,
    writers: list[int],
    rounds: int,
) -> dict[str, Any]:
    """Core frequency droop at each writer count against the lowest one.

    Frequency is `cycles / ref-cycles` and nothing else: the TSC ticks at a
    fixed nominal rate and is not the core clock, so it cannot answer this
    (AGENTS.md section 8.20.1). Droop at W is `1 - f(W) / f(1)`, paired within
    a round so a round's thermal state cancels, and summarised with a BCa 95%
    interval over rounds.

    Every W above the baseline gets its own entry. The pass used to compute one
    number, W=2 against W=1, which cannot speak about the W=4 cells where the
    multi-writer sweep's between-run spread actually appears (Refs #568).

    Pure: it takes counts and returns a summary, so the decision rule is
    testable without `perf` on the host.
    """
    out: dict[str, Any] = {
        "definition": "1 - (cycles/ref-cycles at W) / (cycles/ref-cycles at the baseline W)",
        "baseline_writers": min(writers) if writers else None,
        "rounds_preregistered": rounds,
        "by_writers": {},
    }
    if not writers or not cyc_key or not ref_key:
        return out
    base_w = min(writers)

    def freq(counts: dict[str, float]) -> float | None:
        cyc, ref = counts.get(cyc_key, 0), counts.get(ref_key, 0)
        return (cyc / ref) if ref > 0 and cyc > 0 else None

    for w in sorted(writers):
        if w == base_w:
            continue
        samples: list[float] = []
        for r in range(rounds):
            f_base = freq(round_data.get(r, {}).get(base_w, {}))
            f_w = freq(round_data.get(r, {}).get(w, {}))
            if f_base and f_w:
                samples.append(1.0 - (f_w / f_base))
        entry: dict[str, Any] = {"n_measured": len(samples)}
        if len(samples) >= 3:
            mean_d, ci_lo, ci_hi, droop_ci_method = bca_bootstrap_ci_with_method(
                samples, confidence=0.95
            )
            # Verdict decision rule (sections 8.4 / 8.20): a single run cannot
            # CONFIRM, so a cleared floor is `SINGLE_RUN_PASS` pending a second
            # independent run.
            #   CI_lower > 0.05 -> SINGLE_RUN_PASS (candidate droop)
            #   CI_upper < 0.05 -> REJECTED
            #   spans 0.05      -> INCONCLUSIVE
            if ci_lo > 0.05:
                verdict = "SINGLE_RUN_PASS"
            elif ci_hi < 0.05:
                verdict = "REJECTED"
            else:
                verdict = "INCONCLUSIVE"
            entry.update({
                "droop_mean": round(mean_d, 4),
                "droop_ci_lower": round(ci_lo, 4),
                "droop_ci_method": droop_ci_method,
                "droop_ci_upper": round(ci_hi, 4),
                "verdict": verdict,
            })
        out["by_writers"][str(w)] = entry
    return out


@contextlib.contextmanager
def perf_control_fifos(prefix: str) -> Any:
    """A (ctl, ack) FIFO pair in a temp dir, for perf's `--control=fifo:`.

    The harness's `PerfControl` writes `enable` / `disable` to the ctl FIFO
    around the barrier-to-join window and waits on the ack FIFO, so a perf
    started with `--delay=-1` counts or samples only that window. A FIFO that
    cannot be created is a refusal, never a silent whole-process recording,
    which would fold workload generation, prefill and teardown into the
    measurement (AGENTS.md §8.1).
    """
    with tempfile.TemporaryDirectory(prefix=prefix) as fifo_dir:
        ctl_fifo = os.path.join(fifo_dir, "ctl.fifo")
        ack_fifo = os.path.join(fifo_dir, "ack.fifo")
        try:
            os.mkfifo(ctl_fifo)
            os.mkfifo(ack_fifo)
        except (OSError, AttributeError) as exc:
            raise RuntimeError(
                f"could not create perf control FIFOs in {fifo_dir}: {exc}; refusing to "
                "record without the measured-window control (AGENTS.md §8.1)"
            ) from exc
        yield ctl_fifo, ack_fifo


def perf_control_args(ctl_fifo: str, ack_fifo: str) -> list[str]:
    """perf's side of the FIFO handshake: start disabled, obey the ctl FIFO."""
    return ["--delay=-1", f"--control=fifo:{ctl_fifo},{ack_fifo}"]


def harness_control_args(ctl_fifo: str, ack_fifo: str) -> list[str]:
    """The harness's side: `PerfControl` enables around the measured window."""
    return ["--perf-ctl-fifo", ctl_fifo, "--perf-ack-fifo", ack_fifo]


def run_pmu_pass(
    binary: Path,
    arm: str = "set",
    writers: list[int] | None = None,
    rounds: int = 8,
    quick: bool = False,
) -> dict[str, Any]:
    if platform.system() != "Linux":
        raise RuntimeError(
            f"--pmu requested but host reports {platform.system()} (Linux required per AGENTS.md §8.1)"
        )
    if shutil.which("perf") is None:
        raise RuntimeError(
            "--pmu requested but 'perf' binary not found on PATH (AGENTS.md §8.1)"
        )

    if writers is None:
        writers = [1, 2]

    events = probe_pmu_events()
    if not events:
        raise RuntimeError(
            "--pmu requested but no target PMU events found via 'perf list' (AGENTS.md §8.1)"
        )

    print("\n========================================================================")
    print(f" Hardware PMU Counter Pass (perf stat across {rounds} rounds via FIFO control)")
    print(f" Events: {', '.join(events)} | Arm: {arm} | Writers: {writers}")
    print("========================================================================")

    event_arg = ",".join(events)
    round_data: dict[int, dict[int, dict[str, int]]] = {r: {} for r in range(rounds)}

    # One process per (W, round) cell, as in the throughput pass (METHODOLOGY.md
    # §15), in the same Williams order, so the droop at W is read off cells run
    # under the schedule the throughput it is set against ran under. This pass
    # already ran one W per process; what it gains is the order and a row that
    # is checked against the cell that asked for it.
    for cell in writer_cell_schedule(writers, rounds):
        r, w = cell["round"], cell["writers"]
        with perf_control_fifos(f"perf_fifo_r{r}_w{w}_") as (ctl_fifo, ack_fifo):
            cmd = [
                "perf",
                "stat",
                *perf_control_args(ctl_fifo, ack_fifo),
                "-x,",
                "-e",
                event_arg,
                "--",
                *writer_cell_argv(binary, arm, cell, quick),
                *harness_control_args(ctl_fifo, ack_fifo),
            ]

            proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
            if proc.returncode != 0:
                raise RuntimeError(
                    f"--pmu round {r} W={w} failed (exit {proc.returncode}): {proc.stderr} (AGENTS.md §8.1)"
                )
            writer_cell_row(proc.stdout, arm, cell)
            round_data[r][w] = parse_perf_stat_csv(proc.stderr)

    # Resolve against the keys `perf` REPORTED, not the ones we asked for. On a
    # hybrid host perf accepts a bare `cycles` and reports it back qualified as
    # `cpu_core/cycles/`, so matching on the request silently finds nothing and
    # the droop lands as `n_measured: 0` on every W while the raw counts sitting
    # beside it are complete -- the exact silent-zero §8.1 forbids. Observed on
    # run 34722607239: events `['cycles', 'cpu_core/ref-cycles/', ...]`, counts
    # keyed `cpu_core/cycles/`, droop empty.
    observed: set[str] = set()
    for per_w in round_data.values():
        for counts in per_w.values():
            observed.update(counts)
    cyc_key = _resolve_event_key(observed, events, want_ref=False)
    ref_key = _resolve_event_key(observed, events, want_ref=True)
    if observed and (cyc_key is None or ref_key is None):
        raise RuntimeError(
            "--pmu collected counts but could not resolve the cycles/ref-cycles keys "
            f"from them (observed: {sorted(observed)}; requested: {events}). "
            "Frequency droop cannot be computed and must not be reported as zero "
            "samples (AGENTS.md §8.1)."
        )
    droop_summary = frequency_droop_by_writers(round_data, cyc_key, ref_key, writers, rounds)
    for w, entry in sorted(droop_summary["by_writers"].items(), key=lambda kv: int(kv[0])):
        if "verdict" not in entry:
            continue
        print(
            f"  [Hypothesis A (Frequency Droop) W={w}] Mean drop: "
            f"{entry['droop_mean'] * 100:.2f}% "
            f"[{entry['droop_ci_lower'] * 100:.2f}%, {entry['droop_ci_upper'] * 100:.2f}%] "
            f"| Verdict: {entry['verdict']}"
        )

    return {
        "arm": arm,
        "events": events,
        "cell_isolation": CELL_ISOLATION,
        "rounds_preregistered": rounds,
        "writers": sorted(writers),
        "frequency_droop": droop_summary,
        "raw_counts": round_data,
    }


# Rounds the c2c pass records. Each round contributes one barrier-to-join
# window; everything between windows (per-round prefill, verification, drop)
# is outside the recording. 8 is the harness's own default, and it is the round
# count of the windowed recording that confirmed the control handshake on the
# reference host (map, W = 8, pin 0,2,4,6,8,10,12,14): 8,168 HITM records, where
# the whole-process single-round recording had about 1,100.
C2C_ROUNDS = 8

# The c2c block's statement of what the profile covers. A committed artifact
# carries it so a reader never has to infer whether setup is in the shares.
C2C_WINDOW = (
    "measured barrier-to-join only (perf --delay=-1 --control=fifo, harness "
    "--perf-ctl-fifo); setup and teardown excluded"
)
# The c2c pass is one `perf c2c record` over one harness process that runs the
# same W for every round. Splitting it per round would need one recording per
# process and a merged report; it reads which cache lines carried HITM, not a
# throughput, and it has no cross-W carryover because W never changes. Rounds
# after the first do follow a cell of the same W in the same process, which
# METHODOLOGY.md §15 does not measure, so the block says so.
C2C_CELL_ISOLATION = "one process, one W, every round (not per cell; METHODOLOGY.md §15)"


def c2c_total_records(report_text: str) -> int | None:
    """`Total records` from a `perf c2c report --stdio` header, or None."""
    m = re.search(r"Total records\s*:\s*(\d+)", report_text)
    return int(m.group(1)) if m else None


def run_c2c_pass(
    binary: Path,
    arm: str = "set",
    writers: int = 2,
    quick: bool = False,
    out_dir: Path | None = None,
    rounds: int = C2C_ROUNDS,
) -> dict[str, Any]:
    if rounds < 1:
        raise ValueError(f"--c2c needs at least one round, got {rounds}")
    if platform.system() != "Linux":
        raise RuntimeError(
            f"--c2c requested but host reports {platform.system()} (Linux required per AGENTS.md §8.1)"
        )
    if shutil.which("perf") is None:
        raise RuntimeError(
            "--c2c requested but 'perf' binary not found on PATH (AGENTS.md §8.1)"
        )

    if out_dir is None:
        out_dir = REPO_ROOT / "target" / "c2c"
    out_dir.mkdir(parents=True, exist_ok=True)
    c2c_data = out_dir / f"perf_c2c_{arm}_w{writers}.data"

    print("\n========================================================================")
    print(f" Hardware perf c2c Cache Contention Recording ({arm} W={writers}, {rounds} rounds)")
    print(f" Window: {C2C_WINDOW}")
    print(f" Output: {c2c_data.relative_to(REPO_ROOT)}")
    print("========================================================================")

    # Record only the harness's barrier-to-join windows: perf starts with its
    # events disabled and the harness enables them around each round's
    # measured region. A whole-process recording also samples workload
    # generation (the prefill sort), prefill and teardown.
    with perf_control_fifos(f"perf_c2c_fifo_{arm}_w{writers}_") as (ctl_fifo, ack_fifo):
        cmd_record = [
            "perf",
            "c2c",
            "record",
            "-F",
            "60000",
            *perf_control_args(ctl_fifo, ack_fifo),
            "-o",
            str(c2c_data),
            "--",
            str(binary),
            "--role",
            "throughput",
            "--arm",
            arm,
            "--writers",
            str(writers),
            "--rounds",
            str(rounds),
            *harness_control_args(ctl_fifo, ack_fifo),
        ]
        if quick:
            cmd_record.append("--quick")

        proc = subprocess.run(cmd_record, check=False)
    if proc.returncode != 0:
        raise RuntimeError(f"--c2c record failed (exit {proc.returncode}) (AGENTS.md §8.1)")

    cmd_report = ["perf", "c2c", "report", "--stdio", "-i", str(c2c_data)]
    proc_rep = subprocess.run(cmd_report, capture_output=True, text=True, check=False)
    if proc_rep.returncode != 0:
        raise RuntimeError(
            f"--c2c report failed (exit {proc_rep.returncode}): {proc_rep.stderr} "
            "(an empty recording, e.g. the control handshake never enabled the "
            "events, also fails here) (AGENTS.md §8.1)"
        )
    report_text = proc_rep.stdout
    # With --delay=-1 nothing is sampled until the harness writes `enable`.
    # Zero records therefore means the handshake did not open the window, and
    # the profile would be empty rather than wrong -- still not a result.
    total_records = c2c_total_records(report_text)
    if not total_records:
        raise RuntimeError(
            f"--c2c recorded no samples inside the measured window (Total records: "
            f"{total_records}); the perf control handshake did not enable recording, "
            "and no whole-process recording is substituted (AGENTS.md §8.1)"
        )
    report_file = out_dir / f"c2c_report_{arm}_w{writers}.txt"
    report_file.write_text(report_text)
    print(f"  [c2c Pass] Wrote c2c report to {report_file.relative_to(REPO_ROOT)}")

    summary_lines = report_text.splitlines()[:20]
    hot_lines = c2c_hot_cache_lines(report_text)

    # `perf c2c report` pads the symbol column to a fixed width and truncates
    # Rust v0 manglings inside it -- run 34727964001 returned
    # `_RNvMsc_NtCsfHnohCjqgYz_12`, cut at the crate-name boundary, which names
    # nothing. `perf report` over the same recording prints the symbol
    # untruncated, so the two together give the contended line AND the
    # functions touching it.
    sym_rows: list[str] = []
    cmd_sym = [
        "perf", "report", "--stdio", "--no-children",
        "--sort", "symbol,dso", "-i", str(c2c_data),
    ]
    proc_sym = subprocess.run(cmd_sym, capture_output=True, text=True, check=False)
    if proc_sym.returncode == 0:
        sym_rows = [
            ln.rstrip() for ln in proc_sym.stdout.splitlines()
            if ln.strip() and not ln.lstrip().startswith("#")
        ][:40]
    else:
        # Not fatal: the c2c measurement above already succeeded and is the
        # gate's subject. Say so rather than dropping it silently (§8.1).
        print(
            f"  [c2c Pass] ::notice:: perf report symbol pass failed "
            f"(exit {proc_sym.returncode}); cache lines captured, symbols not"
        )

    return {
        "arm": arm,
        "writers": writers,
        # What the profile covers, so a reader never has to infer whether the
        # HITM shares and symbol_profile include setup (they do not).
        "window": C2C_WINDOW,
        "cell_isolation": C2C_CELL_ISOLATION,
        "rounds": rounds,
        "total_records": total_records,
        "report_path": str(report_file.relative_to(REPO_ROOT)),
        "data_path": str(c2c_data.relative_to(REPO_ROOT)),
        "summary": "\n".join(summary_lines),
        "symbol_profile": sym_rows,
        # The first 20 lines are the trace-event totals: how much HITM traffic
        # there was, never which lines carried it. The whole point of a c2c pass
        # is the address attribution, and the report file it lives in stays on
        # the runner. Carry the shared-cache-line table into the artifact so a
        # committed run can say WHICH line bounced (run 34722607239 measured
        # 7.54 snoop-forwards per insert at W=8 and could not name one).
        "hot_cache_lines": hot_lines,
    }


def c2c_hot_cache_lines(report_text: str, max_rows: int = 160) -> list[str]:
    """The shared-cache-line table out of a `perf c2c report --stdio` dump.

    `perf` prints a "Shared Data Cache Line Table" (older builds: "Shared Cache
    Line Distribution Pareto") listing the contended lines by HITM count, with
    the symbol and offset that touched each. That table is the measurement; the
    trace-event totals above it only say how much traffic there was.

    Returns the table's lines, capped, or an empty list when the report has no
    such section -- a zero-contention run legitimately has none, and this is a
    reporting helper, so an empty list is a real answer and not a silent
    failure. The caller still records the full report path.

    The cap covers the address table AND the per-line detail that follows it,
    because the addresses alone cannot name a structure. Run 34727050294
    attributed 60.1% of HITM to 13 sixty-four-byte-aligned lines inside one
    3,072-byte span -- unmistakably a padded per-slot array rather than trie
    node version words, which would be scattered across the heap -- and still
    could not say which array. The symbols live in the detail rows.
    """
    heads = (
        "Shared Data Cache Line Table",
        "Shared Cache Line Distribution Pareto",
    )
    lines = report_text.splitlines()
    start = None
    for i, ln in enumerate(lines):
        if any(h in ln for h in heads):
            start = i
            break
    if start is None:
        return []
    out: list[str] = []
    for ln in lines[start:]:
        if len(out) >= max_rows:
            break
        out.append(ln)
    return out


def _assert_fallbacks_counted(arm: str, fallbacks: list) -> None:
    """Assert what is deterministic about the fallback counter, and only that.

    The previous form was `all(fb > 0 for fb in fallbacks)`: every round of a
    three-round `--quick` run had to observe at least one lock fallback. Whether
    two writer threads actually collide in a short run is timing, not an
    invariant, and on a GitHub-hosted runner they frequently do not. The same
    commit produced `[0, 1, 0]` and then `[0, 0, 0]` on consecutive runs of the
    same job, so the assertion failed non-deterministically and took the rollup
    gate with it. AGENTS.md section 8.4 is explicit that hard assertions belong on
    deterministic invariants only.

    What the check is actually for is the build/role distinction: that the
    occ-stats binary reports `lock_fallbacks` and the default one does not. That
    is deterministic and is asserted here and at the throughput pass above
    (`all("lock_fallbacks" not in r ...)`), so the pair still fails closed if the
    counter is not wired, mis-parsed, or emitted by the wrong build.

    A zero count across every round is reported rather than asserted away: it
    says the quick run saw no contention, which is information about the host,
    not a defect in the instrument (section 8.1 -- a degradation that is visible
    beats one that is silent).
    """
    if not fallbacks:
        raise AssertionError(f"{arm} W=2: no counter rows at all")
    for fb in fallbacks:
        if not isinstance(fb, int) or isinstance(fb, bool) or fb < 0:
            raise AssertionError(
                f"{arm} W=2 lock_fallbacks must be a non-negative int on every round, "
                f"got {fallbacks!r}")
    if not any(fb > 0 for fb in fallbacks):
        print(f"    note: {arm} W=2 observed no lock fallbacks in {len(fallbacks)} quick "
              f"rounds ({fallbacks}) -- the writers did not collide on this host. The "
              f"counter is present and parsed, which is what this pass verifies; "
              f"contention itself is not an invariant of a short run.")


def _synthetic_reader_rows(
    rounds: int,
    mops: Any,
    fallbacks: Any,
    pin: str,
    reader_ops: int = 100_000,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Rows in the harness's reader-mode schema for every scheduled run.

    `mops(run)` gives a throughput row's `reader_mops`, and `fallbacks(run)` a
    `prev` counters row's `read_fallbacks`. Every counter identity holds, so
    the statistics can be driven to a chosen verdict through the production
    functions.
    """
    t_rows: list[dict[str, Any]] = []
    c_rows: list[dict[str, Any]] = []
    for run in ordered_readers_schedule(rounds):
        fresh = 0 if run["writers"] == 0 else 65_280
        base = {
            "workload_id": ORDERED_READERS_WORKLOAD_ID, "arm": "expanse",
            "cell": f"map_w{run['writers']}_r{run['readers']}_{run['read_op']}_{run['probe']}",
            "keyspace_bits": 64, "prefill": 4096, "hotspot_prefill": 256, "hotspot_base": 1 << 40,
            "fresh_keys": fresh, "write_ops": fresh, "cpu_pin": pin, "tsc_hz": 1,
            "population_after": 4096 + 256 + fresh, **run,
        }
        t_rows.append({
            **base, "role": "throughput",
            "writer_elapsed_s": 1.0 if fresh else None, "writer_mops": fresh / 1e6 if fresh else None,
            "reader_ops": reader_ops, "reader_elapsed_s": 1.0, "reader_mops": mops(run),
        })
        optimistic = run["read_op"] == "prev"
        fb = fallbacks(run) if optimistic else 0
        locked = fb if optimistic else reader_ops
        c_rows.append({
            **base, "role": "counters", "reader_ops": reader_ops,
            "read_ops": reader_ops if optimistic else 0,
            # A fallen-back read made MAX_RETRIES (64) failed attempts; every
            # other read made one successful attempt.
            "read_attempts": reader_ops + 63 * fb if optimistic else 0,
            "read_fallbacks": fb, "locked_reads": locked,
            "lock_fallbacks": 0, "inserts": fresh, "quiesce_calls": locked,
            "fallback_causes": {name: 0 for name in CAUSE_NAMES},
        })
    return t_rows, c_rows


def _expect_value_error(fn: Any, needle: str) -> None:
    """`fn()` must raise `ValueError` naming `needle`: refused, never averaged (§8.1)."""
    try:
        fn()
    except ValueError as exc:
        assert needle in str(exc), (needle, str(exc))
        return
    raise AssertionError(f"expected a ValueError naming {needle!r}; nothing was raised")


def _self_test_ordered_readers(throughput_bin: Path, counters_bin: Path, pin: str) -> None:
    """The ordered-reader instrument (#900): schedule, verdicts, refusals, artifact, harness seam."""
    import check_bench_provenance as cbp  # noqa: PLC0415 -- the gate's own functions judge the artifact

    sys.stderr.write("Testing the ordered-reader instrument (#900, METHODOLOGY.md §12.3-§12.5)...\n")
    rounds = 8
    n = len(ORDERED_READERS_BLOCK)

    # The schedule: per probe block, every cell in every position once and
    # every ordered pair of cells adjacent once over 8 rounds; the block that
    # runs first alternates by round.
    sched = ordered_readers_schedule(rounds)
    assert len(sched) == rounds * len(ORDERED_READERS_PROBES) * n, len(sched)
    for probe in ORDERED_READERS_PROBES:
        positions = {cell: [0] * n for cell in ORDERED_READERS_BLOCK}
        pairs: dict[tuple[Any, Any], int] = {}
        for r in range(rounds):
            block = sorted((x for x in sched if x["round"] == r and x["probe"] == probe),
                           key=lambda x: x["position"])
            assert [x["position"] for x in block] == list(range(n)), block
            order = [(x["read_op"], x["writers"], x["readers"]) for x in block]
            assert sorted(order) == sorted(ORDERED_READERS_BLOCK), order
            for i, cell in enumerate(order):
                positions[cell][i] += 1
            for a, b in zip(order, order[1:]):
                pairs[(a, b)] = pairs.get((a, b), 0) + 1
        assert all(v == [1] * n for v in positions.values()), positions
        assert len(pairs) == n * (n - 1) and all(v == 1 for v in pairs.values()), pairs
    firsts = [next(x["probe"] for x in sched if x["round"] == r) for r in range(rounds)]
    assert firsts == [ORDERED_READERS_PROBES[r % 2] for r in range(rounds)], firsts

    load = {"since": "ordered_readers:throughput", "wall_s": 1.0, "busy_cpus_since_prev": 1.0,
            "own_busy_cpus": 1.0, "foreign_busy_cpus": 0.0}
    # Rounds differ eightfold in throughput; a statistic that is not paired
    # within the round cannot land near the within-round ratio.
    base = [1.0, 5.0, 2.0, 8.0, 3.0, 7.0, 4.0, 6.0]

    def optimistic_times(factor: Any) -> Any:
        return lambda run: base[run["round"]] * (factor(run) if run["read_op"] == "prev" else 1.0)

    def sweep(mops: Any, fallbacks: Any) -> tuple[Any, Any, Any]:
        t, c = _synthetic_reader_rows(rounds, mops, fallbacks, pin)
        cells = summarize_ordered_readers(t, c, rounds, load)
        return t, c, cells

    def no_fallbacks(run: dict[str, Any]) -> int:
        return 0

    def hot_w4(run: dict[str, Any]) -> bool:
        return run["probe"] == "hotspot" and run["writers"] == 4

    # P12.5, all three verdicts, at the call site the driver uses.
    pass_mops = optimistic_times(lambda run: 1.10 * (1.0 + 0.001 * run["round"]))
    t_pass, c_pass, cells_pass = sweep(pass_mops, no_fallbacks)
    p125 = p125_report(t_pass, rounds)
    gate = p125["gate"]
    assert gate["verdict"] == "SINGLE_RUN_PASS", gate
    expect = sum(1.10 * (1.0 + 0.001 * r) for r in range(rounds)) / rounds
    assert abs(gate["ratio_mean"] - expect) < 1e-4, (gate, expect)
    assert gate["ratio_ci_method"] in CI_METHODS, gate
    assert (gate["probe"], gate["writers"], gate["readers"]) == P125_GATE, gate
    assert gate["gated"] is True and p125["control"]["gated"] is False, p125
    assert (p125["control"]["probe"], p125["control"]["writers"], p125["control"]["readers"]) == P125_CONTROL
    assert len(p125["reference"]) == len(ORDERED_READERS_PROBES) * len(ORDERED_READERS_WR) - 2, p125
    t_rej, _, _ = sweep(optimistic_times(lambda run: 0.90 * (1.0 + 0.001 * run["round"])), no_fallbacks)
    assert p125_report(t_rej, rounds)["gate"]["verdict"] == "REJECTED"
    t_inc, _, _ = sweep(optimistic_times(lambda run: 1.20 if run["round"] % 2 == 0 else 0.85), no_fallbacks)
    assert p125_report(t_inc, rounds)["gate"]["verdict"] == "INCONCLUSIVE"

    # P12.4, both verdicts, and the boundary: 100 fallbacks in 100,000 reads a
    # round is exactly 0.1%, which is not below the ceiling.
    p124 = p124_verdict(cells_pass)
    assert p124["verdict"] == "HOLDS" and p124["refuted_cells"] == [], p124
    assert len(p124["per_cell"]) == len(ORDERED_READERS_PROBES) * len(ORDERED_READERS_WR), p124
    for fb, verdict in ((200, "REFUTED"), (100, "REFUTED"), (99, "HOLDS")):
        _, _, cells_fb = sweep(pass_mops, lambda run, fb=fb: fb if hot_w4(run) else 0)
        got = p124_verdict(cells_fb)
        assert got["verdict"] == verdict, (fb, got)
        assert got["refuted_cells"] == ([] if verdict == "HOLDS" else ["hotspot W=4 R=4"]), (fb, got)
    attempts = attempts_report(cells_pass)
    assert attempts["gated"] is False and len(attempts["per_cell"]) == len(p124["per_cell"]), attempts
    assert "available" in attempts["projection"], attempts

    # Refusals: a missing counters row, an unpaired or doubled round, a pin
    # mismatch, a broken identity, a timing field in a counters row.
    _expect_value_error(lambda: summarize_ordered_readers(t_pass, c_pass[1:], rounds, load),
                        "counters rows cover rounds")
    unpaired = [x for x in t_pass
                if (x["probe"], x["writers"], x["readers"], x["read_op"], x["round"]) != ("uniform", 1, 4, "prev_locked", 2)]
    _expect_value_error(lambda: p125_paired_ratio(unpaired, "uniform", 1, 4, rounds), "round 2 is unpaired")
    _expect_value_error(lambda: summarize_ordered_readers(unpaired, c_pass, rounds, load),
                        "throughput rows cover rounds")
    first = t_pass[0]
    _expect_value_error(
        lambda: p125_paired_ratio(t_pass + [dict(first)], first["probe"], first["writers"], first["readers"], rounds),
        "holds two")
    _expect_value_error(lambda: check_row_pins([*t_pass, {**first, "cpu_pin": "0-15"}], pin), "cpu_pin")
    tampered = [dict(x) for x in c_pass]
    i_prev = next(i for i, x in enumerate(tampered) if x["read_op"] == "prev")
    tampered[i_prev]["locked_reads"] += 1
    _expect_value_error(lambda: summarize_ordered_readers(t_pass, tampered, rounds, load), "quiesce_calls")
    leaked = [dict(x) for x in c_pass]
    leaked[0]["reader_mops"] = 1.0
    _expect_value_error(lambda: summarize_ordered_readers(t_pass, leaked, rounds, load), "timing field")

    # The pin: set when unset, refused when anything else is named, and let
    # through with a notice only for a smoke run.
    env: dict[str, str] = {}
    assert resolve_ordered_readers_pin(env, smoke=False) is None
    assert env["EXPANSE_BENCH_PIN"] == ORDERED_READERS_PIN, env
    assert resolve_ordered_readers_pin({"EXPANSE_BENCH_PIN": "14,12,10,8,6,4,2,0"}, smoke=False) is None
    _expect_value_error(lambda: resolve_ordered_readers_pin({"EXPANSE_BENCH_PIN": "0-15"}, smoke=False),
                        "EXPANSE_BENCH_PIN='0-15'")
    _expect_value_error(lambda: resolve_ordered_readers_pin({"EXPANSE_BENCH_PIN_APPLIED": "0-15"}, smoke=False),
                        "EXPANSE_BENCH_PIN_APPLIED='0-15'")
    _expect_value_error(lambda: resolve_ordered_readers_pin({"EXPANSE_BENCH_PIN": "off"}, smoke=False), "'off'")
    assert "EXPANSE_BENCH_PIN='off'" in (resolve_ordered_readers_pin({"EXPANSE_BENCH_PIN": "off"}, smoke=True) or "")

    # The artifact, judged by the provenance gate's own functions.
    prov = new_provenance(suite="concurrency", issue=900, ratio="self-test", repo_root=REPO_ROOT,
                          core_pin=pin, estimators=estimators("self-test"))
    rel = ORDERED_READERS_RESULTS_PATH.relative_to(cbp.BENCH).as_posix()
    assert any(Path(rel).match(g) for g in cbp.ARTIFACT_GLOBS), (rel, cbp.ARTIFACT_GLOBS)
    art = build_ordered_readers_artifact(prov, cells_pass, t_pass, rounds, pin, quick=True)
    assert cbp.findings_for(rel, art) == [], cbp.findings_for(rel, art)
    assert any("--quick" in v for v in art["ordered_readers"]["void"]), art["ordered_readers"]["void"]
    clean = build_ordered_readers_artifact(prov, cells_pass, t_pass, rounds, ORDERED_READERS_PIN, quick=False)
    assert clean["ordered_readers"]["void"] == [] and clean["ordered_readers"]["pin"]["conforms"] is True
    no_rounds = json.loads(json.dumps(art))
    del no_rounds["throughput"][0]["rounds_raw"]
    assert cbp.findings_for(rel, no_rounds), "the provenance gate must see a cell without rounds_raw"
    unlabelled = json.loads(json.dumps(art))
    del unlabelled["ordered_readers"]["p12_5"]["gate"]["ratio_ci_method"]
    assert any("construction label" in f for f in cbp.findings_for(rel, unlabelled)), "a dropped CI label must be seen"
    assert str(REPO_ROOT) not in json.dumps(art), "absolute repo path leaked into the artifact (AGENTS.md §7)"

    # The seam: real harness rows, spliced into a synthetic sweep, flow through
    # the same functions and yield a usable cell (AGENTS.md §8.20.7).
    wanted = {("uniform", "prev", 0, 1), ("hotspot", "prev_locked", 1, 4), ("hotspot", "prev", 1, 4)}
    short = 3
    real_runs = [x for x in ordered_readers_schedule(short)
                 if x["round"] == 0 and (x["probe"], x["read_op"], x["writers"], x["readers"]) in wanted]
    assert len(real_runs) == len(wanted), real_runs
    t3, c3 = _synthetic_reader_rows(short, pass_mops, no_fallbacks, pin)

    def key(x: dict[str, Any]) -> tuple[Any, ...]:
        return (x["probe"], x["read_op"], x["writers"], x["readers"], x["round"])

    for run in real_runs:
        rt = run_reader_invocation(throughput_bin, "throughput", run, quick=True)
        rc = run_reader_invocation(counters_bin, "counters", run, quick=True)
        check_row_pins([rt, rc], pin)
        t3 = [rt if key(x) == key(run) else x for x in t3]
        c3 = [rc if key(x) == key(run) else x for x in c3]
    cells3 = summarize_ordered_readers(t3, c3, short, load)
    real = next(c for c in cells3 if (c["probe"], c["read_op"], c["writers"], c["readers"]) == ("hotspot", "prev", 1, 4))
    assert real["counters_raw"][0]["read_ops"] == real["counters_raw"][0]["reader_ops"] > 0, real["counters_raw"][0]
    assert real["rounds_raw"][0]["reader_mops"] > 0 and real["rounds_raw"][0]["writer_mops"] > 0, real["rounds_raw"][0]
    assert real["rounds_raw"][0]["write_ops"] == 65_280, real["rounds_raw"][0]
    locked = next(c for c in cells3 if (c["probe"], c["read_op"], c["writers"], c["readers"]) == ("hotspot", "prev_locked", 1, 4))
    assert locked["counters_raw"][0]["read_ops"] == 0, locked["counters_raw"][0]
    assert locked["counters_raw"][0]["locked_reads"] == locked["counters_raw"][0]["reader_ops"] > 0
    control = next(c for c in cells3 if (c["probe"], c["read_op"], c["writers"], c["readers"]) == ("uniform", "prev", 0, 1))
    assert control["rounds_raw"][0]["writer_mops"] is None and control["counters_raw"][0]["inserts"] == 0, control
    assert control["rounds_raw"][0]["reader_ops"] == 4096, control["rounds_raw"][0]
    art3 = build_ordered_readers_artifact(prov, cells3, t3, short, pin, quick=True)
    assert cbp.findings_for(rel, art3) == [], cbp.findings_for(rel, art3)

    # The harness refuses what reader mode cannot measure, by name.
    for extra, needle in (
        (["--arm", "set", "--writers", "1", "--readers", "2"], "map arm only"),
        (["--arm", "map", "--writers", "1", "--readers", "2", "--read-op", "bogus"], "unknown --read-op"),
        (["--arm", "map", "--writers", "1", "--readers", "2", "--probe", "bogus"], "unknown --probe"),
        (["--arm", "map", "--writers", "0"], "--writers 0 is accepted only in reader mode"),
    ):
        proc = subprocess.run([str(throughput_bin), "--role", "throughput", *extra, "--quick"],
                              capture_output=True, text=True, check=False)
        assert proc.returncode != 0 and needle in proc.stderr, (extra, proc.returncode, proc.stderr)
    sys.stderr.write("Ordered-reader instrument PASSED\n")


def _self_test_c2c_window() -> None:
    """Pin that `run_c2c_pass` records only the barrier-to-join window.

    Drives the production function with `perf` stubbed at `subprocess.run`, so
    the assertions read the command `run_c2c_pass` actually built and the c2c
    block it actually returned -- not a helper that the call site could stop
    calling (AGENTS.md §8.20.7). A whole-process recording samples workload
    generation, prefill and teardown: the committed diagnostic artifact's
    symbol profile carries the prefill sort's `quicksort` at 3.53%.
    """
    import stat
    from unittest import mock

    eprintln = sys.stderr.write
    eprintln("Testing the c2c pass records the measured window only...\n")

    report = "\n".join([
        "=================================================",
        "            Trace Event Information              ",
        "=================================================",
        "  Total records                     :      54749",
        "  Load Local HITM                   :       8168",
        "",
        "=================================================",
        "           Shared Data Cache Line Table          ",
        "=================================================",
        "      0     1234   41.2%     1238     1238        0",
    ])
    out_dir = REPO_ROOT / "target" / "c2c-selftest"

    def stub(report_text: str, seen: dict[str, Any]) -> Any:
        def fake_run(cmd: list[str], *args: Any, **kwargs: Any) -> Any:
            cmd = list(cmd)
            if cmd[:3] == ["perf", "c2c", "record"]:
                seen["record"] = cmd
                # The FIFOs must exist, as FIFOs, while perf and the harness run.
                for flag in ("--perf-ctl-fifo", "--perf-ack-fifo"):
                    if flag in cmd:
                        p = cmd[cmd.index(flag) + 1]
                        seen[flag] = stat.S_ISFIFO(os.stat(p).st_mode)
                return subprocess.CompletedProcess(cmd, 0)
            if cmd[:3] == ["perf", "c2c", "report"]:
                return subprocess.CompletedProcess(cmd, 0, stdout=report_text, stderr="")
            if cmd[:2] == ["perf", "report"]:
                return subprocess.CompletedProcess(
                    cmd, 0, stdout="  65.52%  [.] olc_insert_map  writer_scaling\n", stderr=""
                )
            raise AssertionError(f"unexpected command in c2c self-test: {cmd}")
        return fake_run

    def drive(report_text: str, seen: dict[str, Any], **patches: Any) -> dict[str, Any]:
        with contextlib.ExitStack() as stack:
            stack.enter_context(mock.patch.object(platform, "system", return_value="Linux"))
            stack.enter_context(mock.patch.object(shutil, "which", return_value="/usr/bin/perf"))
            stack.enter_context(
                mock.patch.object(subprocess, "run", side_effect=stub(report_text, seen))
            )
            for name, value in patches.items():
                stack.enter_context(mock.patch.object(os, name, side_effect=value))
            return run_c2c_pass(
                Path("/nonexistent/writer_scaling"), arm="map", writers=8, out_dir=out_dir
            )

    try:
        seen: dict[str, Any] = {}
        block = drive(report, seen)
        cmd = seen.get("record")
        assert cmd is not None, "run_c2c_pass never invoked perf c2c record"
        sep = cmd.index("--")
        perf_part, harness_part = cmd[:sep], cmd[sep + 1:]

        # perf starts disabled and obeys the control FIFO ...
        assert "--delay=-1" in perf_part, (
            f"perf c2c record must start disabled (--delay=-1): {perf_part}"
        )
        controls = [a for a in perf_part if a.startswith("--control=fifo:")]
        assert len(controls) == 1, f"perf c2c record needs one --control=fifo: {perf_part}"
        # ... and the harness is told to drive that same pair.
        for flag in ("--perf-ctl-fifo", "--perf-ack-fifo"):
            assert flag in harness_part, f"harness command lacks {flag}: {harness_part}"
            assert seen.get(flag) is True, f"{flag} was not a FIFO when perf ran: {seen}"
        ctl = harness_part[harness_part.index("--perf-ctl-fifo") + 1]
        ack = harness_part[harness_part.index("--perf-ack-fifo") + 1]
        assert controls[0] == f"--control=fifo:{ctl},{ack}", (controls, ctl, ack)
        assert not os.path.exists(ctl) and not os.path.exists(ack), "FIFOs outlived the pass"
        assert harness_part[harness_part.index("--rounds") + 1] == str(C2C_ROUNDS), harness_part

        # The artifact's c2c block says what the profile covers.
        assert block.get("window") == C2C_WINDOW, block
        assert "setup and teardown excluded" in block["window"], block
        assert block.get("rounds") == C2C_ROUNDS, block
        assert block.get("total_records") == 54749, block
        assert block["hot_cache_lines"], block

        # Negative control: a FIFO that cannot be created is a refusal, and no
        # recording happens at all -- never a whole-process fallback (§8.1).
        seen_fail: dict[str, Any] = {}
        try:
            drive(report, seen_fail, mkfifo=OSError("mkfifo denied"))
        except RuntimeError as exc:
            assert "AGENTS.md §8.1" in str(exc) and "FIFO" in str(exc), exc
        else:
            raise AssertionError("run_c2c_pass recorded although FIFO creation failed")
        assert "record" not in seen_fail, f"perf ran without its control FIFOs: {seen_fail}"

        # Negative control: an empty window (the handshake never enabled the
        # events) is refused rather than reported as a contention-free profile.
        empty = report.replace("54749", "0")
        try:
            drive(empty, {})
        except RuntimeError as exc:
            assert "handshake" in str(exc) and "AGENTS.md §8.1" in str(exc), exc
        else:
            raise AssertionError("run_c2c_pass accepted a recording with zero records")
    finally:
        shutil.rmtree(out_dir, ignore_errors=True)

    eprintln("c2c measured-window checks PASSED\n")


def _self_test_per_cell_isolation(throughput_bin: Path, counters_bin: Path) -> None:
    """Every timed writer pass runs one W per harness process (METHODOLOGY.md §15).

    Drives the production entry points -- `main()` for the sweep and the
    comparison, `run_pmu_pass` for the droop pass -- with `subprocess.run`
    replaced at the module boundary, so the assertions read the argv the driver
    actually built and the artifact it actually wrote, never a helper the call
    site could stop calling (AGENTS.md §8.20.7). The fake harness emulates the
    real one's writer mode: a `--writers` list prints every W of the round in
    Williams order, and a missing `--position` prints the index in that order.
    The counters pass still reaches the real counters binary.
    """
    import io
    import time
    from unittest import mock

    eprintln = sys.stderr.write
    eprintln("Testing one harness process per timed writer cell (METHODOLOGY.md §15)...\n")
    module = sys.modules[__name__]
    real_run = subprocess.run
    real_summarize = summarize_arm
    variant = "selftest-variant"
    fake_default = REPO_ROOT / "target" / "selftest-per-cell-default" / "release" / "examples" / "writer_scaling"
    fake_variant = REPO_ROOT / "target" / "selftest-per-cell-variant" / "release" / "examples" / "writer_scaling"
    fake_builds = {str(fake_default): DEFAULT_BUILD, str(fake_variant): variant}

    def flag(cmd: list[str], name: str) -> str | None:
        return cmd[cmd.index(name) + 1] if name in cmd else None

    def harness_stdout(cmd: list[str], build: str, tamper: Any) -> str:
        writers = [int(x) for x in (flag(cmd, "--writers") or "1,2,4,8").split(",")]
        rnd = flag(cmd, "--round")
        rounds = [int(rnd)] if rnd is not None else range(int(flag(cmd, "--rounds") or 8))
        pos_flag = flag(cmd, "--position")
        lines = []
        for r in rounds:
            order = [writers[i] for i in williams_positions(len(writers), r)]
            for i, w in enumerate(order):
                row = {
                    "workload_id": "concurrency_writer_map_64bit", "role": "throughput",
                    "arm": "expanse", "cell": f"map_w{w}_r0", "keyspace_bits": 64,
                    "prefill": 4096, "fresh_keys": 4096, "writers": w, "readers": 0,
                    "round": r, "position": int(pos_flag) if pos_flag is not None else i,
                    "write_ops": 4096, "writer_elapsed_s": 0.001,
                    "writer_mops": round((1.0 + 0.4 * w) * (1.0 + 0.01 * r) * (1.1 if build != DEFAULT_BUILD else 1.0), 4),
                    "tsc_hz": 1, "population_after": 8192,
                }
                tamper(row)
                lines.append(json.dumps(row))
        return "\n".join(lines) + "\n"

    def fake_subprocess(timed: list[tuple[str, list[str]]], counters: list[list[str]], tamper: Any) -> Any:
        def fake_run(cmd: Any, *args: Any, **kwargs: Any) -> Any:
            argv = [str(c) for c in cmd]
            if argv and argv[0] in fake_builds:
                build = fake_builds[argv[0]]
                assert flag(argv, "--role") == "throughput", f"a fake throughput binary was run as {argv}"
                timed.append((build, argv))
                return subprocess.CompletedProcess(argv, 0, stdout=harness_stdout(argv, build, tamper), stderr="")
            if argv and argv[0] == str(counters_bin):
                counters.append(argv)
            return real_run(cmd, *args, **kwargs)
        return fake_run

    def fake_build(features: str | None = None, verbose: bool = True) -> tuple[Path, Path]:
        return (fake_default if features is None else fake_variant), counters_bin

    class SteppingClock:
        """`time` for `bench_provenance`, whose `monotonic()` advances 1 ms more per read.

        With the builds stubbed, snapshots the driver takes back to back sit
        well under a millisecond apart, and a stored snapshot's clock is
        rounded to 1 ms, so their difference can come out zero or negative --
        which the accounting refuses on that ground alone. The extra step
        makes every such window positive and still far below the minimum, so
        the window assertion below is decided by the minimum, every run.
        """

        def __init__(self) -> None:
            self.reads = 0

        def monotonic(self) -> float:
            self.reads += 1
            return time.monotonic() + 0.001 * self.reads

        def __getattr__(self, name: str) -> Any:
            return getattr(time, name)

    def drive_main(argv: list[str], tamper: Any = lambda row: None, summarize: Any = None,
                   clock: Any = None) -> dict[str, Any]:
        timed: list[tuple[str, list[str]]] = []
        counters: list[list[str]] = []
        err: BaseException | None = None
        art = None
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "selftest_per_cell_writer_scaling.json"
            with contextlib.ExitStack() as stack:
                if clock is not None:
                    stack.enter_context(mock.patch.object(sys.modules["bench_provenance"], "time", clock))
                stack.enter_context(mock.patch.object(
                    subprocess, "run", side_effect=fake_subprocess(timed, counters, tamper)))
                stack.enter_context(mock.patch.object(module, "build_binaries", side_effect=fake_build))
                if summarize is not None:
                    stack.enter_context(mock.patch.object(module, "summarize_arm", side_effect=summarize))
                stack.enter_context(mock.patch.object(
                    sys, "argv", ["writer_scaling.py", *argv, "--quick", "--out", str(out)]))
                stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                try:
                    rc = main()
                    assert rc == 0, f"main({argv}) returned {rc}"
                except (RuntimeError, ValueError, AssertionError) as exc:
                    err = exc
            if out.exists():
                art = json.loads(out.read_text())
        return {"timed": timed, "counters": counters, "err": err, "artifact": art}

    def assert_one_w_per_process(timed: list[tuple[str, list[str]]], context: str) -> None:
        assert timed, f"{context}: the timed pass invoked the harness zero times"
        for _, cmd in timed:
            w = flag(cmd, "--writers")
            assert w is not None and "," not in w, (
                f"{context}: a timed writer pass invoked the harness with --writers {w!r}: more than "
                f"one W in one process (METHODOLOGY.md §15): {cmd}"
            )
            assert flag(cmd, "--round") is not None and "--rounds" not in cmd, (
                f"{context}: a timed writer invocation must run exactly one round: {cmd}"
            )
            assert flag(cmd, "--position") is not None, (
                f"{context}: a timed writer invocation carries no --position, so its row cannot "
                f"name its scheduled place (METHODOLOGY.md §15): {cmd}"
            )

    def invoked_cells(timed: list[tuple[str, list[str]]]) -> list[tuple[str, int, int, int]]:
        """(build, round, position, W) per invocation, in execution order, read off the argv."""
        return [(b, int(flag(c, "--round")), int(flag(c, "--position")), int(flag(c, "--writers")))
                for b, c in timed]

    def assert_balance(timed: list[tuple[str, list[str]]], builds: tuple[str, ...],
                       writers: list[int], rounds: int, context: str) -> None:
        cells = invoked_cells(timed)
        treatments = [(b, w) for w in writers for b in builds]
        n = len(treatments)
        assert rounds == n, f"{context}: a balance check needs one full cycle ({n} rounds), got {rounds}"
        assert len(cells) == rounds * n, (
            f"{context}: {len(cells)} timed processes, expected rounds x builds x W = {rounds * n}"
        )
        positions = {t: [0] * n for t in treatments}
        pairs: dict[tuple[Any, Any], int] = {}
        for r in range(rounds):
            run = [c for c in cells if c[1] == r]
            order = [(b, w) for b, _, _, w in run]
            assert sorted(order) == sorted(treatments), (
                f"{context}: round {r} ran {order}, not every (build, W) cell once"
            )
            assert [pos for _, _, pos, _ in run] == list(range(n)), (
                f"{context}: round {r} ran positions {[pos for _, _, pos, _ in run]} in execution "
                f"order, expected 0..{n - 1}"
            )
            for i, t in enumerate(order):
                positions[t][i] += 1
            for a, b in zip(order, order[1:]):
                pairs[(a, b)] = pairs.get((a, b), 0) + 1
        if len(builds) > 1:
            for pos in range(n):
                at = collections.Counter(b for b, _, p, _ in cells if p == pos)
                assert all(at[b] == rounds // len(builds) for b in builds), (
                    f"{context}: build interleave unbalanced: position {pos} ran {dict(at)} over "
                    f"{rounds} rounds (expected {rounds // len(builds)} per build)"
                )
        for t, counts in positions.items():
            for pos, c in enumerate(counts):
                assert c == 1, (
                    f"{context}: Williams balance broken: (build, W) = {t} held position {pos} "
                    f"{c} times over {rounds} rounds (expected 1)"
                )
        for a in treatments:
            for b in treatments:
                if a != b:
                    assert pairs.get((a, b), 0) == 1, (
                        f"{context}: Williams carryover balance broken: {a} -> {b} adjacent "
                        f"{pairs.get((a, b), 0)} times over {rounds} rounds (expected 1)"
                    )

    def assert_artifact_matches(res: dict[str, Any], context: str) -> None:
        art = res["artifact"]
        assert art is not None, f"{context}: no artifact was written ({res['err']})"
        assert art["provenance"].get("cell_isolation") == CELL_ISOLATION, (
            f"{context}: provenance.cell_isolation = {art['provenance'].get('cell_isolation')!r}"
        )
        ran = collections.Counter(invoked_cells(res["timed"]))
        rows = collections.Counter()
        for key in ("throughput", "throughput_variant"):
            for cell in art.get(key, []):
                for r in cell["rounds_raw"]:
                    rows[(r["build"], r["round"], r["position"], cell["writers"])] += 1
        assert rows == ran, (
            f"{context}: artifact rows (build, round, position, W) disagree with the invocations "
            f"that produced them: rows only {sorted((rows - ran).elements())}, "
            f"invocations only {sorted((ran - rows).elements())}"
        )

    # 1. The single-build sweep: argv, balance, artifact.
    writers = [1, 2, 4, 8]
    sweep = drive_main(["--arm", "map", "--writers", "1,2,4,8", "--rounds", "4"])
    assert_one_w_per_process(sweep["timed"], "throughput pass")
    if sweep["err"] is not None:
        raise sweep["err"]
    assert_balance(sweep["timed"], (DEFAULT_BUILD,), writers, 4, "throughput pass")
    assert_artifact_matches(sweep, "throughput pass")
    # The counters pass times nothing and may run every cell in one process.
    assert sweep["counters"] and all(flag(c, "--role") == "counters" for c in sweep["counters"]), sweep["counters"]

    # 2. The comparison: every (build, W) cell of a round in its own process,
    #    Williams-balanced over the 2 x len(W) cells.
    comp = drive_main(["--compare", variant, "--arm", "map", "--writers", "1,2,4,8", "--rounds", "8"],
                      clock=SteppingClock())
    assert_one_w_per_process(comp["timed"], "comparison pass")
    if comp["err"] is not None:
        raise comp["err"]
    assert_balance(comp["timed"], (DEFAULT_BUILD, variant), writers, 8, "comparison pass")
    assert_artifact_matches(comp, "comparison pass")
    assert comp["artifact"]["comparison"][0]["per_writer"]["8"]["paired_ratios_raw"], comp["artifact"]["comparison"]

    # 2b. The comparison's load window, read off the artifact main() wrote.
    #     The builds alternate cell by cell, so one phase window per arm covers
    #     both, and a second snapshot taken back to back with the first opens
    #     a window too short for any busy-CPU figure to mean anything.
    loads = comp["artifact"]["provenance"]["loads"]
    phase = [s["label"] for s in loads if s["label"].startswith("arm:map:")]
    assert phase == ["arm:map:comparison"], (
        f"comparison pass: provenance.loads carries phase snapshots {phase} for the map arm, expected "
        f"exactly ['arm:map:comparison']: two phase snapshots for one arm taken back to back leave the "
        f"second a window too short to measure"
    )
    for key in ("throughput", "throughput_variant"):
        for cell in comp["artifact"][key]:
            assert cell["load"]["since"] == "arm:map:comparison", (
                f"comparison pass: a `{key}` cell's load window opens at {cell['load']['since']!r}, not "
                f"at the arm's one comparison snapshot"
            )
    # Every snapshot's window is either long enough to measure or carries no
    # figure. `monotonic_s` is stored to 1 ms, so a window is only judged
    # sub-minimum when it is short by more than that rounding.
    short = 0
    for prev, snap in zip(loads, loads[1:]):
        window = snap["monotonic_s"] - prev["monotonic_s"]
        if window >= MIN_WINDOW_S - 0.001:
            continue
        short += 1
        for k in ("busy_cpus_since_prev", "own_busy_cpus_since_prev", "foreign_busy_cpus_since_prev"):
            assert snap[k] is None, (
                f"comparison pass: snapshot {snap['label']!r} carries {k} = {snap[k]} over a "
                f"{window * 1000:.1f} ms window, below the {MIN_WINDOW_S:.3f} s minimum the jiffy "
                f"accounting can resolve (bench_provenance.MIN_WINDOW_S): it must carry no number"
            )
    assert short, (
        "comparison pass: no snapshot window fell below the minimum, so the assertion that a "
        "sub-minimum window carries no number was never exercised"
    )

    # 3. A row whose position disagrees with the invocation that printed it is
    #    refused, and nothing is written.
    def shift_position(row: dict[str, Any]) -> None:
        if row["round"] == 1 and row["position"] == 2:
            row["position"] = 3

    bad_row = drive_main(["--arm", "map", "--writers", "1,2,4,8", "--rounds", "4"], tamper=shift_position)
    assert bad_row["err"] is not None and "position" in str(bad_row["err"]), (
        f"a row whose position disagrees with its invocation was accepted: {bad_row['err']!r}"
    )
    assert bad_row["artifact"] is None, "an artifact was written from a row that disagrees with its schedule"

    # 4. main() itself refuses an artifact whose summarised rows disagree with
    #    the schedule: the check is pinned at main's call site, not only here.
    corrupted = {"done": False}

    def corrupting_summarize(*args: Any, **kwargs: Any) -> Any:
        cells = real_summarize(*args, **kwargs)
        if not corrupted["done"]:
            cells[-1]["rounds_raw"][0]["position"] += 1
            corrupted["done"] = True
        return cells

    bad_art = drive_main(["--arm", "map", "--writers", "1,2,4,8", "--rounds", "4"], summarize=corrupting_summarize)
    assert corrupted["done"], "summarize_arm was never called"
    assert bad_art["err"] is not None and "per-cell schedule" in str(bad_art["err"]), (
        f"main() wrote an artifact whose rows disagree with the per-cell schedule: {bad_art['err']!r}"
    )
    assert bad_art["artifact"] is None, "main() wrote an artifact whose rows disagree with the per-cell schedule"

    # ... and an artifact that does not say its cells ran one process each.
    good = sweep["artifact"]
    expected: dict[tuple[str, str, int], list[tuple[int, int]]] = {}
    for b, r, pos, w in invoked_cells(sweep["timed"]):
        expected.setdefault(("map", b, w), []).append((r, pos))
    expected = {k: sorted(v) for k, v in expected.items()}
    check_artifact_against_schedule(good, expected)
    no_isolation = json.loads(json.dumps(good))
    del no_isolation["provenance"]["cell_isolation"]
    _expect_value_error(lambda: check_artifact_against_schedule(no_isolation, expected), "cell_isolation")
    relabelled = json.loads(json.dumps(good))
    relabelled["throughput"][0]["rounds_raw"][0]["build"] = variant
    _expect_value_error(lambda: check_artifact_against_schedule(relabelled, expected), "labelled build")

    # 5. The droop pass: one W per `perf stat` process, same balance.
    pmu_timed: list[tuple[str, list[str]]] = []

    def fake_perf(cmd: Any, *args: Any, **kwargs: Any) -> Any:
        argv = [str(c) for c in cmd]
        if argv[:2] == ["perf", "list"]:
            return subprocess.CompletedProcess(argv, 0, stdout="cycles\nref-cycles\n", stderr="")
        if argv[:2] == ["perf", "stat"]:
            harness = argv[argv.index("--") + 1:]
            build = fake_builds[harness[0]]
            pmu_timed.append((build, harness))
            return subprocess.CompletedProcess(
                argv, 0, stdout=harness_stdout(harness, build, lambda row: None),
                stderr="990,,cycles,1,100.00,,\n1000,,ref-cycles,1,100.00,,\n")
        raise AssertionError(f"unexpected command in the PMU self-test: {argv}")

    with contextlib.ExitStack() as stack:
        stack.enter_context(mock.patch.object(platform, "system", return_value="Linux"))
        stack.enter_context(mock.patch.object(shutil, "which", return_value="/usr/bin/perf"))
        stack.enter_context(mock.patch.object(subprocess, "run", side_effect=fake_perf))
        stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
        pmu = run_pmu_pass(fake_default, arm="map", writers=writers, rounds=4, quick=True)
    assert_one_w_per_process(pmu_timed, "PMU pass")
    assert_balance(pmu_timed, (DEFAULT_BUILD,), writers, 4, "PMU pass")
    assert pmu["cell_isolation"] == CELL_ISOLATION, pmu
    assert pmu["frequency_droop"]["by_writers"]["8"]["n_measured"] == 4, pmu["frequency_droop"]

    # 6. The real harness honours --position and refuses it where it cannot hold.
    real_cell = {"round": 1, "position": 3, "writers": 2, "build": DEFAULT_BUILD}
    row = run_writer_cell(throughput_bin, "map", real_cell, quick=True)
    assert (row["round"], row["position"], row["writers"]) == (1, 3, 2), row
    for extra in (["--writers", "1,2", "--round", "0", "--position", "1"],
                  ["--writers", "2", "--position", "1"]):
        proc = subprocess.run([str(throughput_bin), "--role", "throughput", "--arm", "map", *extra, "--quick"],
                              capture_output=True, text=True, check=False)
        assert proc.returncode != 0 and "--position names one cell" in proc.stderr, (extra, proc.stderr)

    eprintln("One process per timed writer cell PASSED\n")


def self_test() -> int:
    eprintln = sys.stderr.write
    eprintln("Running writer_scaling.py self-test...\n")

    # 0. The c2c pass records the measured window only. Runs first: it needs no
    #    build, so a regression here fails before minutes of cargo.
    _self_test_c2c_window()

    # Build both binaries up front
    throughput_bin, counters_bin = build_binaries(verbose=True)

    # 1. Negative control tests: assert error on build/role mismatch (AGENTS.md §2.3)
    eprintln("Testing negative controls (AGENTS.md §2.3 / build/role mismatch)...")
    p_mismatch1 = subprocess.run(
        [str(throughput_bin), "--role", "counters", "--self-test"],
        capture_output=True,
        text=True,
    )
    assert p_mismatch1.returncode != 0, "Expected throughput_bin with --role counters to fail"
    assert "build/role mismatch" in p_mismatch1.stderr or "build/role mismatch" in p_mismatch1.stdout, (
        f"Expected build/role mismatch error message, got: {p_mismatch1.stderr}"
    )

    p_mismatch2 = subprocess.run(
        [str(counters_bin), "--role", "throughput", "--self-test"],
        capture_output=True,
        text=True,
    )
    assert p_mismatch2.returncode != 0, "Expected counters_bin with --role throughput to fail"
    assert "build/role mismatch" in p_mismatch2.stderr or "build/role mismatch" in p_mismatch2.stdout, (
        f"Expected build/role mismatch error message, got: {p_mismatch2.stderr}"
    )
    eprintln("Negative controls PASSED\n")

    # 2. Binary self-tests
    eprintln("Running binary self-tests...")
    p_st_tp = subprocess.run(
        [str(throughput_bin), "--role", "throughput", "--self-test"],
        capture_output=True,
        text=True,
    )
    assert p_st_tp.returncode == 0, f"throughput self-test failed: {p_st_tp.stderr}"

    p_st_cnt = subprocess.run(
        [str(counters_bin), "--role", "counters", "--self-test"],
        capture_output=True,
        text=True,
    )
    assert p_st_cnt.returncode == 0, f"counters self-test failed: {p_st_cnt.stderr}"
    eprintln("Binary self-tests PASSED\n")

    # 2b. Every timed writer cell in a harness process of its own (§15). Early,
    #     because it needs no measurement and says the most when it fails.
    _self_test_per_cell_isolation(throughput_bin, counters_bin)

    # 3. Apply bench_pin to satisfy gate
    pin = bench_pin.apply("writer_scaling.py")
    eprintln(f"Applied core pin: {pin}\n")

    # 4. Quick smoke test across map, set, and str
    eprintln("Testing quick runs across map, set, str...")
    prov = new_provenance(
        suite="concurrency",
        issue=568,
        ratio="Expanse throughput over single-writer baseline C(N) = Mops(W) / Mops(1)",
        repo_root=REPO_ROOT,
    )

    # Pass 1: throughput (load snapshot covers Pass 1 only)
    start_snap = begin_cell(prov, "cell:map:W1:R0")
    t_rows_map, _ = run_throughput_pass(throughput_bin, "map", [1, 2], 3, quick=True)
    load = end_cell(start_snap)
    assert all(r["build"] == DEFAULT_BUILD for r in t_rows_map), t_rows_map

    assert len(t_rows_map) == 6, f"Expected 6 rows (2 writers * 3 rounds), got {len(t_rows_map)}"
    assert all(r["role"] == "throughput" for r in t_rows_map)
    assert all("writer_mops" in r and float(r["writer_mops"]) > 0 for r in t_rows_map)
    assert all("writer_elapsed_s" in r and float(r["writer_elapsed_s"]) > 0 for r in t_rows_map)
    assert all("lock_fallbacks" not in r for r in t_rows_map)

    # Pass 2: counters across all 3 rounds (verifies Requirement 10 and B2 non-zero fallbacks)
    c_rows_map = run_pass(counters_bin, "counters", "map", [1, 2], 3, quick=True)
    assert len(c_rows_map) == 6, f"Expected 6 counter rows, got {len(c_rows_map)}"
    assert all(r["role"] == "counters" for r in c_rows_map)
    w2_map_fbs = [r["lock_fallbacks"] for r in c_rows_map if r["writers"] == 2]
    assert len(w2_map_fbs) == 3
    _assert_fallbacks_counted("map", w2_map_fbs)

    c_rows_set = run_pass(counters_bin, "counters", "set", [1, 2], 3, quick=True)
    assert len(c_rows_set) == 6
    w2_set_fbs = [r["lock_fallbacks"] for r in c_rows_set if r["writers"] == 2]
    assert len(w2_set_fbs) == 3
    _assert_fallbacks_counted("set", w2_set_fbs)

    # str arm is the alpha=1 coarse-mutex reference curve: 0 lock fallbacks by construction
    c_rows_str = run_pass(counters_bin, "counters", "str", [1, 2], 3, quick=True)
    assert len(c_rows_str) == 6
    w2_str_fbs = [r["lock_fallbacks"] for r in c_rows_str if r["writers"] == 2]
    assert len(w2_str_fbs) == 3
    assert all(fb == 0 for fb in w2_str_fbs), f"Expected str W=2 lock_fallbacks == 0, got {w2_str_fbs}"

    cells_str = summarize_arm("str", [1, 2], 3, t_rows_map, c_rows_str, load)
    assert cells_str[0]["total_allocs"] is None
    assert cells_str[0]["total_allocs_per_insert"] is None
    assert cells_str[1]["total_allocs"] is None
    assert cells_str[1]["total_allocs_per_insert"] is None

    # 5. Reduction test
    cells = summarize_arm("map", [1, 2], 3, t_rows_map, c_rows_map, load)
    assert len(cells) == 2
    cell_w1 = cells[0]
    cell_w2 = cells[1]
    assert cell_w1["writers"] == 1
    assert cell_w1["scaling_factor_c_n"] == 1.0
    assert cell_w1["writer_ci_lower"] <= cell_w1["expanse_writer_mops_mean"] <= cell_w1["writer_ci_upper"]
    # Every published interval names the construction that produced it (#880).
    # At W=1 the throughput interval is resampled and so carries a label, while
    # C(1) is 1.0 by definition and carries `None` — a label there would claim
    # an estimator that never ran.
    assert cell_w1["writer_ci_method"] in CI_METHODS, cell_w1["writer_ci_method"]
    assert cell_w1["scaling_factor_c_n_ci_method"] is None, cell_w1["scaling_factor_c_n_ci_method"]
    assert len(cell_w1["counters_raw"]) == 3, f"Expected 3 counters_raw entries, got {len(cell_w1['counters_raw'])}"
    assert "position" in cell_w1["rounds_raw"][0]
    assert "position" in cell_w1["counters_raw"][0]

    assert cell_w2["writers"] == 2
    assert cell_w2["scaling_factor_c_n_ci_lower"] <= cell_w2["scaling_factor_c_n"] <= cell_w2["scaling_factor_c_n_ci_upper"]
    # At W>1 the C(W) interval IS resampled, so it must name its construction.
    assert cell_w2["scaling_factor_c_n_ci_method"] in CI_METHODS, cell_w2["scaling_factor_c_n_ci_method"]
    assert cell_w2["writer_ci_method"] in CI_METHODS, cell_w2["writer_ci_method"]
    # Same reasoning as _assert_fallbacks_counted above, at the aggregated cell:
    # whether two writers collided in a quick run is timing, not an invariant.
    # What is deterministic is that the aggregated cell carries the counter as a
    # non-negative integer, so a cell built from the wrong build or a mis-parsed
    # row still fails closed.
    assert isinstance(cell_w2["lock_fallbacks"], int) and not isinstance(cell_w2["lock_fallbacks"], bool), \
        f"aggregated W=2 lock_fallbacks must be an int, got {cell_w2['lock_fallbacks']!r}"
    assert cell_w2["lock_fallbacks"] >= 0, \
        f"aggregated W=2 lock_fallbacks must be >= 0, got {cell_w2['lock_fallbacks']}"
    if cell_w2["lock_fallbacks"] == 0:
        print("    note: aggregated W=2 cell observed no lock fallbacks -- the writers did "
              "not collide on this host; the counter is present and aggregated, which is "
              "what this assertion verifies")
    assert len(cell_w2["counters_raw"]) == 3, f"Expected 3 counters_raw entries, got {len(cell_w2['counters_raw'])}"
    assert "position" in cell_w2["rounds_raw"][0]
    assert "position" in cell_w2["counters_raw"][0]
    assert "tsc_hz" in cell_w1["rounds_raw"][0]
    assert cell_w1["tsc_hz"] > 0
    assert "retired" in cell_w2["counters_raw"][0]
    assert "total_allocs" in cell_w2["counters_raw"][0]
    assert cell_w2["total_allocs"] > 0
    assert cell_w2["total_allocs_per_insert"] > 0
    for c in cells:
        for r in c["counters_raw"]:
            assert set(r["fallback_causes"]) == set(CAUSE_NAMES), r
            assert sum(r["fallback_causes"].values()) == r["lock_fallbacks"], r
            assert r["inserts"] == r["write_ops"], r
            assert r["quiesce_calls"] == r["lock_fallbacks"], r
            assert (
                r["contention_gate_closed"] + r["contention_retry_exhausted"]
                == r["fallback_causes"]["contention"]
            ), r
            assert (
                r["branch_split_subarray"]
                + r["branch_split_linear"]
                + r["branch_split_prefix"]
                + r["branch_split_remove"]
                + r.get("branch_split_upgrade", 0)
                == r["fallback_causes"]["branch_split"]
            ), r
            assert (
                r.get("cap_expansion_class", 0)
                + r.get("cap_expansion_leaf_full", 0)
                + r.get("cap_expansion_bitmap_near_full", 0)
                + r.get("cap_expansion_map_bitmap_sub", 0)
                + r.get("cap_expansion_remove", 0)
                == r["fallback_causes"]["cap_expansion"]
            ), r
    # The cause shares partition the fallbacks, so they sum to 1 -- but only when
    # there are fallbacks to partition. With none, every share is correctly 0.0
    # and summing to 1.0 would be the wrong assertion, not a passing one. Both
    # branches are checked, so a degenerate cell still fails closed if its shares
    # are anything other than exactly zero (AGENTS.md 8.9 principle 4: a sum
    # identity proves accounting completeness, which is what is being tested
    # here, not that contention occurred).
    # `lock_fallbacks` is the MEDIAN across rounds; the share denominator is the
    # SUM (`total_fallbacks`). Those disagree exactly when fallbacks are rare:
    # seven rounds at 0 and one at 1 gives a median of 0 and a sum of 1, so the
    # shares legitimately read `contention: 1.0` beside `lock_fallbacks: 0`.
    # Phase 4E made that the common case rather than an impossible one, and the
    # old branch -- keyed on the median -- then failed on a correct cell. Key
    # the branch on the sum the shares were actually derived from.
    _shares = cell_w2["fallback_cause_share"]
    _share_total = sum(cell_w2["fallback_causes_total"].values())
    if _share_total > 0:
        assert abs(sum(_shares.values()) - 1.0) < 1e-3, (_shares, _share_total)
    else:
        assert all(v == 0.0 for v in _shares.values()), \
            f"no fallbacks in any round, so every cause share must be exactly 0.0, got {_shares}"

    # 6. Test comparison runner with self-comparison on quick scale
    eprintln("Testing run_comparison (interleaved build x W execution)...")
    cells_def, cells_var, comp_stats, comp_schedule = run_comparison(
        throughput_bin,
        throughput_bin,
        "self_test",
        counters_bin,
        counters_bin,
        "set",
        [1, 2],
        3,
        prov,
        quick=True,
    )
    assert len(cells_def) == 2
    assert len(cells_var) == 2
    assert len(comp_schedule) == 3 * 2 * 2, comp_schedule
    assert {r["build"] for c in cells_var for r in c["rounds_raw"]} == {"self_test"}, cells_var
    assert len(cells_def[0]["counters_raw"]) == 3
    assert len(cells_var[0]["counters_raw"]) == 3
    assert cells_var[0]["variant"] == "self_test"
    assert "per_writer" in comp_stats
    assert "2" in comp_stats["per_writer"]
    w2_comp = comp_stats["per_writer"]["2"]
    assert w2_comp["verdict"] in ("SINGLE_RUN_PASS", "REJECTED", "INCONCLUSIVE")
    assert len(w2_comp["paired_ratios_raw"]) == 3
    # The ratio is a wall-clock point estimate, so its magnitude is not asserted
    # (AGENTS.md §8.4): a self-comparison on a contended host can land anywhere.
    # What is asserted is the arithmetic and the decision rule over whatever
    # was measured.
    raw = w2_comp["paired_ratios_raw"]
    assert all(0 < x < float("inf") for x in raw), raw
    assert abs(w2_comp["ratio_c_variant_over_c_default_mean"] - sum(raw) / len(raw)) < 1e-3, w2_comp
    if w2_comp["ratio_ci_lower"] > 1.0:
        expected_verdict = "SINGLE_RUN_PASS"
    elif w2_comp["ratio_ci_upper"] < 1.0:
        expected_verdict = "REJECTED"
    else:
        expected_verdict = "INCONCLUSIVE"
    assert w2_comp["verdict"] == expected_verdict, w2_comp

    # Also test with counters_bin_variant=None (empty variant counters)
    cells_def_none, cells_var_none, _, _ = run_comparison(
        throughput_bin,
        throughput_bin,
        "self_test_none",
        counters_bin,
        None,
        "set",
        [1, 2],
        3,
        prov,
        quick=True,
    )
    assert len(cells_def_none[0]["counters_raw"]) == 3
    assert len(cells_var_none[0]["counters_raw"]) == 0
    assert cells_var_none[0]["variant"] == "self_test_none"

    # 6b. Directional invariant self-test for inverse comparison (§8.20.7):
    # Under an inverse comparison where default is strictly faster than variant,
    # C_default(W) / C_variant(W) > 1.0 yields SINGLE_RUN_PASS.
    eprintln("Testing directional invariant self-test for inverse comparison (§8.20.7)...")
    rounds_syn = 8
    syn_def = [
        {"round": r, "writers": 1, "writer_mops": 1.0} for r in range(rounds_syn)
    ] + [
        {"round": r, "writers": 2, "writer_mops": 2.0} for r in range(rounds_syn)
    ]
    syn_var = [
        {"round": r, "writers": 1, "writer_mops": 1.0} for r in range(rounds_syn)
    ] + [
        {"round": r, "writers": 2, "writer_mops": 1.5} for r in range(rounds_syn)
    ]
    comp_std = compute_paired_scaling_ratios("set", "test_std", [1, 2], rounds_syn, syn_def, syn_var, inverse=False)
    assert comp_std["per_writer"]["2"]["verdict"] == "REJECTED", comp_std["per_writer"]["2"]
    assert comp_std["per_writer"]["2"]["ratio_c_variant_over_c_default_mean"] == 0.75, comp_std["per_writer"]["2"]

    comp_inv = compute_paired_scaling_ratios("set", "test_inv", [1, 2], rounds_syn, syn_def, syn_var, inverse=True)
    assert comp_inv["per_writer"]["2"]["verdict"] == "SINGLE_RUN_PASS", comp_inv["per_writer"]["2"]
    assert abs(comp_inv["per_writer"]["2"]["ratio_c_variant_over_c_default_mean"] - 1.3333) < 1e-3, comp_inv["per_writer"]["2"]

    comp_inv_feature = compute_paired_scaling_ratios("set", "ablation-unstriped-freelist", [1, 2], rounds_syn, syn_def, syn_var)
    assert comp_inv_feature["per_writer"]["2"]["verdict"] == "SINGLE_RUN_PASS", comp_inv_feature["per_writer"]["2"]
    assert abs(comp_inv_feature["per_writer"]["2"]["ratio_c_variant_over_c_default_mean"] - 1.3333) < 1e-3, comp_inv_feature["per_writer"]["2"]
    # The artifact must say which ratio it holds: the stored field's name reads
    # variant over default in both directions.
    assert comp_std["ratio_direction"] == "c_variant_over_c_default", comp_std
    assert comp_inv["ratio_direction"] == "c_default_over_c_variant", comp_inv
    assert comp_inv_feature["ratio_direction"] == "c_default_over_c_variant", comp_inv_feature
    desc_inv = ratio_description(["ablation-unstriped-freelist"])
    assert "C_default(W) / C_variant(W)" in desc_inv and "C_variant(W) / C_default(W)" not in desc_inv, desc_inv
    desc_fwd = ratio_description(["ablation-striped-epoch"])
    assert "C_variant(W) / C_default(W)" in desc_fwd and "C_default(W) / C_variant(W)" not in desc_fwd, desc_fwd

    # 6c. The frequency-droop rule, on synthetic counts. This runs everywhere:
    # the pass itself needs `perf`, so on a runner without it the only PMU
    # coverage below is that the pass refuses to run — which says nothing about
    # whether its arithmetic or its verdicts are right.
    eprintln("Testing frequency droop decision rule (synthetic counts)...")
    CYC, REF = "cycles", "ref-cycles"

    def counts(freq_ratio: float) -> dict[str, float]:
        # ref-cycles fixed, cycles scaled: f = cycles / ref-cycles.
        return {CYC: 1000.0 * freq_ratio, REF: 1000.0}

    # A real droop at W=4 and none at W=2, in one pass: the per-W split is the
    # point, and a single W=2-vs-W=1 number could not express this.
    rd = {
        r: {
            1: counts(1.00),
            2: counts(0.995 + 0.002 * r),
            4: counts(0.900 + 0.002 * r),
        }
        for r in range(8)
    }
    d = frequency_droop_by_writers(rd, CYC, REF, [1, 2, 4], 8)
    assert d["baseline_writers"] == 1, d
    assert set(d["by_writers"]) == {"2", "4"}, d
    assert d["by_writers"]["4"]["verdict"] == "SINGLE_RUN_PASS", d["by_writers"]["4"]
    assert d["by_writers"]["4"]["droop_ci_lower"] > 0.05, d["by_writers"]["4"]
    assert d["by_writers"]["2"]["verdict"] == "REJECTED", d["by_writers"]["2"]
    assert d["by_writers"]["2"]["droop_ci_upper"] < 0.05, d["by_writers"]["2"]

    # Counters absent (a PMU that returned no ref-cycles) is reported as
    # unmeasured, never as a droop of zero (AGENTS.md §8.1).
    rd_missing = {r: {1: counts(1.0), 4: {CYC: 900.0, REF: 0.0}} for r in range(8)}
    d_missing = frequency_droop_by_writers(rd_missing, CYC, REF, [1, 4], 8)
    assert d_missing["by_writers"]["4"]["n_measured"] == 0, d_missing
    assert "verdict" not in d_missing["by_writers"]["4"], d_missing
    assert "droop_mean" not in d_missing["by_writers"]["4"], d_missing

    # Fewer than three paired rounds cannot carry a BCa interval, so no verdict.
    rd_short = {r: {1: counts(1.0), 4: counts(0.9)} for r in range(2)}
    d_short = frequency_droop_by_writers(rd_short, CYC, REF, [1, 4], 2)
    assert d_short["by_writers"]["4"]["n_measured"] == 2, d_short
    assert "verdict" not in d_short["by_writers"]["4"], d_short

    # c2c hot-line extraction: the trace-event totals say how much HITM traffic
    # there was; only the shared-cache-line table says WHICH line carried it.
    # Run 34722607239 recorded 7.54 snoop-forwards per insert at W=8 and could
    # not name a single line, because the summary was the report's first 20
    # lines -- the header block -- and the report itself stays on the runner.
    _c2c_sample = "\n".join([
        "=================================================",
        "            Trace Event Information              ",
        "=================================================",
        "  Total records                     :      54749",
        "  Load Local HITM                   :       3006",
        "",
        "=================================================",
        "           Shared Data Cache Line Table          ",
        "=================================================",
        "#        Total      Tot  ----- LLC Load Hitm -----",
        "# Index  Records     Hitm    Total  LclHitm  RmtHitm",
        "      0     1234   41.2%     1238     1238        0",
        "      1      567   19.0%      571      571        0",
    ])
    _hot = c2c_hot_cache_lines(_c2c_sample)
    assert _hot, "hot-line table must be extracted when the report has one"
    assert any("Shared Data Cache Line Table" in ln for ln in _hot), _hot
    assert any("41.2%" in ln for ln in _hot), _hot
    # the older perf heading is recognised too
    _pareto = _c2c_sample.replace("Shared Data Cache Line Table",
                                  "Shared Cache Line Distribution Pareto")
    assert c2c_hot_cache_lines(_pareto), "the Pareto heading must also match"
    # a report with no contended lines yields an empty list, not a crash
    assert c2c_hot_cache_lines("Trace Event Information\n  Total records : 0") == []
    # and the cap is honoured
    _long = "Shared Data Cache Line Table\n" + "\n".join(f"row {i}" for i in range(100))
    assert len(c2c_hot_cache_lines(_long, max_rows=5)) == 5

    # No event keys at all: the summary is empty rather than fabricated.
    d_none = frequency_droop_by_writers(rd, None, None, [1, 2, 4], 8)
    assert d_none["by_writers"] == {}, d_none

    # THE DEFECT THIS PINS (AGENTS.md §8.12.3): on a hybrid host `perf` accepts
    # a bare `cycles` and reports it back PMU-qualified as `cpu_core/cycles/`,
    # so resolving the key against the REQUESTED event list finds nothing and
    # every W lands as `n_measured: 0` while complete raw counts sit beside it.
    # Observed on run 34722607239. Resolution must come from the reported keys.
    hybrid_requested = ["cycles", "cpu_core/ref-cycles/", "mem_load_l3_hit_retired.xsnp_fwd"]
    hybrid_observed = {
        "cpu_core/cycles/",
        "cpu_core/ref-cycles/",
        "cpu_core/mem_load_l3_hit_retired.xsnp_fwd/",
    }
    h_cyc = _resolve_event_key(hybrid_observed, hybrid_requested, want_ref=False)
    h_ref = _resolve_event_key(hybrid_observed, hybrid_requested, want_ref=True)
    assert h_cyc == "cpu_core/cycles/", h_cyc
    assert h_ref == "cpu_core/ref-cycles/", h_ref
    # and the resolved keys actually yield samples off hybrid-shaped counts
    rd_hybrid = {
        r: {
            1: {"cpu_core/cycles/": 2000 + r, "cpu_core/ref-cycles/": 1000},
            4: {"cpu_core/cycles/": 1900 + r, "cpu_core/ref-cycles/": 1000},
        }
        for r in range(8)
    }
    d_hybrid = frequency_droop_by_writers(rd_hybrid, h_cyc, h_ref, [1, 4], 8)
    assert d_hybrid["by_writers"]["4"]["n_measured"] == 8, d_hybrid
    # the unqualified request alone must NOT resolve against hybrid counts
    assert _resolve_event_key(set(), hybrid_requested, want_ref=False) == "cycles"

    # 7. Test PMU pass and c2c pass fail-loud on non-Linux / missing perf (AGENTS.md §8.1)
    eprintln("Testing PMU and c2c passes (fail-loud validation)...")
    if platform.system() != "Linux" or shutil.which("perf") is None:
        try:
            run_pmu_pass(throughput_bin, arm="set", writers=[1, 2], rounds=3, quick=True)
            assert False, "Expected run_pmu_pass to raise RuntimeError on non-Linux/missing perf"
        except RuntimeError as exc:
            assert "AGENTS.md §8.1" in str(exc)

        try:
            run_c2c_pass(throughput_bin, arm="set", writers=2, quick=True)
            assert False, "Expected run_c2c_pass to raise RuntimeError on non-Linux/missing perf"
        except RuntimeError as exc:
            assert "AGENTS.md §8.1" in str(exc)
    else:
        # `perf` on PATH does not mean this host can measure: a virtualized
        # runner exposes no hardware events, and `perf list` comes back without
        # the cycles/ref-cycles pair the droop needs. Both outcomes are correct
        # there — a summary, or a refusal that names its reason — and what the
        # self-test owes is that a refusal is loud rather than a zero
        # (AGENTS.md §8.1). Asserting success instead assumed the bench host's
        # capabilities and failed on a GitHub runner, where `perf` is installed
        # and the PMU is not exposed.
        try:
            pmu_res = run_pmu_pass(throughput_bin, arm="set", writers=[1, 2], rounds=3, quick=True)
            assert isinstance(pmu_res, dict)
        except RuntimeError as exc:
            assert "AGENTS.md §8.1" in str(exc), exc

        try:
            c2c_res = run_c2c_pass(throughput_bin, arm="set", writers=2, quick=True)
            assert isinstance(c2c_res, dict)
        except RuntimeError as exc:
            assert "AGENTS.md §8.1" in str(exc), exc

    # A row whose causes do not sum to its fallbacks is refused, not averaged.
    broken = [dict(r) for r in c_rows_map]
    broken[0]["lock_fallbacks"] = int(broken[0]["lock_fallbacks"]) + 1
    try:
        summarize_arm("map", [1, 2], 3, t_rows_map, broken, load)
    except ValueError as exc:
        assert "causes sum to" in str(exc), exc
    else:
        raise AssertionError("summarize_arm accepted a row whose causes do not sum to lock_fallbacks")

    # Negative control: quiesce_calls mismatch
    broken_q = [dict(r) for r in c_rows_map]
    broken_q[0]["quiesce_calls"] = int(broken_q[0]["quiesce_calls"]) + 1
    try:
        summarize_arm("map", [1, 2], 3, t_rows_map, broken_q, load)
    except ValueError as exc:
        assert "quiesce_calls" in str(exc), exc
    else:
        raise AssertionError("summarize_arm accepted a row with quiesce_calls mismatch")

    # Negative control: contention partition mismatch
    broken_c = [dict(r) for r in c_rows_map]
    broken_c[0]["contention_gate_closed"] = int(broken_c[0]["contention_gate_closed"]) + 1
    try:
        summarize_arm("map", [1, 2], 3, t_rows_map, broken_c, load)
    except ValueError as exc:
        assert "contention_gate_closed" in str(exc), exc
    else:
        raise AssertionError("summarize_arm accepted broken contention partition")

    # Negative control: branch_split partition mismatch
    broken_bs = [dict(r) for r in c_rows_map]
    broken_bs[0]["branch_split_subarray"] = int(broken_bs[0]["branch_split_subarray"]) + 1
    try:
        summarize_arm("map", [1, 2], 3, t_rows_map, broken_bs, load)
    except ValueError as exc:
        assert "branch_split partition" in str(exc), exc
    else:
        raise AssertionError("summarize_arm accepted broken branch_split partition")

    # 8. Privacy check: ensure no absolute repo root or home paths leaked into cells (AGENTS.md §7)
    repo_root_str = str(REPO_ROOT)
    for c in cells:
        c_json = json.dumps(c)
        assert (
            repo_root_str not in c_json
        ), f"Absolute repo root path leaked into cell JSON (AGENTS.md §7): {c_json}"
        assert (
            "/home/" not in c_json
        ), f"Home directory path leaked into cell JSON (AGENTS.md §7): {c_json}"

    # 9. Williams square balance property check (count positions and pairs over 1 full cycle)
    t_rows_bal, _ = run_throughput_pass(throughput_bin, "map", [1, 2, 4, 8], 4, quick=True)
    writers_bal = [1, 2, 4, 8]
    n_w = len(writers_bal)
    pos_counts: dict[int, list[int]] = {w: [0] * n_w for w in writers_bal}
    pair_counts: dict[int, dict[int, int]] = {
        w1: {w2: 0 for w2 in writers_bal if w2 != w1} for w1 in writers_bal
    }

    by_round: dict[int, list[dict[str, Any]]] = {}
    for r in t_rows_bal:
        by_round.setdefault(r["round"], []).append(r)

    for round_idx in range(4):
        round_rows = sorted(by_round[round_idx], key=lambda x: x["position"])
        assert len(round_rows) == n_w
        for pos, r in enumerate(round_rows):
            assert r["position"] == pos
            w = r["writers"]
            pos_counts[w][pos] += 1
            if pos > 0:
                prev_w = round_rows[pos - 1]["writers"]
                pair_counts[prev_w][w] += 1

    # Over 1 full cycle of 4 rounds:
    # 1. Every treatment appears in every position exactly once:
    for w in writers_bal:
        for pos in range(n_w):
            assert pos_counts[w][pos] == 1, (
                f"Williams balance error: W={w} appeared in position {pos} "
                f"{pos_counts[w][pos]} times (expected 1)"
            )
    # 2. Every ordered pair of distinct treatments appears as an immediate sequence exactly once:
    for w1 in writers_bal:
        for w2 in writers_bal:
            if w1 != w2:
                assert pair_counts[w1][w2] == 1, (
                    f"Williams carryover balance error: pair ({w1}, {w2}) "
                    f"appeared {pair_counts[w1][w2]} times (expected 1)"
                )

    # 10. The ordered-reader instrument (#900).
    _self_test_ordered_readers(throughput_bin, counters_bin, pin)

    eprintln("writer_scaling.py self-test PASSED\n")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=str, help="Output path for JSON artifact")
    parser.add_argument(
        "--rounds", type=int, default=8, help="Rounds per cell (default: 8, minimum: 3)"
    )
    parser.add_argument(
        "--arm",
        type=str,
        default="all",
        choices=["map", "set", "str", "both", "all"],
        help="Arm to sweep (default: all)",
    )
    parser.add_argument(
        "--writers",
        type=str,
        default="1,2,4,8",
        help="Comma-separated list of writer counts (default: 1,2,4,8, must include 1)",
    )
    parser.add_argument(
        "--features",
        type=str,
        default=None,
        help="Cargo features for single-build throughput pass (e.g. lock-padded)",
    )
    # One comparison selector per run: combining two would otherwise keep
    # one and silently drop the rest.
    comparison = parser.add_mutually_exclusive_group()
    comparison.add_argument(
        "--compare",
        type=str,
        default=None,
        metavar="FEATURE",
        help="Compare default build vs feature build with interleaved (build × W) rounds",
    )
    comparison.add_argument(
        "--compare-padded",
        action="store_true",
        help="Shorthand for --compare lock-padded (Hypothesis B)",
    )
    comparison.add_argument(
        "--variants",
        metavar="VARIANTS",
        default=None,
        help="Comma-separated feature variants to compare against default (or via BENCH_VARIANTS env var)",
    )
    comparison.add_argument(
        "--compare-ablation-alloc",
        action="store_true",
        help="Shorthand for --compare ablation-sharded-alloc (Hypothesis D arm a)",
    )
    comparison.add_argument(
        "--compare-ablation-epoch",
        action="store_true",
        help="Shorthand for --compare ablation-striped-epoch (Hypothesis D arm b)",
    )
    comparison.add_argument(
        "--compare-ablation-unstriped-freelist",
        action="store_true",
        help="Shorthand for --compare ablation-unstriped-freelist (Hypothesis D arm c unstriped)",
    )
    comparison.add_argument(
        "--ordered-readers",
        action="store_true",
        help="Ordered readers on the map (#900, METHODOLOGY.md §12.4): probe x (W, R) x read_op cells "
             "under the pin 0,2,4,6,8,10,12,14, with the P12.4 and P12.5 verdicts "
             "(default output ordered_readers_writer_scaling.json)",
    )
    parser.add_argument(
        "--pmu",
        action="store_true",
        help="Run separate hardware PMU pass via perf stat on set W=1 vs W=2",
    )
    parser.add_argument(
        "--pmu-arm",
        default="map",
        choices=("map", "set", "str"),
        help="Arm the PMU and c2c passes measure (default: map, where the "
             "multi-writer sweep's between-run spread appears)",
    )
    parser.add_argument(
        "--c2c",
        action="store_true",
        help="Run separate perf c2c cacheline contention pass on set W=2",
    )
    parser.add_argument(
        "--diagnostic",
        action="store_true",
        help="Run diagnostic suite (enables --pmu and --c2c, default output to diagnostic_writer_scaling.json)",
    )
    parser.add_argument(
        "--quick",
        action="store_true",
        help="Quick mode with reduced population for fast smoke testing",
    )
    parser.add_argument(
        "--force-quick-out",
        action="store_true",
        help="Allow --quick to write to committed results path (normally forbidden)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run self-test and exit",
    )
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    if args.ordered_readers:
        # A sweep of its own: none of the writer sweep's selectors apply, and
        # accepting one would silently drop it.
        conflicts = [
            flag for flag, on in (
                ("--diagnostic", args.diagnostic), ("--pmu", args.pmu), ("--c2c", args.c2c),
                ("--features", args.features is not None), ("--arm", args.arm != "all"),
                ("--writers", args.writers != "1,2,4,8"),
            ) if on
        ]
        if conflicts:
            sys.stderr.write(f"error: --ordered-readers does not combine with {', '.join(conflicts)}\n")
            return 1
        if args.rounds < 3:
            sys.stderr.write("error: --rounds must be >= 3 for BCa bootstrap confidence intervals\n")
            return 1
        if not args.out:
            args.out = str(ORDERED_READERS_RESULTS_PATH)
        if (args.quick and Path(args.out).resolve() in committed_result_paths()
                and not args.force_quick_out):
            sys.stderr.write(
                "error: --quick output cannot overwrite committed results path "
                f"{Path(args.out).resolve()} without --force-quick-out\n"
            )
            return 1
        return run_ordered_readers(args)

    if args.diagnostic:
        args.pmu = True
        args.c2c = True
        if not args.out:
            args.out = str(DIAGNOSTIC_RESULTS_PATH)

    variant_list: list[str] = []
    if args.variants:
        variant_list.extend(v.strip() for v in args.variants.split(",") if v.strip())
    elif args.compare_padded:
        variant_list.append("lock-padded")
    elif args.compare:
        variant_list.append(args.compare)
    elif selected := [feature for flag, feature in ABLATION_ARMS.items() if getattr(args, flag)]:
        variant_list.extend(selected)
    elif os.environ.get("BENCH_VARIANTS"):
        variant_list.extend(v.strip() for v in os.environ["BENCH_VARIANTS"].split(",") if v.strip())

    if args.rounds < 3:
        sys.stderr.write("error: --rounds must be >= 3 for BCa bootstrap confidence intervals\n")
        return 1

    writers_list = [int(w.strip()) for w in args.writers.split(",") if w.strip()]
    if 1 not in writers_list:
        sys.stderr.write("error: --writers must include 1 to compute single-writer baseline C(N)\n")
        return 1

    n_w = len(writers_list)
    # A comparison's Williams design runs over the 2 x len(W) (build, W) cells.
    n_cells = 2 * n_w if variant_list else n_w
    if n_cells % 2 != 0 or args.rounds % n_cells != 0:
        sys.stderr.write(
            f"notice: Williams square balance requires an even number of cells per round and rounds a "
            f"multiple of it; got {n_cells} cells per round, rounds={args.rounds} — position/carryover "
            f"balance will be incomplete\n"
        )

    if args.quick and args.out:
        out_path = Path(args.out).resolve()
        if out_path in committed_result_paths() and not args.force_quick_out:
            sys.stderr.write(
                "error: --quick output cannot overwrite committed results path "
                f"{out_path} without --force-quick-out\n"
            )
            return 1

    # Apply core pin before any measurement
    core_pin = bench_pin.apply("writer_scaling.py")

    if args.arm in ("all", "both"):
        arms = ["map", "set", "str"] if args.arm == "all" else ["map", "set"]
    else:
        arms = [args.arm]

    throughput_cells: list[dict[str, Any]] = []
    variant_cells: list[dict[str, Any]] = []
    comparison_results: list[dict[str, Any]] = []
    # (arm, build, W) -> the (round, position) cells the timed passes ran; the
    # artifact is checked against it before it is written (§15).
    scheduled: dict[tuple[str, str, int], list[tuple[int, int]]] = {}

    if variant_list:
        bin_default, cnt_default = build_binaries(features=None, verbose=True)

        ratio_desc = ratio_description(variant_list)
        prov = new_provenance(
            suite="concurrency",
            issue=568,
            ratio=ratio_desc,
            repo_root=REPO_ROOT,
            core_pin=core_pin,
            cell_isolation=CELL_ISOLATION,
            cell_schedule=CELL_SCHEDULE_COMPARISON,
        )

        for var in variant_list:
            bin_variant, cnt_variant = build_binaries(features=var, verbose=True)
            for arm in arms:
                cells_d, cells_v, comp_stats, schedule = run_comparison(
                    bin_default,
                    bin_variant,
                    var,
                    cnt_default,
                    cnt_variant,
                    arm,
                    writers_list,
                    args.rounds,
                    prov,
                    quick=args.quick,
                )
                scheduled.update(expected_cells(arm, schedule))
                if not any(c.get("arm") == arm for c in throughput_cells):
                    throughput_cells.extend(cells_d)
                variant_cells.extend(cells_v)
                comparison_results.append(comp_stats)
    else:
        throughput_bin, counters_bin = build_binaries(features=args.features, verbose=True)

        prov = new_provenance(
            suite="concurrency",
            issue=568,
            ratio="Expanse throughput over single-writer baseline C(N) = Mops(W) / Mops(1)",
            repo_root=REPO_ROOT,
            core_pin=core_pin,
            cell_isolation=CELL_ISOLATION,
            cell_schedule=CELL_SCHEDULE_SWEEP,
        )

        feat_label = f" ({args.features})" if args.features else ""
        print("========================================================================")
        print(f" Expanse-Native Multi-Writer Scaling Sweep (Phase 1.5D, Refs #568){feat_label}")
        print(f" Arms: {', '.join(arms)} | Writers: {writers_list} | Rounds: {args.rounds}")
        print(f" Pin: {core_pin}")
        print("========================================================================")

        tp_target = get_throughput_target(args.features)

        for arm in arms:
            cell_label = f"arm:{arm}:writers"
            start_snap = begin_cell(prov, cell_label)

            # Pass 1: throughput (uninstrumented binary, interleaved across W,
            # one harness process per cell -- METHODOLOGY.md §15)
            print(f"\n  [Pass 1/2] Throughput — {arm} arm across W ∈ {writers_list}, one process per cell")
            t_rows, schedule = run_throughput_pass(
                throughput_bin, arm, writers_list, args.rounds, quick=args.quick
            )
            scheduled.update(expected_cells(arm, schedule))

            # End load snapshot immediately after timed Pass 1 so Pass 2 does not dilute load window
            load = end_cell(start_snap)

            # Pass 2: counters (diagnostic occ-stats binary, across all rounds)
            print(f"  [Pass 2/2] Diagnostic counters — {arm} arm across W ∈ {writers_list} (occ-stats build)")
            c_rows = run_pass(counters_bin, "counters", arm, writers_list, args.rounds, quick=args.quick)

            cells = summarize_arm(
                arm, writers_list, args.rounds, t_rows, c_rows, load, throughput_target=tp_target
            )
            for cell in cells:
                w = cell["writers"]
                throughput_cells.append(cell)
                foreign_str = (
                    f"{load['foreign_busy_cpus']:>5.2f}"
                    if load.get("foreign_busy_cpus") is not None
                    else "  n/a"
                )
                print(
                    f"[{arm:>3}] W={w:<2} | Mean: {cell['expanse_writer_mops_mean']:>6.2f} Mops/s "
                    f"[{cell['writer_ci_lower']:>6.2f}, {cell['writer_ci_upper']:>6.2f}] "
                    f"| Median: {cell['expanse_writer_mops_median']:>6.2f} "
                    f"| C(N) = {cell['scaling_factor_c_n']:>5.2f}x "
                    f"[{cell['scaling_factor_c_n_ci_lower']:>5.2f}, {cell['scaling_factor_c_n_ci_upper']:>5.2f}] "
                    f"| Fallbacks: {cell['lock_fallbacks']:>7} ({cell['fallback_rate']*100:>5.2f}%) "
                    f"| Foreign CPUs: {foreign_str}"
                )
                if cell["fallback_causes_total"] and sum(cell["fallback_causes_total"].values()) > 0:
                    shares = "  ".join(
                        f"{name} {cell['fallback_cause_share'][name] * 100:5.2f}%"
                        for name in CAUSE_NAMES
                    )
                    print(f"        causes (share of fallbacks): {shares}")
                    c_tot = cell["fallback_causes_total"]["contention"]
                    if c_tot > 0:
                        c_sh = cell["contention_subset_share"]
                        print(
                            f"        contention breakdown: gate_closed {c_sh['gate_closed']*100:5.2f}% "
                            f"| retry_exhausted {c_sh['retry_exhausted']*100:5.2f}%"
                        )
                    bs_tot = cell["fallback_causes_total"]["branch_split"]
                    if bs_tot > 0:
                        bs_sh = cell["branch_split_subset_share"]
                        print(
                            f"        branch_split breakdown: subarray {bs_sh['subarray']*100:5.2f}% "
                            f"| linear {bs_sh['linear']*100:5.2f}% | prefix {bs_sh['prefix']*100:5.2f}% "
                            f"| remove {bs_sh['remove']*100:5.2f}% | upgrade {bs_sh['upgrade']*100:5.2f}%"
                        )
                    if (
                        cell["gate_blocked_entries_per_insert"] > 0
                        or cell["quiesce_drain_cycles_per_fallback"] > 0
                    ):
                        print(
                            f"        gate/drain: blocked/insert {cell['gate_blocked_entries_per_insert']:.6f} "
                            f"| wait_cyc/insert {cell['gate_wait_cycles_per_insert']:.1f} "
                            f"| drain_cyc/fb {cell['quiesce_drain_cycles_per_fallback']:.1f}"
                        )
                alloc_val = cell.get("total_allocs_per_insert")
                retired_val = cell.get("retired_per_insert", 0)
                if (alloc_val is not None and alloc_val > 0) or (retired_val is not None and retired_val > 0):
                    alloc_str = f"{alloc_val:.4f}" if alloc_val is not None else "null"
                    ret_str = f"{retired_val:.4f}" if retired_val is not None else "0.0000"
                    print(
                        f"        allocs/retires: allocs/insert {alloc_str} "
                        f"| retired/insert {ret_str} "
                        f"| tsc_hz: {cell.get('tsc_hz', 0)}"
                    )

    primary_tp_bin = bin_default if variant_list else throughput_bin
    pmu_results = None
    if args.pmu:
        pmu_results = run_pmu_pass(
            primary_tp_bin,
            arm=args.pmu_arm,
            # The sweep's own writer counts, so the pass can speak about the
            # cells the sweep reports rather than W=2 alone.
            writers=writers_list,
            rounds=max(args.rounds, 8),
            quick=args.quick,
        )

    c2c_results = None
    c2c_error = None
    if args.c2c:
        try:
            c2c_results = run_c2c_pass(
                primary_tp_bin,
                arm=args.pmu_arm,
                writers=max(writers_list),
                quick=args.quick,
                rounds=max(args.rounds, C2C_ROUNDS),
            )
        except RuntimeError as exc:
            c2c_error = str(exc)
            c2c_results = {
                "arm": args.pmu_arm,
                "writers": max(writers_list),
                "window": C2C_WINDOW,
                "cell_isolation": C2C_CELL_ISOLATION,
                "rounds": max(args.rounds, C2C_ROUNDS),
                "error": c2c_error,
                "verdict": "FAILED",
            }

    artifact: dict[str, Any] = {
        "provenance": prov,
        "throughput": throughput_cells,
    }
    if variant_list:
        artifact["throughput_variant"] = variant_cells
        artifact["comparison"] = comparison_results
    if pmu_results is not None:
        artifact["pmu"] = pmu_results
    if c2c_results is not None:
        artifact["c2c"] = c2c_results

    # Refuse, before anything is written, an artifact whose timed rows are not
    # the cells the schedule ran (METHODOLOGY.md §15).
    check_artifact_against_schedule(artifact, scheduled)

    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(artifact, indent=2) + "\n")
        print(f"\nWrote artifact to {args.out}")

    if c2c_error is not None:
        sys.stderr.write(f"perf c2c failed: {c2c_error} (AGENTS.md §8.1)\n")
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
