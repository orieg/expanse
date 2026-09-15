#!/usr/bin/env python3
"""What a comparative benchmark artifact has to carry to be recomputable (#732).

Only `masstree_comparison`'s runner published all of it. `hot_comparison`'s and
`art_comparison`'s took one load average per phase and kept no raw rows;
`hashbrown_comparison`, `redis_zset_engine` and `search_inverted_index` took no
load snapshot at all. This module is that runner's provenance code, lifted so
every comparative runner emits the same fields and a gate can require them.

Four things, and why each is here rather than in a README:

**`rounds_raw`** — every round's samples verbatim. A published median and a
published ratio are both summaries of these; without them a reader cannot
recompute either, and cannot see whether the rounds that produced them were
stable. It is also where `first_arm` lands, so a reader can check that the
arm timed first actually alternated rather than take it on trust.

**`estimators`** — what the ratio column *is*. It is `mean(arm A rounds) /
mean(arm B rounds)` with a two-sample BCa interval, while the per-arm columns
beside it are *medians* of the same rounds. So the ratio is not the quotient of
the two columns next to it, and a reader who divides them and gets a different
number has found the estimator definition, not an error. Saying that in the
artifact is cheaper than saying it under every table.

**`host`** — what a wall-clock number depends on that `platform.platform()`
does not say: the CPU model, the frequency driver and governor, the
transparent-huge-page mode, and the P-core / E-core / SMT masks. On a hybrid
part the P-core mask includes SMT siblings, so `W + R = 16` on an eight-P-core
host runs two threads per physical core, and no interval says so.

**`busy_cpus_since_prev`** — the host's busy CPU between two load snapshots,
in core-equivalents, from `/proc/stat` jiffies. A window shorter than
`MIN_WINDOW_S` reads `None`, not a number: see that constant. The one-minute load average
lags a heavy process by about thirty seconds, which is how a documented 2.2x
baseline shift happened on this project despite a load pre-check passing. The
jiffy delta between two snapshots is exact over the interval it covers, the
harness's own threads included: a single-threaded phase on a quiet host reads
about 1.0, and a concurrent sweep reads its own thread count.

**`own_busy_cpus` / `foreign_busy_cpus`** — the busy-CPU delta above counts the
harness's own threads, so over a concurrent sweep it reads the sweep (5.7
core-equivalents on the reference host) and cannot say whether anything else
was resident. Splitting it needs a second instrument the jiffy line does not
carry: the CPU seconds the runner's *children* consumed, from
`getrusage(RUSAGE_CHILDREN)`, which only the benchmark processes add to.
Divided by the cell's wall time that is the cell's own core-equivalents, and
the remainder — `foreign_busy_cpus` — is everything else on the host during
that cell. It is taken per cell (`begin_cell` / `end_cell`) because a sweep
mixes one-thread and sixteen-thread cells and a whole-sweep number attributes
neither (#568 Step 0).

**`scaling_governor_by_cpu`** — the governor of every CPU in the pin set, not
of `cpu0` alone. `intel_pstate` sets it per policy, so a run that reads
`performance` on `cpu0` can still be executing on a core left at `powersave`;
the artifact records the set it read, so the claim is bounded to it.

Linux-only where it reads `/proc` and `/sys`; every reader degrades to `None`
off Linux rather than inventing a value.
"""

from __future__ import annotations

import os
import platform
import subprocess
import time
from pathlib import Path

try:
    import resource
except ImportError:  # not a POSIX host: no child CPU accounting, recorded as None
    resource = None

__all__ = [
    "USER_HZ", "MIN_WINDOW_JIFFIES", "MIN_WINDOW_S", "Snapshot",
    "cpu_jiffies", "child_cpu_seconds", "load_snapshot", "add_load", "begin_cell",
    "end_cell", "host_facts", "scaling_governor_by_cpu", "expand_cpu_list", "pin_set",
    "raw_rounds", "estimators", "git_sha", "new_provenance", "attach", "body", "rewrite",
]


