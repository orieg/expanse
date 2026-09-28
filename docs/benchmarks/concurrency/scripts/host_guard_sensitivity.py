#!/usr/bin/env python3
"""The host guard's on-pin sensitivity experiment (Refs #1270, METHODOLOGY.md §25).

    host_guard_sensitivity.py run --out F.json [--old-guard-dir DIR] [--blocks 4] [--rounds 18]
    host_guard_sensitivity.py evaluate --in F.json
    host_guard_sensitivity.py inject --level L --cpus LIST --record F.json
    host_guard_sensitivity.py --self-test

`run` measures what a known, continuous foreign load on the pinned CPUs does
to the `concurrency` suite's cells, and what the host guard reads while it
does. For each block, and within it each arm in the block's row of a
four-treatment Williams square over (control, 0.10, 0.25, 0.50 CPUs):

- an injector is started outside this process's tree (a double fork, so no
  `RUSAGE_CHILDREN` of ours or of the suite's ever counts it), pinned to the
  pin set, burning `level` of one CPU in 10 ms periods on its own thread CPU
  clock, and recording the load it achieved;
- one `mixed_concurrency.py` process runs `map` and `set` at 100 % and 50 %
  reads and 1, 4 and 16 threads, `--rounds` rounds;
- `bench_host_guard.py watch` runs beside it with the suite's process as its
  own root, from this checkout and, with `--old-guard-dir`, from the version
  before #1270 as well; `summarize` of each record is kept verbatim.

An idle arm first (`watch --interval 1` over a `sleep`) records the guard's
1 s noise floor, and a negative-control arm last injects 1.00 CPU. Every
window of every cell goes into the artifact under `cells[].rounds_raw`, and
`evaluate` applies §25.4's decision rule to it.

`evaluate` is separate so the decision can be recomputed from the committed
artifact alone, by anyone, with nothing re-run.
"""

from __future__ import annotations

import argparse
import ctypes
import json
import math
import os
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_provenance as bp  # noqa: E402
from bca_bootstrap import bca_bootstrap_ratio_ci_with_method  # noqa: E402
import host_guard_bounds as hb  # noqa: E402

MIXED = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "scripts" / "mixed_concurrency.py"
GUARD = REPO_ROOT / "scripts" / "bench_host_guard.py"

# §25.3: arms, schedule, cells.
CONTROL = 0.0
LEVELS = (0.10, 0.25, 0.50)
ARMS = (CONTROL,) + LEVELS
NEGATIVE_CONTROL_LEVEL = 1.00
# Williams square for four treatments: every arm once per block, in every
# position once, and after every other arm once.
WILLIAMS4 = ((0, 1, 3, 2), (1, 2, 0, 3), (2, 3, 1, 0), (3, 0, 2, 1))
ENGINES = ("map", "set")
WORKLOADS = (100, 50)
THREADS = (1, 4, 16)
ROUNDS = 18
BLOCKS = 4
# Not gated: committed per-window CV above CV_GATE (§25.3).
CV_GATE = 0.05
UNGATED = {("map", 50, 16)}
FAMILY = 0.95
RESAMPLES = 2000
# The negative control's void, as `summarize` prints it (AGENTS.md section 5:
# string-gated, never exit status alone).
VOID_STRING = "the host was disturbed during the run"
PERIOD_S = 0.010
IDLE_WINDOWS = 60
# §25.5: `user`-class load that voids the run, mean over an arm and per sample.
USER_MEAN_VOID = 0.10
USER_SAMPLE_VOID = 1.0


class ExperimentError(RuntimeError):
    """A run that cannot produce an admissible artifact (AGENTS.md section 8.1)."""


def cell_keys() -> list[tuple[str, int, int]]:
    return [(e, w, t) for e in ENGINES for w in WORKLOADS for t in THREADS]


def gated_cells() -> list[tuple[str, int, int]]:
    return [c for c in cell_keys() if c not in UNGATED]


def window_total(row: dict) -> float:
    return (row["read_ops"] + row["write_ops"]) / row["elapsed_s"]


