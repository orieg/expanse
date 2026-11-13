#!/usr/bin/env python3
"""#1682 measurement (c): what skipping a Judy1 array's top node could save.

Runs `set::diag_colour_top::colour_top_skip` (crates/expanse/src/set/
diag_colour_top.rs), an `#[ignore]`d test that replays OQGraph's colour-map
tape against two `ExpanseSet`s, in two variants: `root` tests through
`ExpanseSet::contains`; `child` enters the lookup at the top node's child edge
(level 7). Each process runs one variant under `perf stat`; variants run in
interleaved pairs on one pinned performance core, alternating which goes
first. The per-pair saving is

    (cycles(root) - cycles(child)) / tests

where `tests` is the number of Judy1Test calls the process made. The build
work (tape, arrays, process start) is the same in both variants and cancels.
The interval is BCa 95% over the pairs (scripts/bca_bootstrap.py), with the
construction that produced it recorded beside it.

Fails loudly (AGENTS.md section 8.1) when perf cannot count, when the two
variants' sinks differ, or when `child` sent fewer than 99% of its tests
through the child edge.

Usage (Linux, perf available):

    cargo test --release -p expanse-trie --lib --no-run   # prints the binary
    python3 scripts/colour_top_skip.py --bin <test binary> --cpu 4 \\
        --commit <sha> --host-desc '<cpu, cores, kernel>' --run-id run1 \\
        --out results/colour_top_skip_<sha>_run1.json
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402
from bench_provenance import load_snapshot  # noqa: E402

TEST = "set::diag_colour_top::colour_top_skip"
LINE = re.compile(
    r"DIAG_COLOUR variant=(\w+) n=(\d+) reps=(\d+) calls=(\d+) tests=(\d+) "
    r"via_child=(\d+) via_root=(\d+) sink=(\d+)"
)
# Pre-registered (results/colour_map_1682_preregistration.yaml, measurement c).
DROP_BELOW_CYCLES = 2.0
MIN_CHILD_SHARE = 0.99


def one(bin_path: str, cpu: str, pmu: str, variant: str, n: int, reps: int) -> dict:
    events = [f"{pmu}/cycles/", f"{pmu}/instructions/"] if pmu else ["cycles", "instructions"]
    env = dict(os.environ, DIAG_COLOUR_VARIANT=variant, DIAG_COLOUR_N=str(n),
               DIAG_COLOUR_REPS=str(reps))
    with tempfile.NamedTemporaryFile("r", suffix=".csv") as stat:
        cmd = ["taskset", "-c", cpu, "perf", "stat", "-x,", "-o", stat.name,
               "-e", ",".join(events), "--", bin_path, "--ignored", "--exact", TEST,
               "--nocapture", "--test-threads=1"]
        p = subprocess.run(cmd, env=env, capture_output=True, text=True)
        if p.returncode != 0:
            raise SystemExit(f"{variant} n={n}: exit {p.returncode}\n{p.stderr[-2000:]}")
        counts = {}
        for row in Path(stat.name).read_text().splitlines():
            f = row.split(",")
            if len(f) > 2 and f[2]:
                name = f[2].split("/")[1] if "/" in f[2] else f[2]
                if not f[0].replace(".", "").isdigit():
                    raise SystemExit(f"{variant} n={n}: perf did not count {f[2]}: {f[0]}")
                counts[name] = float(f[0])
    m = LINE.search(p.stdout)
    if not m:
        raise SystemExit(f"{variant} n={n}: no DIAG_COLOUR line in\n{p.stdout[-2000:]}")
    for need in ("cycles", "instructions"):
        if need not in counts:
            raise SystemExit(f"{variant} n={n}: perf gave no {need} row: {counts}")
    return {
        "variant": variant, "cycles": counts["cycles"], "instructions": counts["instructions"],
        "calls": int(m[4]), "tests": int(m[5]), "via_child": int(m[6]),
        "via_root": int(m[7]), "sink": int(m[8]),
    }


def ci(values: list[float]) -> dict:
    mean, lo, hi, method = bca_bootstrap_ci_with_method(values)
    return {"mean": mean, "ci_lower": lo, "ci_upper": hi, "ci_method": method}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--bin", required=True, help="the expanse-trie lib test binary")
    ap.add_argument("--cpu", default="4", help="one performance-core CPU to pin to")
    ap.add_argument("--pmu", default="cpu_core", help="core PMU name ('' on a uniform host)")
    ap.add_argument("--pops", default="10000,100000")
    ap.add_argument("--reps", default="10000:400,100000:40", help="pop:reps per population")
    ap.add_argument("--pairs", type=int, default=10)
    ap.add_argument("--commit", required=True)
    ap.add_argument("--host-desc", required=True)
    ap.add_argument("--run-id", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    if a.pairs < 3:
        raise SystemExit("--pairs must be at least 3 for a BCa interval")
    reps = {int(k): int(v) for k, v in (x.split(":") for x in a.reps.split(","))}
    loads = [load_snapshot("start")]
    cells = []
    for n in (int(x) for x in a.pops.split(",")):
        if n not in reps:
            raise SystemExit(f"--reps names no count for population {n}")
        runs = []
        for i in range(a.pairs):
            order = ("root", "child") if i % 2 == 0 else ("child", "root")
            pair = {v: one(a.bin, a.cpu, a.pmu, v, n, reps[n]) for v in order}
            pair["first"] = order[0]
            runs.append(pair)
        loads.append(load_snapshot(f"after n={n}", loads[-1]))
        sinks = {r[v]["sink"] for r in runs for v in ("root", "child")}
        if len(sinks) != 1:
            raise SystemExit(f"n={n}: the variants' sinks differ: {sorted(sinks)}")
        share = min(r["child"]["via_child"] / r["child"]["tests"] for r in runs)
        if share < MIN_CHILD_SHARE:
            raise SystemExit(f"n={n}: child entered at the child edge for only {share:.4f} of tests")
        cyc = [(r["root"]["cycles"] - r["child"]["cycles"]) / r["root"]["tests"] for r in runs]
        ins = [(r["root"]["instructions"] - r["child"]["instructions"]) / r["root"]["tests"]
               for r in runs]
        cell = {
            "population": n, "reps": reps[n], "pairs": runs, "min_child_share": share,
            "saved_cycles_per_test": ci(cyc), "saved_instructions_per_test": ci(ins),
            "saved_cycles_per_test_samples": cyc,
        }
        cell["drop_option_c"] = cell["saved_cycles_per_test"]["ci_upper"] < DROP_BELOW_CYCLES
        cells.append(cell)
        s = cell["saved_cycles_per_test"]
        print(f"n={n}: saved cycles per test {s['mean']:.3f} [{s['ci_lower']:.3f}, "
              f"{s['ci_upper']:.3f}] {s['ci_method']}; instructions "
              f"{cell['saved_instructions_per_test']['mean']:.3f}; child share {share:.4f}")
    out = {
        "workload_id": "capi_colour_map", "measurement": "1682-c top-node skip upper bound",
        "provenance": {"commit": a.commit, "host": a.host_desc, "run_id": a.run_id,
                       "cpu_pin": a.cpu, "pmu": a.pmu, "estimators": "BCa 95% over pairs",
                       "loads": loads},
        "drop_below_cycles": DROP_BELOW_CYCLES, "cells": cells,
    }
    Path(a.out).parent.mkdir(parents=True, exist_ok=True)
    Path(a.out).write_text(json.dumps(out, indent=1) + "\n")
    print(f"wrote {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
