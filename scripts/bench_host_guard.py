#!/usr/bin/env python3
"""Refuse a busy benchmark host, and void a run the host disturbed (#1210).

    bench_host_guard.py check     --own-root PID [--pin-cpus LIST] [--wait S] [--json F] [--md F]
    bench_host_guard.py watch     --own-root PID --out F.jsonl [--pin-cpus LIST]
    bench_host_guard.py summarize --in F.jsonl [--md F]
    bench_host_guard.py --self-test

## Why

The v0.9 campaign's timed windows overlapped `cc1plus`, `rustc`/`cargo`, and a
kvbench container at about 100% CPU, none of which took the benchmark lock.
The workflow's only defence was a `ps` snapshot printed at the start. `ps`
%CPU is a process's lifetime average, so a long-lived process that has just
become busy reads as nearly idle there, and nothing looked again once the
benchmark started.

## What it measures

Everything comes from two readings of `/proc` a window apart:

- **Foreign load**: host CPU the run's own process tree did not use, in
  core-equivalents. The own tree is `--own-root` and its descendants. Their
  CPU is their `utime+stime`, plus the root's `cutime+cstime` for descendants
  already reaped. (`RUSAGE_CHILDREN` alone would miss a benchmark still running.)
- **On-pin foreign load**: the pinned CPUs' busy time, from the per-CPU
  `/proc/stat` lines, minus the own tree's. The own tree is pinned there, so
  whatever else ran on those CPUs took time or an SMT sibling from the
  benchmark directly. Load elsewhere contends only for shared cache, memory
  bandwidth and package power, and is counted as foreign but not on-pin.
- **Offenders**: each foreign process's CPU over the window, from its own
  `/proc/<pid>/stat` delta, named by `comm`, PID, elapsed time and short cgroup
  id (a container's id). No command line or user is recorded: this output is
  published.
- **PSI** `some avg10` for CPU and IO, where the kernel exposes it.

Thresholds are `bench_provenance.START_*` (the start gate) and `RUN_*_VOID`
(the in-run void boundary, AGENTS.md section 8.17). This file does not restate them.

## Commands

- `check`: before any build. It needs `START_QUIET_WINDOWS` consecutive quiet
  windows of `START_WINDOW_S`, retrying until `--wait` runs out. Otherwise it
  exits 1 and names the offenders. Refusal is decided on measured CPU, never
  on a compiler merely being present.
- `watch`: in the background for the whole run, on CPUs outside the pin set,
  so the sampling itself does not load the measured cores. Each sample is
  appended as one JSON line, with the pinned CPUs' governor. The watcher's own
  CPU is recorded and excluded. It exits when `--own-root` exits.
- `summarize`: at the end. Any sample over a void boundary makes it exit 1: a
  contaminated run is discarded, not reinterpreted (section 8.17).

Off Linux there is no `/proc`. Each command then says so and exits 0: such a
host is never the reference host.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import signal
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_provenance as bp

_UNSAFE = re.compile(r"[^A-Za-z0-9_.:+-]")


def sanitize(text: str, limit: int = 32) -> str:
    """A process name fit for a published table (it is chosen by the process)."""
    return _UNSAFE.sub("?", text.strip())[:limit] or "?"


# --------------------------------------------------------------------------
# /proc readings
# --------------------------------------------------------------------------
@dataclass
class Proc:
    pid: int
    ppid: int
    comm: str
    ticks: int  # utime + stime
    child_ticks: int  # cutime + cstime (reaped descendants)
    start_ticks: int


def parse_stat(line: str) -> Proc:
    """One `/proc/<pid>/stat` line. `comm` may hold spaces and parentheses.

    A line this cannot parse raises: silently dropping a process would hide
    exactly the load this module exists to find.
    """
    head, _, rest = line.rpartition(")")
    pid_s, _, comm = head.partition(" (")
    f = rest.split()
    if len(f) < 20:
        raise ValueError(f"unparseable /proc stat line: {line[:80]!r}")
    # rest starts at field 3 (state): ppid=4 utime=14 stime=15 cutime=16
    # cstime=17 starttime=22, i.e. rest indices 1, 11, 12, 13, 14, 19.
    return Proc(
        pid=int(pid_s), ppid=int(f[1]), comm=comm,
        ticks=int(f[11]) + int(f[12]), child_ticks=int(f[13]) + int(f[14]),
        start_ticks=int(f[19]),
    )


def read_procs(proc: Path) -> dict[int, Proc]:
    out: dict[int, Proc] = {}
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            text = (entry / "stat").read_text()
        except OSError:  # discipline:allow(error-swallowing): the process exited between listing and reading; it has no load left to count
            continue
        p = parse_stat(text)
        out[p.pid] = p
    return out


def read_cpu_ticks(proc: Path) -> dict[int, tuple[int, int]]:
    """`{cpu: (busy, total)}` from the per-CPU lines of `/proc/stat`."""
    out: dict[int, tuple[int, int]] = {}
    for line in (proc / "stat").read_text().splitlines():
        m = re.match(r"cpu(\d+)\s+(.*)", line)
        if not m:
            continue
        vals = [int(v) for v in m.group(2).split()]
        idle = vals[3] + (vals[4] if len(vals) > 4 else 0)  # idle + iowait
        out[int(m.group(1))] = (sum(vals[:8]) - idle, sum(vals[:8]))
    return out


def uptime_s(proc: Path) -> float:
    return float((proc / "uptime").read_text().split()[0])


def cgroup_id(proc: Path, pid: int) -> str:
    """A short id for the process's cgroup: a container's id, or its unit name."""
    try:
        text = (proc / str(pid) / "cgroup").read_text()
    except OSError:
        return ""
    last = text.strip().splitlines()[-1].rsplit("/", 1)[-1] if text.strip() else ""
    m = re.search(r"([0-9a-f]{12})[0-9a-f]{52}", last)
    return m.group(1) if m else sanitize(last.removesuffix(".scope").removesuffix(".service"), 40)


def psi(proc: Path) -> dict[str, float | None]:
    out: dict[str, float | None] = {}
    for res in ("cpu", "io"):
        try:
            m = re.search(r"some avg10=([0-9.]+)", (proc / "pressure" / res).read_text())
            out[res] = float(m.group(1)) if m else None
        except OSError:
            out[res] = None
    return out


@dataclass
class Sample:
    mono: float
    uptime: float
    cpus: dict[int, tuple[int, int]]
    procs: dict[int, Proc]


def sample(proc: Path) -> Sample:
    return Sample(time.monotonic(), uptime_s(proc), read_cpu_ticks(proc), read_procs(proc))


def descendants(procs: dict[int, Proc], root: int) -> set[int]:
    kids: dict[int, list[int]] = {}
    for p in procs.values():
        kids.setdefault(p.ppid, []).append(p.pid)
    out, stack = set(), [root]
    while stack:
        pid = stack.pop()
        if pid in out:
            continue
        out.add(pid)
        stack.extend(kids.get(pid, []))
    return out


def tree_ticks(procs: dict[int, Proc], tree: set[int], root: int) -> int:
    """CPU the tree has used: live members, plus the root's reaped descendants."""
    total = sum(procs[p].ticks for p in tree if p in procs)
    if root in procs:
        total += procs[root].child_ticks
    return total