# --------------------------------------------------------------------------
# injector
# --------------------------------------------------------------------------
def cmd_inject(args) -> int:
    """Burn `level` of one CPU until SIGTERM, then record what was achieved."""
    level = float(args.level)
    if not 0 < level <= 1:
        raise ExperimentError(f"--level must be in (0, 1], got {level}")
    os.sched_setaffinity(0, bp.expand_cpu_list(args.cpus))
    try:
        libc = ctypes.CDLL(None, use_errno=True)
        libc.prctl(15, b"guard-inject", 0, 0, 0)  # PR_SET_NAME: a name the attribution can show
    except (OSError, AttributeError) as exc:
        raise ExperimentError(f"cannot name the injector: {exc}") from exc
    stop = False

    def _stop(_s, _f):
        nonlocal stop
        stop = True

    signal.signal(signal.SIGTERM, _stop)
    rec = Path(args.record)
    rec.write_text(json.dumps({"pid": os.getpid(), "level": level}) + "\n")
    wall0, cpu0 = time.monotonic(), time.thread_time()
    next_period = wall0
    while not stop:
        burn_until = time.thread_time() + level * PERIOD_S
        while time.thread_time() < burn_until and not stop:
            pass
        next_period += PERIOD_S
        delay = next_period - time.monotonic()
        if delay > 0:
            time.sleep(delay)
        else:
            next_period = time.monotonic()  # fell behind: do not try to catch up in a burst
    wall, cpu = time.monotonic() - wall0, time.thread_time() - cpu0
    rec.write_text(json.dumps({"pid": os.getpid(), "level": level, "wall_s": round(wall, 3),
                               "cpu_s": round(cpu, 3), "achieved_cpus": round(cpu / wall, 4)}) + "\n")
    return 0


def start_injector(level: float, cpus: str, record: Path) -> int:
    """Starts an injector that is not our child, and returns its PID."""
    record.unlink(missing_ok=True)
    subprocess.run(["setsid", "-f", sys.executable, str(Path(__file__).resolve()), "inject",
                    "--level", str(level), "--cpus", cpus, "--record", str(record)], check=True)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if record.exists() and record.read_text().strip():
            return int(json.loads(record.read_text())["pid"])
        time.sleep(0.05)
    raise ExperimentError(f"the injector did not start (no {record})")


def stop_injector(pid: int, record: Path) -> dict:
    os.kill(pid, signal.SIGTERM)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        rec = json.loads(record.read_text())
        if "achieved_cpus" in rec:
            return rec
        time.sleep(0.05)
    raise ExperimentError(f"the injector (pid {pid}) did not record its achieved load")


# --------------------------------------------------------------------------
# run
# --------------------------------------------------------------------------
def _guards(old_dir: Path | None) -> dict[str, Path]:
    out = {"new": GUARD}
    if old_dir is not None:
        old = old_dir / "bench_host_guard.py"
        if not old.exists():
            raise ExperimentError(f"--old-guard-dir has no bench_host_guard.py: {old_dir}")
        out["old"] = old
    return out


def _summarize(guard: Path, record: Path) -> dict:
    p = subprocess.run([sys.executable, str(guard), "summarize", "--in", str(record)],
                       capture_output=True, text=True)
    samples = [json.loads(line) for line in record.read_text().splitlines() if line.strip()] if record.exists() else []
    return {"exit": p.returncode, "output": p.stdout + p.stderr, "samples": samples}