def _user_hz() -> int:
    """The unit `/proc/stat` counts CPU time in, ticks per second (`USER_HZ`)."""
    try:
        hz = os.sysconf("SC_CLK_TCK")
    except (AttributeError, ValueError, OSError):
        hz = -1
    return hz if hz > 0 else 100  # 100 is USER_HZ on every Linux ABI


USER_HZ = _user_hz()
# The shortest window a busy-CPU figure is computed over. A figure is a count
# divided by a wall time, and the coarsest count is the jiffy: `/proc/stat`
# reports CPU time in units of 1/USER_HZ seconds, so one unit of one CPU over a
# window of `w` seconds is 1/(USER_HZ * w) core-equivalents. Ten units makes
# that step 0.1 core-equivalent -- a tenth of the foreign-busy void boundary of
# 1.0 (AGENTS.md section 8.17) -- which is 0.1 s at USER_HZ = 100. The other
# operands are finer: `getrusage` reports microseconds, and a snapshot is
# differenced on its unrounded readings (`Snapshot.raw`); only a snapshot read
# back from an artifact falls back to its 1 ms-rounded fields, which move a
# figure over this window by at most 0.01 core-equivalent. Raw readings do not
# make a sub-minimum window measurable: children's CPU time advances only when
# a child is reaped and jiffies only at a tick, so over a window of a few
# milliseconds the quotient is a count step divided by almost nothing, and a
# foreign remainder of that size is what a void rule reads as contamination.
# The minimum makes a figure resolvable, not exact: above it,
# every CPU whose counter advances inside the window still contributes its own
# counting step.
MIN_WINDOW_JIFFIES = 10
MIN_WINDOW_S = MIN_WINDOW_JIFFIES / USER_HZ


class Snapshot(dict):
    """A load snapshot: the fields an artifact stores, plus the readings they were rounded from.

    `monotonic_s` and `child_cpu_s` are stored rounded to 1 ms. Differencing a
    later *unrounded* reading against those rounded fields puts the two
    operands on different grids, and over a short window the grid offset is the
    whole quotient. So the unrounded readings ride on the object (`raw`), where
    `json.dumps` does not see them, and every difference is taken raw against
    raw; the artifact still carries only the rounded fields. A snapshot read
    back from JSON is a plain `dict` with no `raw`, and is differenced on its
    stored fields.
    """

    __slots__ = ("raw",)

    def __init__(self, *args, raw: dict | None = None, **kwargs) -> None:
        super().__init__(*args, **kwargs)
        self.raw = raw or {}


def _reading(snap: dict, key: str):
    """`snap`'s unrounded reading of `key` where it carries one, else the stored field."""
    raw = getattr(snap, "raw", None) or {}
    value = raw.get(key)
    return value if value is not None else snap.get(key)


def cpu_jiffies() -> tuple[int | None, int | None]:
    """`(busy, total)` jiffies from the aggregate `cpu` line of `/proc/stat`.

    Field order is `user nice system idle iowait ...`; idle time is `idle`
    plus `iowait`, and busy is everything else. Returns `(None, None)` off
    Linux or on a malformed line rather than a plausible zero.
    """
    try:
        with open("/proc/stat") as fh:
            vals = [int(x) for x in fh.readline().split()[1:]]
    except (OSError, ValueError, IndexError):
        return None, None
    if len(vals) < 4:
        return None, None
    idle = vals[3] + (vals[4] if len(vals) > 4 else 0)
    return sum(vals) - idle, sum(vals)


def busy_cpus(prev: dict | None, busy: int | None, total: int | None,
              ncpu: int | None = None) -> float | None:
    """Core-equivalents of CPU burnt between `prev` and `(busy, total)`.

    `(busy - prev.busy) / (total - prev.total)` is the fraction of the host's
    total capacity that was not idle over the interval; multiplying by the CPU
    count expresses it in cores. Returns `None` when there is no previous
    snapshot, when either side is off Linux, or when the interval is shorter
    than `MIN_WINDOW_JIFFIES` of wall time: `total` accrues one jiffy per CPU
    per tick, so the wall interval in jiffies is `(total - prev.total) / ncpu`.
    """
    if not prev or busy is None or total is None:
        return None
    p_busy, p_total = prev.get("stat_busy_jiffies"), prev.get("stat_total_jiffies")
    if p_busy is None or p_total is None:
        return None
    dt = total - p_total
    n = ncpu if ncpu is not None else (os.cpu_count() or 1)
    if dt <= 0 or dt < MIN_WINDOW_JIFFIES * n:
        return None
    return round((busy - p_busy) / dt * n, 2)


