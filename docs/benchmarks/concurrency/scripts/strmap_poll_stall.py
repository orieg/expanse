#!/usr/bin/env python3
"""METHODOLOGY §44: how long a periodic `SyncExpanseStrMap::mem_used()` reading stalls writers (#1586).

Runs `crates/expanse/benches/strmap_poll_stall.rs` once per (cell, arm, round),
each in its own process: a cell is (shape, N, W, T), and its three arms
(`none`, `len`, `mem_used`) are separate processes because each one prefills
its own map, so no arm inherits the keys another arm's writers inserted. Each
round runs every (cell, arm) once, with the cell order rotated by a fifth of
the list per round and the arm order within a cell rotated per round, so the
arms are interleaved within each round. The three arms of a cell share one
seed per round.

Each process confines its threads one per CPU (`--cpus`): the poller first,
on a physical core of its own, then the writers on the other cores and, once
those run out, on SMT siblings (§44.3). The list is derived from the pin
`bench_pin.apply` returns and the kernel's core topology, and the artifact
records it.

Before every process, timed or counted, the driver checks the hourly blackout
(minutes :15-:20 UTC, when the reference host runs an rsync, METHODOLOGY §44.8):
if the process, its prefill included, could reach :15, or would start inside the
blackout, it waits until :20. Each pause is recorded under
`provenance.hour_guard.pauses` with its `start_t` and `end_t` (UTC epoch
seconds), and every measured process under `provenance.hour_guard.processes`
with its own. A pause starts only after the previous process has exited (each
process runs through a blocking `subprocess.run`) and ends before the next one
starts, and the driver refuses to write an artifact in which any pause
overlaps a process (`bench_host_guard.pause_problems`). The workflow's host
guard reads these intervals (`bench_host_guard.py summarize --pauses`) and
does not judge a sample taken wholly inside one (METHODOLOGY §44.9).

After the timed rounds, one untimed pass of the `occ-stats` build runs each
`mem_used` cell once and records the gate counters (`GateBlockedEntries`,
`GateWaitCycles`, `QuiesceDrainCycles`) with `cycles_hz()`; no reported
figure or verdict reads them.

It then evaluates S1 and M1 exactly as §44.4 words them. S1 holds iff, in
the `mem_used` arm at N = 2·10⁶, W = 4, T = 1 s, the median over rounds of
the largest insert latency is at least 0.5 × the median poll duration, on
both shapes. M1 is read at N = 2·10⁷, W = 4, T = 1 s per shape: `MATERIAL`
iff the lower bound of `stalled_fraction` >= 0.01 or the lower bound of the
`mem_used` arm's insert p999 >= 10 × the upper bound of the `len` arm's
insert p999; `NOT_MATERIAL` iff both upper bounds lie below their thresholds;
`INTERMEDIATE` otherwise. §44.4 fixes no shape for M1, so the run is
`MATERIAL` if either shape is, `NOT_MATERIAL` if both are, `INTERMEDIATE`
otherwise. A verdict whose cells are contaminated (§8.17), short of rounds,
without a poll or an insert, or not run under the registered placement is
`VOID`.

`--quick` runs the same code on a reduced grid (§44's coordinates mapped onto
it) to scratch output only, refuses any path under `docs/`, and its verdicts
are `VOID` by construction: it is a smoke test, not a measurement.

Usage:
    python3 docs/benchmarks/concurrency/scripts/strmap_poll_stall.py --out ARTIFACT.json
    python3 docs/benchmarks/concurrency/scripts/strmap_poll_stall.py --quick --out SCRATCH.json
    python3 docs/benchmarks/concurrency/scripts/strmap_poll_stall.py --self-test
"""

from __future__ import annotations

import argparse
import itertools
import json
import os
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_host_guard  # noqa: E402
import bench_pin  # noqa: E402
import check_bench_provenance  # noqa: E402
import strmap_stall_bounds as bounds  # noqa: E402
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ci_with_method  # noqa: E402
from bench_provenance import begin_cell, end_cell, new_provenance  # noqa: E402

HARNESS = "strmap_poll_stall"
HARNESS_REL = "crates/expanse/benches/strmap_poll_stall.rs"
ARTIFACT_REL = "concurrency/results/baseline_strmap_poll_stall.json"
PREREGISTRATION = "concurrency METHODOLOGY §44"

# §44.3's grid and §44.6's constants.
SHAPES = ("routes", "short")
KEYS = (2_000_000, 20_000_000)
WRITERS = (1, 4, 8)
PERIODS_MS = (1000, 100)
SECONDS = 30
ROUNDS = 5
ARMS = ("none", "len", "mem_used")
POLLER_ARMS = ("len", "mem_used")
BCA_RESAMPLES = 2000  # bca_bootstrap's default, >= 1,000 (§44.4)
# §44.4's decision points and thresholds.
S1_AT = {"keys": 2_000_000, "writers": 4, "period_ms": 1000}
M1_AT = {"keys": 20_000_000, "writers": 4, "period_ms": 1000}
S1_FACTOR = 0.5
M1_STALLED_FRACTION = 0.01
M1_P999_FACTOR = 10.0
# §8.17: a non-target process above about one core, or a load average above cores/2.
FOREIGN_BUSY_VOID = 1.0

# The reference host runs an hourly rsync at minutes :15-:20 UTC (METHODOLOGY §44.8): no
# process, prefill or window, may overlap it. A process's length is estimated from its
# (shape, N)'s largest prefill seen so far, times PREFILL_MARGIN, or PREFILL_S_PER_KEY per key
# before the first one, plus the window and POST_S for the closing readings and teardown.
HOUR_BLACKOUT_S = (15 * 60, 20 * 60)
PREFILL_S_PER_KEY = 5e-6
PREFILL_MARGIN = 1.5
POST_S = 30.0

# `--quick`: a smoke run of the same code on a reduced grid, not a measurement.
QUICK = {
    "keys": (20_000, 200_000), "writers": (1, 4), "periods_ms": (250, 50), "seconds": 1, "rounds": 3,
}


class DriverError(Exception):
    pass


def grid(quick: bool = False) -> dict[str, Any]:
    """The cell coordinates, and where S1 and M1 are read, for a full or a quick run."""
    if not quick:
        return {"keys": KEYS, "writers": WRITERS, "periods_ms": PERIODS_MS, "seconds": SECONDS,
                "rounds": ROUNDS, "s1": dict(S1_AT), "m1": dict(M1_AT)}
    q = QUICK
    return {"keys": q["keys"], "writers": q["writers"], "periods_ms": q["periods_ms"],
            "seconds": q["seconds"], "rounds": q["rounds"],
            "s1": {"keys": q["keys"][0], "writers": 4, "period_ms": q["periods_ms"][0]},
            "m1": {"keys": q["keys"][1], "writers": 4, "period_ms": q["periods_ms"][0]}}


def cells_for(g: dict[str, Any]) -> list[dict[str, Any]]:
    return [{"shape": s, "keys": n, "writers": w, "period_ms": t}
            for s, n, w, t in itertools.product(SHAPES, g["keys"], g["writers"], g["periods_ms"])]