def run_arm(label: str, level: float, block: int | None, pin: str, work: Path,
            guards: dict[str, Path], rounds: int, prov: dict) -> dict:
    tag = f"{label}_b{block}" if block is not None else label
    inj_rec = work / f"inject_{tag}.json"
    out = work / f"mixed_{tag}.json"
    start = bp.begin_cell(prov, f"arm:{tag}")
    inj_pid = start_injector(level, pin, inj_rec) if level > 0 else None
    try:
        mc = subprocess.Popen([sys.executable, str(MIXED), "--engines", ",".join(ENGINES),
                               "--workloads", ",".join(map(str, WORKLOADS)),
                               "--threads", ",".join(map(str, THREADS)),
                               "--rounds", str(rounds), "--out", str(out)])
        watchers = {}
        for name, guard in guards.items():
            rec = work / f"activity_{name}_{tag}.jsonl"
            rec.unlink(missing_ok=True)
            watchers[name] = (subprocess.Popen([sys.executable, str(guard), "watch", "--own-root", str(mc.pid),
                                                "--pin-cpus", pin, "--out", str(rec)]), rec)
        if mc.wait() != 0:
            raise ExperimentError(f"mixed_concurrency.py failed in arm {tag} (exit {mc.returncode})")
        for proc, _rec in watchers.values():
            proc.wait(timeout=30)
    finally:
        injected = stop_injector(inj_pid, inj_rec) if inj_pid is not None else None
    load = bp.end_cell(start)
    artifact = json.loads(out.read_text())
    return {
        "arm": label, "level": level, "block": block, "injector": injected, "load": load,
        "suite_provenance_commit": artifact["provenance"].get("commit"),
        "suite_loads": artifact["provenance"].get("loads"),
        "throughput": artifact["throughput"],
        "guards": {name: _summarize(guards[name], rec) for name, (_p, rec) in watchers.items()},
    }


def run_idle(pin: str, work: Path, guards: dict[str, Path], prov: dict) -> dict:
    start = bp.begin_cell(prov, "arm:idle")
    sleeper = subprocess.Popen(["sleep", str(IDLE_WINDOWS + 2)])
    watchers = {}
    for name, guard in guards.items():
        rec = work / f"activity_{name}_idle.jsonl"
        rec.unlink(missing_ok=True)
        watchers[name] = (subprocess.Popen([sys.executable, str(guard), "watch", "--own-root", str(sleeper.pid),
                                            "--pin-cpus", pin, "--interval", "1", "--out", str(rec)]), rec)
    sleeper.wait()
    for proc, _rec in watchers.values():
        proc.wait(timeout=30)
    return {"arm": "idle", "load": bp.end_cell(start),
            "guards": {name: _summarize(guards[name], rec) for name, (_p, rec) in watchers.items()}}


def cmd_run(args) -> int:
    if not Path("/proc/stat").exists():
        raise ExperimentError("the experiment reads /proc; it runs on the Linux reference host only")
    if not os.environ.get("EXPANSE_BENCH_LOCK_HELD"):
        raise ExperimentError("run under scripts/bench_lock.py (docs/BENCHMARKING.md rule 8)")
    if args.blocks != BLOCKS or args.rounds != ROUNDS:
        print(f"::warning::--blocks {args.blocks} --rounds {args.rounds} differ from §25.3's "
              f"{BLOCKS} and {ROUNDS}; the artifact is marked not admissible")
    sys.path.insert(0, str(GUARD.parent))
    from bench_host_guard import resolve_pin

    pin = ",".join(map(str, resolve_pin()))
    guards = _guards(Path(args.old_guard_dir) if args.old_guard_dir else None)
    prov = bp.new_provenance("concurrency", 1270, "mean(T injected) / mean(T control), "
                             "T = (read + write) ops per second over one window's own elapsed time; "
                             "windows pooled over blocks, two-sample BCa", REPO_ROOT)
    prov["host"] = bp.host_facts(pin)
    prov["pin_cpus"] = pin
    prov["protocol"] = {"section": "METHODOLOGY.md §25", "blocks": args.blocks, "rounds": args.rounds,
                        "arms": list(ARMS), "negative_control_level": NEGATIVE_CONTROL_LEVEL,
                        "williams": [list(r) for r in WILLIAMS4[:args.blocks]],
                        "admissible_shape": args.blocks == BLOCKS and args.rounds == ROUNDS,
                        "old_guard": "bench_host_guard.py before #1270" if "old" in guards else None}
    work = Path(tempfile.mkdtemp(prefix="guard-sens-", dir=args.work_dir))
    arms = [run_idle(pin, work, guards, prov)]
    for b in range(args.blocks):
        for a in WILLIAMS4[b % len(WILLIAMS4)]:
            level = ARMS[a]
            label = "control" if level == CONTROL else f"inject_{level:.2f}"
            print(f"== block {b}: {label}", flush=True)
            arms.append(run_arm(label, level, b, pin, work, guards, args.rounds, prov))
    print("== negative control", flush=True)
    arms.append(run_arm(f"inject_{NEGATIVE_CONTROL_LEVEL:.2f}", NEGATIVE_CONTROL_LEVEL, None, pin, work,
                        guards, args.rounds, prov))
    bp.add_load(prov, "end")
    artifact = assemble(prov, arms)
    artifact["evaluation"] = evaluate(artifact)
    Path(args.out).write_text(json.dumps(artifact, indent=1) + "\n")
    print("\n".join(report(artifact["evaluation"])))
    return 0