def child_cpu_seconds() -> float | None:
    """CPU seconds consumed by this process's reaped children, user plus system.

    Only the benchmark processes the runner spawns add to it, so its delta over
    a cell is the cell's own CPU time and nothing else's. `None` where the
    platform has no `getrusage`.
    """
    if resource is None:
        return None
    ru = resource.getrusage(resource.RUSAGE_CHILDREN)
    return ru.ru_utime + ru.ru_stime


def own_busy_cpus(prev: dict | None, child_cpu_s: float | None, monotonic_s: float) -> float | None:
    """Core-equivalents the runner's own children consumed since `prev`.

    `(child CPU seconds now - then) / (wall seconds now - then)`. `None` when
    there is no previous snapshot, when either side lacks the accounting, or
    when the wall interval is shorter than `MIN_WINDOW_S`: a window too short
    to measure has no number, not a small or a negative one (section 8.1).
    """
    if not prev or child_cpu_s is None:
        return None
    # Raw against raw where `prev` still carries its readings (see `Snapshot`).
    p_child, p_mono = _reading(prev, "child_cpu_s"), _reading(prev, "monotonic_s")
    if p_child is None or p_mono is None:
        return None
    wall = monotonic_s - p_mono
    if wall < MIN_WINDOW_S:
        return None
    return round((child_cpu_s - p_child) / wall, 2)


def foreign_busy_cpus(host: float | None, own: float | None) -> float | None:
    """What was busy on the host that was not the runner's own children.

    `None` unless both sides are numbers: a difference with a missing operand
    is not a smaller number, it is no number (section 8.1).
    """
    if host is None or own is None:
        return None
    return round(host - own, 2)


def load_snapshot(label: str, prev: dict | None = None) -> dict:
    """Load averages, plus the host's busy CPU since `prev` in core-equivalents.

    Also the runner's own children's CPU since `prev` (`own_busy_cpus_since_prev`)
    and the remainder (`foreign_busy_cpus_since_prev`). `since` names the
    snapshot the `*_since_prev` fields are differenced against, so a reader
    knows which interval — the one that ended here, not the one starting — they
    describe.
    """
    try:
        one, five, fifteen = os.getloadavg()
    except OSError:
        one = five = fifteen = float("nan")
    busy, total = cpu_jiffies()
    child = child_cpu_seconds()
    mono = time.monotonic()
    host = busy_cpus(prev, busy, total)
    own = own_busy_cpus(prev, child, mono)
    return Snapshot({
        "label": label,
        "since": prev.get("label") if prev else None,
        "load1": round(one, 2), "load5": round(five, 2), "load15": round(fifteen, 2),
        "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "stat_busy_jiffies": busy, "stat_total_jiffies": total,
        "child_cpu_s": None if child is None else round(child, 3),
        "monotonic_s": round(mono, 3),
        "busy_cpus_since_prev": host,
        "own_busy_cpus_since_prev": own,
        "foreign_busy_cpus_since_prev": foreign_busy_cpus(host, own),
    }, raw={"monotonic_s": mono, "child_cpu_s": child})


def add_load(prov: dict, label: str) -> None:
    """Appends a snapshot to `prov["loads"]`, differenced against the last one."""
    loads = prov.setdefault("loads", [])
    loads.append(load_snapshot(label, loads[-1] if loads else None))


def begin_cell(prov: dict, label: str) -> dict:
    """`add_load(prov, label)` before a cell; returns the snapshot for `end_cell`.

    The label convention is `cell:<arm>:W<writers>:R<readers>`, one snapshot
    per timed or counted cell, so the artifact's load series has an entry at
    every cell boundary and not one per phase.
    """
    add_load(prov, label)
    return prov["loads"][-1]