# --------------------------------------------------------------------------
# assessment
# --------------------------------------------------------------------------
def assess(a: Sample, b: Sample, pin: list[int], own_root: int, self_pids: set[int] = frozenset()) -> dict:
    """What happened on the host between two samples, in core-equivalents."""
    dt = b.mono - a.mono
    if dt < bp.MIN_WINDOW_S:
        raise ValueError(f"window {dt:.3f}s is below MIN_WINDOW_S {bp.MIN_WINDOW_S}s; not measurable")
    hz = bp.USER_HZ
    pin_set = set(pin)
    on_busy = off_busy = 0
    for cpu, (busy, _tot) in b.cpus.items():
        if cpu not in a.cpus:
            continue
        d = busy - a.cpus[cpu][0]
        if cpu in pin_set:
            on_busy += d
        else:
            off_busy += d
    own_a = descendants(a.procs, own_root) - set(self_pids)
    own_b = descendants(b.procs, own_root) - set(self_pids)
    own = max(0, tree_ticks(b.procs, own_b, own_root) - tree_ticks(a.procs, own_a, own_root))
    self_ticks = sum(
        b.procs[p].ticks - a.procs[p].ticks for p in self_pids if p in a.procs and p in b.procs
    )
    scale = hz * dt
    on_pin_foreign = (on_busy - own) / scale
    foreign = (on_busy + off_busy - own - self_ticks) / scale

    offenders = []
    own_all = own_a | own_b | set(self_pids)
    for pid, pb in b.procs.items():
        if pid in own_all or pid not in a.procs or a.procs[pid].start_ticks != pb.start_ticks:
            continue
        pct = (pb.ticks - a.procs[pid].ticks) / scale * 100.0
        if pct >= 1.0:
            offenders.append({
                "pid": pid,
                "comm": sanitize(pb.comm),
                "cpu_pct": round(pct, 1),
                "etime_s": int(b.uptime - pb.start_ticks / hz),
            })
    offenders.sort(key=lambda o: -o["cpu_pct"])
    return {
        "window_s": round(dt, 3),
        "foreign_busy_cpus": round(foreign, 3),
        "on_pin_foreign_busy_cpus": round(on_pin_foreign, 3),
        "own_busy_cpus": round(own / scale, 3),
        "self_busy_cpus": round(self_ticks / scale, 3),
        "offenders": offenders[:10],
    }