def assemble(prov: dict, arms: list[dict]) -> dict:
    """The artifact: one cell per (arm, block, engine, workload, threads)."""
    cells = []
    for arm in arms:
        for c in arm.get("throughput", []):
            cells.append({
                "arm": arm["arm"], "level": arm["level"], "block": arm["block"],
                "engine_key": c["engine_key"], "read_pct": c["read_pct"], "threads": c["threads"],
                "total_ops_s_mean": c["total_ops_s_mean"], "load": c["load"], "rounds_raw": c["rounds_raw"],
            })
    slim = []
    for arm in arms:
        slim.append({k: v for k, v in arm.items() if k != "throughput"})
    return {"provenance": prov, "arms": slim, "cells": cells}


# --------------------------------------------------------------------------
# evaluate: §25.4
# --------------------------------------------------------------------------
def _windows(cells: list[dict], level: float, key: tuple, blocks=None) -> list[float]:
    e, w, t = key
    out = []
    for c in cells:
        if (c["level"] == level and c["engine_key"] == e and c["read_pct"] == w and c["threads"] == t
                and c["block"] is not None and (blocks is None or c["block"] in blocks)):
            out.extend(window_total(r) for r in c["rounds_raw"])
    return out


def _mean(xs):
    return sum(xs) / len(xs)


