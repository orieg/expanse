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
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import subprocess
import sys
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


def get_binaries() -> tuple[Path, Path]:
    throughput_bin = THROUGHPUT_TARGET / "release" / "examples" / "writer_scaling"
    counters_bin = COUNTERS_TARGET / "release" / "examples" / "writer_scaling"
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


def build_binaries(verbose: bool = True) -> tuple[Path, Path]:
    """Two builds, never one (AGENTS.md §6 / hot_concurrent.rs:36-42).

    Throughput comes from uninstrumented build (refuses occ-stats).
    Counters come from diagnostic build (--features occ-stats, refuses timing).
    Both builds use explicit, isolated target dirs so neither overwrites the other
    nor depends on external CARGO_TARGET_DIR environment settings.
    """
    throughput_bin, counters_bin = get_binaries()
    if verbose:
        print("building throughput binary (default features, uninstrumented) ...")
    tp_env = dict(os.environ)
    tp_env["CARGO_TARGET_DIR"] = str(THROUGHPUT_TARGET)
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "-p",
            "expanse-trie",
            "--example",
            "writer_scaling",
        ],
        cwd=str(REPO_ROOT),
        env=tp_env,
        check=True,
    )
    if verbose:
        _print_binary_info("throughput", throughput_bin)

    if verbose:
        print("building diagnostic counters binary (--features occ-stats) ...")
    cnt_env = dict(os.environ)
    cnt_env["CARGO_TARGET_DIR"] = str(COUNTERS_TARGET)
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "-p",
            "expanse-trie",
            "--features",
            "occ-stats",
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
        "--rounds",
        str(rounds),
    ]
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

    expected_count = len(writers) * rounds
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
            bs_sum = bs_sub + bs_lin + bs_pfx + bs_rem
            if bs_sum != int(causes["branch_split"]):
                raise ValueError(
                    f"{arm} W={w} round {r['round']}: branch_split partition ({bs_sub} + {bs_lin} + "
                    f"{bs_pfx} + {bs_rem} = {bs_sum}) != causes['branch_split'] ({causes['branch_split']})"
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
        total_q_calls = sum(int(r.get("quiesce_calls", 0)) for r in c_rows_w)
        total_q_drain = sum(int(r.get("quiesce_drain_cycles", 0)) for r in c_rows_w)
        total_bs_sub = sum(int(r.get("branch_split_subarray", 0)) for r in c_rows_w)
        total_bs_lin = sum(int(r.get("branch_split_linear", 0)) for r in c_rows_w)
        total_bs_pfx = sum(int(r.get("branch_split_prefix", 0)) for r in c_rows_w)
        total_bs_rem = sum(int(r.get("branch_split_remove", 0)) for r in c_rows_w)

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
        }

        tp_rel = THROUGHPUT_TARGET.relative_to(REPO_ROOT)
        cnt_rel = COUNTERS_TARGET.relative_to(REPO_ROOT)

        cell: dict[str, Any] = {
            "workload_id": first_t["workload_id"],
            "arm": arm,
            "writers": w,
            "readers": 0,
            "prefill": first_t["prefill"],
            "fresh_keys": first_t["fresh_keys"],
            "rounds": rounds,
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
            "gate_blocked_entries_per_insert": gate_blocked_entries_per_insert,
            "gate_wait_cycles_per_insert": gate_wait_cycles_per_insert,
            "quiesce_drain_cycles_per_fallback": quiesce_drain_cycles_per_fallback,
            "lock_restarts_per_insert": restarts_per_insert,
            "build_provenance": {
                "throughput": f"{tp_rel}/release/examples/writer_scaling (uninstrumented)",
                "counters": f"{cnt_rel}/release/examples/writer_scaling (--features occ-stats)",
            },
            "rounds_raw": [
                {
                    "round": r["round"],
                    "position": r["position"],
                    "writer_mops": r["writer_mops"],
                    "writer_elapsed_s": r["writer_elapsed_s"],
                    "write_ops": r["write_ops"],
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
                == r["fallback_causes"]["branch_split"]
            ), r
    assert abs(sum(cell_w2["fallback_cause_share"].values()) - 1.0) < 1e-3, cell_w2["fallback_cause_share"]
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

    # 6. Privacy check: ensure no absolute repo root or home paths leaked into cells (AGENTS.md §7)
    repo_root_str = str(REPO_ROOT)
    for c in cells:
        c_json = json.dumps(c)
        assert (
            repo_root_str not in c_json
        ), f"Absolute repo root path leaked into cell JSON (AGENTS.md §7): {c_json}"
        assert (
            "/home/" not in c_json
        ), f"Home directory path leaked into cell JSON (AGENTS.md §7): {c_json}"

    # 7. Williams square balance property check (count positions and pairs over 1 full cycle)
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
        if out_path == COMMITTED_RESULTS_PATH.resolve() and not args.force_quick_out:
            sys.stderr.write(
                "error: --quick output cannot overwrite committed results path "
                f"{COMMITTED_RESULTS_PATH} without --force-quick-out\n"
            )
            return 1

    # Apply core pin before any measurement
    core_pin = bench_pin.apply("writer_scaling.py")

    # Build both binaries up front into their isolated target dirs
    throughput_bin, counters_bin = build_binaries(verbose=True)

    if args.arm in ("all", "both"):
        arms = ["map", "set", "str"] if args.arm == "all" else ["map", "set"]
    else:
        arms = [args.arm]

    prov = new_provenance(
        suite="concurrency",
        issue=568,
        ratio="Expanse throughput over single-writer baseline C(N) = Mops(W) / Mops(1)",
        repo_root=REPO_ROOT,
        core_pin=core_pin,
    )

    throughput_cells: list[dict[str, Any]] = []

    print("========================================================================")
    print(" Expanse-Native Multi-Writer Scaling Sweep (Phase 1.5D, Refs #568)")
    print(f" Arms: {', '.join(arms)} | Writers: {writers_list} | Rounds: {args.rounds}")
    print(f" Pin: {core_pin}")
    print("========================================================================")

    for arm in arms:
        cell_label = f"arm:{arm}:writers"
        start_snap = begin_cell(prov, cell_label)

        # Pass 1: throughput (uninstrumented binary, interleaved across W)
        print(f"\n  [Pass 1/2] Throughput — {arm} arm across W ∈ {writers_list} (uninstrumented build)")
        t_rows = run_pass(throughput_bin, "throughput", arm, writers_list, args.rounds, quick=args.quick)

        # End load snapshot immediately after timed Pass 1 so Pass 2 does not dilute load window
        load = end_cell(start_snap)

        # Pass 2: counters (diagnostic occ-stats binary, across all rounds)
        print(f"  [Pass 2/2] Diagnostic counters — {arm} arm across W ∈ {writers_list} (occ-stats build)")
        c_rows = run_pass(counters_bin, "counters", arm, writers_list, args.rounds, quick=args.quick)

        cells = summarize_arm(arm, writers_list, args.rounds, t_rows, c_rows, load)
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
                        f"| remove {bs_sh['remove']*100:5.2f}%"
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

    artifact = {
        "provenance": prov,
        "throughput": throughput_cells,
    }

    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(artifact, indent=2) + "\n")
        print(f"\nWrote artifact to {args.out}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
