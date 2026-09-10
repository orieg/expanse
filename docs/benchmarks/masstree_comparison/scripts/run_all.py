#!/usr/bin/env python3
"""Runner for the Masstree comparison suite (#661).

Drives every cell of ``METHODOLOGY.md`` §5, one process per cell (§3.6), and
harvests BCa 95% intervals for the wall-clock pillars.

What this runner does that ``hot_comparison``'s does not, all recorded in the
methodology:

1. **The validation gate runs first and is fatal** (§3.2, §9). Nothing is
   recorded if ``masstree_validate`` fails.

2. **The memory pillar carries two instruments per cell** (§3.3): the shared
   allocator census, whose Masstree figure is quantized to the 2 MiB pool slab
   and carries its measured slack and the ``QUANTUM_DOMINATED`` flag, and each
   engine's own node census. The two are written side by side and never
   combined into one ratio.

3. **The Masstree column is a predicate, not a precondition** (§3.4). A string
   cell whose population contains keys beyond ``MASSTREE_MAXKEYLEN`` still
   runs — the Expanse side is never restricted — and its Masstree column
   carries ``NOT_REPRESENTABLE_MASSTREE`` with the key count.

4. **The concurrent cells reproduce ``hot_comparison`` §11.4 exactly**, so the
   two routes to the write-concurrency loss are read side by side and the
   Expanse column is a replication of #692's.
"""

import json
import os
import platform
import subprocess
import sys
import time
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"
CRATE = REPO_ROOT / "crates" / "expanse-hot-bench" / "Cargo.toml"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bca_bootstrap import bca_bootstrap_ratio_ci  # noqa: E402
from bench_ab import ab_provenance, interleave  # noqa: E402
from bench_provenance import (  # noqa: E402
    add_load, begin_cell, end_cell, estimators, git_sha, host_facts, load_snapshot, raw_rounds,
)
from masstree_envelope import census_quantum_dominated  # noqa: E402

# The HOT suite's λ targets, so the ExpanseMap column is the same cells (§5).
LAMBDA_TARGETS = [1.0, 2.0, 4.0, 8.0, 15.0, 23.0, 30.0, 38.0, 46.0, 61.0]
STRUCTURED_DISTS = ["sequential", "clustered", "sparse"]
STRUCTURED_MEMORY_N = 1_000_000
LATENCY_POPULATIONS = [10_000, 100_000, 1_000_000]
DISTRIBUTIONS = ["sequential", "clustered", "sparse", "random"]
SCAN_K = [10, 100, 1000]
PILLARS = ["lookup_hit", "lookup_miss", "insert", "scan"]

# String cells (§5): the HOT suite's shapes and its population sweep.
SHAPES = ["short", "counter", "prefixed", "skewed", "beyond"]
STRING_MEMORY_POPULATIONS = [1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000,
                             125_000, 150_000, 200_000, 500_000, 1_000_000]

# Concurrent cells (§5): hot_comparison §11.4, W + R <= 16.
CONCURRENT_WRITE_SCALING = [1, 2, 4, 8, 16]
CONCURRENT_MIXED_WRITERS = [0, 1, 2, 4, 8]
CONCURRENT_MIXED_READERS = 8
# Two-commit mode (#568 PR 3, docs/BENCHMARKING.md rule 18): the readers-alone
# reference and the gate cell at R = 8; C1 stays the full writer-count control.
AB_MIXED_WRITERS = [0, 1]
# One process per round per build, so the pair sees the same host minute.
AB_ROUNDS = 15
CONCURRENT_HEALTH_WRITERS = [1, 2, 4, 8]
CONCURRENT_ARMS = ["map", "str"]
OCC_STATS_TARGET = CRATE.parent / "target-occ-stats"

MASSTREE_COMMIT = "1119842"


def population_for_lambda(lam: float) -> int:
    """Population that puts a 64-bit uniform random map at occupancy `lam`."""
    return max(1000, int(round(lam * 2 ** 16)))


LATENCY_RAW = ("first_arm", "masstree_ns_per_op", "expanse_ns_per_op")

# Every counter a health row carries, all kept verbatim per round. The first
# ten are the protocol counters the cells have always emitted (`locked_reads`
# was emitted and dropped by the runner until #568); the rest are the
# attribution counters #568's Step 0 adds. A row missing any of them is an
# error, never a zero: a counter that was not recorded is not a count of nothing.
HEALTH_RAW = (
    "restart_share", "fallback_share", "read_ops", "read_attempts", "read_fallbacks",
    "sample_spins", "write_ops", "locked_reads",
    "handoffs", "retired", "freed_raw", "sample_spin_cycles", "branch_replacements",
    "deep_cascades", "root_rewrites", "cycles_hz", "reader_elapsed_s", "writer_elapsed_s",
    "lock_restarts", "lock_spins", "lock_hold_cycles", "lock_fallbacks",
)
# The shares derived from those counters per round, then summarised as median
# with range like the counters themselves. Each is a ratio of two counters from
# the same round, so its median is a median of per-round ratios — not the
# quotient of two medians, which is what the README's `sample_spins ÷ read_ops`
# column has always been and stays.
HEALTH_DERIVED = (
    "locked_share", "unconditional_share", "handoffs_per_write", "replacements_per_write",
    "deep_cascade_share", "root_rewrite_share", "spin_time_share",
)