def evaluate(artifact: dict) -> dict:
    cells = artifact["cells"]
    prov = artifact["provenance"]
    blocks = sorted({c["block"] for c in cells if c["block"] is not None})
    gated = gated_cells()
    conf = hb.bonferroni_confidence(len(gated), FAMILY)
    out: dict = {"confidence": conf, "gated_cells": [list(k) for k in gated], "levels": {}}

    def ratio(num, den):
        r, lo, hi, method = bca_bootstrap_ratio_ci_with_method(num, den, conf, RESAMPLES)
        return {"ratio": r, "ci_lower": lo, "ci_upper": hi, "ci_method": method, "n": [len(num), len(den)]}

    # A/A: the control's first half of blocks against its second.
    half = len(blocks) // 2
    aa = {}
    for key in gated:
        r = ratio(_windows(cells, CONTROL, key, blocks[:half]), _windows(cells, CONTROL, key, blocks[half:]))
        r["excludes_1"] = r["ci_lower"] > 1 or r["ci_upper"] < 1
        aa["/".join(map(str, key))] = r
    out["a_a"] = aa
    aa_flagged = [k for k, r in aa.items() if r["excludes_1"]]

    harmful_levels = []
    for level in LEVELS:
        per_cell = {}
        for key in cell_keys():
            inj, ctl = _windows(cells, level, key), _windows(cells, CONTROL, key)
            r = ratio(inj, ctl)
            per_block = []
            for b in blocks:
                bi, bc = _windows(cells, level, key, [b]), _windows(cells, CONTROL, key, [b])
                per_block.append(_mean(bi) / _mean(bc))
            r["per_block"] = per_block
            r["blocks_below_1"] = sum(1 for x in per_block if x < 1)
            r["gated"] = key in gated
            r["harmful"] = (r["gated"] and r["ci_upper"] < 1
                            and r["blocks_below_1"] >= math.ceil(0.75 * len(blocks)))
            r["fair_share_loss"] = hb.fair_share_loss(level, key[2], hb.PIN_CPUS)
            per_cell["/".join(map(str, key))] = r
        harmful = any(r["harmful"] for r in per_cell.values())
        if harmful:
            harmful_levels.append(level)
        out["levels"][f"{level:.2f}"] = {"harmful": harmful, "cells": per_cell}

    current = bp.RUN_ON_PIN_VOID
    if aa_flagged:
        verdict, boundary = "INCONCLUSIVE", current
    else:
        verdict = "DECIDED"
        boundary = None
        for level in LEVELS:
            if level in harmful_levels:
                break
            boundary = level
        if boundary is None:
            boundary = LEVELS[0]
            verdict = "DECIDED_BELOW_RESOLUTION"
    out["verdict"] = verdict
    out["aa_flagged"] = aa_flagged
    out["boundary"] = boundary
    out["start_on_pin_max"] = round(min(0.1, 0.4 * boundary), 4)

    # The guard's readings, per guard and arm.
    readings: dict = {}
    for arm in artifact["arms"]:
        for name, g in arm.get("guards", {}).items():
            on = [s["on_pin_foreign_busy_cpus"] for s in g["samples"]]
            if not on:
                continue
            tag = arm["arm"] + ("" if arm.get("block") is None else f"_b{arm['block']}")
            readings.setdefault(name, {})[tag] = {
                "n": len(on), "mean": round(_mean(on), 4), "min": min(on), "max": max(on),
                "negative": sum(1 for x in on if x < 0),
                "over_boundary": sum(1 for x in on if x > boundary),
                "over_current": sum(1 for x in on if x > current),
                "summarize_exit": g["exit"],
                "achieved_cpus": (arm.get("injector") or {}).get("achieved_cpus"),
            }
    out["guard_readings"] = readings

    neg = [a for a in artifact["arms"] if a.get("block") is None and a["arm"].startswith("inject_")]
    nc = {"ran": bool(neg)}
    if neg:
        g = neg[0]["guards"]["new"]
        samples = g["samples"]
        on_pin_tasks = [sum(s["attribution"]["classes_on_pin"].values()) for s in samples if "attribution" in s]
        nc.update({
            "summarize_exit": g["exit"],
            "void_string_present": VOID_STRING in g["output"],
            "over_boundary": sum(1 for s in samples if s["on_pin_foreign_busy_cpus"] > boundary),
            "samples": len(samples),
            "mean_on_pin_task_cpus": round(_mean(on_pin_tasks), 4) if on_pin_tasks else None,
            "achieved_cpus": (neg[0].get("injector") or {}).get("achieved_cpus"),
        })
        nc["pass"] = (g["exit"] == 1 and nc["void_string_present"] and nc["over_boundary"] > 0)
    out["negative_control"] = nc
    # §25.5: load from outside the experiment. The injector and the old guard
    # run in the driver's session and are classed `runner`; `user` is
    # everything else that is not a kernel thread.
    contaminated = []
    for arm in artifact["arms"]:
        g = arm.get("guards", {}).get("new")
        if not g:
            continue
        users = [s["attribution"]["classes"]["user"] for s in g["samples"] if "attribution" in s]
        if users and (_mean(users) > USER_MEAN_VOID or max(users) > USER_SAMPLE_VOID):
            contaminated.append({"arm": arm["arm"], "block": arm.get("block"),
                                 "user_mean": round(_mean(users), 4), "user_max": max(users)})
    out["contaminated_arms"] = contaminated
    governors = sorted({gv for a in artifact["arms"] for g in a.get("guards", {}).values()
                        for s in g["samples"] for gv in (s.get("governor") or [])})
    out["governors_seen"] = governors
    out["admissible"] = (bool(prov.get("protocol", {}).get("admissible_shape")) and len(governors) <= 1
                         and not contaminated and nc.get("pass", False))
    return out


