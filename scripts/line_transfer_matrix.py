#!/usr/bin/env python3
"""Cross-core line-transfer matrix for the reference host (#568, PR 0).

Drives `crates/expanse/examples/line_transfer.rs` over every unordered pair
of physical performance cores (one logical CPU per core), plus each core's
SMT sibling pair, in both waiting modes, and records the `pause` calibration
for the first P-core. Every cell is repeated and carries a BCa 95% interval
over its repeats (AGENTS.md section 8.4); the artifact carries host facts and
load snapshots (section 8.17) so it is admissible as a published number.

The result is a host property, not an engine result: it is the `t_line` /
`t_wake` input to the contention bound that decides how many contended
line transfers per insert a multi-writer design can afford (#568 PR 4), and
it converts an `occ_stats` spin *count* into time (`pause`).

This script sets thread affinity itself, per pair, which is why it is listed
in `scripts/check_bench_pin.py::DIRECT_EXEMPT`: a suite-wide pin applied
behind its back would be its own subject.

Usage (reference host only; Linux, hybrid or uniform):

    python3 scripts/line_transfer_matrix.py --out docs/benchmarks/concurrency/results/line_transfer.json
    python3 scripts/line_transfer_matrix.py --self-test
"""
from __future__ import annotations

import argparse
import itertools
import json
import os
import platform
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from bca_bootstrap import bca_bootstrap_ci  # noqa: E402
from bench_provenance import add_load, host_facts, load_snapshot, git_sha  # noqa: E402

MODES = ("spin", "park")
WORKLOAD_ID = "line_transfer_pair"


class Preflight(Exception):
    """A named infrastructure cause, with the fix. Always fatal."""


def read_cpu_list(path: str) -> list[int] | None:
    """Expands a kernel CPU list such as `0-15,20` to ints; None if absent."""
    try:
        text = Path(path).read_text().strip()
    except OSError:
        return None
    out: list[int] = []
    for part in text.split(","):
        part = part.strip()
        if not part:
            continue
        if "-" in part:
            lo, hi = part.split("-", 1)
            out.extend(range(int(lo), int(hi) + 1))
        else:
            out.append(int(part))
    return out


def physical_cores(cpus: list[int]) -> dict[int, list[int]]:
    """Maps `core_id` -> its logical CPUs among `cpus`, from sysfs topology."""
    cores: dict[int, list[int]] = {}
    for cpu in cpus:
        p = f"/sys/devices/system/cpu/cpu{cpu}/topology/core_id"
        try:
            core = int(Path(p).read_text().strip())
        except OSError as e:
            raise Preflight(f"cannot read {p}: {e}; the topology is needed to pick one CPU per core")
        cores.setdefault(core, []).append(cpu)
    return cores


def pick_cpus() -> tuple[list[int], dict[int, list[int]], str]:
    """The performance CPUs (hybrid) or all CPUs (uniform), their cores, and which."""
    p = read_cpu_list("/sys/devices/cpu_core/cpus")
    if p:
        return p, physical_cores(p), "cpu_core"
    allc = read_cpu_list("/sys/devices/system/cpu/online")
    if not allc:
        raise Preflight("neither /sys/devices/cpu_core/cpus nor /sys/devices/system/cpu/online is readable")
    return allc, physical_cores(allc), "online"


def binary_path() -> Path:
    exe = REPO_ROOT / "target" / "release" / "examples" / "line_transfer"
    if not exe.is_file():
        raise Preflight(
            f"{exe} does not exist; build it first: "
            "cargo build --release -p expanse-trie --example line_transfer"
        )
    return exe


