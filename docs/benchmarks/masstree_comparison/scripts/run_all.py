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
CONCURRENT_HEALTH_WRITERS = [1, 2, 4, 8]
CONCURRENT_ARMS = ["map", "str"]
OCC_STATS_TARGET = CRATE.parent / "target-occ-stats"

MASSTREE_COMMIT = "1119842"


def population_for_lambda(lam: float) -> int:
    """Population that puts a 64-bit uniform random map at occupancy `lam`."""
    return max(1000, int(round(lam * 2 ** 16)))


def load_snapshot(label: str) -> dict:
    try:
        one, five, fifteen = os.getloadavg()
    except OSError:
        one = five = fifteen = float("nan")
    return {"label": label, "load1": round(one, 2), "load5": round(five, 2),
            "load15": round(fifteen, 2), "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}


def git_sha() -> str:
    explicit = os.environ.get("EXPANSE_BENCH_COMMIT")
    if explicit:
        return explicit
    try:
        return subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT,
                                       stderr=subprocess.DEVNULL).decode().strip()
    except Exception:
        return "unknown"


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
                    })
                    print(f"  {pillar:<12} {arm:>3} {dist:<9} {order:<8} {table:<10} N={n:<8} ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}]")
    return {"memory": memory, "latency": latency}


def concurrent_cell(arm: str, writers: int, readers: int, env: dict) -> dict:
    rows = run_cell([str(binary("masstree_concurrent")), arm, str(writers), str(readers)], env)
    if not rows:
        raise RuntimeError(f"concurrent cell {arm} W={writers} R={readers} emitted no rows")
    head = rows[0]
    cell = {"workload_id": head["workload_id"], "arm": arm, "dist": head["dist"],
            "writers": writers, "readers": readers, "prefill": head["prefill"],
            "fresh_keys": head["fresh_keys"], "rounds": len(rows),
            "cpus_allowed": head["cpus_allowed"], "pin_applied": head["pin_applied"]}
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


