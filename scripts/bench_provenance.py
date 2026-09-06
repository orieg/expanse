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
in core-equivalents, from `/proc/stat` jiffies. The one-minute load average
lags a heavy process by about thirty seconds, which is how a documented 2.2x
baseline shift happened on this project despite a load pre-check passing. The
jiffy delta between two snapshots is exact over the interval it covers, the
harness's own threads included: a single-threaded phase on a quiet host reads
about 1.0, and a concurrent sweep reads its own thread count.

Linux-only where it reads `/proc` and `/sys`; every reader degrades to `None`
off Linux rather than inventing a value.
"""

from __future__ import annotations

import os
import platform
import subprocess
import time
from pathlib import Path

__all__ = [
    "cpu_jiffies", "load_snapshot", "add_load", "host_facts", "raw_rounds",
    "estimators", "git_sha", "new_provenance", "attach", "body", "rewrite",
]


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
    snapshot, when either side is off Linux, or when no time passed.
    """
    if not prev or busy is None or total is None:
        return None
    p_busy, p_total = prev.get("stat_busy_jiffies"), prev.get("stat_total_jiffies")
    if p_busy is None or p_total is None:
        return None
    dt = total - p_total
    if dt <= 0:
        return None
    n = ncpu if ncpu is not None else (os.cpu_count() or 1)
    return round((busy - p_busy) / dt * n, 2)


def load_snapshot(label: str, prev: dict | None = None) -> dict:
    """Load averages, plus the host's busy CPU since `prev` in core-equivalents."""
    try:
        one, five, fifteen = os.getloadavg()
    except OSError:
        one = five = fifteen = float("nan")
    busy, total = cpu_jiffies()
    return {
        "label": label,
        "load1": round(one, 2), "load5": round(five, 2), "load15": round(fifteen, 2),
        "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "stat_busy_jiffies": busy, "stat_total_jiffies": total,
        "busy_cpus_since_prev": busy_cpus(prev, busy, total),
    }


def add_load(prov: dict, label: str) -> None:
    """Appends a snapshot to `prov["loads"]`, differenced against the last one."""
    loads = prov.setdefault("loads", [])
    loads.append(load_snapshot(label, loads[-1] if loads else None))


def _read(path: str) -> str | None:
    try:
        with open(path) as fh:
            return fh.read().strip()
    except OSError:
        return None


def host_facts() -> dict:
    """What a wall-clock number depends on that `platform.platform()` does not say."""
    model = None
    try:
        with open("/proc/cpuinfo") as fh:
            for line in fh:
                if line.startswith("model name"):
                    model = line.split(":", 1)[1].strip()
                    break
    except OSError:
        pass
    return {
        "cpu_model": model or platform.processor() or platform.machine(),
        "cpus_online": os.cpu_count(),
        # Hybrid parts: the P-core mask includes SMT siblings, so W + R = 16 on
        # an eight-P-core host runs two threads per physical core.
        "cpu_core_cpus": _read("/sys/devices/cpu_core/cpus"),
        "cpu_atom_cpus": _read("/sys/devices/cpu_atom/cpus"),
        "cpu0_thread_siblings": _read("/sys/devices/system/cpu/cpu0/topology/thread_siblings_list"),
        "scaling_driver": _read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_driver"),
        "scaling_governor": _read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
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
