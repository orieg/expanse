#!/usr/bin/env python3
"""AArch64 acquire-load A/B: default Linux build against `+rcpc` (Refs #1191).

`aarch64-unknown-linux-gnu` enables neither `rcpc` nor `lse` by default, so an
`Ordering::Acquire` load lowers to `ldar` (RCsc) and a read-modify-write atomic
to an out-of-line `__aarch64_*` helper call. `-C target-feature=+rcpc` lowers
the same loads to `ldapr` (RCpc), which the architecture lets an implementation
order past an earlier `stlr`; `ldar` it does not. This driver asks whether that
difference is visible in wall-clock throughput on the `Sync*` paths.

What it runs, all from `crates/expanse/examples/writer_scaling.rs`, one timed
cell per harness process (METHODOLOGY.md §15):
- the writer cells of the `map` and `set` arms at W in `--writers`
  (default 1,2,4), R = 0 -- W = 1 is the control cell (AGENTS.md §8.20.2);
- one reader-heavy mixed cell on the map: W = 1 writer, R = `--mixed-readers`
  readers doing optimistic `get` on the uniform prefill.

Builds are labelled by the RUSTFLAGS they add, never by features:
`default` (none), `rcpc` (`-C target-feature=+rcpc`) and, with `--with-lse`,
`rcpc_lse` (`-C target-feature=+rcpc,+lse`), a separate arm whose ratios are
reported apart because it changes two things at once.

Within every round the (build, cell) treatments of one arm run in the order of
that round's row of a Williams design (`writer_scaling.williams_positions`), so
position and host drift land on every build alike. The statistic per cell is
the per-round paired ratio T_variant / T_default (throughput; > 1 means the
default build is slower) with a BCa 95% interval over rounds (AGENTS.md §8.4);
for W >= 2 also the scaling ratio C_variant(W) / C_default(W) of §8.20.2.

One run cannot confirm a delta (docs/BENCHMARKING.md rule 18): the workflow is
dispatched twice and the two artifacts are read together.

Usage:
    python3 docs/benchmarks/concurrency/scripts/rcpc_ab.py --out rcpc_ab.json [--rounds 12] [--with-lse]
    python3 docs/benchmarks/concurrency/scripts/rcpc_ab.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
import sys
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
sys.path.insert(0, str(HERE))

import bench_pin  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402
from bench_provenance import begin_cell, end_cell, new_provenance  # noqa: E402
from writer_scaling import williams_positions, writer_cell_argv, writer_cell_row  # noqa: E402

BUILDS = {
    "default": "",
    "rcpc": "-C target-feature=+rcpc",
    "rcpc_lse": "-C target-feature=+rcpc,+lse",
}
MIXED_WORKLOAD_ID = "concurrency_ordered_readers_map_64bit"


def build(label: str, verbose: bool = True) -> Path:
    """One release build of the harness, in a target dir of its own."""
    target = REPO_ROOT / "target" / f"rcpc-ab-{label}"
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target)
    env["RUSTFLAGS"] = BUILDS[label]
    if verbose:
        print(f"building {label} (RUSTFLAGS={BUILDS[label]!r}) ...", flush=True)
    subprocess.run(
        ["cargo", "build", "--release", "-p", "expanse-trie", "--example", "writer_scaling"],
        cwd=str(REPO_ROOT), env=env, check=True,
    )
    binary = target / "release" / "examples" / "writer_scaling"
    if not binary.exists():
        raise RuntimeError(f"build {label}: {binary} missing after a successful cargo build (AGENTS.md §8.1)")
    return binary


def treatments(arm: str, writers: list[int], builds: list[str]) -> list[dict[str, Any]]:
    if arm == "mixed":
        return [{"build": b, "cell": "mixed"} for b in builds]
    return [{"build": b, "cell": f"W{w}", "writers": w} for w in writers for b in builds]


def schedule(arm: str, writers: list[int], builds: list[str], rounds: int) -> list[dict[str, Any]]:
    """Every timed invocation of one arm, in execution order (Williams rows)."""
    tr = treatments(arm, writers, builds)
    out = []
    for r in range(rounds):
        for pos, idx in enumerate(williams_positions(len(tr), r)):
            out.append({**tr[idx], "arm": arm, "round": r, "position": pos})
    return out


def run_cell(binary: Path, cell: dict[str, Any], mixed_readers: int) -> dict[str, Any]:
    if cell["arm"] != "mixed":
        cmd = writer_cell_argv(binary, cell["arm"], cell, quick=False)
        proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
        if proc.returncode != 0:
            raise RuntimeError(f"cell {cell} failed (exit {proc.returncode}):\n{proc.stderr}")
        row = writer_cell_row(proc.stdout, cell["arm"], cell)
        return {"value": float(row["writer_mops"]), "row": row}
    cmd = [str(binary), "--role", "throughput", "--arm", "map", "--writers", "1",
           "--readers", str(mixed_readers), "--read-op", "get", "--probe", "uniform",
           "--round", str(cell["round"]), "--position", str(cell["position"])]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(f"cell {cell} failed (exit {proc.returncode}):\n{proc.stderr}")
    rows = [json.loads(ln) for ln in proc.stdout.splitlines()
            if ln.strip().startswith("{") and ln.strip().endswith("}")]
    rows = [r for r in rows if r.get("role") == "throughput" and r.get("workload_id") == MIXED_WORKLOAD_ID]
    if len(rows) != 1:
        raise RuntimeError(f"mixed cell {cell} emitted {len(rows)} throughput rows, expected 1 (AGENTS.md §8.1)")
    row = rows[0]
    for k in ("round", "position"):
        if row.get(k) != cell[k]:
            raise RuntimeError(f"mixed cell {cell}: row carries {k}={row.get(k)!r}")
    if not row.get("reader_mops") or not row.get("writer_mops"):
        raise RuntimeError(f"mixed cell {cell}: missing reader_mops/writer_mops in {row}")
    return {"value": float(row["reader_mops"]), "writer_mops": float(row["writer_mops"]), "row": row}


def interval(xs: list[float]) -> dict[str, Any]:
    mean, lo, hi, method = bca_bootstrap_ci_with_method(xs, confidence=0.95)
    if lo > 1.0:
        label = "SINGLE_RUN_ABOVE_PARITY"
    elif hi < 1.0:
        label = "SINGLE_RUN_BELOW_PARITY"
    else:
        label = "INCONCLUSIVE_spans_parity"
    return {"mean": round(mean, 5), "ci_lower": round(lo, 5), "ci_upper": round(hi, 5),
            "ci_method": method, "label": label, "raw": [round(x, 6) for x in xs]}


def paired(rows: list[dict[str, Any]], variants: list[str], writers: list[int]) -> dict[str, Any]:
    """Per-round paired ratios variant / default, per arm and cell."""
    by: dict[tuple[str, str, str, int], dict[str, Any]] = {}
    for r in rows:
        by[(r["arm"], r["cell"], r["build"], r["round"])] = r
    rounds = sorted({r["round"] for r in rows})
    arms = sorted({r["arm"] for r in rows})
    out: dict[str, Any] = {}
    for v in variants:
        for arm in arms:
            cells = ["mixed"] if arm == "mixed" else [f"W{w}" for w in writers]
            for c in cells:
                lvl = [by[(arm, c, v, k)]["value"] / by[(arm, c, "default", k)]["value"] for k in rounds]
                entry: dict[str, Any] = {"throughput_ratio": interval(lvl)}
                if arm == "mixed":
                    wr = [by[(arm, c, v, k)]["writer_mops"] / by[(arm, c, "default", k)]["writer_mops"]
                          for k in rounds]
                    entry["writer_throughput_ratio"] = interval(wr)
                elif c != "W1":
                    sc = [(by[(arm, c, v, k)]["value"] / by[(arm, "W1", v, k)]["value"])
                          / (by[(arm, c, "default", k)]["value"] / by[(arm, "W1", "default", k)]["value"])
                          for k in rounds]
                    entry["scaling_ratio"] = interval(sc)
                out[f"{v}/default:{arm}:{c}"] = entry
    return out


def host_census() -> dict[str, Any]:
    info: dict[str, Any] = {"machine": platform.machine(), "nproc": os.cpu_count()}
    try:
        text = Path("/proc/cpuinfo").read_text()
        for key in ("CPU implementer", "CPU part", "CPU variant", "CPU revision", "Features"):
            for line in text.splitlines():
                if line.startswith(key):
                    info[key.lower().replace(" ", "_")] = line.split(":", 1)[1].strip()
                    break
    except OSError:
        info["cpuinfo"] = None
    try:
        info["rustc"] = subprocess.check_output(["rustc", "--version"], text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        info["rustc"] = None
    return info


def run(args: argparse.Namespace) -> int:
    writers = [int(w) for w in args.writers.split(",")]
    if 1 not in writers:
        sys.stderr.write("error: --writers must include 1, the control cell (AGENTS.md §8.20.2)\n")
        return 1
    if args.rounds < 3:
        sys.stderr.write("error: --rounds must be >= 3 for a BCa interval\n")
        return 1
    builds = ["default", "rcpc"] + (["rcpc_lse"] if args.with_lse else [])
    census = host_census()
    feats = set((census.get("features") or "").split())
    if "lrcpc" not in feats:
        sys.stderr.write(f"error: host does not report lrcpc (Features: {census.get('features')!r}); "
                         "a +rcpc build would fault or measure nothing (AGENTS.md §8.1)\n")
        return 1
    if args.with_lse and "atomics" not in feats:
        sys.stderr.write("error: host does not report atomics (LSE); refusing the rcpc_lse arm\n")
        return 1
    pin = bench_pin.apply("rcpc_ab")
    binaries = {b: build(b) for b in builds}

    prov = new_provenance(
        "concurrency_rcpc_ab", 1191,
        "per-round paired throughput ratio variant / default (and C_variant(W) / C_default(W) for W >= 2); "
        "BCa 95% over rounds",
        repo_root=REPO_ROOT, core_pin=pin, cell_isolation="process", host_census=census,
        builds={b: {"rustflags": BUILDS[b]} for b in builds},
    )
    arms = [a for a in args.arms.split(",") if a]
    rows: list[dict[str, Any]] = []
    cells_load: dict[str, Any] = {}
    # One schedule per arm; each round runs every arm's Williams row, arm order rotated by round.
    scheds = {a: schedule(a, writers, builds, args.rounds) for a in arms}
    for r in range(args.rounds):
        order = arms[r % len(arms):] + arms[: r % len(arms)]
        for a in order:
            snap = begin_cell(prov, f"round:{r}:arm:{a}")
            for cell in (c for c in scheds[a] if c["round"] == r):
                res = run_cell(binaries[cell["build"]], cell, args.mixed_readers)
                row = {k: cell[k] for k in ("arm", "cell", "build", "round", "position")}
                row["value"] = res["value"]
                if "writer_mops" in res:
                    row["writer_mops"] = res["writer_mops"]
                row["harness_row"] = res["row"]
                rows.append(row)
                print(f"  r{r} {a:5s} {cell['cell']:5s} {cell['build']:8s} {res['value']:.4f}", flush=True)
            cells_load[f"round:{r}:arm:{a}"] = end_cell(snap)
    want = sum(len(s) for s in scheds.values())
    if len(rows) != want:
        raise RuntimeError(f"ran {len(rows)} cells, scheduled {want} (AGENTS.md §8.1)")
    begin_cell(prov, "end")
    art = {
        "provenance": prov,
        "design": {"writers": writers, "arms": arms, "builds": builds, "rounds": args.rounds,
                   "mixed_readers": args.mixed_readers,
                   "value": "writer cells: writer_mops; mixed cell: reader_mops (writer_mops ratio reported apart)"},
        "load_by_round_arm": cells_load,
        "rows": rows,
        "comparisons": paired(rows, [b for b in builds if b != "default"], writers),
    }
    Path(args.out).write_text(json.dumps(art, indent=1) + "\n")
    for k, v in art["comparisons"].items():
        t = v["throughput_ratio"]
        extra = ""
        if "scaling_ratio" in v:
            s = v["scaling_ratio"]
            extra = f"  C ratio {s['mean']:.4f} [{s['ci_lower']:.4f}, {s['ci_upper']:.4f}]"
        if "writer_throughput_ratio" in v:
            s = v["writer_throughput_ratio"]
            extra = f"  writer {s['mean']:.4f} [{s['ci_lower']:.4f}, {s['ci_upper']:.4f}]"
        print(f"{k:32s} T ratio {t['mean']:.4f} [{t['ci_lower']:.4f}, {t['ci_upper']:.4f}] {t['label']}{extra}")
    return 0


def self_test() -> int:
    # Schedule: every treatment once per round, positions 0..n-1.
    s = schedule("map", [1, 2, 4], ["default", "rcpc"], 6)
    for r in range(6):
        cells = [c for c in s if c["round"] == r]
        assert sorted(c["position"] for c in cells) == list(range(6)), cells
        assert len({(c["build"], c["cell"]) for c in cells}) == 6, cells
    # Each treatment holds each position once over n rounds (Williams, n even).
    for t in {(c["build"], c["cell"]) for c in s}:
        assert sorted(c["position"] for c in s if (c["build"], c["cell"]) == t) == list(range(6)), t
    # Paired ratios: a variant 10% faster at every cell gives 1.1 exactly, C ratio 1.0.
    rows = []
    for r in range(4):
        for b, f in (("default", 1.0), ("rcpc", 1.1)):
            for w in (1, 2):
                rows.append({"arm": "map", "cell": f"W{w}", "build": b, "round": r,
                             "value": f * w * (1 + 0.01 * r)})
            rows.append({"arm": "mixed", "cell": "mixed", "build": b, "round": r,
                         "value": f * 3.0, "writer_mops": 2.0})
    c = paired(rows, ["rcpc"], [1, 2])
    assert abs(c["rcpc/default:map:W2"]["throughput_ratio"]["mean"] - 1.1) < 1e-9, c
    assert abs(c["rcpc/default:map:W2"]["scaling_ratio"]["mean"] - 1.0) < 1e-9, c
    assert "scaling_ratio" not in c["rcpc/default:map:W1"], c
    assert abs(c["rcpc/default:mixed:mixed"]["writer_throughput_ratio"]["mean"] - 1.0) < 1e-9, c
    assert c["rcpc/default:mixed:mixed"]["throughput_ratio"]["ci_method"], c
    print("rcpc_ab self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default="rcpc_ab.json")
    ap.add_argument("--rounds", type=int, default=12)
    ap.add_argument("--writers", default="1,2,4")
    ap.add_argument("--arms", default="map,set,mixed")
    ap.add_argument("--mixed-readers", type=int, default=3)
    ap.add_argument("--with-lse", action="store_true", help="add the rcpc_lse arm (+rcpc,+lse)")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    return run(a)


if __name__ == "__main__":
    sys.exit(main())