BINS = ["masstree_validate", "masstree_latency", "masstree_string_latency", "masstree_memory"]


def build(env: dict) -> None:
    print("building the Masstree arm at -C target-cpu=haswell (--features masstree) ...")
    args = ["cargo", "build", "--release", "--manifest-path", str(CRATE), "--features", "masstree"]
    for b in BINS:
        args += ["--bin", b]
    subprocess.run(args, check=True, env=env)


def build_concurrent(env: dict) -> None:
    """Two builds, never one: the default build times, the occ-stats build counts."""
    print("building masstree_concurrent (--features masstree) ...")
    subprocess.run(["cargo", "build", "--release", "--manifest-path", str(CRATE),
                    "--features", "masstree", "--bin", "masstree_concurrent", "--bin", "masstree_memory"],
                   check=True, env=env)
    print("building the diagnostic masstree_concurrent (--features masstree,occ-stats) ...")
    occ_env = dict(env)
    occ_env["CARGO_TARGET_DIR"] = str(OCC_STATS_TARGET)
    subprocess.run(["cargo", "build", "--release", "--manifest-path", str(CRATE),
                    "--features", "masstree,occ-stats", "--bin", "masstree_concurrent"],
                   check=True, env=occ_env)


def binary(name: str) -> Path:
    target = os.environ.get("CARGO_TARGET_DIR")
    root = Path(target) if target else (CRATE.parent / "target")
    return root / "release" / name


def run_cell(args: list, env: dict) -> list:
    proc = subprocess.run(args, capture_output=True, text=True, env=env)
    if proc.returncode != 0:
        raise RuntimeError(f"cell failed ({proc.returncode}): {' '.join(str(a) for a in args)}\n{proc.stderr.strip()}")
    return [json.loads(line) for line in proc.stdout.splitlines() if line.startswith("{")]


def validate(env: dict) -> str:
    print("\n[0] masstree_validate — the gate (§9)")
    proc = subprocess.run([str(binary("masstree_validate"))], capture_output=True, text=True, env=env)
    sys.stdout.write(proc.stdout)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit("masstree_validate FAILED — no Masstree cell is recorded (§8.1)")
    return proc.stdout


def verdict(lo: float, hi: float) -> str:
    return "BOUNDARY_RESULT" if lo <= 1.0 <= hi else ("expanse" if lo > 1.0 else "masstree")


def memory_row(row: dict) -> dict:
    """Attaches the §3.3 flag, computed by the envelope function, not by hand."""
    if row["masstree_alloc_bytes_per_key"] is None:
        row["masstree_quantum_dominated"] = None
        row["masstree_label"] = "NOT_REPRESENTABLE_MASSTREE"
        return row
    n = row["population"]
    alloc = int(round(row["masstree_alloc_bytes_per_key"] * n))
    struct = int(round(row["masstree_structural_bytes_per_key"] * n))
    row["masstree_quantum_dominated"] = census_quantum_dominated(alloc, struct)
    row["masstree_label"] = "QUANTUM_DOMINATED" if row["masstree_quantum_dominated"] else "ok"
    return row


def sweep_memory(env: dict, quick: bool) -> dict:
    lambdas = LAMBDA_TARGETS[:4] if quick else LAMBDA_TARGETS
    cells = []
    print("  integer map, random, λ sweep")
    for lam in lambdas:
        n = population_for_lambda(lam)
        rows = run_cell([str(binary("masstree_memory")), "map", "random", str(n)], env)
        if len(rows) != 1:
            raise RuntimeError("memory cell emitted %d rows" % len(rows))
        row = memory_row(rows[0])
        row["lambda_target"] = lam
        cells.append(row)
        print(f"  memory map random λ≈{lam:<5} N={n:<9} allocator: Masstree {row['masstree_alloc_bytes_per_key']:.2f} "
              f"[{row['masstree_label']}] Expanse {row['expanse_alloc_bytes_per_key']:.2f} B/key; "
              f"structural {row['masstree_structural_bytes_per_key']:.2f} vs mem_used {row['expanse_mem_used_bytes_per_key']:.2f}")
    if not quick:
        print("  integer map, structured distributions at N = 1M")
        for dist in STRUCTURED_DISTS:
            rows = run_cell([str(binary("masstree_memory")), "map", dist, str(STRUCTURED_MEMORY_N)], env)
            row = memory_row(rows[0])
            row["lambda_target"] = None
            cells.append(row)
            print(f"  memory map {dist:<10} N={STRUCTURED_MEMORY_N:<9} allocator: Masstree {row['masstree_alloc_bytes_per_key']:.2f} "
                  f"[{row['masstree_label']}] Expanse {row['expanse_alloc_bytes_per_key']:.2f} B/key")
    return {"cells": cells}