def end_cell(start: dict) -> dict:
    """Attribution over the cell that began at `start`, once it has finished.

    Not appended to `loads` — the next cell's `begin_cell` is that boundary —
    but stored on the cell itself, so `cell["load"]["foreign_busy_cpus"]` is
    the number for *this* cell and the gate can require it per cell.
    `busy_cpus_since_prev` is the host over the cell's own wall time,
    `own_busy_cpus` the runner's children over the same window, and their
    difference is what else was resident.
    """
    busy, total = cpu_jiffies()
    child = child_cpu_seconds()
    mono = time.monotonic()
    host = busy_cpus(start, busy, total)
    own = own_busy_cpus(start, child, mono)
    start_mono = _reading(start, "monotonic_s")
    return {
        "since": start.get("label"),
        "wall_s": None if start_mono is None else round(mono - start_mono, 3),
        "busy_cpus_since_prev": host,
        "own_busy_cpus": own,
        "foreign_busy_cpus": foreign_busy_cpus(host, own),
    }


def _read(path: str) -> str | None:
    try:
        with open(path) as fh:
            return fh.read().strip()
    except OSError:
        return None


def expand_cpu_list(cpu_list: str) -> list[int]:
    """`"0-3,8"` -> `[0, 1, 2, 3, 8]`, the kernel's cpulist grammar.

    The grammar `scripts/bench_pin.py::expand` parses, duplicated so this
    module stays importable on its own. Raises `ValueError` on anything else:
    a pin set that cannot be parsed is not silently the empty set.
    """
    out: set[int] = set()
    for part in cpu_list.split(","):
        part = part.strip()
        if not part:
            continue
        if "-" in part:
            lo, _, hi = part.partition("-")
            out.update(range(int(lo), int(hi) + 1))
        else:
            out.add(int(part))
    return sorted(out)


_SYSFS_CPU = "/sys/devices/system/cpu"


def pin_set(pin: str | None) -> tuple[str | None, str]:
    """The CPU set a run is confined to, and where that answer came from.

    `pin` is `EXPANSE_BENCH_PIN_APPLIED`: a cpulist when `bench_pin.sh` pinned
    the shell; `none` (uniform host), `off` (deliberately unpinned) or unset
    otherwise, in which case the run could execute on any online CPU and the
    online list is the set — read from sysfs, and `None` off Linux rather than
    a guess from `os.cpu_count()`.
    """
    if pin and pin not in ("none", "off", "unset"):
        return pin, "EXPANSE_BENCH_PIN_APPLIED"
    return _read(f"{_SYSFS_CPU}/online"), "online"


def scaling_governor_by_cpu(cpus: str | None) -> dict | None:
    """`{"0": "powersave", ...}` for every CPU in the cpulist `cpus`.

    `None` when sysfs has no CPU tree (not Linux) or when `cpus` is `None`; a
    CPU without a cpufreq policy maps to `None` individually. Never a value
    copied from `cpu0` to the others.
    """
    if cpus is None or not os.path.isdir(_SYSFS_CPU):
        return None
    return {str(c): _read(f"{_SYSFS_CPU}/cpu{c}/cpufreq/scaling_governor")
            for c in expand_cpu_list(cpus)}