def verdict(a: dict, foreign_max: float, on_pin_max: float, proc_max: float) -> list[str]:
    """Why a window exceeds the bounds; empty when it does not."""
    why = []
    if a["foreign_busy_cpus"] > foreign_max:
        why.append(f"foreign load {a['foreign_busy_cpus']:.2f} > {foreign_max} core-equivalents")
    if a["on_pin_foreign_busy_cpus"] > on_pin_max:
        why.append(f"foreign load on the pinned CPUs {a['on_pin_foreign_busy_cpus']:.2f} > {on_pin_max}")
    for o in a["offenders"]:
        if o["cpu_pct"] >= proc_max:
            why.append(f"{o['comm']} (pid {o['pid']}, running {o['etime_s']}s) at {o['cpu_pct']:.0f}% >= {proc_max:.0f}%")
    return why


def offender_table(offenders: list[dict]) -> list[str]:
    if not offenders:
        return ["(no foreign process at or above 1% of a CPU)"]
    rows = ["  pid     %cpu  elapsed  cgroup        command"]
    for o in offenders:
        rows.append(f"  {o['pid']:<7} {o['cpu_pct']:>5.1f}  {o['etime_s']:>6}s  {o.get('cgroup', ''):<12}  {o['comm']}")
    return rows


# --------------------------------------------------------------------------
# pin set
# --------------------------------------------------------------------------
def resolve_pin(sysfs: Path = Path("/sys/devices")) -> list[int]:
    """The CPUs the run will be pinned to, as `bench_pin.sh` decides them."""
    applied = os.environ.get("EXPANSE_BENCH_PIN_APPLIED") or os.environ.get("EXPANSE_BENCH_PIN") or ""
    if applied and applied not in ("off", "none", "unset"):
        return bp.expand_cpu_list(applied)
    core = sysfs / "cpu_core" / "cpus"
    if applied != "off" and core.exists():
        return bp.expand_cpu_list(core.read_text().strip())
    online = sysfs / "system" / "cpu" / "online"
    return bp.expand_cpu_list(online.read_text().strip()) if online.exists() else []


# --------------------------------------------------------------------------
# commands
# --------------------------------------------------------------------------
def _gh_output(**kv: str) -> None:
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as fh:
            for k, v in kv.items():
                fh.write(f"{k}={v}\n")


def _with_cgroups(proc: Path, a: dict) -> dict:
    for o in a["offenders"]:
        o["cgroup"] = cgroup_id(proc, o["pid"])
    return a


