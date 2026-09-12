#!/usr/bin/env python3
"""Expanse-native writer scaling driver across physical P-cores (Refs #568, Phase 1.5D).

Runs `crates/expanse/examples/writer_scaling.rs` across W in {1, 2, 4, 8} on physical
P-cores for map (64-bit), set (63-bit), and str arms.

Two builds, never one (AGENTS.md §6 / hot_concurrent.rs:36-42):
- Pass 1 (throughput): uninstrumented release build, interleaved across W within each round,
  balancing position and first-order carryover across rounds (Williams design).
  Emits elapsed_s and writer_mops. Refuses to run if occ-stats is enabled.
- Pass 2 (counters): diagnostic build (--features occ-stats), captures exact lock_fallbacks
  and write_ops across all rounds. Refuses to emit elapsed_s or writer_mops.

Computes:
- expanse_writer_mops_mean as headline point estimate with BCa 95% bootstrap CI (AGENTS.md §8.4)
- expanse_writer_mops_median as auxiliary field for historical continuity
- scaling factor C(N) = T(W) / T(1) with paired bootstrap BCa 95% CI across interleaved rounds
- lock fallbacks and fallback rate from the diagnostic occ-stats pass
- str arm as the alpha=1 coarse-mutex reference curve (0 lock fallbacks by construction)

Usage:
    python3 docs/benchmarks/concurrency/scripts/writer_scaling.py --out docs/benchmarks/concurrency/results/baseline_writer_scaling.json
    python3 docs/benchmarks/concurrency/scripts/writer_scaling.py --self-test

`--self-test` is run by the `writer-scaling-selftest` CI job, gated on the
`concurrency-instrument` path filter: this file and the harness example it
builds and parses. `scripts/gate.sh` does not run it, because that mirrors CI's
`lint` and `test` jobs and this is neither -- run it by hand when changing
either end of the contract.
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci  # noqa: E402
from bench_provenance import (  # noqa: E402
    begin_cell,
    end_cell,
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
    "compare_ablation_freelist": "ablation-striped-freelist",
}
ABLATION_RESULTS_PATHS = tuple(
    REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / f"ablation_{arm}_writer_scaling.json"
    for arm in ("alloc", "epoch", "freelist")
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
        mean_mops, ci_lower, ci_upper = bca_bootstrap_ci(mops_samples, confidence=0.95)
        median_mops = sorted(mops_samples)[len(mops_samples) // 2]

        # 2. Scaling factor C(N) via paired bootstrap across interleaved rounds
        if w == 1:
            cn_mean = 1.0
            cn_ci_lower = 1.0
            cn_ci_upper = 1.0
            cn_median = 1.0
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
            cn_mean, cn_ci_lower, cn_ci_upper = bca_bootstrap_ci(
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
            "expanse_writer_mops_median": round(median_mops, 4),
            "scaling_factor_c_n": round(cn_mean, 4),
            "scaling_factor_c_n_mean": round(cn_mean, 4),
            "scaling_factor_c_n_ci_lower": round(cn_ci_lower, 4),
            "scaling_factor_c_n_ci_upper": round(cn_ci_upper, 4),
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
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    """Runs interleaved (build × W) execution within each round.

    Within each round r, default and variant builds are alternated to eliminate
    thermal drift and host load confounding between builds (Williams design).
    Computes paired bootstrap BCa 95% CI on C_variant(w) / C_default(w).
    """
    print("\n========================================================================")
    print(f" Interleaved (build × W) Execution: default vs {variant_name}")
    print(f" Arm: {arm} | Writers: {writers_list} | Rounds: {rounds}")
    print("========================================================================")

    all_t_rows_default: list[dict[str, Any]] = []
    all_t_rows_variant: list[dict[str, Any]] = []

    start_snap_def = begin_cell(prov, f"arm:{arm}:default")
    start_snap_var = begin_cell(prov, f"arm:{arm}:{variant_name}")

    for round_idx in range(rounds):
        # Alternate order within round to balance first-runner order bias
        if round_idx % 2 == 0:
            rows_d = run_pass(
                bin_default, "throughput", arm, writers_list, rounds=rounds, round_opt=round_idx, quick=quick
            )
            rows_v = run_pass(
                bin_variant, "throughput", arm, writers_list, rounds=rounds, round_opt=round_idx, quick=quick
            )
        else:
            rows_v = run_pass(
                bin_variant, "throughput", arm, writers_list, rounds=rounds, round_opt=round_idx, quick=quick
            )
            rows_d = run_pass(
                bin_default, "throughput", arm, writers_list, rounds=rounds, round_opt=round_idx, quick=quick
            )
        all_t_rows_default.extend(rows_d)
        all_t_rows_variant.extend(rows_v)

    load_def = end_cell(start_snap_def)
    load_var = end_cell(start_snap_var)

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
        arm, writers_list, rounds, all_t_rows_default, c_rows_def, load_def, throughput_target=tp_target_def
    )
    cells_variant = summarize_arm(
        arm, writers_list, rounds, all_t_rows_variant, c_rows_var, load_var, throughput_target=tp_target_var
    )
    for c in cells_variant:
        c["variant"] = variant_name

    # Compute paired C_variant(w) / C_default(w) per round
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

            paired_ratios.append(c_v / c_d)

        mean_ratio, ci_lower, ci_upper = bca_bootstrap_ci(paired_ratios, confidence=0.95)
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
            "ratio_median": round(median_ratio, 4),
            "verdict": verdict,
            "paired_ratios_raw": [round(x, 6) for x in paired_ratios],
        }
        comparison_stats["per_writer"][str(w)] = stat_entry

        print(
            f"  [Comparison W={w:<2}] Ratio C_{variant_name}({w}) / C_default({w}): "
            f"Mean {mean_ratio:.4f} [{ci_lower:.4f}, {ci_upper:.4f}] | "
            f"Verdict: {verdict}"
        )

    return cells_default, cells_variant, comparison_stats


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
    round_data: dict[int, dict[int, dict[str, int]]] = {}

    for r in range(rounds):
        round_data[r] = {}
        for w in writers:
            with tempfile.TemporaryDirectory(prefix=f"perf_fifo_r{r}_w{w}_") as fifo_dir:
                ctl_fifo = os.path.join(fifo_dir, "ctl.fifo")
                ack_fifo = os.path.join(fifo_dir, "ack.fifo")
                os.mkfifo(ctl_fifo)
                os.mkfifo(ack_fifo)

                cmd = [
                    "perf",
                    "stat",
                    "--delay=-1",
                    f"--control=fifo:{ctl_fifo},{ack_fifo}",
                    "-x,",
                    "-e",
                    event_arg,
                    "--",
                    str(binary),
                    "--role",
                    "throughput",
                    "--arm",
                    arm,
                    "--writers",
                    str(w),
                    "--rounds",
                    "1",
                    "--round",
                    str(r),
                    "--perf-ctl-fifo",
                    ctl_fifo,
                    "--perf-ack-fifo",
                    ack_fifo,
                ]
                if quick:
                    cmd.append("--quick")

                proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
                if proc.returncode != 0:
                    raise RuntimeError(
                        f"--pmu round {r} W={w} failed (exit {proc.returncode}): {proc.stderr} (AGENTS.md §8.1)"
                    )
                counts = parse_perf_stat_csv(proc.stderr)
                round_data[r][w] = counts

    cyc_key = next((k for k in events if "cycles" in k and "ref" not in k), None)
    ref_key = next((k for k in events if "ref" in k), None)

    droop_samples: list[float] = []
    if cyc_key and ref_key and 1 in writers and 2 in writers:
        for r in range(rounds):
            w1_counts = round_data.get(r, {}).get(1, {})
            w2_counts = round_data.get(r, {}).get(2, {})
            c1, r1 = w1_counts.get(cyc_key, 0), w1_counts.get(ref_key, 0)
            c2, r2 = w2_counts.get(cyc_key, 0), w2_counts.get(ref_key, 0)
            if r1 > 0 and r2 > 0:
                f1 = c1 / r1
                f2 = c2 / r2
                droop_samples.append(1.0 - (f2 / f1))

    droop_summary: dict[str, Any] = {
        "rounds_preregistered": rounds,
        "n_measured": len(droop_samples),
    }
    if len(droop_samples) >= 3:
        mean_d, ci_lo, ci_hi = bca_bootstrap_ci(droop_samples, confidence=0.95)
        # Verdict decision rule (§8.4 / §8.20):
        # A single run cannot CONFIRM; confirmation requires a second independent run across two committed artifacts.
        # CI_lower > 0.05 -> SINGLE_RUN_PASS (candidate droop confirmed pending run 2)
        # CI_upper < 0.05 -> REJECTED
        # CI spans 0.05   -> INCONCLUSIVE (data cannot reject or confirm)
        if ci_lo > 0.05:
            verdict = "SINGLE_RUN_PASS"
        elif ci_hi < 0.05:
            verdict = "REJECTED"
        else:
            verdict = "INCONCLUSIVE"
        droop_summary.update({
            "droop_mean": round(mean_d, 4),
            "droop_ci_lower": round(ci_lo, 4),
            "droop_ci_upper": round(ci_hi, 4),
            "verdict": verdict,
        })
        print(
            f"  [Hypothesis A (Frequency Droop)] Mean drop: {mean_d * 100:.2f}% "
            f"[{ci_lo * 100:.2f}%, {ci_hi * 100:.2f}%] | Verdict: {verdict}"
        )

    return {
        "arm": arm,
        "events": events,
        "rounds_preregistered": rounds,
        "n_measured": len(droop_samples),
        "frequency_droop": droop_summary,
        "raw_counts": round_data,
    }


def run_c2c_pass(
    binary: Path,
    arm: str = "set",
    writers: int = 2,
    quick: bool = False,
    out_dir: Path | None = None,
) -> dict[str, Any]:
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

    cmd_record = [
        "perf",
        "c2c",
        "record",
        "-F",
        "60000",
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
        "1",
    ]
    if quick:
        cmd_record.append("--quick")

    print("\n========================================================================")
    print(f" Hardware perf c2c Cache Contention Recording ({arm} W={writers})")
    print(f" Output: {c2c_data.relative_to(REPO_ROOT)}")
    print("========================================================================")

    proc = subprocess.run(cmd_record, check=False)
    if proc.returncode != 0:
        raise RuntimeError(f"--c2c record failed (exit {proc.returncode}) (AGENTS.md §8.1)")

    cmd_report = ["perf", "c2c", "report", "--stdio", "-i", str(c2c_data)]
    proc_rep = subprocess.run(cmd_report, capture_output=True, text=True, check=False)
    if proc_rep.returncode != 0:
        raise RuntimeError(
            f"--c2c report failed (exit {proc_rep.returncode}): {proc_rep.stderr} (AGENTS.md §8.1)"
        )
    report_text = proc_rep.stdout
    report_file = out_dir / f"c2c_report_{arm}_w{writers}.txt"
    report_file.write_text(report_text)
    print(f"  [c2c Pass] Wrote c2c report to {report_file.relative_to(REPO_ROOT)}")

    summary_lines = report_text.splitlines()[:20]

    return {
        "arm": arm,
        "writers": writers,
        "report_path": str(report_file.relative_to(REPO_ROOT)),
        "data_path": str(c2c_data.relative_to(REPO_ROOT)),
        "summary": "\n".join(summary_lines),
    }


def self_test() -> int:
    eprintln = sys.stderr.write
    eprintln("Running writer_scaling.py self-test...\n")

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
    t_rows_map = run_pass(throughput_bin, "throughput", "map", [1, 2], 3, quick=True)
    load = end_cell(start_snap)

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
    assert all(fb > 0 for fb in w2_map_fbs), f"Expected map W=2 lock_fallbacks > 0, got {w2_map_fbs}"

    c_rows_set = run_pass(counters_bin, "counters", "set", [1, 2], 3, quick=True)
    assert len(c_rows_set) == 6
    w2_set_fbs = [r["lock_fallbacks"] for r in c_rows_set if r["writers"] == 2]
    assert len(w2_set_fbs) == 3
    assert all(fb > 0 for fb in w2_set_fbs), f"Expected set W=2 lock_fallbacks > 0, got {w2_set_fbs}"

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
    assert len(cell_w1["counters_raw"]) == 3, f"Expected 3 counters_raw entries, got {len(cell_w1['counters_raw'])}"
    assert "position" in cell_w1["rounds_raw"][0]
    assert "position" in cell_w1["counters_raw"][0]

    assert cell_w2["writers"] == 2
    assert cell_w2["scaling_factor_c_n_ci_lower"] <= cell_w2["scaling_factor_c_n"] <= cell_w2["scaling_factor_c_n_ci_upper"]
    assert cell_w2["lock_fallbacks"] > 0
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
    assert abs(sum(cell_w2["fallback_cause_share"].values()) - 1.0) < 1e-3, cell_w2["fallback_cause_share"]

    # 6. Test comparison runner with self-comparison on quick scale
    eprintln("Testing run_comparison (interleaved build x W execution)...")
    cells_def, cells_var, comp_stats = run_comparison(
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
    cells_def_none, cells_var_none, _ = run_comparison(
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
        pmu_res = run_pmu_pass(throughput_bin, arm="set", writers=[1, 2], rounds=3, quick=True)
        assert isinstance(pmu_res, dict)
        c2c_res = run_c2c_pass(throughput_bin, arm="set", writers=2, quick=True)
        assert isinstance(c2c_res, dict)

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
    t_rows_bal = run_pass(
        throughput_bin, "throughput", "map", [1, 2, 4, 8], 4, quick=True
    )
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
        "--compare-ablation-freelist",
        action="store_true",
        help="Shorthand for --compare ablation-striped-freelist (Hypothesis D arm c)",
    )
    parser.add_argument(
        "--pmu",
        action="store_true",
        help="Run separate hardware PMU pass via perf stat on set W=1 vs W=2",
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
    if n_w % 2 != 0 or args.rounds % n_w != 0:
        sys.stderr.write(
            f"notice: Williams square balance requires even writer count and rounds multiple of len(writers); "
            f"got len(writers)={n_w}, rounds={args.rounds} — position/carryover balance will be incomplete\n"
        )

    if args.quick and args.out:
        out_path = Path(args.out).resolve()
        if (
            out_path
            in (
                COMMITTED_RESULTS_PATH.resolve(),
                DIAGNOSTIC_RESULTS_PATH.resolve(),
                PADDED_RESULTS_PATH.resolve(),
                *(p.resolve() for p in ABLATION_RESULTS_PATHS),
            )
            and not args.force_quick_out
        ):
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

    if variant_list:
        bin_default, cnt_default = build_binaries(features=None, verbose=True)

        ratio_desc = (
            f"Expanse {variant_list[0]} throughput over default baseline C_variant(W) / C_default(W)"
            if len(variant_list) == 1
            else f"Expanse variants ({', '.join(variant_list)}) throughput over default baseline C_variant(W) / C_default(W)"
        )
        prov = new_provenance(
            suite="concurrency",
            issue=568,
            ratio=ratio_desc,
            repo_root=REPO_ROOT,
            core_pin=core_pin,
        )

        for var in variant_list:
            bin_variant, cnt_variant = build_binaries(features=var, verbose=True)
            for arm in arms:
                cells_d, cells_v, comp_stats = run_comparison(
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

            # Pass 1: throughput (uninstrumented binary, interleaved across W)
            print(f"\n  [Pass 1/2] Throughput — {arm} arm across W ∈ {writers_list}")
            t_rows = run_pass(throughput_bin, "throughput", arm, writers_list, args.rounds, quick=args.quick)

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
            arm="set",
            writers=[1, 2],
            rounds=max(args.rounds, 8),
            quick=args.quick,
        )

    c2c_results = None
    c2c_error = None
    if args.c2c:
        try:
            c2c_results = run_c2c_pass(
                primary_tp_bin,
                arm="set",
                writers=2,
                quick=args.quick,
            )
        except RuntimeError as exc:
            c2c_error = str(exc)
            c2c_results = {
                "arm": "set",
                "writers": 2,
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
