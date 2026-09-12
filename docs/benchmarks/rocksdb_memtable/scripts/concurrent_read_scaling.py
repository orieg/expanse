#!/usr/bin/env python3
"""Driver for the RocksDB memtable concurrent read-scaling arm (#802).

Pre-registered in `docs/benchmarks/rocksdb_memtable/METHODOLOGY.md` section 5.
That section is the contract; this file executes it and nothing more. It does
not choose thresholds, and it does not decide verdicts.

What it owns, and why the C++ binary does not:

  * **The rounds and their order.** `bench_memtable_concurrent` runs exactly one
    (mode, readers) cell per invocation. This driver invokes it so that
    `(mode x readers)` interleaves *within* each round (AGENTS.md 8.20.2) --
    a block of all R=1 followed by a block of all R=7 confounds thermal drift
    with the effect the sweep is trying to see.

  * **The load snapshots.** A concurrent artifact owes, per timed cell,
    `load.foreign_busy_cpus` -- the host's busy CPU over that cell minus the
    runner's own children's -- and `host.scaling_governor_by_cpu` for the pin
    set (`scripts/check_bench_provenance.py`). A process boundary per cell is
    what makes that measurable at all, which is the other reason the binary
    emits one cell rather than a sweep.

  * **The intervals.** Paired BCa 95% bootstrap on `S(R) = T(R)/T(1)`, paired
    because both arms of the ratio come from the same round. The binary emits
    raw per-cell throughput and never an interval or a ratio.

Usage:
    python3 docs/benchmarks/rocksdb_memtable/scripts/concurrent_read_scaling.py \
        --out docs/benchmarks/rocksdb_memtable/results/baseline_concurrent_reads.json
    python3 docs/benchmarks/rocksdb_memtable/scripts/concurrent_read_scaling.py --quick
    python3 docs/benchmarks/rocksdb_memtable/scripts/concurrent_read_scaling.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_provenance as prov  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci  # noqa: E402

BENCH = REPO_ROOT / "integrations" / "rocksdb" / "build" / "bench_memtable_concurrent"
DEFAULT_OUT = REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable" / "results" / "baseline_concurrent_reads.json"

#: METHODOLOGY section 5.2. `idle` is the control; `free` is reported and never gated.
MODES = ("idle", "paced", "free")
#: One writer thread accompanies every cell, so R = 7 is 8 threads on 8 physical
#: P-cores. R = 8 would put 9 runnable threads on 8 cores.
READERS = (1, 2, 4, 7)
PACED_RATE = 250000.0
CSV_FIELDS = ("round", "writer_mode", "readers", "read_ops", "write_ops", "elapsed_s", "read_mops")


def parse_row(text: str) -> dict:
    """Parses the single CSV data row the binary emits, or raises.

    Fails loudly on a missing or malformed row rather than returning a default:
    a cell that produced nothing must stop the sweep, not contribute a zero
    (AGENTS.md 8.1).
    """
    rows = [ln.strip() for ln in text.splitlines()
            if ln.strip() and not ln.startswith("#") and not ln.startswith("round,")]
    if len(rows) != 1:
        raise RuntimeError(f"expected exactly one CSV data row, got {len(rows)}:\n{text}")
    parts = rows[0].split(",")
    if len(parts) != len(CSV_FIELDS):
        raise RuntimeError(f"expected {len(CSV_FIELDS)} CSV fields, got {len(parts)}: {rows[0]!r}")
    out = dict(zip(CSV_FIELDS, parts))
    return {
        "round": int(out["round"]),
        "writer_mode": out["writer_mode"],
        "readers": int(out["readers"]),
        "read_ops": int(out["read_ops"]),
        "write_ops": int(out["write_ops"]),
        "elapsed_s": float(out["elapsed_s"]),
        "read_mops": float(out["read_mops"]),
    }


def duty_cycle(write_ops: int, elapsed_s: float, insert_ns: float) -> float:
    """Writer's measured lock duty: achieved rate x insert cost.

    Computed from the achieved write rate, never from the offered one --
    METHODOLOGY section 5.2 requires the cell to record what the writer actually
    managed, because a paced writer can miss its schedule.
    """
    if elapsed_s <= 0.0:
        raise ValueError(f"elapsed_s must be > 0, got {elapsed_s}")
    return (write_ops / elapsed_s) * insert_ns * 1e-9


def scaling_ratios(rows: list[dict], mode: str) -> dict:
    """Paired BCa 95% interval on S(R) = T(R)/T(1) for one writer mode.

    Paired: the numerator and denominator of each resampled ratio come from the
    same round, so the interval carries the round-to-round covariance instead of
    treating the two arms as independent samples.
    """
    by_round: dict[int, dict[int, float]] = {}
    for r in rows:
        if r["writer_mode"] != mode:
            continue
        by_round.setdefault(r["round"], {})[r["readers"]] = r["read_mops"]

    out: dict[str, dict] = {}
    rounds = sorted(by_round)
    for R in sorted({r["readers"] for r in rows if r["writer_mode"] == mode}):
        if R == 1:
            continue
        per_round = []
        for rd in rounds:
            cell = by_round[rd]
            if 1 not in cell or R not in cell or cell[1] <= 0.0:
                continue
            per_round.append(cell[R] / cell[1])
        if len(per_round) < 3:
            # BCa needs n >= 3; say so rather than emit a degenerate interval.
            out[f"S({R})"] = {"point": None, "ci": None, "n": len(per_round),
                              "why_no_interval": "fewer than 3 paired rounds"}
            continue
        point, lo, hi = bca_bootstrap_ci(per_round)
        out[f"S({R})"] = {"point": point, "ci": [lo, hi], "n": len(per_round),
                          "rounds_raw": per_round}
    return out


def run_sweep(bench: Path, rounds: int, window_s: float, readers: tuple,
              modes: tuple, paced_rate: float, provenance: dict) -> list[dict]:
    rows: list[dict] = []
    # `new_provenance` already took the opening snapshot; a second one here
    # would leave two cells labelled `start` and make `since` ambiguous.
    for rd in range(rounds):
        # Interleave (mode x readers) within the round, not across it.
        for mode in modes:
            for R in readers:
                label = f"cell:{mode}:R{R}:round{rd}"
                start = prov.begin_cell(provenance, label)
                cmd = [str(bench), "--mode", mode, "--readers", str(R),
                       "--round", str(rd), "--window-seconds", str(window_s),
                       "--paced-rate", str(paced_rate)]
                res = subprocess.run(cmd, capture_output=True, text=True)
                if res.returncode != 0:
                    raise RuntimeError(
                        f"{label}: {' '.join(cmd)} exited {res.returncode}\n"
                        f"stdout:\n{res.stdout}\nstderr:\n{res.stderr}")
                row = parse_row(res.stdout)
                row["load"] = prov.end_cell(start)
                row["cell"] = label
                rows.append(row)
    prov.add_load(provenance, "end")
    return rows


def build_artifact(rows: list[dict], provenance: dict, insert_ns: float,
                   window_s: float, paced_rate: float) -> dict:
    cells = []
    for r in rows:
        c = dict(r)
        c["writer_duty_cycle"] = (
            duty_cycle(r["write_ops"], r["elapsed_s"], insert_ns)
            if r["writer_mode"] != "idle" else 0.0
        )
        c["rounds_raw"] = [{"round": r["round"], "read_mops": r["read_mops"],
                            "read_ops": r["read_ops"], "write_ops": r["write_ops"],
                            "elapsed_s": r["elapsed_s"]}]
        cells.append(c)

    payload = {
        "schema": "expanse.baseline.v1",
        "kind": "wall_clock_bca",
        "suite": "rocksdb_concurrent",
        "fixture": "integrations/rocksdb/benches/bench_memtable_concurrent.cc",
        "workload_id": "rocksdb_memtable_concurrent_read_scaling",
        "pre_registration": "docs/benchmarks/rocksdb_memtable/METHODOLOGY.md section 5",
        "settings": {"window_seconds": window_s, "paced_rate_ops_per_s": paced_rate,
                     "insert_ns_used_for_duty": insert_ns},
        "cells": cells,
        "scaling": {mode: scaling_ratios(rows, mode) for mode in sorted({r["writer_mode"] for r in rows})},
        "verdicts": None,
        "why_no_verdicts": (
            "Verdicts are read against METHODOLOGY section 5.3 by a reviewer; this driver "
            "emits the intervals and does not decide PASS/REFUTED/BOUNDARY_RESULT."
        ),
    }
    # `attach` RETURNS the carrying dict; it does not mutate in place. Dropping
    # the return shipped an artifact with no provenance block at all, which the
    # self-test below is what caught.
    return prov.attach(payload, provenance)


def self_test() -> int:
    fails: list[str] = []

    def check(name, got, want):
        if got != want:
            fails.append(f"{name}: got {got!r}, want {want!r}")

    # --- CSV parsing ------------------------------------------------------
    good = ("# rocksdb_memtable_concurrent_read_scaling\n"
            "round,writer_mode,readers,read_ops,write_ops,elapsed_s,read_mops\n"
            "2,paced,4,123456,6789,2.001000,0.0617\n")
    row = parse_row(good)
    check("round", row["round"], 2)
    check("mode", row["writer_mode"], "paced")
    check("readers", row["readers"], 4)
    check("read_ops", row["read_ops"], 123456)
    check("write_ops", row["write_ops"], 6789)
    for name, text in (("no data row", "# only a comment\n"),
                       ("two data rows", good + "3,idle,1,1,0,1.0,0.1\n"),
                       ("short row", "1,idle,2\n")):
        try:
            parse_row(text)
        except RuntimeError:
            pass
        except Exception as exc:  # noqa: BLE001
            fails.append(f"{name}: raised {type(exc).__name__}, expected RuntimeError")
        else:
            fails.append(f"{name}: did not raise")

    # --- duty cycle, from the ACHIEVED rate -------------------------------
    # 250,000 inserts in 1 s at 226.142 ns each = 5.654% of wall time.
    d = duty_cycle(250000, 1.0, 226.14201718)
    if abs(d - 0.05653550) > 1e-6:
        fails.append(f"duty_cycle: got {d}, want ~0.0565355")
    check("idle duty", duty_cycle(0, 1.0, 226.142), 0.0)
    try:
        duty_cycle(1, 0.0, 226.142)
    except ValueError:
        pass
    else:
        fails.append("duty_cycle with elapsed_s=0 did not raise")

    # --- paired scaling ---------------------------------------------------
    # A perfectly flat structure: every round has T(4) == T(1), so S(4) == 1.
    flat = []
    for rd in range(5):
        base = 1.0 + 0.01 * rd
        flat.append({"round": rd, "writer_mode": "paced", "readers": 1, "read_mops": base})
        flat.append({"round": rd, "writer_mode": "paced", "readers": 4, "read_mops": base})
    s_flat = scaling_ratios(flat, "paced")
    if abs(s_flat["S(4)"]["point"] - 1.0) > 1e-9:
        fails.append(f"flat S(4) point: {s_flat['S(4)']['point']}")
    lo, hi = s_flat["S(4)"]["ci"]
    if not (abs(lo - 1.0) < 1e-9 and abs(hi - 1.0) < 1e-9):
        fails.append(f"flat S(4) interval should be degenerate at 1.0, got [{lo}, {hi}]")
    # Pairing matters: the same marginals with the ratio varying per round must
    # still produce a ratio near 4, which an unpaired treatment would smear.
    linear = []
    for rd in range(5):
        base = 1.0 + 0.5 * rd          # strong round-to-round drift
        linear.append({"round": rd, "writer_mode": "paced", "readers": 1, "read_mops": base})
        linear.append({"round": rd, "writer_mode": "paced", "readers": 4, "read_mops": 4 * base})
    s_lin = scaling_ratios(linear, "paced")
    if abs(s_lin["S(4)"]["point"] - 4.0) > 1e-9:
        fails.append(f"paired S(4) under drift: {s_lin['S(4)']['point']} (pairing lost?)")
    # Too few rounds: report why, never a degenerate interval.
    few = [{"round": 0, "writer_mode": "idle", "readers": 1, "read_mops": 1.0},
           {"round": 0, "writer_mode": "idle", "readers": 2, "read_mops": 2.0}]
    s_few = scaling_ratios(few, "idle")
    if s_few["S(2)"]["ci"] is not None or "fewer than 3" not in s_few["S(2)"]["why_no_interval"]:
        fails.append(f"n<3 should carry no interval and say why: {s_few['S(2)']}")

    # --- artifact shape the provenance gate reads -------------------------
    rows = []
    for rd in range(4):
        for mode in ("idle", "paced"):
            for R in (1, 2):
                rows.append({"round": rd, "writer_mode": mode, "readers": R,
                             "read_ops": 1000 * R, "write_ops": 0 if mode == "idle" else 500,
                             "elapsed_s": 1.0, "read_mops": 0.1 * R,
                             "cell": f"cell:{mode}:R{R}:round{rd}",
                             "load": {"since": "start", "wall_s": 1.0,
                                      "busy_cpus_since_prev": 1.5, "own_busy_cpus": 1.2,
                                      "foreign_busy_cpus": 0.3}})
    p = prov.new_provenance("rocksdb_concurrent", 802, "T(R)/T(1)", repo_root=REPO_ROOT)
    art = build_artifact(rows, p, 226.142, 2.0, PACED_RATE)
    for key in ("schema", "cells", "scaling", "provenance"):
        if key not in art:
            fails.append(f"artifact missing {key}")
    if art.get("provenance", {}).get("host") is None:
        fails.append("artifact provenance carries no host block")
    if "estimators" not in art.get("provenance", {}):
        fails.append("artifact provenance carries no estimators block")
    for c in art["cells"]:
        if not c.get("rounds_raw"):
            fails.append("a cell carries no rounds_raw")
            break
        fb = c.get("load", {}).get("foreign_busy_cpus")
        if not isinstance(fb, (int, float)) or isinstance(fb, bool):
            fails.append("a cell carries no numeric load.foreign_busy_cpus")
            break
    # The idle control must record a zero duty, not a missing one.
    idle = [c for c in art["cells"] if c["writer_mode"] == "idle"]
    if not idle or any(c["writer_duty_cycle"] != 0.0 for c in idle):
        fails.append("idle cells must carry writer_duty_cycle 0.0")

    if fails:
        print("concurrent_read_scaling.py --self-test: FAILED")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("concurrent_read_scaling.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--bench", type=Path, default=BENCH)
    ap.add_argument("--rounds", type=int, default=5)
    ap.add_argument("--window-seconds", type=float, default=2.0)
    ap.add_argument("--paced-rate", type=float, default=PACED_RATE)
    ap.add_argument("--insert-ns", type=float, default=None,
                    help="insert cost used to turn an achieved write rate into a duty cycle; "
                         "defaults to the suite artifact's fillrandom cell")
    ap.add_argument("--quick", action="store_true",
                    help="smoke shape only: 1 round, short window, R in {1,2}. Writes under "
                         "results/quick/ so it cannot overwrite a committed baseline (8.5)")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    if not args.bench.is_file():
        print(f"::error::benchmark binary not found: {args.bench}\n"
              f"build it with: make -C integrations/rocksdb bench-concurrent", file=sys.stderr)
        return 1

    insert_ns = args.insert_ns
    if insert_ns is None:
        sys.path.insert(0, str(REPO_ROOT / "scripts"))
        from rocksdb_locate_bound import load_arms
        insert_ns = load_arms()["insert_ns"]

    readers, modes, rounds, window_s = READERS, MODES, args.rounds, args.window_seconds
    out = args.out
    if args.quick:
        readers, modes, rounds, window_s = (1, 2), ("idle", "paced"), 1, 0.25
        out = REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable" / "results" / "quick" / out.name

    provenance = prov.new_provenance(
        "rocksdb_concurrent", 802, "T(R)/T(1)", repo_root=REPO_ROOT,
        pre_registration="docs/benchmarks/rocksdb_memtable/METHODOLOGY.md section 5",
    )
    provenance["host"] = prov.host_facts(os.environ.get("EXPANSE_BENCH_PIN_APPLIED"))
    provenance["estimators"] = prov.estimators(
        ratio="paired BCa 95% on S(R) = T(R)/T(1), resampled over rounds",
        columns="per-cell aggregate read Mops/s",
        raw="rounds_raw",
    )

    rows = run_sweep(args.bench, rounds, window_s, readers, modes, args.paced_rate, provenance)

    # A concurrent artifact owes a numeric `load.foreign_busy_cpus` on every
    # timed cell (scripts/check_bench_provenance.py). The delta is read from
    # /proc/stat, which exists on the Linux reference host and not on a macOS
    # dev box -- where every cell would carry null and the artifact would be
    # rejected later, after the sweep had been paid for. Refuse now and name
    # the reason (AGENTS.md 8.1). `--quick` is exempt: it writes under
    # results/quick/ and is a shape smoke, never a baseline.
    if not args.quick:
        blind = [r["cell"] for r in rows
                 if not isinstance(r["load"].get("foreign_busy_cpus"), (int, float))
                 or isinstance(r["load"].get("foreign_busy_cpus"), bool)]
        if blind:
            print(f"::error::{len(blind)} of {len(rows)} cells carry no busy-CPU delta "
                  f"(first: {blind[0]}). The host exposes no /proc/stat, so this run cannot "
                  f"say whether anything else was resident while it was taken, and a "
                  f"committed artifact must (AGENTS.md 8.17). Run it on the reference host, "
                  f"or use --quick for a shape smoke under results/quick/.", file=sys.stderr)
            return 1

    art = build_artifact(rows, provenance, insert_ns, window_s, args.paced_rate)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(art, indent=2) + "\n")
    print(f"wrote {out} ({len(rows)} cells, {rounds} rounds)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