def cmd_check(args, proc: Path) -> int:
    pin = bp.expand_cpu_list(args.pin_cpus) if args.pin_cpus else resolve_pin()
    deadline = time.monotonic() + args.wait
    quiet = 0
    history: list[dict] = []
    prev = sample(proc)
    while True:
        time.sleep(bp.START_WINDOW_S)
        cur = sample(proc)
        a = _with_cgroups(proc, assess(prev, cur, pin, args.own_root, {os.getpid()}))
        prev = cur
        why = verdict(a, bp.START_FOREIGN_MAX, bp.START_ON_PIN_MAX, bp.START_PROCESS_MAX_PCT)
        a["quiet"] = not why
        a["why"] = why
        history.append(a)
        quiet = quiet + 1 if not why else 0
        if quiet >= bp.START_QUIET_WINDOWS:
            ok = True
            break
        if time.monotonic() >= deadline and len(history) >= bp.START_QUIET_WINDOWS:
            ok = False
            break
    last = history[-1]
    report = {
        "verdict": "quiet" if ok else "busy",
        "pin_cpus": pin,
        "psi_some_avg10": psi(proc),
        "loadavg": (proc / "loadavg").read_text().split()[:3],
        "thresholds": {
            "foreign_max": bp.START_FOREIGN_MAX, "on_pin_max": bp.START_ON_PIN_MAX,
            "process_max_pct": bp.START_PROCESS_MAX_PCT, "window_s": bp.START_WINDOW_S,
            "quiet_windows": bp.START_QUIET_WINDOWS,
        },
        "windows": history,
    }
    if args.json:
        Path(args.json).write_text(json.dumps(report, indent=2) + "\n")
    lines = [
        f"host guard: {report['verdict']} after {len(history)} window(s) of {bp.START_WINDOW_S:g}s; "
        f"foreign {last['foreign_busy_cpus']:.2f}, on the pinned CPUs {last['on_pin_foreign_busy_cpus']:.2f} "
        f"core-equivalents; loadavg {' '.join(report['loadavg'])}",
        *offender_table(last["offenders"]),
    ]
    if args.md:
        Path(args.md).write_text(
            "<details>\n<summary><b>Host guard (live /proc sample before the run)</b></summary>\n\n```text\n"
            + "\n".join(lines) + "\n```\n\n</details>\n"
        )
    print("\n".join(lines))
    if ok:
        return 0
    for w in last["why"]:
        print(f"::error::refusing to start: the host is busy: {w}")
    if args.github_output:
        _gh_output(fail_reason="host_busy")
    return 1


def cmd_watch(args, proc: Path) -> int:
    pin = bp.expand_cpu_list(args.pin_cpus) if args.pin_cpus else resolve_pin()
    stop = False

    def _stop(_s, _f):
        nonlocal stop
        stop = True

    signal.signal(signal.SIGTERM, _stop)
    signal.signal(signal.SIGINT, _stop)
    # Sample from outside the pin set, so the sampling does not take time on
    # the CPUs being measured; on a host with no CPU outside it, stay unpinned
    # at low priority and say so in the record.
    online_path = Path("/sys/devices/system/cpu/online")
    online = set(bp.expand_cpu_list(online_path.read_text().strip())) if online_path.exists() else set()
    outside = sorted(online - set(pin))
    placed = "unplaced"
    if outside and hasattr(os, "sched_setaffinity"):
        try:
            os.sched_setaffinity(0, outside)
            placed = ",".join(map(str, outside))
        except OSError as exc:  # discipline:allow(error-swallowing): recorded as watch_cpus=unplaced in every sample
            print(f"::warning::host watcher could not move off the pinned CPUs: {exc.strerror}", file=sys.stderr)
    try:
        os.nice(10)
    except OSError:  # discipline:allow(error-swallowing): a lower priority is a courtesy; the sample's own CPU is measured either way
        pass
    me = {os.getpid()}
    prev = sample(proc)
    with open(args.out, "a", encoding="utf-8") as fh:
        while not stop:
            time.sleep(args.interval)
            cur = sample(proc)
            if args.own_root not in cur.procs:
                break  # the run is over
            a = _with_cgroups(proc, assess(prev, cur, pin, args.own_root, me))
            prev = cur
            a["t"] = round(time.time(), 1)
            a["watch_cpus"] = placed
            a["psi_some_avg10"] = psi(proc)
            a["governor"] = sorted(set(filter(None, (bp.scaling_governor_by_cpu(",".join(map(str, pin))) or {}).values())))
            fh.write(json.dumps(a) + "\n")
            fh.flush()
    return 0