def sweep_string_memory(env: dict, quick: bool) -> dict:
    pops = STRING_MEMORY_POPULATIONS[:4] if quick else STRING_MEMORY_POPULATIONS
    shapes = ["short", "prefixed", "beyond"] if quick else SHAPES
    cells = []
    for dist in shapes:
        for n in pops:
            rows = run_cell([str(binary("masstree_memory")), "str", dist, str(n)], env)
            row = memory_row(rows[0])
            cells.append(row)
            mt = row["masstree_alloc_bytes_per_key"]
            mt_s = (f"{mt:.2f} [{row['masstree_label']}]" if mt is not None
                    else f"withheld ({row['masstree_not_representable']} > {255} B)")
            print(f"  memory str {dist:<9} N={n:<9} allocator: Masstree {mt_s:>28} Expanse {row['expanse_alloc_bytes_per_key']:.2f} B/key")
    return {"cells": cells}


def sweep_latency(env: dict, quick: bool) -> dict:
    pops = [10_000] if quick else LATENCY_POPULATIONS
    dists = ["random"] if quick else DISTRIBUTIONS
    cells = []
    for pillar in PILLARS:
        for dist in dists:
            for n in pops:
                for k in (SCAN_K if pillar == "scan" else [0]):
                    args = [str(binary("masstree_latency")), pillar, dist, str(n)]
                    if pillar == "scan":
                        args.append(str(k))
                    rows = run_cell(args, env)
                    mt = [r["masstree_ns_per_op"] for r in rows]
                    exp = [r["expanse_ns_per_op"] for r in rows]
                    # Masstree ÷ Expanse: above 1.0 means Expanse is faster (§5).
                    ratio, lo, hi = bca_bootstrap_ratio_ci(mt, exp, num_resamples=2000, seed=42)
                    head = rows[0]
                    cells.append({
                        "workload_id": head["workload_id"], "pillar": pillar, "arm": "map", "dist": dist,
                        "keyspace_bits": 64, "population": head["population"], "lambda": head["lambda"],
                        "scan_k": k, "rounds": len(rows),
                        "masstree_ns_per_op_median": round(sorted(mt)[len(mt) // 2], 4),
                        "expanse_ns_per_op_median": round(sorted(exp)[len(exp) // 2], 4),
                        "masstree_over_expanse": round(ratio, 4),
                        "ci_lower": round(lo, 4), "ci_upper": round(hi, 4),
                        "verdict": verdict(lo, hi),
                        "rounds_raw": raw_rounds(rows, LATENCY_RAW),
                    })
                    print(f"  {pillar:<12} map {dist:<10} N={n:<8} k={k:<5} ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}]")
    return {"cells": cells}


def sweep_string_latency(env: dict, quick: bool) -> dict:
    pops = [10_000] if quick else LATENCY_POPULATIONS
    shapes = ["short", "prefixed", "beyond"] if quick else SHAPES
    cells = []
    for pillar in PILLARS:
        for dist in shapes:
            for n in pops:
                for k in (SCAN_K if pillar == "scan" else [0]):
                    args = [str(binary("masstree_string_latency")), pillar, dist, str(n)]
                    if pillar == "scan":
                        args.append(str(k))
                    rows = run_cell(args, env)
                    head = rows[0]
                    exp = [r["expanse_ns_per_op"] for r in rows]
                    exp_med = round(sorted(exp)[len(exp) // 2], 4)
                    cell = {
                        "workload_id": head["workload_id"], "pillar": pillar, "arm": "str", "dist": dist,
                        "population": head["population"], "mean_key_len": head["mean_key_len"],
                        "masstree_not_representable": head["masstree_not_representable"],
                        "scan_k": k, "rounds": len(rows), "expanse_ns_per_op_median": exp_med,
                        "rounds_raw": raw_rounds(rows, LATENCY_RAW),
                    }
                    if head["masstree_not_representable"] == 0 and head["masstree_ns_per_op"] is not None:
                        mt = [r["masstree_ns_per_op"] for r in rows]
                        ratio, lo, hi = bca_bootstrap_ratio_ci(mt, exp, num_resamples=2000, seed=42)
                        cell.update({
                            "masstree_ns_per_op_median": round(sorted(mt)[len(mt) // 2], 4),
                            "masstree_over_expanse": round(ratio, 4),
                            "ci_lower": round(lo, 4), "ci_upper": round(hi, 4),
                            "verdict": verdict(lo, hi),
                        })
                        print(f"  {pillar:<12} str {dist:<9} N={n:<8} k={k:<5} ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}]")
                    else:
                        cell.update({"masstree_ns_per_op_median": None, "masstree_over_expanse": None,
                                     "ci_lower": None, "ci_upper": None, "verdict": "NOT_REPRESENTABLE_MASSTREE"})
                        print(f"  {pillar:<12} str {dist:<9} N={n:<8} k={k:<5} Expanse {exp_med:.2f} ns/op; "
                              f"Masstree: {head['masstree_not_representable']} keys beyond MASSTREE_MAXKEYLEN — column withheld")
                    cells.append(cell)
    return {"cells": cells}


def sweep_sensitivity(env: dict, quick: bool) -> dict:
    """§10.2 and §10.3: the same population shuffled, and the concurrent table.

    Published as its own table; never merged with the registered cells and
    never given a verdict against §6.
    """
    n = 10_000 if quick else 1_000_000
    memory, latency = [], []
    variants = [("sorted", "single"), ("shuffled", "single"), ("sorted", "concurrent")]
    for arm, dists in (("map", ["random"]), ("str", ["short", "prefixed"])):
        for dist in dists:
            for order, table in variants:
                if table == "concurrent" and dist == "prefixed":
                    continue
                rows = run_cell([str(binary("masstree_memory")), arm, dist, str(n), order, table], env)
                row = memory_row(rows[0])
                memory.append(row)
                print(f"  memory {arm:>3} {dist:<9} {order:<8} {table:<10} N={n:<9} Masstree {row['masstree_alloc_bytes_per_key']:.2f} "
                      f"(structural {row['masstree_structural_bytes_per_key']:.2f}, fill {row['masstree_leaf_fill']:.3f}, "
                      f"unsettled {row['masstree_unsettled_bytes_per_key']:.2f}) Expanse {row['expanse_alloc_bytes_per_key']:.2f} B/key")
                for pillar in ("lookup_hit", "insert"):
                    exe = binary("masstree_latency") if arm == "map" else binary("masstree_string_latency")
                    rows = run_cell([str(exe), pillar, dist, str(n), order, table], env)
                    mt = [r["masstree_ns_per_op"] for r in rows]
                    exp = [r["expanse_ns_per_op"] for r in rows]
                    ratio, lo, hi = bca_bootstrap_ratio_ci(mt, exp, num_resamples=2000, seed=42)
                    latency.append({
                        "workload_id": rows[0]["workload_id"], "pillar": pillar, "arm": arm, "dist": dist,
                        "order": order, "table": table, "population": rows[0]["population"], "rounds": len(rows),
                        "masstree_ns_per_op_median": round(sorted(mt)[len(mt) // 2], 4),
                        "expanse_ns_per_op_median": round(sorted(exp)[len(exp) // 2], 4),
                        "masstree_over_expanse": round(ratio, 4), "ci_lower": round(lo, 4), "ci_upper": round(hi, 4),
                        "verdict": verdict(lo, hi),
                        "rounds_raw": raw_rounds(rows, LATENCY_RAW),
                    })
                    print(f"  {pillar:<12} {arm:>3} {dist:<9} {order:<8} {table:<10} N={n:<8} ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}]")
    return {"memory": memory, "latency": latency}


def concurrent_cell(arm: str, writers: int, readers: int, env: dict) -> dict:
    rows = run_cell([str(binary("masstree_concurrent")), arm, str(writers), str(readers)], env)
    if not rows:
        raise RuntimeError(f"concurrent cell {arm} W={writers} R={readers} emitted no rows")
    return reduce_throughput(rows, arm, writers, readers)


def reduce_throughput(rows: list, arm: str, writers: int, readers: int) -> dict:
    """One cell's rows into medians, the BCa ratio interval and `rounds_raw`."""
    head = rows[0]
    cell = {"workload_id": head["workload_id"], "arm": arm, "dist": head["dist"],
            "writers": writers, "readers": readers, "prefill": head["prefill"],
            "fresh_keys": head["fresh_keys"], "rounds": len(rows),
            "cpus_allowed": head["cpus_allowed"], "pin_applied": head["pin_applied"],
            "rounds_raw": raw_rounds(rows, ("masstree_writer_mops", "expanse_writer_mops",
                                            "masstree_reader_mops", "expanse_reader_mops"))}
    for role in ("writer", "reader"):
        mt = [r[f"masstree_{role}_mops"] for r in rows if r[f"masstree_{role}_mops"] is not None]
        exp = [r[f"expanse_{role}_mops"] for r in rows if r[f"expanse_{role}_mops"] is not None]
        if not mt or not exp:
            continue
        # Expanse ÷ Masstree throughput: above 1.0 means Expanse is faster (§5).
        ratio, lo, hi = bca_bootstrap_ratio_ci(exp, mt, num_resamples=2000, seed=42)
        cell[f"masstree_{role}_mops_median"] = round(sorted(mt)[len(mt) // 2], 4)
        cell[f"expanse_{role}_mops_median"] = round(sorted(exp)[len(exp) // 2], 4)
        cell[f"{role}_expanse_over_masstree"] = round(ratio, 4)
        cell[f"{role}_ci_lower"] = round(lo, 4)
        cell[f"{role}_ci_upper"] = round(hi, 4)
        cell[f"{role}_verdict"] = verdict(lo, hi)
        print(f"  {role:<6} {arm:>3} W={writers:<2} R={readers:<2} Masstree {cell[f'masstree_{role}_mops_median']:>7.3f}  "
              f"Expanse {cell[f'expanse_{role}_mops_median']:>7.3f} Mops/s  ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}] {cell[f'{role}_verdict']}")
    return cell


def _share(num, den):
    """`num / den`, or `None` when there were no operations to take a share of."""
    return None if den == 0 else num / den


def health_shares(r: dict, readers: int) -> dict:
    """The derived ratios of one health round (#568 Step 0).

    `spin_time_share` is the readers' time inside `SeqVersion::sample` as a
    share of their wall time: cycles spent spinning, converted through the
    host's cycle rate, over `readers × reader_elapsed_s`. Zero by definition
    with no readers. Every other share is a counter over the ops that could
    have raised it; a share of zero ops is `None`, not zero — the string arm
    before #744 had `read_ops = 0` (§10.5), and its shares were not 0%.
    """
    if readers == 0:
        spin = 0.0
    else:
        spin_s = _share(r["sample_spin_cycles"], r["cycles_hz"])
        spin = _share(spin_s, readers * r["reader_elapsed_s"]) if spin_s is not None else None
    return {
        "locked_share": _share(r["locked_reads"], r["read_ops"]),
        "unconditional_share": _share(r["locked_reads"] - r["read_fallbacks"], r["read_ops"]),
        "handoffs_per_write": _share(r["handoffs"], r["write_ops"]),
        "replacements_per_write": _share(r["branch_replacements"], r["write_ops"]),
        "deep_cascade_share": _share(r["deep_cascades"], r["write_ops"]),
        "root_rewrite_share": _share(r["root_rewrites"], r["write_ops"]),
        "spin_time_share": spin,
    }


def med_range(vals: list) -> dict | None:
    """Median with range; `None` if any round could not produce the value."""
    if any(v is None for v in vals):
        return None
    vals = sorted(vals)
    return {"median": vals[len(vals) // 2], "min": vals[0], "max": vals[-1]}


def reduce_health(rows: list, arm: str, writers: int, readers: int) -> dict:
    """One health cell from its rounds: every counter and every derived share.

    Fails loud on a round missing any counter in `HEALTH_RAW` (AGENTS.md §8.1)
    — the harness emits all of them, and a row without one is a harness or
    build mismatch, not a zero.
    """
    if not rows:
        raise RuntimeError(f"health cell {arm} W={writers} R={readers} emitted no rows")
    for r in rows:
        missing = [k for k in HEALTH_RAW if k not in r]
        if missing:
            raise RuntimeError(
                f"health cell {arm} W={writers} R={readers} round {r.get('round')} lacks "
                f"{missing} — every health counter must be emitted; a missing counter is "
                f"not 0 (§8.1)"
            )
    cell = {"workload_id": rows[0]["workload_id"], "arm": arm, "dist": rows[0]["dist"],
            "writers": writers, "readers": readers, "rounds": len(rows)}
    for key in HEALTH_RAW:
        cell[key] = med_range([r[key] for r in rows])
    shares = [health_shares(r, readers) for r in rows]
    for key in HEALTH_DERIVED:
        cell[key] = med_range([s[key] for s in shares])
    cell["cpus_allowed"] = rows[0]["cpus_allowed"]
    cell["pin_applied"] = rows[0]["pin_applied"]
    cell["rounds_raw"] = raw_rounds(rows, HEALTH_RAW)
    cell["starvation_flag"] = cell["fallback_share"]["median"] >= 0.01
    return cell


def health_cell(arm: str, writers: int, readers: int, env: dict) -> dict:
    exe = OCC_STATS_TARGET / "release" / "masstree_concurrent"
    rows = run_cell([str(exe), arm, str(writers), str(readers), "--health"], env)
    cell = reduce_health(rows, arm, writers, readers)
    locked = "n/a" if cell["locked_share"] is None else f"{cell['locked_share']['median']:.4%}"
    spin = "n/a" if cell["spin_time_share"] is None else f"{cell['spin_time_share']['median']:.2%}"
    print(f"  health {arm:>3} W={writers:<2} R={readers:<2} restart {cell['restart_share']['median']:.4%}  "
          f"fallback {cell['fallback_share']['median']:.4%}  locked {locked}  spin-time {spin}"
          f"{'  STARVATION' if cell['starvation_flag'] else ''}")
    return cell


def ab_cell(arm: str, writers: int, readers: int, base_bin: Path, env: dict) -> dict:
    """One cell with the base and head builds interleaved round by round.

    Each build's rounds are reduced with `reduce_throughput`, exactly as a
    single-build cell is; the cell carries both reductions and every round
    of both builds (`rounds_raw`, tagged `build`).
    """
    head_bin = binary("masstree_concurrent")
    base_rows, head_rows = interleave(base_bin, head_bin, [arm, str(writers), str(readers)],
                                      AB_ROUNDS, env, run_cell)
    print(f"  base build ({len(base_rows)} rounds):")
    base = reduce_throughput(base_rows, arm, writers, readers)
    print(f"  head build ({len(head_rows)} rounds):")
    head = reduce_throughput(head_rows, arm, writers, readers)
    cell = {k: v for k, v in head.items() if k not in ("rounds_raw",) and not k.endswith(("_median", "_lower", "_upper", "_verdict")) and "_over_" not in k}
    for build in (base, head):
        build.pop("rounds_raw", None)
    cell["rounds"] = AB_ROUNDS
    cell["base"] = base
    cell["head"] = head
    cell["rounds_raw"] = raw_rounds(base_rows + head_rows, ("build", *("masstree_writer_mops", "expanse_writer_mops", "masstree_reader_mops", "expanse_reader_mops")))
    return cell


def sweep_ab(env: dict, prov: dict, base_bin: Path) -> dict:
    """The two-commit sweep (#568 PR 3): C1 at every writer count, C2 at
    W = 0 and W = 1 with eight readers, both builds interleaved per round;
    then the head build's health cell at W = 1 R = 8 (the base build's
    counters are the committed Step 0 artifacts)."""

    def attributed(kind: str, arm: str, w: int, r: int) -> dict:
        start = begin_cell(prov, f"cell:{arm}:W{w}:R{r}")
        if kind == "ab":
            cell = ab_cell(arm, w, r, base_bin, env)
        else:
            cell = health_cell(arm, w, r, env)
        cell["load"] = end_cell(start)
        return cell

    throughput, health = [], []
    for arm in CONCURRENT_ARMS:
        print(f"\n  C1 write scaling, base vs head — {arm} arm")
        for w in CONCURRENT_WRITE_SCALING:
            c = attributed("ab", arm, w, 0)
            c["pillar"] = "C1"
            throughput.append(c)
        print(f"\n  C2 readers alongside writers, base vs head — {arm} arm")
        for w in AB_MIXED_WRITERS:
            c = attributed("ab", arm, w, CONCURRENT_MIXED_READERS)
            c["pillar"] = "C2"
            throughput.append(c)
    for arm in CONCURRENT_ARMS:
        print(f"\n  H protocol health at the head build — {arm} arm (write scaling, R=0)")
        for w in CONCURRENT_WRITE_SCALING:
            health.append(attributed("health", arm, w, 0))
        print(f"\n  H protocol health at the head build — {arm} arm W=1 R={CONCURRENT_MIXED_READERS}")
        health.append(attributed("health", arm, 1, CONCURRENT_MIXED_READERS))
    return {"throughput": throughput, "health": health}


def sweep_concurrent(env: dict, quick: bool, prov: dict) -> dict:
    """MC1 / MC2, one process per cell.

    A load snapshot is taken into `prov` before every throughput and health
    cell (#568 Step 0) and the cell carries its own `load` block — the host's
    busy CPU over the cell, the runner's own children's share of it, and the
    difference — so a cell that ran beside something else is visible as that
    cell, not as a whole-sweep average.
    """
    write_w = [1, 4] if quick else CONCURRENT_WRITE_SCALING
    mixed_w = [0, 4] if quick else CONCURRENT_MIXED_WRITERS
    health_w = [4] if quick else CONCURRENT_HEALTH_WRITERS
    lambdas = LAMBDA_TARGETS[:2] if quick else LAMBDA_TARGETS

    def attributed(kind: str, arm: str, w: int, r: int) -> dict:
        start = begin_cell(prov, f"cell:{arm}:W{w}:R{r}")
        cell = (concurrent_cell if kind == "throughput" else health_cell)(arm, w, r, env)
        cell["load"] = end_cell(start)
        return cell

    throughput, health, memory = [], [], []
    for arm in CONCURRENT_ARMS:
        print(f"\n  C1 write scaling — {arm} arm")
        for w in write_w:
            c = attributed("throughput", arm, w, 0)
            c["pillar"] = "C1"
            throughput.append(c)
        print(f"\n  C2 readers alongside writers — {arm} arm")
        for w in mixed_w:
            c = attributed("throughput", arm, w, CONCURRENT_MIXED_READERS)
            c["pillar"] = "C2"
            throughput.append(c)
    for arm in CONCURRENT_ARMS:
        print(f"\n  H protocol health — {arm} arm (occ-stats build, Expanse side only)")
        for w in health_w:
            health.append(attributed("health", arm, w, CONCURRENT_MIXED_READERS))
    print("\n  M memory — Masstree single writer vs SyncExpanseMap, build-only")
    for lam in lambdas:
        n = population_for_lambda(lam)
        rows = run_cell([str(binary("masstree_memory")), "sync", "random", str(n)], env)
        row = memory_row(rows[0])
        row["lambda_target"] = lam
        memory.append(row)
        print(f"  memory sync λ≈{lam:<5} N={n:<9} allocator: Masstree {row['masstree_alloc_bytes_per_key']:.2f} "
              f"[{row['masstree_label']}] SyncExpanseMap {row['expanse_alloc_bytes_per_key']:.2f} B/key")
    return {"throughput": throughput, "health": health, "memory": memory}


# --------------------------------------------------------------------------
# self-test: the health reduction, driven by a fixture (no harness needed)
# --------------------------------------------------------------------------

def _health_row(round_: int, **over) -> dict:
    """One health round with every counter, at values whose shares are exact."""
    row = {
        "workload_id": "masstree_conc_map_64bit", "role": "health", "arm": "map",
        "dist": "random", "writers": 1, "readers": 8, "round": round_,
        "read_ops": 1000, "read_attempts": 1050, "read_fallbacks": 10,
        "sample_spins": 900, "write_ops": 200, "locked_reads": 100,
        "handoffs": 50, "retired": 300, "freed_raw": 250,
        "sample_spin_cycles": 3_000_000_000, "branch_replacements": 400,
        "deep_cascades": 20, "root_rewrites": 2, "cycles_hz": 3_000_000_000,
        "reader_elapsed_s": 0.5, "writer_elapsed_s": 0.4,
        "lock_restarts": 0, "lock_spins": 0, "lock_hold_cycles": 0, "lock_fallbacks": 0,
        "restart_share": 50 / 1050, "fallback_share": 0.01,
        "cpus_allowed": "0-15", "pin_applied": "0-15",
    }
    row.update(over)
    return row


def _self_test() -> int:
    failures = []

    def check(name, cond):
        if not cond:
            failures.append(name)

    # Three rounds whose locked share is 0.10, 0.12, 0.08: the published median
    # must be the median of per-round ratios (0.10), not a ratio of medians.
    rows = [_health_row(0), _health_row(1, locked_reads=120), _health_row(2, locked_reads=80)]
    cell = reduce_health(rows, "map", 1, 8)
    want = {
        "locked_share": 100 / 1000,
        "unconditional_share": (100 - 10) / 1000,
        "handoffs_per_write": 50 / 200,
        "replacements_per_write": 400 / 200,
        "deep_cascade_share": 20 / 200,
        "root_rewrite_share": 2 / 200,
        # 3e9 cycles at 3e9 Hz = 1 s spinning, over 8 readers × 0.5 s.
        "spin_time_share": 1.0 / (8 * 0.5),
    }
    for key, val in want.items():
        got = cell[key]
        check(f"{key} median {got and got['median']} != {val}",
              got is not None and abs(got["median"] - val) < 1e-12)
    check("locked_share range is [0.08, 0.12]",
          abs(cell["locked_share"]["min"] - 0.08) < 1e-12 and abs(cell["locked_share"]["max"] - 0.12) < 1e-12)
    check("every counter is summarised", all(isinstance(cell[k], dict) for k in HEALTH_RAW))
    check("rounds_raw carries every counter on every round",
          all(all(k in r for k in HEALTH_RAW) for r in cell["rounds_raw"]) and len(cell["rounds_raw"]) == 3)
    check("starvation flag fires at 1%", cell["starvation_flag"] is True)
    check("the dist survives", cell["dist"] == "random")
    check("locked_reads median is the raw counter", cell["locked_reads"]["median"] == 100)

    # No readers: the spin-time share is zero by definition, not a division.
    alone = reduce_health([_health_row(0, readers=0, reader_elapsed_s=0.0)], "map", 1, 0)
    check("spin_time_share is 0.0 with no readers", alone["spin_time_share"] == {"median": 0.0, "min": 0.0, "max": 0.0})

    # No read ops at all (the §10.5 string reader before #744): a share of
    # nothing is None, never 0.
    unread = reduce_health([_health_row(0, read_ops=0, locked_reads=0, read_fallbacks=0)], "str", 1, 8)
    check("locked_share is None with no read ops", unread["locked_share"] is None)

    # THE NEGATIVE CASE: a row without `locked_reads` — the counter the runner
    # silently dropped until #568 — is an error naming the field, not a zero.
    short = [_health_row(0), _health_row(1)]
    del short[1]["locked_reads"]
    try:
        reduce_health(short, "map", 1, 8)
        failures.append("a row missing locked_reads was reduced instead of refused")
    except RuntimeError as exc:
        check("the refusal names locked_reads and the round", "locked_reads" in str(exc) and "round 1" in str(exc))

    for msg in failures:
        print(f"  FAIL {msg}")
    if failures:
        print(f"run_all.py --self-test: {len(failures)} failure(s)")
        return 1
    print("run_all.py --self-test: all checks passed")
    return 0


def _flag_value(name: str) -> str | None:
    """`--flag VALUE` from argv, or None."""
    if name not in sys.argv:
        return None
    i = sys.argv.index(name)
    if i + 1 >= len(sys.argv):
        print(f"{name} needs a value", file=sys.stderr)
        sys.exit(2)
    return sys.argv[i + 1]


def run_ab(env: dict, provenance: dict, out_dir: Path, base_bin: Path, base_commit: str) -> int:
    """The two-commit concurrent sweep into results/baseline_concurrent_ab.json."""
    print(f"\n[ab] two-commit concurrent sweep: base {base_commit} ({base_bin}) vs head {provenance['commit']}")
    prov = dict(provenance)
    prov["loads"] = [load_snapshot("start")]
    prov["ab"] = ab_provenance(base_bin, base_commit, provenance["commit"], AB_ROUNDS)
    prov["estimators"] = estimators(
        provenance["estimators"]["ratio"] + "; each of `base` and `head` is that reduction over its own "
        "rounds, the two builds having alternated round by round inside the cell (scripts/bench_ab.py)")
    res = sweep_ab(env, prov, base_bin)
    add_load(prov, "after-ab")
    out = out_dir / "baseline_concurrent_ab.json"
    out.write_text(json.dumps({"provenance": prov, **res}, indent=2) + "\n")
    print(f"wrote {out}")
    start_load = prov["loads"][0]["load1"]
    if start_load > 2.0:
        print("WARNING: load average above 2 at the sweep start — another process was running (docs/BENCHMARKING.md rule 2)")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return _self_test()
    quick = "--quick" in sys.argv
    concurrent = "--concurrent" in sys.argv or "--only-concurrent" in sys.argv
    only_concurrent = "--only-concurrent" in sys.argv
    ab_base_bin, ab_base_commit = _flag_value("--ab-base-bin"), _flag_value("--ab-base-commit")
    if (ab_base_bin is None) != (ab_base_commit is None):
        print("--ab-base-bin and --ab-base-commit go together", file=sys.stderr)
        return 2
    if ab_base_bin is not None:
        if quick or (concurrent and not only_concurrent):
            print("the two-commit mode is its own sweep: no --quick, no single-threaded phases", file=sys.stderr)
            return 2
        concurrent = only_concurrent = True
    env = dict(os.environ)
    env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-cpu=haswell"

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR
    out_dir.mkdir(parents=True, exist_ok=True)
    if quick:
        print("QUICK MODE — reduced sweep, writing to gitignored results/quick/ (§8.5)")

    if ab_base_bin is None:
        build(env)
    if concurrent:
        build_concurrent(env)

    provenance = {
        "suite": "masstree_comparison", "issue": 661, "commit": git_sha(REPO_ROOT),
        "masstree_commit": MASSTREE_COMMIT,
        "cpu": platform.processor() or platform.machine(), "platform": platform.platform(),
        "host": host_facts(),
        "estimators": estimators(
            "mean(Masstree rounds) / mean(Expanse rounds) — or Expanse / Masstree for throughput — "
            "with a two-sample BCa 95% interval (scripts/bca_bootstrap.py)"
        ),
        "rustflags": env["RUSTFLAGS"].strip(),
        "cxx_flags": "-march=haswell -O3 -std=c++17 -DNDEBUG (assertions, preconditions and invariants off)",
        "allocator": "glibc malloc, superpages on (METHODOLOGY §3.3)",
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "loads": [load_snapshot("start")], "quick": quick,
    }

    # The two-commit sweep needs only the concurrent binaries: no validate gate
    # (that binary is not built) and no single-threaded phase.
    if ab_base_bin is not None:
        return run_ab(env, provenance, out_dir, Path(ab_base_bin), ab_base_commit)

    validate_log = validate(env)
    add_load(provenance, "after-validate")
    (out_dir / "validate.log").write_text(validate_log)

    # The single-threaded phases run first, on the host as the start snapshot
    # found it. The concurrent sweep runs LAST: its own 16 threads push the
    # 1-minute load average to ~5 and it takes minutes to decay, so anything
    # timed after it would carry a load series that looks like contamination
    # and is only the sweep's own decay (docs/BENCHMARKING.md rule 2 gates
    # the concurrent phase on the snapshot taken as it starts).
    if not only_concurrent:
        print("\n[1/5] memory — integer map (λ sweep and structured distributions)")
        memory = sweep_memory(env, quick)
        add_load(provenance, "after-memory")
        print("\n[2/5] memory — string map (population sweep)")
        smem = sweep_string_memory(env, quick)
        add_load(provenance, "after-string-memory")
        print("\n[3/5] latency — integer map")
        latency = sweep_latency(env, quick)
        add_load(provenance, "after-latency")
        print("\n[4/5] latency — string map")
        slat = sweep_string_latency(env, quick)
        add_load(provenance, "after-string-latency")
        print("\n[5/5] sensitivity — insertion order (§10.2) and the concurrent table (§10.3)")
        order = sweep_sensitivity(env, quick)
        add_load(provenance, "end")

        (out_dir / "baseline_memory.json").write_text(json.dumps({"provenance": provenance, **memory}, indent=2) + "\n")
        (out_dir / "baseline_string_memory.json").write_text(json.dumps({"provenance": provenance, **smem}, indent=2) + "\n")
        (out_dir / "baseline_latency.json").write_text(json.dumps({"provenance": provenance, **latency}, indent=2) + "\n")
        (out_dir / "baseline_string_latency.json").write_text(json.dumps({"provenance": provenance, **slat}, indent=2) + "\n")
        (out_dir / "baseline_sensitivity.json").write_text(json.dumps({"provenance": provenance, **order}, indent=2) + "\n")
        loads = [s["load1"] for s in provenance["loads"]]
        print(f"\nload average across the single-threaded phases: {loads}")
        if max(loads) - min(loads) > 2.0:
            print("WARNING: load shifted by more than 2 during the run — the comparison is contaminated (docs/BENCHMARKING.md rule 2)")
        print(f"wrote {out_dir}/baseline_memory.json, baseline_string_memory.json, baseline_latency.json, "
              f"baseline_string_latency.json, baseline_sensitivity.json, validate.log")

    if concurrent:
        print("\n[concurrent] MC1 / MC2 — one process per cell, threads inside the P-core pin")
        conc_prov = dict(provenance)
        conc_prov["loads"] = [load_snapshot("start")]
        conc = sweep_concurrent(env, quick, conc_prov)
        add_load(conc_prov, "after-concurrent")
        (out_dir / "baseline_concurrent.json").write_text(json.dumps({"provenance": conc_prov, **conc}, indent=2) + "\n")
        print(f"wrote {out_dir}/baseline_concurrent.json")
        start_load = conc_prov["loads"][0]["load1"]
        print(f"\nconcurrent sweep: load average at start {start_load} (after: {conc_prov['loads'][-1]['load1']} — "
              f"includes the sweep's own threads, which is why it runs last)")
        if start_load > 2.0:
            print("WARNING: load average above 2 at the concurrent start — another process was running (docs/BENCHMARKING.md rule 2)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
