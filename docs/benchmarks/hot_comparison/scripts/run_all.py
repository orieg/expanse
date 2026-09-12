#!/usr/bin/env python3
"""Master runner for the HOT (Height Optimized Trie) comparison suite (#660).

Drives every cell of the suite, one process per cell, and harvests BCa 95%
intervals for the wall-clock pillars.

Two things about this runner differ from the other suites', both for reasons
recorded in ``docs/benchmarks/hot_comparison/METHODOLOGY.md``:

1. **One process per cell.** HOT's node pool is a function-local ``static``
   (§9.2), so a build in one process leaves reusable nodes on its free lists and
   the next build in that process undercounts by up to 3.3x. Every cell is its
   own invocation; nothing is batched to save startup.

2. **The memory pillar sweeps λ, not N** (§9.6). Per-key cost for this engine is
   a sawtooth in expanse occupancy, so a single-population cell is a cherry-pick
   whichever side of the cascade it lands on. The sweep picks λ targets and
   computes the population each arm needs to reach them — which is also what
   makes the two arms comparable despite Arm A's 63-bit domain, since halving
   the keyspace is exactly a doubling of density.
"""

import json
import os
import platform
import subprocess
import sys
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

# λ targets spanning the LEAF_CAP cascade (§9.4: LEAF_CAP = 32, so the cascade
# sits around λ ≈ 30). Both arms are driven to the same occupancies, which is
# the axis §9.6 publishes against.
LAMBDA_TARGETS = [1.0, 2.0, 4.0, 8.0, 15.0, 23.0, 30.0, 38.0, 46.0, 61.0]

# Latency cells stay on the populations `art_comparison/` used, so the two
# suites' *Expanse* columns are relatable (§7.5).
LATENCY_POPULATIONS = [10_000, 100_000, 1_000_000]
DISTRIBUTIONS = ["sequential", "clustered", "sparse", "random"]
SCAN_K = [10, 100, 1000]
ARMS = ["set", "map"]

# The §12.2 sensitivity pair (#733). The shared generator sorts the population
# and every registered cell in this suite was built in that order, so each
# insert verdict is a sorted-order verdict. This pair measures the same
# population in both build orders and is published as its own table, never
# merged with the registered cells and never given a verdict against §6.
SENSITIVITY_N = 1_000_000
SENSITIVITY_DIST = "random"
SENSITIVITY_ORDERS = ["sorted", "shuffled"]
SENSITIVITY_PILLARS = ["lookup_hit", "insert"]

# The concurrent arm (#692, METHODOLOGY.md §11.4). Every cell keeps
# writers + readers <= 16 so no thread can leave the P-core pin (decision 3).
CONCURRENT_WRITE_SCALING = [1, 2, 4, 8, 16]            # C1: W, R = 0
CONCURRENT_MIXED_WRITERS = [0, 1, 2, 4, 8]             # C2: W, R = 8
CONCURRENT_MIXED_READERS = 8
# Two-commit mode (#568 PR 3, docs/BENCHMARKING.md rule 18): the readers-alone
# reference and the gate cell at R = 8; C1 stays the full writer-count control.
AB_MIXED_WRITERS = [0, 1]
# One process per round per build, so the pair sees the same host minute.
AB_ROUNDS = 15
CONCURRENT_HEALTH_WRITERS = [1, 2, 4, 8]               # H: the C2 cells with writers
CONCURRENT_ARMS = ["set", "map"]

# The per-round samples every latency cell keeps verbatim (#732), so a published
# median and ratio can be recomputed from the artifact.
LATENCY_RAW = ("first_arm", "order", "hot_ns_per_op", "expanse_ns_per_op")
# The concurrent cells summarise their rounds into medians and a BCa interval
# exactly as the latency cells do, so they owe the same raw rows (#732): a
# published median cannot be recomputed from the artifact without them.
CONCURRENT_RAW = ("rowex_writer_mops", "expanse_writer_mops",
                  "rowex_reader_mops", "expanse_reader_mops")
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
CONCURRENT_MEMORY_ARMS = {"set": "rowex_set", "map": "rowex_map"}
# A second target dir for the diagnostic build, so the two feature sets can
# never overwrite each other's binary (decision 5).
OCC_STATS_TARGET = CRATE.parent / "target-occ-stats"