def cell_id(c: dict[str, Any], arm: str | None = None) -> str:
    base = f"{c['shape']}:N{c['keys']}:W{c['writers']}:T{c['period_ms']}ms"
    return base if arm is None else f"{base}:{arm}"


def round_order(cells: list[dict[str, Any]], r: int, rounds: int) -> list[tuple[dict[str, Any], str]]:
    """Round `r`'s (cell, arm) sequence: cells rotated by r/rounds of the list, arms by r."""
    k = (r * len(cells)) // rounds
    rotated = cells[k:] + cells[:k]
    a = r % len(ARMS)
    arms = ARMS[a:] + ARMS[:a]
    return [(c, arm) for c in rotated for arm in arms]


# --- Placement --------------------------------------------------------------

def core_groups(cpus: list[int], siblings: dict[int, list[int]]) -> list[list[int]]:
    """`cpus` grouped by physical core (`siblings[c]` lists c's core), cores in CPU order."""
    seen: set[int] = set()
    groups = []
    for c in sorted(cpus):
        if c in seen:
            continue
        g = sorted(x for x in siblings.get(c, [c]) if x in cpus)
        if c not in g:
            g = sorted(set(g) | {c})
        seen.update(g)
        groups.append(g)
    return groups


def placement(cpus: list[int], siblings: dict[int, list[int]], writers: int) -> list[int]:
    """The CPU list the harness takes as `--cpus`: the poller first, then one per writer.

    The poller takes the first CPU of the first core. Writers take the first
    CPU of every other core, then those cores' SMT siblings, and only then the
    poller's sibling, so the poller keeps a core of its own for as long as the
    pin allows. Refuses a pin too small for the poller and the writers (§8.1).
    """
    groups = core_groups(cpus, siblings)
    if not groups:
        raise DriverError("the pin names no CPUs")
    poller = groups[0][0]
    order = [g[0] for g in groups[1:]]
    order += [x for g in groups[1:] for x in g[1:]]
    order += groups[0][1:]
    if len(order) < writers:
        raise DriverError(f"the pin {cpus} has {len(order)} CPUs besides the poller's, fewer than {writers} writers")
    return [poller, *order[:writers]]


def read_siblings(cpus: list[int]) -> dict[int, list[int]] | None:
    """The kernel's core siblings for each CPU, or None where sysfs does not publish them."""
    out: dict[int, list[int]] = {}
    for c in cpus:
        p = Path(f"/sys/devices/system/cpu/cpu{c}/topology/thread_siblings_list")
        if not p.is_file():
            return None
        out[c] = bench_pin.expand(p.read_text().strip())
    return out


def host_cpus(applied: str) -> list[int] | None:
    """The CPUs the run may place threads on: the applied pin, else the process's affinity."""
    if not hasattr(os, "sched_getaffinity"):
        return None
    if applied not in ("none", "off"):
        return bench_pin.expand(applied)
    return sorted(os.sched_getaffinity(0))


# --- The hourly pause --------------------------------------------------------