def health_cell(arm: str, writers: int, readers: int, env: dict) -> dict:
    exe = OCC_STATS_TARGET / "release" / "masstree_concurrent"
    rows = run_cell([str(exe), arm, str(writers), str(readers), "--health"], env)
    if not rows:
        raise RuntimeError(f"health cell {arm} W={writers} R={readers} emitted no rows")

    def med_range(key: str) -> dict:
        vals = sorted(r[key] for r in rows)
        return {"median": vals[len(vals) // 2], "min": vals[0], "max": vals[-1]}

    cell = {"workload_id": rows[0]["workload_id"], "arm": arm, "dist": rows[0]["dist"],
            "writers": writers, "readers": readers, "rounds": len(rows),
            "restart_share": med_range("restart_share"), "fallback_share": med_range("fallback_share"),
            "read_ops": med_range("read_ops"), "read_attempts": med_range("read_attempts"),
            "read_fallbacks": med_range("read_fallbacks"), "sample_spins": med_range("sample_spins"),
            "write_ops": med_range("write_ops"),
            "cpus_allowed": rows[0]["cpus_allowed"], "pin_applied": rows[0]["pin_applied"]}
    cell["starvation_flag"] = cell["fallback_share"]["median"] >= 0.01
    print(f"  health {arm:>3} W={writers:<2} R={readers:<2} restart {cell['restart_share']['median']:.4%}  "
          f"fallback {cell['fallback_share']['median']:.4%}{'  STARVATION' if cell['starvation_flag'] else ''}")
    return cell


def sweep_concurrent(env: dict, quick: bool) -> dict:
    write_w = [1, 4] if quick else CONCURRENT_WRITE_SCALING
    mixed_w = [0, 4] if quick else CONCURRENT_MIXED_WRITERS
    health_w = [4] if quick else CONCURRENT_HEALTH_WRITERS
    lambdas = LAMBDA_TARGETS[:2] if quick else LAMBDA_TARGETS
    throughput, health, memory = [], [], []
    for arm in CONCURRENT_ARMS:
        print(f"\n  C1 write scaling — {arm} arm")
        for w in write_w:
            c = concurrent_cell(arm, w, 0, env)
            c["pillar"] = "C1"
            throughput.append(c)
        print(f"\n  C2 readers alongside writers — {arm} arm")
        for w in mixed_w:
            c = concurrent_cell(arm, w, CONCURRENT_MIXED_READERS, env)
            c["pillar"] = "C2"
            throughput.append(c)
    for arm in CONCURRENT_ARMS:
        print(f"\n  H protocol health — {arm} arm (occ-stats build, Expanse side only)")
        for w in health_w:
            health.append(health_cell(arm, w, CONCURRENT_MIXED_READERS, env))
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


def main() -> int:
    quick = "--quick" in sys.argv
    concurrent = "--concurrent" in sys.argv or "--only-concurrent" in sys.argv
    only_concurrent = "--only-concurrent" in sys.argv
    env = dict(os.environ)
    env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-cpu=haswell"

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR
    out_dir.mkdir(parents=True, exist_ok=True)
    if quick:
        print("QUICK MODE — reduced sweep, writing to gitignored results/quick/ (§8.5)")

    build(env)
    if concurrent:
        build_concurrent(env)

    provenance = {
        "suite": "masstree_comparison", "issue": 661, "commit": git_sha(),
        "masstree_commit": MASSTREE_COMMIT,
        "cpu": platform.processor() or platform.machine(), "platform": platform.platform(),
        "rustflags": env["RUSTFLAGS"].strip(),
        "cxx_flags": "-march=haswell -O3 -std=c++17 -DNDEBUG (assertions, preconditions and invariants off)",
        "allocator": "glibc malloc, superpages on (METHODOLOGY §3.3)",
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "loads": [load_snapshot("start")], "quick": quick,
    }

    validate_log = validate(env)
    provenance["loads"].append(load_snapshot("after-validate"))
    (out_dir / "validate.log").write_text(validate_log)

    if concurrent:
        print("\n[concurrent] MC1 / MC2 — one process per cell, threads inside the P-core pin")
        conc_prov = dict(provenance)
        conc_prov["loads"] = list(provenance["loads"])
        conc = sweep_concurrent(env, quick)
        conc_prov["loads"].append(load_snapshot("after-concurrent"))
        (out_dir / "baseline_concurrent.json").write_text(json.dumps({"provenance": conc_prov, **conc}, indent=2) + "\n")
        print(f"wrote {out_dir}/baseline_concurrent.json")
        start_load = conc_prov["loads"][0]["load1"]
        print(f"\nload average at start: {start_load} (after: {conc_prov['loads'][-1]['load1']} — includes the sweep's own threads)")
        if start_load > 2.0:
            print("WARNING: load average above 2 at start — another process was running (docs/BENCHMARKING.md rule 2)")
        if only_concurrent:
            return 0
        provenance["loads"].append(load_snapshot("after-concurrent"))

    print("\n[1/5] memory — integer map (λ sweep and structured distributions)")
    memory = sweep_memory(env, quick)
    provenance["loads"].append(load_snapshot("after-memory"))
    print("\n[2/5] memory — string map (population sweep)")
    smem = sweep_string_memory(env, quick)
    provenance["loads"].append(load_snapshot("after-string-memory"))
    print("\n[3/5] latency — integer map")
    latency = sweep_latency(env, quick)
    provenance["loads"].append(load_snapshot("after-latency"))
    print("\n[4/5] latency — string map")
    slat = sweep_string_latency(env, quick)
    provenance["loads"].append(load_snapshot("after-string-latency"))
    print("\n[5/5] sensitivity — insertion order (§10.2) and the concurrent table (§10.3)")
    order = sweep_sensitivity(env, quick)
    provenance["loads"].append(load_snapshot("end"))

    (out_dir / "baseline_memory.json").write_text(json.dumps({"provenance": provenance, **memory}, indent=2) + "\n")
    (out_dir / "baseline_string_memory.json").write_text(json.dumps({"provenance": provenance, **smem}, indent=2) + "\n")
    (out_dir / "baseline_latency.json").write_text(json.dumps({"provenance": provenance, **latency}, indent=2) + "\n")
    (out_dir / "baseline_string_latency.json").write_text(json.dumps({"provenance": provenance, **slat}, indent=2) + "\n")
    (out_dir / "baseline_sensitivity.json").write_text(json.dumps({"provenance": provenance, **order}, indent=2) + "\n")

    loads = [s["load1"] for s in provenance["loads"]]
    print(f"\nload average across the run: {loads}")
    if max(loads) - min(loads) > 2.0:
        print("WARNING: load shifted by more than 2 during the run — the comparison is contaminated (docs/BENCHMARKING.md rule 2)")
    print(f"wrote {out_dir}/baseline_memory.json, baseline_string_memory.json, baseline_latency.json, baseline_string_latency.json, baseline_sensitivity.json, validate.log")
    return 0


if __name__ == "__main__":
    sys.exit(main())