def one_cell(exe: Path, a: int, b: int | None, mode: str, trips: int) -> dict:
    args = [str(exe), str(a), str(b if b is not None else a), mode, str(trips)]
    proc = subprocess.run(args, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise Preflight(f"{' '.join(args)} exited {proc.returncode}: {proc.stderr.strip()[:400]}")
    rows = [json.loads(line) for line in proc.stdout.splitlines() if line.startswith("{")]
    if len(rows) != 1 or rows[0].get("workload_id") != WORKLOAD_ID:
        raise Preflight(f"{' '.join(args)} emitted {len(rows)} rows, expected one {WORKLOAD_ID} row")
    return rows[0]


def summarise(label: str, kind: str, a: int, b: int | None, mode: str,
              rows: list[dict], seed: int) -> dict:
    ns = [r["ns_per_transfer"] for r in rows]
    mean, lo, hi = bca_bootstrap_ci(ns, seed=seed)
    return {
        "workload_id": WORKLOAD_ID,
        "label": label,
        "kind": kind,
        "cpu_a": a,
        "cpu_b": b,
        "mode": mode,
        "repeats": len(rows),
        "ns_per_transfer": {"mean": mean, "ci_lower": lo, "ci_upper": hi},
        "rounds_raw": rows,
    }


def run_matrix(exe: Path, repeats: int, trips: int, quick: bool, seed: int) -> dict:
    cpus, cores, source = pick_cpus()
    core_ids = sorted(cores)
    reps = [cores[c][0] for c in core_ids]  # one logical CPU per physical core
    if quick:
        reps = reps[:3]
    prov = {
        "suite": "concurrency",
        "issue": 568,
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "cpu_source": source,
        "cpus_considered": cpus,
        "physical_cores": {str(k): v for k, v in cores.items()},
        "representatives": reps,
        "trips_per_invocation": trips,
        "repeats": repeats,
        "estimators": {
            "point": "mean over repeats of ns per one-way transfer (elapsed / (2 x round trips))",
            "interval": "BCa 95% over repeats, bca_bootstrap.py",
        },
        "loads": [load_snapshot("start")],
        "quick": quick,
    }
    cells: list[dict] = []
    # Pause calibration on the first representative: one thread, no transfer.
    rows = [one_cell(exe, reps[0], None, "pause", trips) for _ in range(repeats)]
    cells.append(summarise(f"pause@{reps[0]}", "pause", reps[0], None, "pause", rows, seed))
    add_load(prov, "after-pause")
    # Cross-core pairs, one CPU per physical core, both modes.
    for mode in MODES:
        for a, b in itertools.combinations(reps, 2):
            rows = [one_cell(exe, a, b, mode, trips) for _ in range(repeats)]
            cells.append(summarise(f"{a}-{b}/{mode}", "cross-core", a, b, mode, rows, seed))
        add_load(prov, f"after-cross-{mode}")
    # SMT sibling pairs (two logical CPUs of one core), where the host has them.
    for mode in MODES:
        for c in core_ids:
            if len(cores[c]) >= 2:
                a, b = cores[c][0], cores[c][1]
                if quick and a not in reps:
                    continue
                rows = [one_cell(exe, a, b, mode, trips) for _ in range(repeats)]
                cells.append(summarise(f"{a}-{b}/{mode}", "smt-sibling", a, b, mode, rows, seed))
        add_load(prov, f"after-smt-{mode}")
    add_load(prov, "end")
    return {"provenance": prov, "cells": cells}


def render(artifact: dict) -> str:
    """A Markdown summary: pause cost, and per-mode min / median / max across cross-core pairs."""
    cells = artifact["cells"]
    out = ["| kind | mode | cells | min ns | median ns | max ns |", "|---|---|--:|--:|--:|--:|"]
    for kind in ("pause", "cross-core", "smt-sibling"):
        for mode in ("pause",) + MODES:
            sel = [c for c in cells if c["kind"] == kind and c["mode"] == mode]
            if not sel:
                continue
            means = sorted(c["ns_per_transfer"]["mean"] for c in sel)
            med = means[len(means) // 2]
            out.append(f"| {kind} | {mode} | {len(sel)} | {means[0]:.1f} | {med:.1f} | {means[-1]:.1f} |")
    return "\n".join(out)


def self_test() -> int:
    """Pins the summary shape and the topology grouping on synthetic input."""
    rows = [{"workload_id": WORKLOAD_ID, "cpu_a": 0, "cpu_b": 2, "mode": "spin",
             "round_trips": 10, "elapsed_s": 1e-6 * (i + 1), "ns_per_transfer": 40.0 + i}
            for i in range(5)]
    cell = summarise("0-2/spin", "cross-core", 0, 2, "spin", rows, seed=1)
    m = cell["ns_per_transfer"]
    assert 41.9 < m["mean"] < 42.1, m
    assert m["ci_lower"] <= m["mean"] <= m["ci_upper"], m
    assert cell["repeats"] == 5 and len(cell["rounds_raw"]) == 5
    # A pair whose rows disagree on the workload id is refused upstream; the
    # summary itself must keep raw rounds verbatim (section 8.17).
    assert cell["rounds_raw"][3]["ns_per_transfer"] == 43.0
    art = {"cells": [cell, summarise("pause@0", "pause", 0, None, "pause", rows, seed=1)]}
    text = render(art)
    assert "| cross-core | spin | 1 |" in text and "| pause | pause | 1 |" in text, text
    # Topology grouping: two logical CPUs per core map to one core id.
    fake = {0: [0, 1], 1: [2, 3]}
    reps = [fake[c][0] for c in sorted(fake)]
    assert reps == [0, 2]
    assert list(itertools.combinations(reps, 2)) == [(0, 2)]
    print("line_transfer_matrix.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", type=Path, help="artifact path (JSON); printed to stdout when omitted")
    ap.add_argument("--repeats", type=int, default=7)
    ap.add_argument("--trips", type=int, default=200_000, help="round trips per invocation")
    ap.add_argument("--quick", action="store_true", help="three representative cores only; scratch output")
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if platform.system() != "Linux":
        raise Preflight(f"line_transfer pins with sched_setaffinity; this host is {platform.system()}. Run it on the reference host.")
    if args.repeats < 3:
        raise Preflight("BCa needs at least 3 repeats")
    if args.quick and args.out and "results/quick" not in str(args.out) and "scratch" not in str(args.out):
        raise Preflight("--quick output must go to a gitignored scratch path (AGENTS.md section 8.5)")
    art = run_matrix(binary_path(), args.repeats, args.trips, args.quick, args.seed)
    text = json.dumps(art, indent=2) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text)
        print(f"wrote {args.out} ({len(art['cells'])} cells)")
    else:
        sys.stdout.write(text)
    print(render(art))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Preflight as e:
        print(f"::error::line_transfer_matrix.py: {e}", file=sys.stderr)
        sys.exit(1)