def keyspace_bits(arm: str) -> int:
    """Arm A is restricted to 63 bits by HOT's inline payload (§9.6)."""
    return 63 if arm == "set" else 64


def population_for_lambda(arm: str, lam: float) -> int:
    """Population that puts `arm` at occupancy `lam`."""
    expanses = 2 ** (16 - (64 - keyspace_bits(arm)))
    return max(1000, int(round(lam * expanses)))


def build(env: dict) -> None:
    """Builds both binaries once, at the ISA target §3.3 binds both arms to."""
    print("building hot_memory_curve and hot_latency at -C target-cpu=haswell ...")
    subprocess.run(
        ["cargo", "build", "--release", "--manifest-path", str(CRATE),
         "--bin", "hot_memory_curve", "--bin", "hot_latency"],
        check=True, env=env,
    )


def binary(name: str) -> Path:
    target = os.environ.get("CARGO_TARGET_DIR")
    root = Path(target) if target else (CRATE.parent / "target")
    return root / "release" / name


def run_cell(args: list, env: dict) -> list:
    """Runs one cell in its own process and returns its JSON lines."""
    proc = subprocess.run(args, capture_output=True, text=True, env=env)
    if proc.returncode != 0:
        raise RuntimeError(
            f"cell failed ({proc.returncode}): {' '.join(str(a) for a in args)}\n{proc.stderr.strip()}"
        )
    return [json.loads(line) for line in proc.stdout.splitlines() if line.startswith("{")]


def sweep_memory(env: dict, quick: bool) -> dict:
    targets = LAMBDA_TARGETS[:4] if quick else LAMBDA_TARGETS
    cells = []
    for arm in ARMS:
        for lam in targets:
            n = population_for_lambda(arm, lam)
            rows = run_cell([str(binary("hot_memory_curve")), arm, str(n)], env)
            if len(rows) != 1:
                raise RuntimeError(f"memory cell emitted {len(rows)} rows, expected 1")
            row = rows[0]
            row["lambda_target"] = lam
            cells.append(row)
            print(f"  memory {arm:>3} λ≈{lam:<5} N={n:<9} "
                  f"HOT {row['hot_alloc_bytes_per_key']:.2f}  "
                  f"Expanse {row['expanse_alloc_bytes_per_key']:.2f} B/key")
    return {"cells": cells}


def payload_delta_check(cells: list) -> dict:
    """What the two arms can actually check about each other: the payload delta.

    Arm A pairs a *set* against HOT's inline-value model; Arm B pairs a *map*
    against HOT's pointer model. They hold different payloads, so their Expanse
    curves cannot superimpose and a ratio between them is not a density test —
    at matched λ the map should sit a value word above the set, and the *gap*,
    not the ratio, is what should stay flat. The ratio drifts purely because the
    set's base cost falls with density while the added word does not.

    The density model's real falsifier is same-flavour and cross-keyspace —
    ``ExpanseSet`` at 63 bits and λ must equal ``ExpanseSet`` at 64 bits and the
    same λ — which ``keyspace_density_probe`` measures directly (§9.4: 63-bit at
    N reproduces 64-bit at 2N to two decimals). That check is not repeated here;
    this one reports the payload delta and flags it if it stops being flat.
    """
    by_lambda = {}
    for c in cells:
        by_lambda.setdefault(c["lambda_target"], {})[c["arm"]] = c
    deltas = []
    for lam, arms in sorted(by_lambda.items()):
        if "set" in arms and "map" in arms:
            s = arms["set"]["expanse_mem_used_bytes_per_key"]
            m = arms["map"]["expanse_mem_used_bytes_per_key"]
            deltas.append({"lambda": lam, "map_minus_set": round(m - s, 4)})
    spread = None
    if len(deltas) >= 2:
        vals = [d["map_minus_set"] for d in deltas]
        spread = round(max(vals) - min(vals), 4)
    return {
        "per_lambda": deltas,
        "spread_bytes": spread,
        "note": "map - set at matched occupancy; expected ≈ one value word and flat. "
                "Not a density check — the arms hold different payloads. The density "
                "falsifier is same-flavour cross-keyspace, measured by keyspace_density_probe.",
    }