def cmd_summarize(args) -> int:
    samples = []
    try:
        for line in Path(args.inp).read_text().splitlines():
            if line.strip():
                samples.append(json.loads(line))
    except OSError:
        print(f"::error::no host-activity record at {args.inp}: the run cannot be shown undisturbed")
        return 1
    bad = []
    governors = set()
    for s in samples:
        why = verdict(s, bp.RUN_FOREIGN_VOID, bp.RUN_ON_PIN_VOID, bp.RUN_PROCESS_VOID_PCT)
        if why:
            bad.append((s, why))
        governors.update(s.get("governor") or [])
    worst_on = max((s["on_pin_foreign_busy_cpus"] for s in samples), default=0.0)
    worst = max((s["foreign_busy_cpus"] for s in samples), default=0.0)
    lines = [
        f"host activity during the run: {len(samples)} sample(s); worst foreign {worst:.2f}, "
        f"worst on the pinned CPUs {worst_on:.2f} core-equivalents; governor on the pinned CPUs: "
        f"{', '.join(sorted(governors)) or 'unknown'}",
    ]
    for s, why in bad[:5]:
        lines.append(f"  at {time.strftime('%H:%M:%S', time.gmtime(s['t']))}Z: " + "; ".join(why))
        lines += offender_table(s["offenders"][:3])
    if len(governors) > 1:
        bad.append(({}, ["the governor on the pinned CPUs changed during the run"]))
        lines.append("  the governor on the pinned CPUs changed during the run")
    if not samples:
        bad.append(({}, ["no sample was taken"]))
        lines.append("  no sample was taken, so the run cannot be shown undisturbed")
    if args.md:
        Path(args.md).write_text(
            "<details>\n<summary><b>Host activity during the run</b></summary>\n\n```text\n"
            + "\n".join(lines) + "\n```\n\n</details>\n"
        )
    print("\n".join(lines))
    if bad:
        print(f"::error::the host was disturbed during the run ({len(bad)} sample(s) over the void boundary); "
              "the run is discarded, not reinterpreted (AGENTS.md section 8.17)")
        return 1
    return 0


# --------------------------------------------------------------------------
# self-test: synthetic /proc trees
# --------------------------------------------------------------------------
def _stat_line(pid, ppid, comm, ticks, child=0, start=100):
    # fields 3..22: state ppid pgrp session tty tpgid flags minflt cminflt
    # majflt cmajflt utime stime cutime cstime priority nice threads itreal starttime
    return (f"{pid} ({comm}) S {ppid} 1 1 0 -1 0 0 0 0 0 {ticks} 0 {child} 0 20 0 1 0 {start} 0 0\n")


def _write_proc(root: Path, cpus: dict[int, int], procs: list[tuple], uptime: float = 1000.0) -> None:
    import shutil

    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    lines = ["cpu  0 0 0 0 0 0 0 0 0 0"]
    for cpu, busy in cpus.items():
        # user=busy, idle=10000-busy, everything else 0
        lines.append(f"cpu{cpu} {busy} 0 0 {100000 - busy} 0 0 0 0 0 0")
    (root / "stat").write_text("\n".join(lines) + "\n")
    (root / "uptime").write_text(f"{uptime} 0\n")
    (root / "loadavg").write_text("0.10 0.20 0.30 1/100 1\n")
    for pid, ppid, comm, ticks, child, start in procs:
        d = root / str(pid)
        d.mkdir()
        (d / "stat").write_text(_stat_line(pid, ppid, comm, ticks, child, start))
        (d / "cgroup").write_text(f"0::/system.slice/docker-{'ab' * 32}.scope\n" if comm == "kvbench" else "0::/user.slice\n")


