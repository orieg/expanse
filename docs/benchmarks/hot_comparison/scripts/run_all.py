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
import time
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"
CRATE = REPO_ROOT / "crates" / "expanse-hot-bench" / "Cargo.toml"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bca_bootstrap import bca_bootstrap_ratio_ci  # noqa: E402

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

# The concurrent arm (#692, METHODOLOGY.md §10.4). Every cell keeps
# writers + readers <= 16 so no thread can leave the P-core pin (decision 3).
CONCURRENT_WRITE_SCALING = [1, 2, 4, 8, 16]            # C1: W, R = 0
CONCURRENT_MIXED_WRITERS = [0, 1, 2, 4, 8]             # C2: W, R = 8
CONCURRENT_MIXED_READERS = 8
CONCURRENT_HEALTH_WRITERS = [1, 2, 4, 8]               # H: the C2 cells with writers
CONCURRENT_ARMS = ["set", "map"]
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


def load_snapshot(label: str) -> dict:
    """Load average, recorded before and between comparison runs.

    A benchmark host under contention invalidates the run regardless of what the
    numbers say (``docs/BENCHMARKING.md`` rule 2).
    """
    try:
        one, five, fifteen = os.getloadavg()
    except OSError:
        one = five = fifteen = float("nan")
    return {"label": label, "load1": round(one, 2), "load5": round(five, 2),
            "load15": round(fifteen, 2), "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}


def git_sha() -> str:
    """The commit under test, for provenance (§8.7).

    A checkout rsynced to a benchmark host without its `.git` cannot answer
    `rev-parse`; `EXPANSE_BENCH_COMMIT` then names the commit explicitly rather
    than letting the artifact say `unknown`.
    """
    explicit = os.environ.get("EXPANSE_BENCH_COMMIT")
    if explicit:
        return explicit
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT,
            stderr=subprocess.DEVNULL,
        ).decode().strip()
    except Exception:
        return "unknown"


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
                        })
                        print(f"  {pillar:<12} {arm:>3} {dist:<10} N={n:<8} k={k:<5} "
                              f"ratio {ratio:.3f} [{lo:.3f}, {hi:.3f}]")
    return {"cells": cells}


def build_concurrent_binaries(env: dict) -> None:
    """Two builds, never one (§10.3 decision 5).

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
    above 1.000 means Expanse is faster (§10.4).
    """
    rows = run_cell([str(binary("hot_concurrent")), arm, str(writers), str(readers)], env)
    if not rows:
        raise RuntimeError(f"concurrent cell {arm} W={writers} R={readers} emitted no rows")
    head = rows[0]
    cell = {
        "workload_id": head["workload_id"], "arm": arm,
        "writers": writers, "readers": readers,
        "keyspace_bits": head["keyspace_bits"], "prefill": head["prefill"],
        "fresh_keys": head["fresh_keys"], "rounds": len(rows),
        "cpus_allowed": head["cpus_allowed"], "pin_applied": head["pin_applied"],
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


def health_cell(arm: str, writers: int, readers: int, env: dict) -> dict:
    """Event ratios from the diagnostic build; nothing here is a timing."""
    occ_env = dict(env)
    exe = OCC_STATS_TARGET / "release" / "hot_concurrent"
    rows = run_cell([str(exe), arm, str(writers), str(readers), "--health"], occ_env)
    if not rows:
        raise RuntimeError(f"health cell {arm} W={writers} R={readers} emitted no rows")

    def med_range(key: str) -> dict:
        vals = sorted(r[key] for r in rows)
        return {"median": vals[len(vals) // 2], "min": vals[0], "max": vals[-1]}

    cell = {
        "workload_id": rows[0]["workload_id"], "arm": arm,
        "writers": writers, "readers": readers, "rounds": len(rows),
        "restart_share": med_range("restart_share"),
        "fallback_share": med_range("fallback_share"),
        "read_ops": med_range("read_ops"),
        "read_attempts": med_range("read_attempts"),
        "read_fallbacks": med_range("read_fallbacks"),
        "sample_spins": med_range("sample_spins"),
        "write_ops": med_range("write_ops"),
        "cpus_allowed": rows[0]["cpus_allowed"], "pin_applied": rows[0]["pin_applied"],
    }
    # The §10.5.3 falsifier, evaluated on the median: 1% or more is reader
    # starvation and is reported as a protocol-health finding.
    cell["starvation_flag"] = cell["fallback_share"]["median"] >= 0.01
    print(f"  health {arm:>3} W={writers:<2} R={readers:<2} "
          f"restart {cell['restart_share']['median']:.4%}  "
          f"fallback {cell['fallback_share']['median']:.4%}"
          f"{'  STARVATION' if cell['starvation_flag'] else ''}")
    return cell


def sweep_concurrent(env: dict, quick: bool) -> dict:
    """C1, C2, H and M of METHODOLOGY.md §10.4, one process per cell."""
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

    if concurrent:
        # Build before anything is timed and before the lock matters: the
        # first `rowex` build also compiles libtbb.
        build_concurrent_binaries(env)
    if not only_concurrent:
        build(env)

    provenance = {
        "suite": "hot_comparison",
        "issue": 660,
        "commit": git_sha(),
        "hot_commit": "96bf6fb",
        "cpu": platform.processor() or platform.machine(),
        "platform": platform.platform(),
        "rustflags": env["RUSTFLAGS"].strip(),
        "cxx_flags": "-march=haswell -O3 -std=c++17 -DNDEBUG",
        "loads": [load_snapshot("start")],
        "quick": quick,
    }

    if concurrent:
        print("\n[concurrent] HOT-ROWEX arm (#692, §10) — one process per cell, "
              "threads inside the P-core pin")
        conc_prov = dict(provenance)
        conc_prov["issue"] = 692
        conc_prov["tbb_commit"] = "4c73c3b"
        conc_prov["pin_applied"] = os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset")
        conc = sweep_concurrent(env, quick)
        conc_prov["loads"].append(load_snapshot("after-concurrent"))
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
        if only_concurrent:
            return 0
        provenance["loads"].append(load_snapshot("after-concurrent"))

    print("\n[1/2] memory pillar — sweeping λ across the LEAF_CAP cascade")
    memory = sweep_memory(env, quick)
    provenance["loads"].append(load_snapshot("after-memory"))

    memory["payload_delta"] = payload_delta_check(memory["cells"])

    print("\n[2/2] latency pillars")
    latency = sweep_latency(env, quick)
    provenance["loads"].append(load_snapshot("end"))

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
    return 0


if __name__ == "__main__":
    sys.exit(main())
