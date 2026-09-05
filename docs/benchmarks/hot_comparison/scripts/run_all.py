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
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT
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


def main() -> int:
    quick = "--quick" in sys.argv
    env = dict(os.environ)
    env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-cpu=haswell"

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR
    out_dir.mkdir(parents=True, exist_ok=True)
    if quick:
        print("QUICK MODE — reduced sweep, writing to gitignored results/quick/ (§8.5)")

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