def hour_guard_wait(now_s: float, est_s: float, blackout: tuple[int, int] = HOUR_BLACKOUT_S) -> float:
    """Seconds to wait so that `[start, start + est_s]` overlaps no hour's blackout.

    `now_s` is a UTC epoch time. A process that would reach a blackout's start, or
    that would start inside one, waits until that blackout ends (:20), and the
    check repeats from there. Inclusive at :15: a process due to end exactly at
    :15:00 waits.
    """
    if est_s < 0:
        raise DriverError(f"a process estimate must be >= 0, got {est_s}")
    if est_s + (blackout[1] - blackout[0]) >= 3600:
        raise DriverError(f"a {est_s:.0f} s process cannot fit between two blackouts")
    start = now_s
    while True:
        h = int(start // 3600)
        hit = None
        for hh in (h - 1, h, h + 1, h + 2):
            b0, b1 = hh * 3600 + blackout[0], hh * 3600 + blackout[1]
            if start < b1 and start + est_s >= b0:
                hit = b1
                break
        if hit is None:
            return start - now_s
        start = float(hit)


def process_estimate_s(c: dict[str, Any], seconds: int, prefills: dict[tuple[str, int], float]) -> float:
    """One process's length for the hourly guard: prefill, window and POST_S."""
    seen = prefills.get((c["shape"], c["keys"]))
    prefill = seen * PREFILL_MARGIN if seen is not None else c["keys"] * PREFILL_S_PER_KEY
    return prefill + seconds + POST_S


def guard_pause(pauses: list[dict[str, Any]], label: str, est_s: float,
                clock=time.time, sleep=time.sleep) -> float:
    """Waits out the hourly blackout before process `label`, and records any pause.

    Called only between processes: the previous one has exited, and the next
    has not started. The recorded interval is `[start_t, end_t]` as read from
    `clock` before and after the sleep, so it holds only time in which the
    driver ran nothing.
    """
    now = clock()
    wait = hour_guard_wait(now, est_s)
    if wait > 0:
        sleep(wait)
        pauses.append({"before": label, "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now)),
                       "wait_s": round(wait, 1), "process_estimate_s": round(est_s, 1),
                       "start_t": now, "end_t": clock()})
    return wait


def run_recorded(processes: list[dict[str, Any]], label: str, fn, clock=time.time) -> dict[str, Any]:
    """Runs one measured process through `fn` and records its `[start_t, end_t]`."""
    t0 = clock()
    row = fn()
    processes.append({"label": label, "start_t": t0, "end_t": clock()})
    return row


def check_pauses(hour_guard: dict[str, Any]) -> None:
    """Refuses a record in which a pause overlaps a measured process (§8.1)."""
    why = bench_host_guard.pause_problems(hour_guard)
    if why:
        raise DriverError("the pause record is inconsistent: " + "; ".join(why[:3]))


def note_prefill(prefills: dict[tuple[str, int], float], c: dict[str, Any], row: dict[str, Any]) -> None:
    k = (c["shape"], c["keys"])
    prefills[k] = max(prefills.get(k, 0.0), float(row["prefill_s"]))


# --- Build and run ----------------------------------------------------------

def build(features: tuple[str, ...] = ()) -> Path:
    out = subprocess.run(
        ["cargo", "build", "--release", "-p", "expanse-trie", "--bench", HARNESS,
         *(["--features", ",".join(features)] if features else []),
         "--message-format=json-render-diagnostics"],
        cwd=REPO_ROOT, check=True, stdout=subprocess.PIPE, text=True,
    ).stdout
    exes = [json.loads(ln).get("executable") for ln in out.splitlines() if ln.startswith("{")]
    exes = [e for e in exes if e and Path(e).name.startswith(HARNESS)]
    if len(exes) != 1:
        raise DriverError(f"expected one {HARNESS} executable, found {exes}")
    return Path(exes[0])


def probe(exe: Path, counters: bool = False) -> dict[str, Any]:
    """Refuses a binary without `--mode identify`, or not the build expected (§8.1)."""
    proc = subprocess.run([str(exe), "--mode", "identify"],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
    lines = [ln for ln in proc.stdout.splitlines() if ln.startswith("{")]
    if proc.returncode != 0 or len(lines) != 1:
        raise DriverError(f"the {HARNESS} harness at {exe} has no `--mode identify` "
                          f"(exit {proc.returncode})\n{proc.stderr.strip()[-400:]}")
    row = json.loads(lines[0])
    if row.get("occ_stats") is not counters:
        raise DriverError(f"the build at {exe} reports occ_stats={row.get('occ_stats')!r}, expected {counters!r}")
    return row


def child_pin(applied: str) -> str | None:
    if applied in ("none", "off") or not hasattr(os, "sched_setaffinity"):
        return None
    return applied


def check_affinity(row: dict[str, Any], pin: str | None, where: str) -> None:
    """Refuses a row whose process ran on CPUs other than the pin asked for (§8.1)."""
    if pin is None:
        return
    got = row.get("cpu_affinity")
    if got is None or bench_pin.expand(str(got)) != bench_pin.expand(pin):
        raise DriverError(f"{where}: the process ran on CPUs {got!r}, not the requested pin {pin}")


def run_one(exe: Path, c: dict[str, Any], arm: str, r: int, seconds: int, seed: int,
            pin: str | None = None, cpus: list[int] | None = None) -> dict[str, Any]:
    argv = [str(exe), "--mode", "run", "--shape", c["shape"], "--keys", str(c["keys"]),
            "--writers", str(c["writers"]), "--arm", arm, "--period-ms", str(c["period_ms"]),
            "--seconds", str(seconds), "--round", str(r), "--seed", str(seed)]
    if cpus is not None:
        argv += ["--cpus", ",".join(map(str, cpus))]
    preexec = (bench_pin.child_affinity(pin)
               if pin is not None and hasattr(os, "sched_setaffinity") else None)
    out = subprocess.run(argv, check=True, stdout=subprocess.PIPE, text=True,
                         preexec_fn=preexec).stdout.strip().splitlines()
    where = f"{cell_id(c, arm)} round {r}"
    if len(out) != 1:
        raise DriverError(f"{where}: expected one JSON row, got {len(out)} lines")
    row = json.loads(out[0])
    for k in ("shape", "keys", "writers", "period_ms"):
        if row.get(k) != c[k]:
            raise DriverError(f"{where}: the row reports {k}={row.get(k)!r}")
    if row.get("arm") != arm:
        raise DriverError(f"{where}: the row reports arm={row.get('arm')!r}")
    check_affinity(row, pin, where)
    want = None if cpus is None else {"poller": cpus[0], "writers": cpus[1:c["writers"] + 1]}
    if row.get("placement") != want:
        raise DriverError(f"{where}: the row reports placement {row.get('placement')!r}, not {want!r}")
    return row


# --- Per-round statistics ---------------------------------------------------

def poll_durations(row: dict[str, Any]) -> list[int]:
    return [int(p[1]) for p in row.get("polls", [])]


def median_poll_ns(row: dict[str, Any]) -> float:
    d = poll_durations(row)
    if not d:
        raise DriverError(f"round {row.get('round')}: the row has no poll")
    return float(statistics.median(d))


def throughput(row: dict[str, Any]) -> float:
    return row["inserts"] / row["elapsed_s"]


def round_stalled_fraction(row: dict[str, Any]) -> float:
    """§44.4: `stalled_fraction` from the round's measured (median) poll duration."""
    return bounds.stalled_fraction(median_poll_ns(row) / 1e9, row["period_ms"] / 1e3)


INSERT_STATS = ("p50_ns", "p99_ns", "p999_ns", "max_ns")


def interval(values: list[float]) -> dict[str, Any]:
    point, lo, hi, method = bca_bootstrap_ci_with_method(values, num_resamples=BCA_RESAMPLES)
    return {"mean": point, "ci_lower": lo, "ci_upper": hi, "ci_method": method,
            "median": statistics.median(values), "n": len(values)}


def summarise(cell: dict[str, Any]) -> dict[str, Any]:
    """§44.4's per-cell report: every statistic with its BCa interval over rounds."""
    rows = cell["rounds_raw"]
    out: dict[str, Any] = {f"insert_{k}": interval([float(r["insert"][k]) for r in rows])
                           for k in INSERT_STATS}
    out["throughput_ops_s"] = interval([throughput(r) for r in rows])
    if cell["arm"] in POLLER_ARMS and all(poll_durations(r) for r in rows):
        out["poll_median_ns"] = interval([median_poll_ns(r) for r in rows])
        out["stalled_fraction"] = interval([round_stalled_fraction(r) for r in rows])
    return out


def paired_deltas(a: dict[str, Any], b: dict[str, Any]) -> dict[str, Any]:
    """Per-round paired differences `a - b` (same cell, same round), with BCa intervals."""
    ra = {r["round"]: r for r in a["rounds_raw"]}
    rb = {r["round"]: r for r in b["rounds_raw"]}
    paired = sorted(set(ra) & set(rb))
    out: dict[str, Any] = {"rounds": paired}
    if len(paired) < 3:
        return out
    for k in INSERT_STATS:
        out[f"insert_{k}"] = interval([float(ra[i]["insert"][k] - rb[i]["insert"][k]) for i in paired])
    out["throughput_ops_s"] = interval([throughput(ra[i]) - throughput(rb[i]) for i in paired])
    return out


# --- Voids ------------------------------------------------------------------

def cell_voids(cell: dict[str, Any], rounds: int, quick: bool, load_avg_void: float) -> list[str]:
    """Why a cell cannot decide anything, or [] (§8.17, §8.1)."""
    where = cell["cell"]
    v: list[str] = []
    rows = cell.get("rounds_raw", [])
    if sorted(r.get("round") for r in rows) != list(range(rounds)):
        v.append(f"{where}: rounds {[r.get('round') for r in rows]} are not 0..{rounds - 1}")
    for r in rows:
        at = f"{where} round {r.get('round')}"
        if not r.get("inserts"):
            v.append(f"{at}: no insert completed")
        if cell["arm"] in POLLER_ARMS and not poll_durations(r):
            v.append(f"{at}: no poll completed")
        if not quick and r.get("placement") is None:
            v.append(f"{at}: threads were not placed one per core (§44.3's pin)")
        load = r.get("load") or {}
        foreign = load.get("foreign_busy_cpus")
        if isinstance(foreign, (int, float)) and not isinstance(foreign, bool):
            if foreign > FOREIGN_BUSY_VOID:
                v.append(f"{at}: foreign busy CPU {foreign} above {FOREIGN_BUSY_VOID} (§8.17)")
        elif not quick:
            v.append(f"{at}: no numeric load.foreign_busy_cpus; the window cannot be shown quiet (§8.17)")
        one = (r.get("load_start") or {}).get("load1")
        if isinstance(one, (int, float)) and one > load_avg_void:
            v.append(f"{at}: load average {one} above {load_avg_void} at the window's start (§8.17)")
    return v


def find(cells: list[dict[str, Any]], **want) -> dict[str, Any]:
    hits = [c for c in cells if all(c[k] == v for k, v in want.items())]
    if len(hits) != 1:
        raise DriverError(f"expected one cell matching {want}, found {len(hits)}")
    return hits[0]


def evaluate(cells: list[dict[str, Any]], g: dict[str, Any], quick: bool,
             load_avg_void: float, run_voids: list[str] | None = None) -> dict[str, Any]:
    """S1 and M1 as METHODOLOGY §44.4 words them."""
    run_voids = list(run_voids or [])
    if quick:
        run_voids.append("quick run: a reduced grid, not §44's registered cells")
    rounds = g["rounds"]

    # S1: per shape, median over rounds of the largest insert latency vs 0.5 x the median poll duration.
    s1: dict[str, Any] = {"at": g["s1"], "rule": "median(max insert) >= 0.5 x median(poll duration), "
                          "mem_used arm, both shapes", "shapes": {}}
    s1_voids = list(run_voids)
    for shape in SHAPES:
        c = find(cells, shape=shape, arm="mem_used", **g["s1"])
        voids = cell_voids(c, rounds, quick, load_avg_void)
        s1_voids += voids
        rows = c["rounds_raw"]
        entry: dict[str, Any] = {"cell": c["cell"], "voids": voids}
        if rows and all(poll_durations(r) for r in rows):
            max_med = statistics.median(float(r["insert"]["max_ns"]) for r in rows)
            poll_med = statistics.median(median_poll_ns(r) for r in rows)
            entry.update({"median_max_insert_ns": max_med, "median_poll_ns": poll_med,
                          "bound_ns": S1_FACTOR * poll_med, "holds": max_med >= S1_FACTOR * poll_med})
        s1["shapes"][shape] = entry
    if s1_voids:
        s1["verdict"] = "VOID"
    else:
        s1["verdict"] = "PASS" if all(e["holds"] for e in s1["shapes"].values()) else "FAIL"
    s1["voids"] = s1_voids

    # M1: per shape, then MATERIAL if either shape is, NOT_MATERIAL if both are.
    m1: dict[str, Any] = {"at": g["m1"], "rule": (
        f"MATERIAL iff stalled_fraction ci_lower >= {M1_STALLED_FRACTION} or mem_used p999 ci_lower >= "
        f"{M1_P999_FACTOR:g} x len p999 ci_upper; NOT_MATERIAL iff stalled_fraction ci_upper < "
        f"{M1_STALLED_FRACTION} and mem_used p999 ci_upper < {M1_P999_FACTOR:g} x len p999 ci_upper; "
        "INTERMEDIATE otherwise; the run is MATERIAL if either shape is, NOT_MATERIAL if both are"),
        "shapes": {}}
    m1_voids = list(run_voids)
    for shape in SHAPES:
        mu = find(cells, shape=shape, arm="mem_used", **g["m1"])
        ln = find(cells, shape=shape, arm="len", **g["m1"])
        voids = cell_voids(mu, rounds, quick, load_avg_void) + cell_voids(ln, rounds, quick, load_avg_void)
        m1_voids += voids
        entry = {"cells": [mu["cell"], ln["cell"]], "voids": voids}
        ok = (len(mu["rounds_raw"]) >= 3 and len(ln["rounds_raw"]) >= 3
              and all(poll_durations(r) for r in mu["rounds_raw"]))
        if ok:
            sf = interval([round_stalled_fraction(r) for r in mu["rounds_raw"]])
            mp = interval([float(r["insert"]["p999_ns"]) for r in mu["rounds_raw"]])
            lp = interval([float(r["insert"]["p999_ns"]) for r in ln["rounds_raw"]])
            p999_bound = M1_P999_FACTOR * lp["ci_upper"]
            c1 = sf["ci_lower"] >= M1_STALLED_FRACTION
            c2 = mp["ci_lower"] >= p999_bound
            below = sf["ci_upper"] < M1_STALLED_FRACTION and mp["ci_upper"] < p999_bound
            entry.update({"stalled_fraction": sf, "mem_used_p999_ns": mp, "len_p999_ns": lp,
                          "p999_bound_ns": p999_bound, "clause_stalled_fraction": c1, "clause_p999": c2,
                          "outcome": "MATERIAL" if (c1 or c2) else "NOT_MATERIAL" if below else "INTERMEDIATE"})
        else:
            m1_voids.append(f"{shape}: M1's cells lack the rounds or polls an interval needs")
        m1["shapes"][shape] = entry
    if m1_voids:
        m1["verdict"] = "VOID"
    else:
        outs = [e["outcome"] for e in m1["shapes"].values()]
        m1["verdict"] = ("MATERIAL" if "MATERIAL" in outs
                         else "NOT_MATERIAL" if all(o == "NOT_MATERIAL" for o in outs) else "INTERMEDIATE")
    m1["voids"] = m1_voids
    return {"S1": s1, "M1": m1}


# --- Counters pass ----------------------------------------------------------

COUNTER_KEYS = ("gate_blocked_entries", "gate_wait_cycles", "quiesce_calls", "quiesce_drain_cycles",
                "locked_reads", "contention_gate_closed")


def counters_report(row: dict[str, Any]) -> dict[str, Any]:
    """One window's gate counters, cycle counts converted with the window's `cycles_hz()` (§8.20.1)."""
    c = row.get("counters")
    if not isinstance(c, dict):
        raise DriverError("the row has no `counters`: the binary was not built with occ-stats")
    hz = c.get("tsc_hz")
    if not hz:
        raise DriverError(f"the row's tsc_hz is {hz!r}: cycle counts cannot be converted")
    return {"counters": {k: c[k] for k in COUNTER_KEYS}, "tsc_hz": hz,
            "gate_wait_s": c["gate_wait_cycles"] / hz, "quiesce_drain_s": c["quiesce_drain_cycles"] / hz,
            "writer_cpu_ns": c.get("writer_cpu_ns"), "wall_s": row["elapsed_s"],
            "polls": len(poll_durations(row)), "inserts": row["inserts"],
            "note": "occ-stats build: no reported figure or verdict reads this pass"}


# --- Measure ----------------------------------------------------------------

def refuse_docs_path(out_path: Path) -> None:
    if (REPO_ROOT / "docs").resolve() in out_path.resolve().parents:
        raise DriverError(f"--quick writes scratch output only, never under docs/ ({out_path})")


def measure(out_path: Path, quick: bool = False) -> dict[str, Any]:
    if quick:
        refuse_docs_path(out_path)
    g = grid(quick)
    rounds = g["rounds"]
    applied = bench_pin.apply("strmap_poll_stall.py")
    prov = new_provenance("concurrency", 1586, "§44 string-map memory-reading stall (no ratio gate)",
                          REPO_ROOT, cell_isolation="process", rounds=rounds, core_pin_applied=applied,
                          preregistration=PREREGISTRATION, quick=quick, seconds=g["seconds"],
                          bca_resamples=BCA_RESAMPLES)
    cpus = host_cpus(applied)
    siblings = read_siblings(cpus) if cpus else None
    prov["placement_rule"] = ("poller on the first CPU of the first core; writers on the first CPU of every "
                              "other core, then their SMT siblings, then the poller's sibling")
    prov["placement_cpus"] = cpus if siblings else None
    exe = build()
    probe(exe)
    pin = child_pin(applied)
    cells = cells_for(g)
    by: dict[str, dict[str, Any]] = {}
    pauses: list[dict[str, Any]] = []
    processes: list[dict[str, Any]] = []
    prefills: dict[tuple[str, int], float] = {}
    prov["hour_guard"] = {"blackout_utc_minutes": [HOUR_BLACKOUT_S[0] // 60, HOUR_BLACKOUT_S[1] // 60],
                          "prefill_s_per_key_default": PREFILL_S_PER_KEY, "prefill_margin": PREFILL_MARGIN,
                          "post_s": POST_S, "pauses": pauses, "processes": processes}
    for r in range(rounds):
        for c, arm in round_order(cells, r, rounds):
            cid = cell_id(c, arm)
            place = placement(cpus, siblings, c["writers"]) if siblings else None
            seed = 0x1586_0000 + 97 * r + cells.index(c)
            guard_pause(pauses, f"cell:{cid}:r{r}", process_estimate_s(c, g["seconds"], prefills))
            start = begin_cell(prov, f"cell:{cid}:r{r}")
            row = run_recorded(processes, f"cell:{cid}:r{r}",
                               lambda: run_one(exe, c, arm, r, g["seconds"], seed, pin, place))
            note_prefill(prefills, c, row)
            row["load"] = end_cell(start)
            row["load_start"] = {"label": start.get("label"), "load1": start.get("load1")}
            cell = by.setdefault(cid, {"cell": cid, **c, "arm": arm, "rounds_raw": []})
            cell["rounds_raw"].append(row)
    out_cells = [by[cell_id(c, arm)] for c in cells for arm in ARMS]
    for cell in out_cells:
        loads = [row["load"] for row in cell["rounds_raw"]]
        cell["load"] = max(loads, key=lambda ld: ld.get("foreign_busy_cpus") or 0.0)
        cell["summary"] = summarise(cell)
    controls = []
    for c in cells:
        none_, len_, mu = (by[cell_id(c, a)] for a in ARMS)
        controls.append({"cell": cell_id(c), "len_minus_none": paired_deltas(len_, none_),
                         "mem_used_minus_len": paired_deltas(mu, len_)})
    ncpu = os.cpu_count() or 1
    art: dict[str, Any] = {"provenance": prov, "throughput": out_cells, "len_controls": controls}
    run_voids = [] if quick or rounds == ROUNDS else [f"round count {rounds} is not the registered {ROUNDS}"]
    art["verdicts"] = evaluate(out_cells, g, quick, ncpu / 2, run_voids)

    # The untimed occ-stats pass of each mem_used cell (§44.3, Counters).
    cexe = build(("occ-stats",))
    probe(cexe, counters=True)
    windows = []
    for c in cells:
        cid = cell_id(c, "mem_used")
        place = placement(cpus, siblings, c["writers"]) if siblings else None
        guard_pause(pauses, f"counters:{cid}", process_estimate_s(c, g["seconds"], prefills))
        start = begin_cell(prov, f"counters:{cid}")
        row = run_recorded(processes, f"counters:{cid}",
                           lambda: run_one(cexe, c, "mem_used", 0, g["seconds"], 0x1586_C000 + cells.index(c),
                                           pin, place))
        windows.append({"cell": cid, **counters_report(row), "load": end_cell(start)})
    art["counters"] = windows
    if quick:
        art["note"] = "quick smoke run on a reduced grid: not a measurement, decides nothing"
    check_pauses(prov["hour_guard"])
    check_provenance(art, quick)
    out_path.write_text(json.dumps(art, indent=2) + "\n")
    return art


def check_provenance(art: dict[str, Any], quick: bool) -> None:
    """Runs `check_bench_provenance` on the artifact before it is written.

    A real run fails on any finding. A quick run is checked under the real
    artifact's name too, but off Linux the load snapshots cannot close a
    measured window, so its findings are printed, not raised.
    """
    findings = check_bench_provenance.findings_for(ARTIFACT_REL, json.loads(json.dumps(art)))
    if findings and not quick:
        raise DriverError("the artifact fails the provenance gate: " + "; ".join(findings))
    for f in findings:
        print(f"::notice::quick run, provenance finding (not a measurement): {f}", file=sys.stderr)


def verdict_lines(v: dict[str, Any]) -> list[str]:
    lines = [f"S1 at {v['S1']['at']}: {v['S1']['verdict']}"]
    for shape, e in v["S1"]["shapes"].items():
        if "holds" in e:
            lines.append(f"  {shape}: median max insert {e['median_max_insert_ns']:,.0f} ns vs bound "
                         f"{e['bound_ns']:,.0f} ns (0.5 x median poll {e['median_poll_ns']:,.0f} ns)")
    lines.append(f"M1 at {v['M1']['at']}: {v['M1']['verdict']}")
    for shape, e in v["M1"]["shapes"].items():
        if "outcome" in e:
            sf, mp, lp = e["stalled_fraction"], e["mem_used_p999_ns"], e["len_p999_ns"]
            lines.append(f"  {shape}: {e['outcome']}; stalled_fraction [{sf['ci_lower']:.4f}, {sf['ci_upper']:.4f}] "
                         f"({sf['ci_method']}); mem_used p999 [{mp['ci_lower']:,.0f}, {mp['ci_upper']:,.0f}] ns vs "
                         f"10 x len p999 upper {e['p999_bound_ns']:,.0f} ns (len p999 median "
                         f"{lp['median']:,.0f} ns)")
    for key in ("S1", "M1"):
        for reason in v[key]["voids"][:10]:
            lines.append(f"  {key} void: {reason}")
    return lines


# --- Self-test --------------------------------------------------------------

def _row(c: dict[str, Any], arm: str, r: int, max_ns: float, poll_ns: float | None, p999: float,
         foreign: float = 0.1, inserts: int = 1000, placed: bool = True) -> dict[str, Any]:
    j = 1 + 0.02 * (r % 3)
    polls = [] if poll_ns is None or arm == "none" else [[i, int(poll_ns * j), 1] for i in range(5)]
    return {"round": r, **c, "arm": arm, "elapsed_s": 1.0, "inserts": inserts,
            "insert": {"p50_ns": 500, "p99_ns": 2000, "p999_ns": int(p999 * j), "max_ns": int(max_ns * j)},
            "polls": polls, "placement": {"poller": 0, "writers": [2]} if placed else None,
            "load": {"foreign_busy_cpus": foreign}, "load_start": {"load1": 1.0}}


def _cells(g: dict[str, Any], *, mu_max: float = 30e6, poll: float = 40e6, mu_p999: float = 5e5,
           len_p999: float = 5e3, short_mu_max: float | None = None, short_mu_p999: float | None = None,
           short_poll: float | None = None, rounds: int = 5, **kw) -> list[dict[str, Any]]:
    out = []
    for c in cells_for(g):
        for arm in ARMS:
            short = c["shape"] == "short"
            mx = short_mu_max if short and short_mu_max is not None else mu_max
            pl = short_poll if short and short_poll is not None else poll
            mp = short_mu_p999 if short and short_mu_p999 is not None else mu_p999
            rows = [_row(c, arm, r, mx if arm == "mem_used" else 50e3,
                         pl if arm == "mem_used" else 100.0,
                         mp if arm == "mem_used" else len_p999, **kw) for r in range(rounds)]
            out.append({"cell": cell_id(c, arm), **c, "arm": arm, "rounds_raw": rows})
    return out


def _refused(fn, *args) -> str:
    try:
        fn(*args)
    except DriverError as e:
        return str(e)
    raise AssertionError(f"{fn.__name__}{args} was not refused")


def self_test_verdicts() -> None:
    g = grid()

    def ev(cells, **kw):
        return evaluate(cells, g, False, 12.0, **kw)

    # Baseline: a 40 ms walk at T = 1 s holds writers ~30 ms; p999 100x the control's.
    v = ev(_cells(g))
    assert v["S1"]["verdict"] == "PASS" and v["M1"]["verdict"] == "MATERIAL", v
    for e in v["M1"]["shapes"].values():
        for k in ("stalled_fraction", "mem_used_p999_ns", "len_p999_ns"):
            assert e[k]["ci_method"] and e[k]["ci_lower"] <= e[k]["mean"] <= e[k]["ci_upper"]

    # S1 negative controls, one per clause: the 0.5 factor, and each shape independently.
    assert ev(_cells(g, mu_max=15e6))["S1"]["verdict"] == "FAIL"           # 15 ms < 0.5 x 40 ms, both shapes
    assert ev(_cells(g, mu_max=21e6))["S1"]["verdict"] == "PASS"           # just above the bound
    assert ev(_cells(g, short_mu_max=15e6))["S1"]["verdict"] == "FAIL"     # routes holds, short does not
    assert ev(_cells(g, short_poll=100e6))["S1"]["verdict"] == "FAIL"      # short's walk longer than its stall x 2

    # M1 clause 1 alone: stalled fraction 0.04, p999 under 10x the control's upper bound.
    v = ev(_cells(g, mu_p999=6e3))
    e = v["M1"]["shapes"]["routes"]
    assert e["clause_stalled_fraction"] and not e["clause_p999"] and v["M1"]["verdict"] == "MATERIAL", e
    # M1 clause 2 alone: a 1 ms walk (fraction 0.001) but p999 100x the control's.
    v = ev(_cells(g, poll=1e6, mu_max=1e6))
    e = v["M1"]["shapes"]["routes"]
    assert e["clause_p999"] and not e["clause_stalled_fraction"] and v["M1"]["verdict"] == "MATERIAL", e
    # Neither, both upper bounds below: NOT_MATERIAL.
    v = ev(_cells(g, poll=1e6, mu_max=1e6, mu_p999=6e3))
    assert v["M1"]["verdict"] == "NOT_MATERIAL", v["M1"]
    # Clause 1 straddles its threshold (fraction 0.0098..0.0102 across rounds): INTERMEDIATE.
    v = ev(_cells(g, poll=0.98e7 / 1.0, mu_max=1e7, mu_p999=6e3))
    e = v["M1"]["shapes"]["routes"]
    assert e["stalled_fraction"]["ci_lower"] < 0.01 <= e["stalled_fraction"]["ci_upper"], e["stalled_fraction"]
    assert e["outcome"] == "INTERMEDIATE" and v["M1"]["verdict"] == "INTERMEDIATE", e
    # Clause 2 reads the interval, not the medians: p999 at 10x the control's median but below its bound.
    v = ev(_cells(g, poll=1e6, mu_max=1e6, mu_p999=5.04e4))
    e = v["M1"]["shapes"]["routes"]
    assert e["mem_used_p999_ns"]["median"] >= 10 * e["len_p999_ns"]["median"], e
    assert not e["clause_p999"] and e["outcome"] == "INTERMEDIATE", e
    # The threshold is 10 x the control's upper bound, not its median: a lower bound between the two is not MATERIAL.
    e = ev(_cells(g, poll=1e6, mu_max=1e6, mu_p999=5.12e4))["M1"]["shapes"]["routes"]
    assert 10 * e["len_p999_ns"]["median"] < e["mem_used_p999_ns"]["ci_lower"] < e["p999_bound_ns"], e
    assert not e["clause_p999"] and e["outcome"] == "INTERMEDIATE", e
    # Either shape decides MATERIAL; NOT_MATERIAL needs both.
    v = ev(_cells(g, poll=1e6, mu_max=1e6, mu_p999=6e3, short_poll=40e6, short_mu_max=30e6))
    assert v["M1"]["shapes"]["routes"]["outcome"] == "NOT_MATERIAL"
    assert v["M1"]["shapes"]["short"]["outcome"] == "MATERIAL" and v["M1"]["verdict"] == "MATERIAL"

    # VOIDs: a contaminated round, a missing poll, a missing insert, a missing round, no placement, load average.
    assert ev(_cells(g, foreign=1.5))["S1"]["verdict"] == "VOID"
    assert ev(_cells(g, foreign=1.5))["M1"]["verdict"] == "VOID"
    assert ev(_cells(g, poll=None))["S1"]["verdict"] == "VOID"
    assert ev(_cells(g, inserts=0))["M1"]["verdict"] == "VOID"
    assert ev(_cells(g, rounds=4))["M1"]["verdict"] == "VOID"
    assert ev(_cells(g, placed=False))["S1"]["verdict"] == "VOID"
    cells = _cells(g)
    find(cells, shape="routes", arm="len", **g["m1"])["rounds_raw"][2]["load_start"]["load1"] = 13.0
    v = ev(cells)
    assert v["M1"]["verdict"] == "VOID" and v["S1"]["verdict"] == "PASS", v
    cells = _cells(g)
    del find(cells, shape="short", arm="mem_used", **g["m1"])["rounds_raw"][0]["load"]["foreign_busy_cpus"]
    assert ev(cells)["M1"]["verdict"] == "VOID"
    assert ev(_cells(g), run_voids=["round count 4"])["S1"]["verdict"] == "VOID"
    # A quick run never decides.
    gq = grid(quick=True)
    v = evaluate(_cells(gq, rounds=3), gq, True, 12.0)
    assert v["S1"]["verdict"] == "VOID" and v["M1"]["verdict"] == "VOID"
    assert any("quick run" in x for x in v["S1"]["voids"])

    # The per-cell report and the controls carry labelled intervals.
    cells = _cells(g)
    s = summarise(find(cells, shape="routes", arm="mem_used", **g["m1"]))
    assert set(s) == {"insert_p50_ns", "insert_p99_ns", "insert_p999_ns", "insert_max_ns",
                      "throughput_ops_s", "poll_median_ns", "stalled_fraction"}
    assert all(x["ci_method"] for x in s.values())
    assert "poll_median_ns" not in summarise(find(cells, shape="routes", arm="none", **g["m1"]))
    d = paired_deltas(find(cells, shape="routes", arm="mem_used", **g["m1"]),
                      find(cells, shape="routes", arm="len", **g["m1"]))
    assert d["rounds"] == [0, 1, 2, 3, 4] and d["insert_p999_ns"]["mean"] > 0


def self_test_grid_and_placement() -> None:
    g = grid()
    cells = cells_for(g)
    assert len(cells) == 24 and len({cell_id(c) for c in cells}) == 24
    orders = [round_order(cells, r, ROUNDS) for r in range(ROUNDS)]
    for o in orders:
        assert len(o) == 72 and sorted(cell_id(c, a) for c, a in o) == sorted(cell_id(c, a) for c in cells for a in ARMS)
    assert [o[0][0] for o in orders] == [cells[(r * 24) // 5] for r in range(ROUNDS)]
    assert [tuple(a for _, a in o[:3]) for o in orders[:3]] == [ARMS, ARMS[1:] + ARMS[:1], ARMS[2:] + ARMS[:2]]
    # The registered points are cells of the grid.
    for at in (S1_AT, M1_AT):
        assert any(all(c[k] == v for k, v in at.items()) for c in cells)
    # §44.6's constants.
    assert (KEYS, WRITERS, PERIODS_MS, SECONDS, ROUNDS) == ((2_000_000, 20_000_000), (1, 4, 8), (1000, 100), 30, 5)
    assert (S1_FACTOR, M1_STALLED_FRACTION, M1_P999_FACTOR) == (0.5, 0.01, 10.0) and BCA_RESAMPLES >= 1000
    # The reference host's pin: 8 P-cores with SMT siblings (0,1), (2,3), ..., (14,15).
    sib = {c: [c - c % 2, c - c % 2 + 1] for c in range(16)}
    cpus = list(range(16))
    assert placement(cpus, sib, 1) == [0, 2]
    assert placement(cpus, sib, 4) == [0, 2, 4, 6, 8]
    assert placement(cpus, sib, 8) == [0, 2, 4, 6, 8, 10, 12, 14, 3]   # the 8th writer on 2's sibling
    assert placement(cpus, sib, 15) == [0, *range(2, 16, 2), *range(3, 16, 2), 1]
    assert "fewer than 16 writers" in _refused(placement, cpus, sib, 16)
    # A pin of one CPU per core: no siblings to fall back on.
    assert placement([0, 2, 4, 6], sib, 3) == [0, 2, 4, 6]
    assert "fewer than 4" in _refused(placement, [0, 2, 4, 6], sib, 4)
    # A uniform host without SMT.
    assert placement([0, 1, 2], {0: [0], 1: [1], 2: [2]}, 2) == [0, 1, 2]
    # Quick refuses docs/ output; a scratch path is accepted.
    assert "never under docs/" in _refused(refuse_docs_path, REPO_ROOT / "docs" / "x.json")
    refuse_docs_path(Path(tempfile.gettempdir()) / "x.json")


def self_test_identity_and_counters() -> None:
    with tempfile.TemporaryDirectory() as d:
        def fake(name: str, body: str) -> Path:
            p = Path(d) / name
            p.write_text("#!/bin/sh\n" + body + "\n")
            p.chmod(0o755)
            return p
        on = fake("on", """echo '{"mode":"identify","occ_stats":true,"cpu_affinity":"0"}'""")
        off = fake("off", """echo '{"mode":"identify","occ_stats":false,"cpu_affinity":"0"}'""")
        old = fake("old", "echo 'missing --mode' >&2; exit 101")
        assert probe(off)["occ_stats"] is False and probe(on, counters=True)["occ_stats"] is True
        assert "occ_stats" in _refused(probe, on)
        assert "occ_stats" in _refused(probe, off, True)
        assert "no `--mode identify`" in _refused(probe, old)
        # run_one refuses a row from another cell, another arm, other CPUs or another placement.
        c = cells_for(grid())[0]
        good = {**c, "arm": "len", "cpu_affinity": "0", "placement": {"poller": 0, "writers": [2]}}

        def liar(**over):
            return fake("liar", "echo '" + json.dumps({**good, **over}) + "'")
        assert run_one(liar(), c, "len", 0, 1, 1, None, [0, 2])["arm"] == "len"
        assert "arm=" in _refused(run_one, liar(arm="mem_used"), c, "len", 0, 1, 1, None, [0, 2])
        assert "keys=" in _refused(run_one, liar(keys=7), c, "len", 0, 1, 1, None, [0, 2])
        assert "placement" in _refused(run_one, liar(placement=None), c, "len", 0, 1, 1, None, [0, 2])
        mine = str(min(os.sched_getaffinity(0))) if hasattr(os, "sched_getaffinity") else "0"
        assert "ran on CPUs '999'" in _refused(run_one, liar(cpu_affinity="999"), c, "len", 0, 1, 1, mine, [0, 2])
    check_affinity({"cpu_affinity": "0-3"}, "0,1,2,3", "x")
    assert "ran on CPUs None" in _refused(check_affinity, {}, "0-3", "x")
    # Counters: converted with the window's own tsc_hz; refused without them.
    row = {"elapsed_s": 30.0, "inserts": 10, "polls": [[0, 1, 1]],
           "counters": {"gate_blocked_entries": 5, "gate_wait_cycles": 3_000_000_000, "quiesce_calls": 29,
                        "quiesce_drain_cycles": 1_000_000, "locked_reads": 29, "contention_gate_closed": 0,
                        "tsc_hz": 3_000_000_000, "writer_cpu_ns": 1}}
    rep = counters_report(row)
    assert rep["gate_wait_s"] == 1.0 and rep["quiesce_drain_s"] == 1e6 / 3e9 and rep["polls"] == 1
    assert "no `counters`" in _refused(counters_report, {"elapsed_s": 1.0})
    row["counters"]["tsc_hz"] = 0
    assert "tsc_hz" in _refused(counters_report, row)


def self_test_harness_source() -> None:
    """The harness times only the insert: key generation and the membership check precede the clock."""
    src = (REPO_ROOT / HARNESS_REL).read_text()
    code = "\n".join(ln for ln in src.splitlines() if not ln.lstrip().startswith("//"))
    body = code[code.index("fn writer_loop"):]
    body = body[:body.index("\n}\n")]
    i_contains = body.index("m.contains_key(key)")
    i_clock = body.index("let t = Instant::now();")
    i_insert = body.index("m.insert(key, w)")
    i_elapsed = body.index("t.elapsed()")
    assert i_contains < i_clock < i_insert < i_elapsed, (i_contains, i_clock, i_insert, i_elapsed)
    assert body.count("Instant::now()") == 1
    # The prefill runs before the barrier that opens the window.
    main = code[code.index("fn main()"):]
    assert main.index("prefill(&m, &shape, keys)") < main.index("barrier.wait();")
    assert "| `workload_id` | `strmap_poll_stall` |" in src


def self_test_hour_guard() -> None:
    hour = 1_800_000_000 - 1_800_000_000 % 3600  # an hour boundary, UTC

    def at(m: int, sec: int = 0) -> float:
        return float(hour + 60 * m + sec)
    assert hour_guard_wait(at(10), 200) == 0                    # ends at :13:20
    assert hour_guard_wait(at(10), 300) == at(20) - at(10)      # ends at :15:00 exactly: waits
    assert hour_guard_wait(at(10), 400) == at(20) - at(10)      # would reach :16:40
    assert hour_guard_wait(at(17), 10) == at(20) - at(17)       # starts inside the blackout
    assert hour_guard_wait(at(15), 0) == at(20) - at(15)
    assert hour_guard_wait(at(20), 3000) == 0                   # :20 to :70 = :10 of the next hour
    assert hour_guard_wait(at(25), 3200) == at(80) - at(25)     # reaches :18 of the next hour
    assert hour_guard_wait(at(59), 1000) == at(80) - at(59)     # crosses the hour into :15:40
    assert hour_guard_wait(at(14, 59), 0.5) == 0                # ends at :14:59.5
    assert hour_guard_wait(at(14, 59), 1.0) == at(20) - at(14, 59)  # ends at :15:00
    for start, est in ((at(m, s), e) for m in range(0, 60, 7) for s in (0, 31) for e in (5, 130, 900)):
        w = hour_guard_wait(start, est)
        b, e_ = start + w, start + w + est
        for hh in range(-1, 3):
            b0, b1 = hour + hh * 3600 + 900, hour + hh * 3600 + 1200
            assert not (b < b1 and e_ >= b0), (start, est, w)
    assert "cannot fit" in _refused(hour_guard_wait, at(0), 3600 - 300)
    assert "must be >= 0" in _refused(hour_guard_wait, at(0), -1)
    # The estimate: the default per key until a prefill is seen, then the largest seen x the margin.
    c = {"shape": "short", "keys": 20_000_000}
    assert process_estimate_s(c, 30, {}) == 20_000_000 * PREFILL_S_PER_KEY + 30 + POST_S
    pre: dict = {}
    note_prefill(pre, c, {"prefill_s": 40.0})
    note_prefill(pre, c, {"prefill_s": 30.0})
    assert process_estimate_s(c, 30, pre) == 40.0 * PREFILL_MARGIN + 30 + POST_S
    # guard_pause records a pause and sleeps it; no pause, no record. A fake clock, advanced
    # by the fake sleep and the fake processes, stands in for the host's.
    now = [at(10)]
    slept: list = []

    def nap(sec: float) -> None:
        slept.append(sec)
        now[0] += sec

    def clock() -> float:
        return now[0]

    def proc(sec: float):
        def fn():
            now[0] += sec
            return {"ran": sec}
        return fn
    pauses: list = []
    processes: list = []
    assert run_recorded(processes, "cell:w", proc(100), clock=clock) == {"ran": 100}   # :10:00-:11:40
    assert guard_pause(pauses, "cell:x", 400, clock=clock, sleep=nap) == at(20) - at(11, 40)
    assert slept == [at(20) - at(11, 40)] and pauses[0]["before"] == "cell:x"
    assert pauses[0]["wait_s"] == round(at(20) - at(11, 40), 1)
    run_recorded(processes, "cell:x", proc(400), clock=clock)                       # :20:00-:26:40
    assert guard_pause(pauses, "cell:y", 100, clock=clock, sleep=nap) == 0
    assert len(pauses) == 1 and len(slept) == 1
    run_recorded(processes, "cell:y", proc(100), clock=clock)
    # The pause is exactly the time between the previous process's exit and the next one's start.
    assert pauses[0]["start_t"] == processes[0]["end_t"] == at(11, 40), (pauses, processes)
    assert pauses[0]["end_t"] == processes[1]["start_t"] == at(20), (pauses, processes)
    check_pauses({"pauses": pauses, "processes": processes})
    # A pause overlapping a measured process is refused, by the driver and the guard alike: one the
    # previous process outlives, one inside which the next process starts, and one inside the pause.
    for bad in (dict(processes[0], end_t=at(12)), dict(processes[1], start_t=at(19)),
                {"label": "bg", "start_t": at(15), "end_t": at(16)}):
        got = _refused(check_pauses, {"pauses": pauses, "processes": [bad]})
        assert "overlaps measured process" in got, (bad, got)
    # The call sites: every process measure() starts is guarded first, and its prefill noted.
    src = (Path(__file__).resolve()).read_text()
    body = src[src.index("def measure("):src.index("def check_provenance(")]
    code = "\n".join(ln.split("#", 1)[0] for ln in body.splitlines())  # measure() has no '#' in strings
    runs = [i for i in range(len(code)) if code.startswith("run_one(", i)]
    guards = [i for i in range(len(code)) if code.startswith("guard_pause(", i)]
    assert len(runs) == len(guards) == 2, (runs, guards)
    assert all(g < r for g, r in zip(guards, runs)) and guards[1] > runs[0]
    assert code.count("note_prefill(prefills") == 1 and code.index("note_prefill(") > runs[0]
    # Every measured process is recorded: each run_one( is the body of a run_recorded(processes, ...)
    # lambda, the record reaches the artifact, and it is checked before the artifact is written.
    recs = [i for i in range(len(code)) if code.startswith("run_recorded(processes,", i)]
    assert len(recs) == 2 and all(rec < r and code[rec:r].rstrip().endswith("lambda:") and "run_one(" not in code[rec:r]
                                  for rec, r in zip(recs, runs)), (recs, runs)
    assert '"processes": processes}' in code and code.count('check_pauses(prov["hour_guard"])') == 1
    assert code.index("check_pauses(") < code.index("out_path.write_text(")
    # A pause begins only after the previous process has exited: every process runs through a
    # blocking subprocess.run, and nothing in this file starts a process it does not wait for.
    for word in ("Popen", "os.fork", "os.spawn", "asyncio", "threading", "multiprocessing"):
        assert word not in src.replace(f'"{word}"', ""), word
    one = src[src.index("def run_one("):src.index("# --- Per-round statistics")]
    assert one.count("subprocess.run(") == 1, one


def self_test() -> int:
    bounds.self_test()
    self_test_hour_guard()
    assert CI_METHOD_BCA
    self_test_grid_and_placement()
    self_test_verdicts()
    self_test_identity_and_counters()
    self_test_harness_source()
    print("strmap_poll_stall.py self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--out", type=Path)
    ap.add_argument("--quick", action="store_true", help="a reduced-grid smoke run; scratch output only")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.out:
        ap.error("--out is required")
    try:
        art = measure(args.out, args.quick)
    except DriverError as e:
        print(f"strmap_poll_stall.py: {e}", file=sys.stderr)
        return 1
    for line in verdict_lines(art["verdicts"]):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