def report(ev: dict) -> list[str]:
    lines = [f"verdict {ev['verdict']}; boundary {ev['boundary']}; start gate {ev['start_on_pin_max']}; "
             f"per-cell confidence {ev['confidence']:.5f}; admissible {ev['admissible']}"]
    if ev["aa_flagged"]:
        lines.append(f"A/A flagged: {', '.join(ev['aa_flagged'])}")
    for level, lv in ev["levels"].items():
        lines.append(f"-- {level} CPUs: {'HARMFUL' if lv['harmful'] else 'no gated cell harmed'}")
        for key, r in lv["cells"].items():
            lines.append(f"   {key:<12} {r['ratio']:.4f} [{r['ci_lower']:.4f}, {r['ci_upper']:.4f}] {r['ci_method']}"
                         f"  blocks<1 {r['blocks_below_1']}/{len(r['per_block'])}"
                         f"  fair-share {-r['fair_share_loss']:+.4f}{'' if r['gated'] else '  (not gated)'}"
                         f"{'  HARMFUL' if r['harmful'] else ''}")
    for name, arms in ev["guard_readings"].items():
        for tag, g in arms.items():
            lines.append(f"guard {name} {tag}: n {g['n']} mean {g['mean']:+.3f} min {g['min']:+.3f} "
                         f"max {g['max']:+.3f} negative {g['negative']} over boundary {g['over_boundary']} "
                         f"summarize exit {g['summarize_exit']} achieved {g['achieved_cpus']}")
    lines.append(f"negative control: {ev['negative_control']}")
    return lines


def cmd_evaluate(args) -> int:
    artifact = json.loads(Path(args.inp).read_text())
    ev = evaluate(artifact)
    print("\n".join(report(ev)))
    if artifact.get("evaluation") and artifact["evaluation"] != json.loads(json.dumps(ev)):
        print("::error::the artifact's recorded evaluation differs from a fresh one")
        return 1
    return 0


# --------------------------------------------------------------------------
# self-test: the decision rule on synthetic windows
# --------------------------------------------------------------------------
def _synthetic(effects: dict[float, float], aa_shift: float = 0.0, seed: int = 7, rounds: int = 6) -> dict:
    """Windows around 100 with CV 0.005, `rounds` per block; `effects[level]`
    scales every W = 16 cell of that level; `aa_shift` scales the control's
    second half of blocks."""
    import random

    rng = random.Random(seed)
    cells = []
    for b in range(BLOCKS):
        for level in ARMS:
            for (e, w, t) in cell_keys():
                scale = 1.0
                if level in effects and t == 16:
                    scale *= effects[level]
                if level == CONTROL and b >= BLOCKS // 2:
                    scale *= 1 + aa_shift
                rows = [{"read_ops": 100.0 * scale * (1 + rng.gauss(0, 0.005)), "write_ops": 0.0, "elapsed_s": 1.0}
                        for _ in range(rounds)]
                cells.append({"arm": "x", "level": level, "block": b, "engine_key": e, "read_pct": w,
                              "threads": t, "rounds_raw": rows})
    return {"provenance": {"protocol": {"admissible_shape": True}}, "arms": [], "cells": cells}