def host_facts(pin: str | None = None) -> dict:
    """What a wall-clock number depends on that `platform.platform()` does not say.

    `pin` is the applied core pin (`EXPANSE_BENCH_PIN_APPLIED` when omitted);
    the per-CPU governor map is read for exactly that set, and the artifact
    says which set it was.
    """
    model = None
    try:
        with open("/proc/cpuinfo") as fh:
            for line in fh:
                if line.startswith("model name"):
                    model = line.split(":", 1)[1].strip()
                    break
    except OSError:
        pass
    if pin is None:
        pin = os.environ.get("EXPANSE_BENCH_PIN_APPLIED")
    cpus, source = pin_set(pin)
    return {
        "cpu_model": model or platform.processor() or platform.machine(),
        "cpus_online": os.cpu_count(),
        # Hybrid parts: the P-core mask includes SMT siblings, so W + R = 16 on
        # an eight-P-core host runs two threads per physical core.
        "cpu_core_cpus": _read("/sys/devices/cpu_core/cpus"),
        "cpu_atom_cpus": _read("/sys/devices/cpu_atom/cpus"),
        "cpu0_thread_siblings": _read(f"{_SYSFS_CPU}/cpu0/topology/thread_siblings_list"),
        "scaling_driver": _read(f"{_SYSFS_CPU}/cpu0/cpufreq/scaling_driver"),
        "scaling_governor": _read(f"{_SYSFS_CPU}/cpu0/cpufreq/scaling_governor"),
        # Per CPU over the pin set, never cpu0's value copied: intel_pstate sets
        # the governor per policy, and the set it was read for is recorded so
        # the claim is bounded to it.
        "scaling_governor_by_cpu": scaling_governor_by_cpu(cpus),
        "scaling_governor_pin_set": cpus,
        "scaling_governor_pin_source": source,
        "transparent_hugepage": _read("/sys/kernel/mm/transparent_hugepage/enabled"),
        "platform": platform.platform(),
    }


def raw_rounds(rows: list, keys: tuple) -> list:
    """Every round's samples verbatim, so a median or a ratio can be recomputed."""
    return [{k: r.get(k) for k in ("round", *keys)} for r in rows]


def estimators(ratio: str, columns: str | None = None, raw: str | None = None) -> dict:
    """The `provenance.estimators` block: what each published column *is*."""
    return {
        "ratio": ratio,
        "columns": columns or (
            "per-arm columns are medians of the same rounds; the ratio column is not "
            "the quotient of the two median columns beside it"
        ),
        "raw": raw or "every cell carries rounds_raw, the per-round samples verbatim",
    }


def git_sha(repo_root: Path | str | None = None) -> str:
    """The commit under test (AGENTS.md section 8.7).

    A checkout rsynced to a benchmark host without its `.git` cannot answer
    `rev-parse`; `EXPANSE_BENCH_COMMIT` then names the commit explicitly rather
    than letting the artifact say `unknown`.
    """
    explicit = os.environ.get("EXPANSE_BENCH_COMMIT")
    if explicit:
        return explicit
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=str(repo_root) if repo_root else None, stderr=subprocess.DEVNULL,
        ).decode().strip()
    except Exception:
        return "unknown"


def new_provenance(suite: str, issue: int, ratio: str, repo_root: Path | str | None = None,
                   **extra) -> dict:
    """A provenance header carrying every field `check_bench_provenance.py` requires."""
    prov = {
        "suite": suite,
        "issue": issue,
        "commit": git_sha(repo_root),
        "host": host_facts(),
        "estimators": estimators(ratio),
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "loads": [load_snapshot("start")],
    }
    prov.update(extra)
    return prov


def attach(payload, prov: dict):
    """Returns `payload` carrying `prov`, whatever shape the harness emitted.

    A harness that emits a JSON object gains a `provenance` key. One that emits
    a bare array cannot, so it is wrapped as `{"provenance": ..., "cells": [...]}`
    — read it back through `body()`, which accepts both shapes so a generator
    keeps working against artifacts written before the wrap.
    """
    if isinstance(payload, dict):
        out = dict(payload)
        out["provenance"] = prov
        return out
    return {"provenance": prov, "cells": payload}


def rewrite(paths, prov: dict) -> int:
    """Re-stamps `prov` into artifacts already on disk.

    A runner that writes one artifact per bench writes them *inside* its loop,
    so the end-of-run load snapshot is taken after the last file is closed and
    never reaches any of them. Rather than move the writes, the runner takes its
    final snapshot and re-stamps; the numbers are untouched.
    """
    import json as _json
    n = 0
    for path in paths:
        p = Path(path)
        if not p.is_file():
            continue
        obj = _json.loads(p.read_text())
        if not isinstance(obj, dict):
            continue
        obj["provenance"] = prov
        p.write_text(_json.dumps(obj, indent=2) + "\n")
        n += 1
    return n


def body(obj):
    """The payload of an artifact, whether or not it was wrapped by `attach`."""
    if isinstance(obj, dict) and "provenance" in obj and "cells" in obj:
        return obj["cells"]
    return obj