def self_test() -> int:
    hz = bp.USER_HZ
    pin = [0, 1]  # "P-cores"; 2 and 3 are "E-cores"
    root = 10  # the run's shell
    # parse_stat copes with a comm holding spaces and parentheses.
    p = parse_stat("42 (a (b) c) S 7 1 1 0 -1 0 0 0 0 0 5 6 7 8 20 0 1 0 99 0 0")
    assert p and p.pid == 42 and p.ppid == 7 and p.comm == "a (b) c" and p.ticks == 11 and p.child_ticks == 15 and p.start_ticks == 99, p

    def samp(tmp: Path, t: float, cpus, procs, uptime=1000.0) -> Sample:
        _write_proc(tmp, cpus, procs, uptime)
        s = sample(tmp)
        s.mono = t
        return s

    with tempfile.TemporaryDirectory() as td:
        t = Path(td, "proc")
        base = [(1, 0, "systemd", 0, 0, 1), (root, 1, "bash", 0, 0, 500), (11, root, "bench", 0, 0, 600)]

        # 1. Quiet: only the run's own benchmark burns a pinned CPU.
        a = samp(t, 0.0, {0: 0, 1: 0, 2: 0, 3: 0}, base)
        b = samp(t, 1.0, {0: hz, 1: 0, 2: 0, 3: 0}, [(1, 0, "systemd", 0, 0, 1), (root, 1, "bash", 0, 0, 500), (11, root, "bench", hz, 0, 600)])
        r = assess(a, b, pin, root)
        assert abs(r["own_busy_cpus"] - 1.0) < 1e-6 and abs(r["on_pin_foreign_busy_cpus"]) < 1e-6, r
        assert not verdict(r, bp.START_FOREIGN_MAX, bp.START_ON_PIN_MAX, bp.START_PROCESS_MAX_PCT), r

        # 2. The motivating defect: a container process that has run for
        # hours (low lifetime %CPU, what `ps` showed) is now at 100% of an
        # E-core. Foreign, off-pin, and named with its container id.
        procs_a = base + [(50, 1, "kvbench", 360_000 * hz // 100, 0, 1_000)]
        procs_b = base[:2] + [(11, root, "bench", hz, 0, 600), (50, 1, "kvbench", 360_000 * hz // 100 + hz, 0, 1_000)]
        a = samp(t, 0.0, {0: 0, 1: 0, 2: 0, 3: 0}, procs_a, uptime=10_000_000.0)
        b = samp(t, 1.0, {0: hz, 1: 0, 2: hz, 3: 0}, procs_b, uptime=10_000_001.0)
        lifetime_pct = 100.0 * (360_000 * hz // 100) / hz / (10_000_000.0 - 1_000 / hz)
        assert lifetime_pct < 5.0, lifetime_pct  # what `ps` would have printed
        r = _with_cgroups(t, assess(a, b, pin, root))
        assert abs(r["foreign_busy_cpus"] - 1.0) < 1e-6 and abs(r["on_pin_foreign_busy_cpus"]) < 1e-6, r
        assert r["offenders"][0]["comm"] == "kvbench" and r["offenders"][0]["cpu_pct"] == 100.0, r
        assert r["offenders"][0]["cgroup"] == "ab" * 6, r
        why = verdict(r, bp.START_FOREIGN_MAX, bp.START_ON_PIN_MAX, bp.START_PROCESS_MAX_PCT)
        assert any("kvbench" in w for w in why) and any("foreign load 1.00" in w for w in why), why
        # Off-pin, it is not yet on-pin contamination...
        assert not any("pinned CPUs" in w for w in why), why

        # 3. ...but the same process on a pinned CPU (an SMT sibling of the
        # benchmark) is, at a far lower level.
        procs_b3 = base[:2] + [(11, root, "bench", hz, 0, 600), (50, 1, "kvbench", 360_000 * hz // 100 + hz // 4, 0, 1_000)]
        b = samp(t, 1.0, {0: hz, 1: hz // 4, 2: 0, 3: 0}, procs_b3, uptime=10_000_001.0)
        r = assess(a, b, pin, root)
        assert abs(r["on_pin_foreign_busy_cpus"] - 0.25) < 0.02, r
        assert any("pinned CPUs" in w for w in verdict(r, bp.START_FOREIGN_MAX, bp.START_ON_PIN_MAX, bp.START_PROCESS_MAX_PCT))

        # 4. A child the run reaped mid-window is own, not foreign: its CPU
        # has moved into the root's cutime.
        a = samp(t, 0.0, {0: 0, 1: 0, 2: 0, 3: 0}, base)
        b = samp(t, 1.0, {0: hz, 1: 0, 2: 0, 3: 0}, [(1, 0, "systemd", 0, 0, 1), (root, 1, "bash", 0, hz, 500)])
        r = assess(a, b, pin, root)
        assert abs(r["on_pin_foreign_busy_cpus"]) < 1e-6 and abs(r["own_busy_cpus"] - 1.0) < 1e-6, r

        # 5. A window below MIN_WINDOW_S is not measurable, never "quiet".
        a = samp(t, 0.0, {0: 0, 1: 0, 2: 0, 3: 0}, base)
        b = samp(t, bp.MIN_WINDOW_S / 2, {0: 0, 1: 0, 2: 0, 3: 0}, base)
        try:
            assess(a, b, pin, root)
        except ValueError:
            pass
        else:
            raise AssertionError("a sub-minimum window must be refused")

        # 6. A compiler that is present but idle is not a reason to refuse.
        idle = base + [(60, 1, "cc1plus", 500, 0, 700)]
        a = samp(t, 0.0, {0: 0, 1: 0, 2: 0, 3: 0}, idle)
        b = samp(t, 1.0, {0: 0, 1: 0, 2: 0, 3: 0}, idle)
        r = assess(a, b, pin, root)
        assert not verdict(r, bp.START_FOREIGN_MAX, bp.START_ON_PIN_MAX, bp.START_PROCESS_MAX_PCT), r

        # 7. summarize voids a run with one bad sample, and one whose
        # governor changed; a clean record passes.
        jl = Path(td, "act.jsonl")
        clean = {"t": 0, "foreign_busy_cpus": 0.1, "on_pin_foreign_busy_cpus": 0.02, "offenders": [], "governor": ["powersave"]}
        jl.write_text(json.dumps(clean) + "\n")
        ns = argparse.Namespace(inp=str(jl), md=None)
        with open(os.devnull, "w") as dn:
            old, sys.stdout = sys.stdout, dn
            try:
                assert cmd_summarize(ns) == 0
                hot = dict(clean, on_pin_foreign_busy_cpus=0.5)
                jl.write_text(json.dumps(clean) + "\n" + json.dumps(hot) + "\n")
                assert cmd_summarize(ns) == 1
                flip = dict(clean, governor=["performance"])
                jl.write_text(json.dumps(clean) + "\n" + json.dumps(flip) + "\n")
                assert cmd_summarize(ns) == 1
                jl.write_text("")
                assert cmd_summarize(ns) == 1, "an empty record cannot show a quiet run"
            finally:
                sys.stdout = old

        # 8. The pin set follows bench_pin.sh: an explicit list, else cpu_core.
        sysfs = Path(td, "sys")
        (sysfs / "cpu_core").mkdir(parents=True)
        (sysfs / "cpu_core" / "cpus").write_text("0-15\n")
        saved = {k: os.environ.pop(k, None) for k in ("EXPANSE_BENCH_PIN", "EXPANSE_BENCH_PIN_APPLIED")}
        try:
            assert resolve_pin(sysfs) == list(range(16))
            os.environ["EXPANSE_BENCH_PIN"] = "0,2,4"
            assert resolve_pin(sysfs) == [0, 2, 4]
        finally:
            for k, v in saved.items():
                if v is None:
                    os.environ.pop(k, None)
                else:
                    os.environ[k] = v

    assert sanitize("k`v\nx") == "k?v?x"
    print("bench_host_guard.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    c = sub.add_parser("check")
    c.add_argument("--own-root", type=int, default=os.getppid())
    c.add_argument("--pin-cpus")
    c.add_argument("--wait", type=float, default=0.0, help="seconds to keep retrying a busy host")
    c.add_argument("--json")
    c.add_argument("--md")
    c.add_argument("--github-output", action="store_true")
    w = sub.add_parser("watch")
    w.add_argument("--own-root", type=int, required=True)
    w.add_argument("--pin-cpus")
    w.add_argument("--out", required=True)
    w.add_argument("--interval", type=float, default=bp.WATCH_INTERVAL_S)
    s = sub.add_parser("summarize")
    s.add_argument("--in", dest="inp", required=True)
    s.add_argument("--md")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    proc = Path("/proc")
    if args.cmd in ("check", "watch") and not (proc / "stat").exists():
        print(f"::notice::no /proc on this host; the host guard's {args.cmd} did not run (a non-Linux host is never the reference host)")
        return 0
    if args.cmd == "check":
        return cmd_check(args, proc)
    if args.cmd == "watch":
        return cmd_watch(args, proc)
    if args.cmd == "summarize":
        return cmd_summarize(args)
    ap.error("a command is required")
    return 2


if __name__ == "__main__":
    sys.exit(main())