def self_test() -> int:
    # The schedule is a Williams square: each arm once per block and once per
    # position, and each ordered pair of neighbours exactly once.
    for row in WILLIAMS4:
        assert sorted(row) == [0, 1, 2, 3], row
    for pos in range(4):
        assert sorted(r[pos] for r in WILLIAMS4) == [0, 1, 2, 3]
    pairs = [(r[i], r[i + 1]) for r in WILLIAMS4 for i in range(3)]
    assert len(set(pairs)) == 12 == len(pairs), pairs
    assert len(gated_cells()) == hb.GATED_CELLS == 11
    assert ("map", 50, 16) not in gated_cells()

    global RESAMPLES
    saved, RESAMPLES = RESAMPLES, 1000
    try:
        # No effect anywhere: nothing harmed, the boundary is the largest level.
        ev = evaluate(_synthetic({}))
        assert ev["verdict"] == "DECIDED" and ev["boundary"] == 0.50, (ev["verdict"], ev["boundary"])
        assert ev["start_on_pin_max"] == 0.1
        assert not ev["admissible"], "no negative control ran, so the run is not admissible"
        # 0.50 costs W = 16 cells 3%: the boundary is 0.25.
        ev = evaluate(_synthetic({0.50: 0.97}))
        assert ev["boundary"] == 0.25 and ev["levels"]["0.50"]["harmful"], ev["boundary"]
        assert ev["levels"]["0.50"]["cells"]["map/100/16"]["harmful"]
        assert not ev["levels"]["0.50"]["cells"]["map/100/1"]["harmful"]
        # A harmed ungated cell alone does not decide anything.
        assert not ev["levels"]["0.25"]["harmful"]
        # 0.25 harms, 0.50 does not: the boundary stops at the first harmful level.
        ev = evaluate(_synthetic({0.25: 0.97}))
        assert ev["boundary"] == 0.10, ev["boundary"]
        assert ev["start_on_pin_max"] == 0.04, ev["start_on_pin_max"]
        # Every level harms: the boundary is the smallest level tested, never
        # an extrapolation below it.
        ev = evaluate(_synthetic({0.10: 0.97, 0.25: 0.96, 0.50: 0.95}))
        assert ev["verdict"] == "DECIDED_BELOW_RESOLUTION" and ev["boundary"] == 0.10, ev
        # The control drifting between halves makes the run inconclusive and
        # leaves the boundary where it is.
        ev = evaluate(_synthetic({0.50: 0.97}, aa_shift=0.03))
        assert ev["verdict"] == "INCONCLUSIVE" and ev["boundary"] == bp.RUN_ON_PIN_VOID, ev["verdict"]
    finally:
        RESAMPLES = saved

    # The negative control is string-gated: exit 1 without the diagnostic
    # string does not pass.
    art = _synthetic({})
    sample = {"on_pin_foreign_busy_cpus": 0.9,
              "attribution": {"classes": {"runner": 1.0, "user": 0.0}, "classes_on_pin": {"runner": 0.9}}}
    art["arms"] = [{"arm": "inject_1.00", "block": None, "injector": {"achieved_cpus": 0.99},
                    "guards": {"new": {"exit": 1, "output": "crashed", "samples": [sample]}}}]
    saved, RESAMPLES = RESAMPLES, 1000
    try:
        assert not evaluate(art)["negative_control"]["pass"]
        art["arms"][0]["guards"]["new"]["output"] = f"::error::{VOID_STRING} (1 sample(s) ...)"
        ev = evaluate(art)
        assert ev["negative_control"]["pass"] and ev["admissible"] and not ev["contaminated_arms"], ev
        # Load from outside the experiment (`user` class) voids the run.
        noisy = {"on_pin_foreign_busy_cpus": 0.0, "attribution": {"classes": {"user": 0.2}, "classes_on_pin": {}}}
        art["arms"].append({"arm": "control", "block": 0, "guards": {"new": {"exit": 0, "output": "", "samples": [noisy]}}})
        ev = evaluate(art)
        assert ev["contaminated_arms"] and not ev["admissible"], ev["contaminated_arms"]
    finally:
        RESAMPLES = saved
    print("host_guard_sensitivity.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    r = sub.add_parser("run")
    r.add_argument("--out", required=True)
    r.add_argument("--old-guard-dir")
    r.add_argument("--blocks", type=int, default=BLOCKS)
    r.add_argument("--rounds", type=int, default=ROUNDS)
    r.add_argument("--work-dir", default=None)
    e = sub.add_parser("evaluate")
    e.add_argument("--in", dest="inp", required=True)
    i = sub.add_parser("inject")
    i.add_argument("--level", required=True)
    i.add_argument("--cpus", required=True)
    i.add_argument("--record", required=True)
    args = ap.parse_args()
    try:
        if args.self_test:
            return self_test()
        if args.cmd == "run":
            return cmd_run(args)
        if args.cmd == "evaluate":
            return cmd_evaluate(args)
        if args.cmd == "inject":
            return cmd_inject(args)
    except ExperimentError as exc:
        sys.stderr.write(f"host_guard_sensitivity.py: {exc}\n")
        return 1
    ap.error("a command is required")
    return 2


if __name__ == "__main__":
    sys.exit(main())