def sweep_latency(env: dict, quick: bool) -> dict:
    pops = [10_000] if quick else LATENCY_POPULATIONS
    dists = ["random"] if quick else DISTRIBUTIONS
    cells = []
    for arm in ARMS:
        for pillar in ["lookup_hit", "lookup_miss", "insert", "scan"]:
            for dist in dists:
                for n in pops:
                    ks = SCAN_K if pillar == "scan" else [0]
                    for k in ks:
                        args = [str(binary("hot_latency")), arm, pillar, dist, str(n)]
                        if pillar == "scan":
                            args.append(str(k))
                        rows = run_cell(args, env)
                        hot = [r["hot_ns_per_op"] for r in rows]
                        exp = [r["expanse_ns_per_op"] for r in rows]
                        # Ratio of two independently sampled means, gated on the
                        # CI lower bound (§8.4). Above 1.0 means Expanse is
                        # faster, since HOT is the numerator.
                        ratio, lo, hi = bca_bootstrap_ratio_ci(hot, exp, num_resamples=2000, seed=42)
                        head = rows[0]
                        cells.append({
                            "workload_id": head["workload_id"],
                            "pillar": pillar, "arm": arm, "dist": dist,
                            "keyspace_bits": head["keyspace_bits"],
                            "population": head["population"],
                            "lambda": head["lambda"], "scan_k": k,
                            "rounds": len(rows),
                            "hot_ns_per_op_median": round(sorted(hot)[len(hot) // 2], 4),
                            "expanse_ns_per_op_median": round(sorted(exp)[len(exp) // 2], 4),
                            "hot_over_expanse": round(ratio, 4),
                            "ci_lower": round(lo, 4), "ci_upper": round(hi, 4),
                            # A cell whose interval spans parity claims no winner.
                            "verdict": ("BOUNDARY_RESULT" if lo <= 1.0 <= hi
                                        else ("expanse" if lo > 1.0 else "hot")),
                            "rounds_raw": raw_rounds(rows, LATENCY_RAW),
                        })
                        print(f"  {pillar:<12} {arm:>3} {dist:<10} N={n:<8} k={k:<5} "
                              f"ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}]")
    return {"cells": cells}


def sweep_sensitivity(env: dict, quick: bool) -> dict:
    """§12.2: the same population built sorted and shuffled, both instruments.

    Published as its own table. These cells carry no verdict against the §6
    pre-registration — the registered rows were locked on sorted order and
    reconciling them against a different workload in place is exactly what
    AGENTS.md §8.7 forbids.
    """
    n = 10_000 if quick else SENSITIVITY_N
    memory, latency = [], []
    for arm in ARMS:
        for order in SENSITIVITY_ORDERS:
            rows = run_cell([str(binary("hot_memory_curve")), arm, str(n), order], env)
            if len(rows) != 1:
                raise RuntimeError(f"sensitivity memory cell emitted {len(rows)} rows, expected 1")
            memory.append(rows[0])
            print(f"  memory {arm:>3} {order:<8} N={n:<9} "
                  f"HOT {rows[0]['hot_alloc_bytes_per_key']:.2f}  "
                  f"Expanse {rows[0]['expanse_alloc_bytes_per_key']:.2f} B/key  "
                  f"mem_used {rows[0]['expanse_mem_used_bytes_per_key']:.2f}")
            for pillar in SENSITIVITY_PILLARS:
                rows = run_cell([str(binary("hot_latency")), arm, pillar,
                                 SENSITIVITY_DIST, str(n), order], env)
                hot = [r["hot_ns_per_op"] for r in rows]
                exp = [r["expanse_ns_per_op"] for r in rows]
                ratio, lo, hi = bca_bootstrap_ratio_ci(hot, exp, num_resamples=2000, seed=42)
                head = rows[0]
                latency.append({
                    "workload_id": head["workload_id"], "pillar": pillar, "arm": arm,
                    "dist": SENSITIVITY_DIST, "order": order,
                    "keyspace_bits": head["keyspace_bits"], "population": head["population"],
                    "lambda": head["lambda"], "rounds": len(rows),
                    "hot_ns_per_op_median": round(sorted(hot)[len(hot) // 2], 4),
                    "expanse_ns_per_op_median": round(sorted(exp)[len(exp) // 2], 4),
                    "hot_over_expanse": round(ratio, 4),
                    "ci_lower": round(lo, 4), "ci_upper": round(hi, 4),
                    "verdict": ("BOUNDARY_RESULT" if lo <= 1.0 <= hi
                                else ("expanse" if lo > 1.0 else "hot")),
                    "rounds_raw": raw_rounds(rows, LATENCY_RAW),
                })
                print(f"  {pillar:<12} {arm:>3} {order:<8} N={n:<8} "
                      f"ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}]")
    return {"memory": memory, "latency": latency}


def build_concurrent_binaries(env: dict) -> None:
    """Two builds, never one (§11.3 decision 5).

    The default build carries the throughput cells and the ROWEX memory arms;
    the `occ-stats` build carries the health cells and refuses to time anything.
    They go to separate target dirs so neither can overwrite the other.
    """
    print("building hot_concurrent and hot_memory_curve with --features rowex "
          "(libtbb is built from HOT's nested submodule on first use) ...")
    subprocess.run(
        ["cargo", "build", "--release", "--manifest-path", str(CRATE),
         "--features", "rowex", "--bin", "hot_concurrent", "--bin", "hot_memory_curve"],
        check=True, env=env,
    )
    print("building the diagnostic hot_concurrent with --features rowex,occ-stats ...")
    occ_env = dict(env)
    occ_env["CARGO_TARGET_DIR"] = str(OCC_STATS_TARGET)
    subprocess.run(
        ["cargo", "build", "--release", "--manifest-path", str(CRATE),
         "--features", "rowex,occ-stats", "--bin", "hot_concurrent"],
        check=True, env=occ_env,
    )


def concurrent_cell(arm: str, writers: int, readers: int, env: dict) -> dict:
    """One throughput cell in its own process, harvested into BCa intervals.

    Ratios are Expanse ÷ ROWEX throughput, so — as everywhere in this suite —
    above 1.000 means Expanse is faster (§11.4).
    """
    rows = run_cell([str(binary("hot_concurrent")), arm, str(writers), str(readers)], env)
    if not rows:
        raise RuntimeError(f"concurrent cell {arm} W={writers} R={readers} emitted no rows")
    return reduce_throughput(rows, arm, writers, readers)


def reduce_throughput(rows: list, arm: str, writers: int, readers: int) -> dict:
    """One cell's rows into medians, the BCa ratio interval and `rounds_raw`."""
    head = rows[0]
    cell = {
        "workload_id": head["workload_id"], "arm": arm,
        "writers": writers, "readers": readers,
        "keyspace_bits": head["keyspace_bits"], "prefill": head["prefill"],
        "fresh_keys": head["fresh_keys"], "rounds": len(rows),
        "cpus_allowed": head["cpus_allowed"], "pin_applied": head["pin_applied"],
        "rounds_raw": raw_rounds(rows, CONCURRENT_RAW),
    }
    for role in ("writer", "reader"):
        hot = [r[f"rowex_{role}_mops"] for r in rows if r[f"rowex_{role}_mops"] is not None]
        exp = [r[f"expanse_{role}_mops"] for r in rows if r[f"expanse_{role}_mops"] is not None]
        if not hot or not exp:
            continue
        ratio, lo, hi = bca_bootstrap_ratio_ci(exp, hot, num_resamples=2000, seed=42)
        cell[f"rowex_{role}_mops_median"] = round(sorted(hot)[len(hot) // 2], 4)
        cell[f"expanse_{role}_mops_median"] = round(sorted(exp)[len(exp) // 2], 4)
        cell[f"{role}_expanse_over_rowex"] = round(ratio, 4)
        cell[f"{role}_ci_lower"] = round(lo, 4)
        cell[f"{role}_ci_upper"] = round(hi, 4)
        cell[f"{role}_verdict"] = ("BOUNDARY_RESULT" if lo <= 1.0 <= hi
                                   else ("expanse" if lo > 1.0 else "rowex"))
        print(f"  {role:<6} {arm:>3} W={writers:<2} R={readers:<2} "
              f"ROWEX {cell[f'rowex_{role}_mops_median']:>7.3f}  "
              f"Expanse {cell[f'expanse_{role}_mops_median']:>7.3f} Mops/s  "
              f"ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}] {cell[f'{role}_verdict']}")
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
    have raised it; a share of zero ops is `None`, not zero.
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
    cell = {
        "workload_id": rows[0]["workload_id"], "arm": arm,
        "writers": writers, "readers": readers, "rounds": len(rows),
    }
    for key in HEALTH_RAW:
        cell[key] = med_range([r[key] for r in rows])
    shares = [health_shares(r, readers) for r in rows]
    for key in HEALTH_DERIVED:
        cell[key] = med_range([s[key] for s in shares])
    cell["cpus_allowed"] = rows[0]["cpus_allowed"]
    cell["pin_applied"] = rows[0]["pin_applied"]
    cell["rounds_raw"] = raw_rounds(rows, HEALTH_RAW)
    # The §11.5.3 falsifier, evaluated on the median: 1% or more is reader
    # starvation and is reported as a protocol-health finding.
    cell["starvation_flag"] = cell["fallback_share"]["median"] >= 0.01
    return cell


def health_cell(arm: str, writers: int, readers: int, env: dict) -> dict:
    """Event ratios from the diagnostic build; nothing here is a timing."""
    occ_env = dict(env)
    exe = OCC_STATS_TARGET / "release" / "hot_concurrent"
    rows = run_cell([str(exe), arm, str(writers), str(readers), "--health"], occ_env)
    cell = reduce_health(rows, arm, writers, readers)
    locked = "n/a" if cell["locked_share"] is None else f"{cell['locked_share']['median']:.4%}"
    spin = "n/a" if cell["spin_time_share"] is None else f"{cell['spin_time_share']['median']:.2%}"
    print(f"  health {arm:>3} W={writers:<2} R={readers:<2} "
          f"restart {cell['restart_share']['median']:.4%}  "
          f"fallback {cell['fallback_share']['median']:.4%}  "
          f"locked {locked}  spin-time {spin}"
          f"{'  STARVATION' if cell['starvation_flag'] else ''}")
    return cell


def ab_cell(arm: str, writers: int, readers: int, base_bin: Path, env: dict) -> dict:
    """One cell with the base and head builds interleaved round by round.

    Each build's rounds are reduced with `reduce_throughput`, exactly as a
    single-build cell is; the cell carries both reductions and every round
    of both builds (`rounds_raw`, tagged `build`).
    """
    head_bin = binary("hot_concurrent")
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
    cell["rounds_raw"] = raw_rounds(base_rows + head_rows, ("build", *CONCURRENT_RAW))
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
    """C1, C2, H and M of METHODOLOGY.md §11.4, one process per cell.

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
    for arm in CONCURRENT_ARMS:
        print(f"\n  M memory — ROWEX {arm} arm vs Sync wrapper, build-only, single writer")
        for lam in lambdas:
            n = population_for_lambda(arm, lam)
            rows = run_cell([str(binary("hot_memory_curve")), CONCURRENT_MEMORY_ARMS[arm], str(n)], env)
            if len(rows) != 1:
                raise RuntimeError(f"memory cell emitted {len(rows)} rows, expected 1")
            row = rows[0]
            row["lambda_target"] = lam
            memory.append(row)
            print(f"  memory {arm:>3} λ≈{lam:<5} N={n:<9} "
                  f"ROWEX {row['hot_alloc_bytes_per_key']:.2f}  "
                  f"Sync-Expanse {row['expanse_alloc_bytes_per_key']:.2f} B/key")
    return {"throughput": throughput, "health": health, "memory": memory}


# --------------------------------------------------------------------------
# self-test: the health reduction, driven by a fixture (no harness needed)
# --------------------------------------------------------------------------

def _health_row(round_: int, **over) -> dict:
    """One health round with every counter, at values whose shares are exact."""
    row = {
        "workload_id": "hot_rowex_set_63bit", "role": "health", "arm": "set",
        "writers": 1, "readers": 8, "round": round_,
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
    cell = reduce_health(rows, "set", 1, 8)
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
    check("locked_reads median is the raw counter", cell["locked_reads"]["median"] == 100)

    # No readers: the spin-time share is zero by definition, not a division.
    alone = reduce_health([_health_row(0, readers=0, reader_elapsed_s=0.0)], "set", 1, 0)
    check("spin_time_share is 0.0 with no readers", alone["spin_time_share"] == {"median": 0.0, "min": 0.0, "max": 0.0})

    # No read ops at all: a share of nothing is None, never 0.
    unread = reduce_health([_health_row(0, read_ops=0, locked_reads=0, read_fallbacks=0)], "set", 1, 8)
    check("locked_share is None with no read ops", unread["locked_share"] is None)

    # THE NEGATIVE CASE: a row without `locked_reads` — the counter the runner
    # silently dropped until #568 — is an error naming the field, not a zero.
    short = [_health_row(0), _health_row(1)]
    del short[1]["locked_reads"]
    try:
        reduce_health(short, "set", 1, 8)
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
    prov["issue"] = 692
    prov["tbb_commit"] = "4c73c3b"
    prov["pin_applied"] = os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset")
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
    sensitivity = "--sensitivity" in sys.argv or "--only-sensitivity" in sys.argv
    only_sensitivity = "--only-sensitivity" in sys.argv
    env = dict(os.environ)
    env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-cpu=haswell"

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR
    out_dir.mkdir(parents=True, exist_ok=True)
    if quick:
        print("QUICK MODE — reduced sweep, writing to gitignored results/quick/ (§8.5)")

    if concurrent:
        # Build before anything is timed and before the lock matters: the
        # first `rowex` build also compiles libtbb.
        build_concurrent_binaries(env)
    if not only_concurrent:
        build(env)

    provenance = {
        "suite": "hot_comparison",
        "issue": 660,
        "commit": git_sha(REPO_ROOT),
        "hot_commit": "96bf6fb",
        "cpu": platform.processor() or platform.machine(),
        "platform": platform.platform(),
        "host": host_facts(),
        "estimators": estimators(
            "mean(HOT rounds) / mean(Expanse rounds) — or Expanse / ROWEX for throughput — "
            "with a two-sample BCa 95% interval (scripts/bca_bootstrap.py)"
        ),
        "rustflags": env["RUSTFLAGS"].strip(),
        "cxx_flags": "-march=haswell -O3 -std=c++17 -DNDEBUG",
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "loads": [],
        "quick": quick,
    }

    add_load(provenance, "start")

    # Ordering (#727): the single-threaded phases run FIRST, the concurrent
    # sweep LAST. A cell in that sweep runs up to sixteen busy threads, which
    # lifts the 1-minute load average to about 5 and leaves it decaying for
    # minutes. A single-threaded phase timed inside that decay publishes a load
    # series that reads as contamination under docs/BENCHMARKING.md rule 2 and
    # is only the sweep's own tail — `masstree_comparison` discarded a full run
    # for exactly that (start 0.88, 4.79 after the concurrent join) before its
    # runner was ordered this way.
    if not only_concurrent:
        if sensitivity:
            print("\n[sensitivity] §12.2 — the same population sorted and shuffled")
            sens = sweep_sensitivity(env, quick)
            add_load(provenance, "after-sensitivity")
            (out_dir / "baseline_sensitivity.json").write_text(
                json.dumps({"provenance": provenance, **sens}, indent=2) + "\n")
            print(f"wrote {out_dir}/baseline_sensitivity.json")
            if only_sensitivity:
                return 0

        print("\n[1/2] memory pillar — sweeping λ across the LEAF_CAP cascade")
        memory = sweep_memory(env, quick)
        add_load(provenance, "after-memory")

        memory["payload_delta"] = payload_delta_check(memory["cells"])

        print("\n[2/2] latency pillars")
        latency = sweep_latency(env, quick)
        add_load(provenance, "end")

        (out_dir / "baseline_memory_curve.json").write_text(
            json.dumps({"provenance": provenance, **memory}, indent=2) + "\n")
        (out_dir / "baseline_latency.json").write_text(
            json.dumps({"provenance": provenance, **latency}, indent=2) + "\n")

        loads = [snap["load1"] for snap in provenance["loads"]]
        print(f"\nload average across the run: {loads}")
        if max(loads) - min(loads) > 2.0:
            print("WARNING: load shifted by more than 2 during the run — "
                  "the comparison is contaminated (docs/BENCHMARKING.md rule 2)")
        print(f"wrote {out_dir}/baseline_memory_curve.json and baseline_latency.json")

    if ab_base_bin is not None:
        return run_ab(env, provenance, out_dir, Path(ab_base_bin), ab_base_commit)

    if concurrent:
        print("\n[concurrent] HOT-ROWEX arm (#692, §11) — one process per cell, "
              "threads inside the P-core pin")
        conc_prov = dict(provenance)
        conc_prov["issue"] = 692
        conc_prov["tbb_commit"] = "4c73c3b"
        conc_prov["pin_applied"] = os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset")
        # `dict()` is a shallow copy, so this phase needs its own list or its
        # snapshots and the single-threaded phases' are the same object and each
        # artifact publishes the other's load series.
        conc_prov["loads"] = []
        add_load(conc_prov, "start")
        conc = sweep_concurrent(env, quick, conc_prov)
        add_load(conc_prov, "after-concurrent")
        (out_dir / "baseline_concurrent.json").write_text(
            json.dumps({"provenance": conc_prov, **conc}, indent=2) + "\n")
        print(f"wrote {out_dir}/baseline_concurrent.json")
        # Rule 2 applies to load the benchmark did not cause. A cell here runs
        # up to 16 busy threads, so the 1-minute average *after* the sweep is
        # the sweep itself and cannot flag a co-resident process; the start
        # snapshot can, and is the one that gates.
        start_load = conc_prov["loads"][0]["load1"]
        print(f"\nload average at start: {start_load} (after: "
              f"{conc_prov['loads'][-1]['load1']} — includes the sweep's own threads)")
        if start_load > 2.0:
            print("WARNING: load average above 2 at start — another process was "
                  "running; the comparison is contaminated (docs/BENCHMARKING.md rule 2)")

    return 0


if __name__ == "__main__":
    sys.exit(main())
