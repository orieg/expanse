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

- **Foreign load**: the CPU of every task outside the run's own process
  tree, plus interrupt, softirq and steal time, in core-equivalents. The own
  tree is `--own-root` and its descendants. A task's CPU is its
  `utime+stime`, plus its `cutime+cstime` for the children it reaped.
- **On-pin foreign load**: the part of it on the pinned CPUs: the foreign
  tasks that last ran there at either end of the window, and those CPUs'
  interrupt time. Whatever ran there took time or an SMT sibling from the
  benchmark directly. Load elsewhere contends only for shared cache, memory
  bandwidth and package power, and is counted as foreign but not on-pin.
- **Counters minus own tree**: the busy time of the `/proc/stat` CPU lines
  less the own tree's CPU. It is recorded and not judged. With no foreign
  task or interrupt time recorded, bench_baremetal runs 37480277219 and
  37481727076 read it between -0.20 and +0.44 core-equivalents across the
  cells of one sweep, and the first version of this guard voided those runs
  on it. A negative reading can also hide a foreign task of the same size
  (self-test case 12a). Why the two operands disagree is not established.
  They are different clocks (the CPU lines are accumulated at the scheduler
  tick; a process's `utime+stime` sum to its exact run time), but unbiased
  tick sampling over a 2 s window gives an error several times smaller than
  the readings, so that alone does not explain them. The own tree is read
  immediately after `/proc/stat`, so the operands describe the same instant
  (`sample`, #1270).
- **Attribution** (#1270): the foreign load split into task classes —
  kernel threads (by kind: kworker, ksoftirqd, rcu, migration, other), the
  runner's own processes (its cgroup, outside the run's tree), the guard
  itself, and user processes — each with its sum, host-wide and on the pinned
  CPUs; the CPU of children that foreign tasks reaped, all counted on the
  pinned CPUs; interrupt, softirq and steal time from the per-CPU counters;
  and what the pinned CPUs' counters hold beyond the own tree and all of
  those (`unattributed_on_pin`). The void rule reads
  the sums: every class counts as foreign (`docs/BENCHMARKING.md` rule 8).
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
PF_KTHREAD = 0x00200000  # `flags` bit of a kernel thread (include/linux/sched.h)


@dataclass
class Proc:
    pid: int
    ppid: int
    comm: str
    ticks: int  # utime + stime
    child_ticks: int  # cutime + cstime (reaped descendants)
    start_ticks: int
    flags: int = 0
    processor: int = -1  # the CPU its group leader last ran on
    threads: int = 1
    # The CPUs its threads last ran on, read for a multi-threaded process:
    # field 39 of the process line is the leader's alone, while utime+stime
    # are the whole group's.
    thread_cpus: tuple[int, ...] = ()

    def last_cpus(self) -> set[int]:
        return set(self.thread_cpus) | {self.processor}


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
    # rest starts at field 3 (state): ppid=4 flags=9 utime=14 stime=15
    # cutime=16 cstime=17 num_threads=20 starttime=22 processor=39, i.e. rest
    # indices 1, 6, 11, 12, 13, 14, 17, 19 and 36.
    return Proc(
        pid=int(pid_s), ppid=int(f[1]), comm=comm,
        ticks=int(f[11]) + int(f[12]), child_ticks=int(f[13]) + int(f[14]),
        start_ticks=int(f[19]), flags=int(f[6]),
        processor=int(f[36]) if len(f) > 36 else -1,
        threads=int(f[17]),
    )


class IncompleteScan(RuntimeError):
    """`/proc` did not show every process, so foreign load cannot be summed."""


def _thread_cpus(entry: Path) -> tuple[int, ...]:
    cpus = set()
    try:
        tasks = list((entry / "task").iterdir())
    except FileNotFoundError:  # discipline:allow(error-swallowing): the process exited after its stat line was read; its leader's CPU stands
        return ()
    for task in tasks:
        try:
            cpus.add(parse_stat((task / "stat").read_text()).processor)
        except (FileNotFoundError, ProcessLookupError):  # discipline:allow(error-swallowing): the thread exited between listing and reading
            continue
    return tuple(sorted(cpus))


def read_procs(proc: Path) -> dict[int, Proc]:
    """Every process in `proc`. Raises `IncompleteScan` unless all are visible.

    Foreign load is a sum over the processes listed here, so a process this
    cannot see is load it reports as absent. A process that exits between the
    listing and the read is gone and is skipped. One that exists and cannot be
    read (`hidepid`, a PID namespace, a permission) is not: the scan stops.
    PID 1 is always present on a host whose processes are all visible.
    """
    out: dict[int, Proc] = {}
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            text = (entry / "stat").read_text()
        except (FileNotFoundError, ProcessLookupError):  # discipline:allow(error-swallowing): the process exited between listing and reading; it has no load left to count
            continue
        except OSError as exc:
            raise IncompleteScan(f"cannot read {entry / 'stat'}: {exc.strerror}; foreign load cannot be summed") from exc
        p = parse_stat(text)
        if p.threads > 1 and not p.flags & PF_KTHREAD:
            p.thread_cpus = _thread_cpus(entry)
        out[p.pid] = p
    if 1 not in out:
        raise IncompleteScan(f"{proc} lists no PID 1: this process cannot see the host's processes "
                             f"(a PID namespace, or `hidepid`); foreign load cannot be summed")
    return out


@dataclass(frozen=True)
class CpuTicks:
    """One CPU's counters from `/proc/stat`, in USER_HZ ticks."""
    busy: int
    total: int
    irq: int = 0
    softirq: int = 0
    steal: int = 0


def read_cpu_ticks(proc: Path) -> dict[int, CpuTicks]:
    """`{cpu: CpuTicks}` from the per-CPU lines of `/proc/stat`.

    `busy` is everything but idle and iowait. Interrupt, softirq and steal
    time are kept apart as well: with IRQ time accounting (the stock Ubuntu
    kernel on a stable TSC) the kernel charges them to no task, so they are
    busy time no process's `utime + stime` can explain.
    """
    out: dict[int, CpuTicks] = {}
    for line in (proc / "stat").read_text().splitlines():
        m = re.match(r"cpu(\d+)\s+(.*)", line)
        if not m:
            continue
        vals = [int(v) for v in m.group(2).split()] + [0] * 8
        idle = vals[3] + vals[4]  # idle + iowait
        out[int(m.group(1))] = CpuTicks(
            busy=sum(vals[:8]) - idle, total=sum(vals[:8]),
            irq=vals[5], softirq=vals[6], steal=vals[7],
        )
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


def cgroup_path(proc: Path, pid: int) -> str:
    """The process's cgroup path as the kernel prints it (compared, never published)."""
    try:
        text = (proc / str(pid) / "cgroup").read_text().strip()
    except OSError:
        return ""
    return text.splitlines()[-1].partition("::")[2] if text else ""


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
    cpus: dict[int, CpuTicks]
    procs: dict[int, Proc]
    own_read_lag_s: float = 0.0  # from the /proc/stat read to the own tree's
    own_read_passes: int = 0


OWN_REREAD_MAX_PASSES = 5


def _reread(proc: Path, pids: set[int]) -> dict[int, Proc]:
    out = {}
    for pid in pids:
        try:
            out[pid] = parse_stat((proc / str(pid) / "stat").read_text())
        except OSError:  # discipline:allow(error-swallowing): reaped since the scan; its CPU is now in its reaper's cutime, which the next pass reads
            continue
    return out


def sample(proc: Path, own_root: int | None = None, exclude: frozenset[int] | set[int] = frozenset(),
           _after_scan=None) -> Sample:
    """One reading of the host, with the run's own tree read beside `/proc/stat`.

    The whole process table is scanned first (for attribution), then
    `/proc/stat` is read, then the own tree is read again. Foreign load is the
    pinned CPUs' busy time minus the own tree's CPU, so those two must be read
    at the same instant: the first version read `/proc/stat` and then scanned
    every process, so the own tree was read later than the CPU counters by
    however far into the scan it came. When that lag differs between the two
    ends of a window, the own tree's CPU is counted over a slightly different
    interval than the CPUs' busy time: with eight busy cores, a lag 40 ms
    longer at one sample moves a 2 s window's reading by 8 x 0.04 / 2 = 0.16
    core-equivalents, negative in the window before and positive in the one
    after. The voided runs of #1270 show exactly that pairing.

    The own tree is re-read in passes until two consecutive passes find the
    same live processes, so a child reaped mid-read is counted once, in its
    reaper's cutime, never twice and never not at all.
    """
    procs = read_procs(proc)
    own = (descendants(procs, own_root) - set(exclude)) if own_root is not None else set()
    if _after_scan is not None:
        _after_scan()
    t0 = time.monotonic()
    cpus = read_cpu_ticks(proc)
    t1 = time.monotonic()
    passes = 0
    fresh: dict[int, Proc] = {}
    live = own
    while own:
        passes += 1
        fresh = _reread(proc, own)
        if set(fresh) == live or passes >= OWN_REREAD_MAX_PASSES:
            break
        live = set(fresh)
    t2 = time.monotonic()
    for pid in own:
        if pid in fresh:
            procs[pid] = fresh[pid]
        else:
            procs.pop(pid, None)
    return Sample((t0 + t1) / 2, uptime_s(proc), cpus, procs, round(t2 - t1, 6), passes)


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


def tree_ticks(procs: dict[int, Proc], tree: set[int]) -> int:
    """CPU the tree has used: each live member's own, plus what it has reaped.

    A member's `cutime+cstime` holds the CPU of the children it waited for, so
    summing both over the live members counts every process once: a running
    one by its own ticks, a finished one inside the ticks of whoever reaped
    it. Counting only the root's reaped children is not enough: a build's
    `rustc` processes are reaped by `cargo`, which is still running, and
    their CPU then appears nowhere but in cargo's `cutime` (run 36339992457
    read its own build as 11.3 core-equivalents of foreign load that way).
    """
    return sum(procs[p].ticks + procs[p].child_ticks for p in tree if p in procs)


# --------------------------------------------------------------------------
# assessment
# --------------------------------------------------------------------------
def assess(a: Sample, b: Sample, pin: list[int], own_root: int, self_pids: set[int] = frozenset(),
           run_cgroup: str | None = None, cgroup_of=None) -> dict:
    """What happened on the host between two samples, in core-equivalents.

    `run_cgroup` is the cgroup of the run's own root and `cgroup_of(pid)` reads
    another process's; together they tell the runner's own processes apart
    from the rest (`task_class`). Without them no task is classed `runner`.
    """
    dt = b.mono - a.mono
    if dt < bp.MIN_WINDOW_S:
        raise ValueError(f"window {dt:.3f}s is below MIN_WINDOW_S {bp.MIN_WINDOW_S}s; not measurable")
    hz = bp.USER_HZ
    pin_set = set(pin)
    on_busy = off_busy = 0
    intr = {"on": {"irq": 0, "softirq": 0, "steal": 0}, "off": {"irq": 0, "softirq": 0, "steal": 0}}
    for cpu, cb in b.cpus.items():
        if cpu not in a.cpus:
            continue
        ca = a.cpus[cpu]
        d = cb.busy - ca.busy
        side = "on" if cpu in pin_set else "off"
        if side == "on":
            on_busy += d
        else:
            off_busy += d
        for k in intr[side]:
            intr[side][k] += getattr(cb, k) - getattr(ca, k)
    own_a = descendants(a.procs, own_root) - set(self_pids)
    own_b = descendants(b.procs, own_root) - set(self_pids)
    own = max(0, tree_ticks(b.procs, own_b) - tree_ticks(a.procs, own_a))
    self_ticks = sum(
        b.procs[p].ticks - a.procs[p].ticks for p in self_pids if p in a.procs and p in b.procs
    )
    scale = hz * dt

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
    at, foreign_ticks, on_pin_foreign_ticks = attribute(
        a, b, pin_set, own_all, set(self_pids), on_busy - own, intr, scale, run_cgroup, cgroup_of)
    return {
        "window_s": round(dt, 3),
        # Judged: tasks outside the own tree, and interrupt time.
        "foreign_busy_cpus": round(foreign_ticks / scale, 3),
        "on_pin_foreign_busy_cpus": round(on_pin_foreign_ticks / scale, 3),
        # Recorded, not judged: tick-sampled CPU counters less the own tree's
        # exact CPU time. What the two fields above were before #1280's runs.
        "busy_minus_own_cpus": round((on_busy + off_busy - own - self_ticks) / scale, 3),
        "on_pin_busy_minus_own_cpus": round((on_busy - own) / scale, 3),
        "own_busy_cpus": round(own / scale, 3),
        "self_busy_cpus": round(self_ticks / scale, 3),
        "offenders": offenders[:10],
        "own_read_lag_s": b.own_read_lag_s,
        "own_read_passes": b.own_read_passes,
        "attribution": at,
    }


# --------------------------------------------------------------------------
# attribution: which kinds of task the foreign load came from (#1270)
# --------------------------------------------------------------------------
TASK_CLASSES = ("kernel", "runner", "user", "sampler")
KERNEL_KINDS = (("kworker", "kworker"), ("ksoftirqd", "ksoftirqd"), ("rcu", "rcu"), ("migration", "migration"))


def kernel_kind(comm: str) -> str:
    for prefix, kind in KERNEL_KINDS:
        if comm.startswith(prefix):
            return kind
    return "other"


def task_class(p: Proc, self_pids: set[int], run_cgroup: str | None, cgroup_of) -> str:
    """Which kind of foreign task this is.

    - `sampler`: the guard itself.
    - `kernel`: a kernel thread (`PF_KTHREAD`): kworker, ksoftirqd, rcu_*, ...
    - `runner`: a process in the run's own cgroup but outside its tree — for a
      CI job, the runner service (`Runner.Worker`, its log upload); for a
      session on the host, that session's other processes.
    - `user`: everything else.
    """
    if p.pid in self_pids:
        return "sampler"
    if p.flags & PF_KTHREAD or p.pid == 2 or p.ppid == 2:
        return "kernel"
    if run_cgroup and cgroup_of(p.pid) == run_cgroup:
        return "runner"
    return "user"


def attribute(a: Sample, b: Sample, pin_set: set[int], own_all: set[int], self_pids: set[int],
              on_pin_residual_ticks: float, intr: dict, scale: float, run_cgroup: str | None,
              cgroup_of=None) -> tuple[dict, int, int]:
    """The foreign load, by task class and interrupt time.

    Returns the breakdown, and the two sums the void rule is judged on, in
    ticks: host-wide (without the guard's own CPU) and on the pinned CPUs.

    A task's CPU over the window is `(utime + stime + cutime + cstime)` at the
    end less at the start. A task that exited inside the window is no longer
    listed: its reaper's `cutime + cstime` rose by its lifetime total, so its
    total at the start of the window is subtracted, which leaves the CPU it
    used inside the window and nothing from before it.

    A task's own CPU is put on the pinned CPUs when any of its threads last
    ran there at either end of the window. A task that ran there only between
    the two samples is missed, so the on-pin sum is a lower bound for tasks
    still alive. The CPU of reaped children is all put on the pinned CPUs,
    since nothing records where an exited process ran: that part is an upper
    bound, chosen so a short-lived process on a pinned CPU cannot pass.

    Interrupt, softirq and steal time come from the per-CPU counters. Without
    `CONFIG_IRQ_TIME_ACCOUNTING` the kernel charges most hardirq time to the
    interrupted task, so on a pinned CPU it reads as the run's own.

    `on_pin_residual_ticks` is the pinned CPUs' counters less the own tree.
    What the tasks and interrupts there do not account for of it is
    `unattributed_on_pin`. It is recorded and not judged (module docstring).
    """
    cgroup_of = cgroup_of or (lambda _pid: "")
    cg_cache: dict[int, str] = {}

    def cg(pid: int) -> str:
        if pid not in cg_cache:
            cg_cache[pid] = cgroup_of(pid)
        return cg_cache[pid]

    def total(p: Proc) -> int:
        return p.ticks + p.child_ticks

    host = dict.fromkeys(TASK_CLASSES, 0)
    on_pin = dict.fromkeys(TASK_CLASSES, 0)
    kinds: dict[str, int] = {}
    reaped = 0  # CPU of foreign children reaped in the window, net of what exited tasks brought with them
    for pid in set(a.procs) | set(b.procs):
        if pid in own_all and pid not in self_pids:
            continue
        pa, pb = a.procs.get(pid), b.procs.get(pid)
        if pa and pb and pa.start_ticks != pb.start_ticks:
            # A reused PID: the old process exited, a new one was born.
            cls_old = task_class(pa, self_pids, run_cgroup, cg)
            host[cls_old] -= total(pa)
            reaped -= total(pa)
            pa = None
        if pb is None:
            # Exited and reaped: its reaper's cutime+cstime rose by its whole
            # lifetime total, of which only the part after sample `a` belongs
            # to this window. Take the earlier part back out.
            host[task_class(pa, self_pids, run_cgroup, cg)] -= total(pa)
            reaped -= total(pa)
            continue
        d_own = pb.ticks - (pa.ticks if pa else 0)
        d_child = pb.child_ticks - (pa.child_ticks if pa else 0)
        if d_own == 0 and d_child == 0:
            continue
        cls = task_class(pb, self_pids, run_cgroup, cg)
        host[cls] += d_own + d_child
        reaped += d_child
        if pb.last_cpus() & pin_set or (pa is not None and pa.last_cpus() & pin_set):
            on_pin[cls] += d_own
        if cls == "kernel":
            k = kernel_kind(pb.comm)
            kinds[k] = kinds.get(k, 0) + d_own + d_child
    # An exited task's lifetime total can land in a reaper outside this sum
    # (the run's own tree), leaving a class negative. Negative foreign CPU
    # does not exist.
    host = {k: max(0, v) for k, v in host.items()}
    reaped = max(0, reaped)

    def ce(ticks: float) -> float:
        return round(ticks / scale, 3)

    intr_on = sum(intr["on"].values())
    # Where a reaped child ran is not recorded anywhere: all of it is counted
    # on the pinned CPUs, so a short-lived foreign process there cannot pass.
    tasks_on = sum(on_pin.values()) + reaped
    foreign_ticks = sum(v for k, v in host.items() if k != "sampler") + intr_on + sum(intr["off"].values())
    return {
        "classes": {k: ce(v) for k, v in host.items()},
        "classes_on_pin": {k: ce(v) for k, v in on_pin.items()},
        "reaped_children_on_pin": ce(reaped),
        "kernel_kinds": {k: ce(v) for k, v in sorted(kinds.items())},
        "interrupts_on_pin": {k: ce(v) for k, v in intr["on"].items()},
        "interrupts_off_pin": {k: ce(v) for k, v in intr["off"].items()},
        "unattributed_on_pin": ce(on_pin_residual_ticks - tasks_on - intr_on),
        "placement": "a task's own CPU is placed on the pinned CPUs when any of its threads last ran there at "
                     "either end of the window; the CPU of children it reaped is always placed there",
    }, foreign_ticks, tasks_on + intr_on


def verdict(a: dict, foreign_max: float, on_pin_max: float, proc_max: float) -> list[str]:
    """Why a window exceeds the bounds; empty when it does not."""
    why = []
    if a["foreign_busy_cpus"] > foreign_max:
        why.append(f"foreign load {a['foreign_busy_cpus']:.3f} > {foreign_max:g} core-equivalents")
    if a["on_pin_foreign_busy_cpus"] > on_pin_max:
        why.append(f"foreign load on the pinned CPUs {a['on_pin_foreign_busy_cpus']:.3f} > {on_pin_max:g}")
    for o in a["offenders"]:
        if o["cpu_pct"] >= proc_max:
            why.append(f"{o['comm']} (pid {o['pid']}, running {o['etime_s']}s) at {o['cpu_pct']:.1f}% >= {proc_max:g}%")
    return why


def attribution_lines(a: dict) -> list[str]:
    """What a window's foreign load was made of, by task class, as text.

    Every class is printed with its sum, including zero, so a void that no
    single process explains still says which kinds of task it did see.
    """
    at = a.get("attribution")
    if not at:
        return ["  (recorded before the attribution existed, #1270: no task-class breakdown)"]
    on, intr_on = at["classes_on_pin"], at["interrupts_on_pin"]
    tasks_on = sum(on.values())
    reaped_on = at.get("reaped_children_on_pin", 0.0)
    intr_sum = sum(intr_on.values())
    lines = [
        f"  on the pinned CPUs: tasks {tasks_on:.2f} "
        f"({', '.join(f'{k} {v:.2f}' for k, v in on.items())}) + reaped children {reaped_on:.2f} "
        f"+ interrupts {intr_sum:.2f} "
        f"({', '.join(f'{k} {v:.2f}' for k, v in intr_on.items())}); unattributed {at['unattributed_on_pin']:.2f}"
        + ("" if "on_pin_busy_minus_own_cpus" in a else " (judged: this record predates the task-sum rule)"),
        f"  host-wide foreign tasks: {', '.join(f'{k} {v:.2f}' for k, v in at['classes'].items())}"
        + (f"; kernel threads: {', '.join(f'{k} {v:.2f}' for k, v in at['kernel_kinds'].items())}" if at["kernel_kinds"] else ""),
    ]
    lag = a.get("own_read_lag_s")
    if lag is not None:
        bound = a["own_busy_cpus"] * lag / a["window_s"] if a.get("window_s") else 0.0
        lines.append(f"  own tree read {lag * 1e3:.2f} ms after the CPU counters "
                     f"({a.get('own_read_passes', 0)} pass(es)); at its load that moves the reading by at most {bound:.3f}")
    if "on_pin_busy_minus_own_cpus" in a:
        lines.append(f"  CPU counters less the own tree on the pinned CPUs: {a['on_pin_busy_minus_own_cpus']:.2f} "
                     f"(recorded, not judged: the two are different clocks)")
    elif at["unattributed_on_pin"] > tasks_on + intr_sum:
        lines.append("  no task or interrupt time accounts for most of it")
    return lines


def offender_table(offenders: list[dict]) -> list[str]:
    if not offenders:
        return ["(no single foreign process at or above 1% of a CPU)"]
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
    me = {os.getpid()}
    run_cg = cgroup_path(proc, args.own_root)
    prev = sample(proc, args.own_root, me)
    while True:
        time.sleep(bp.START_WINDOW_S)
        cur = sample(proc, args.own_root, me)
        a = _with_cgroups(proc, assess(prev, cur, pin, args.own_root, me, run_cg,
                                       lambda pid: cgroup_path(proc, pid)))
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
        *attribution_lines(last),
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
    run_cg = cgroup_path(proc, args.own_root)
    prev = sample(proc, args.own_root, me)
    with open(args.out, "a", encoding="utf-8") as fh:
        while not stop:
            time.sleep(args.interval)
            cur = sample(proc, args.own_root, me)
            if args.own_root not in cur.procs:
                break  # the run is over
            a = _with_cgroups(proc, assess(prev, cur, pin, args.own_root, me, run_cg,
                                           lambda pid: cgroup_path(proc, pid)))
            prev = cur
            a["t"] = round(time.time(), 1)
            a["watch_cpus"] = placed
            a["psi_some_avg10"] = psi(proc)
            a["governor"] = sorted(set(filter(None, (bp.scaling_governor_by_cpu(",".join(map(str, pin))) or {}).values())))
            fh.write(json.dumps(a) + "\n")
            fh.flush()
    return 0


# The longest stretch a record may go without a sample, in watch intervals.
WATCH_GAP_FACTOR = 3.0


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
        lines += attribution_lines(s)
        lines += offender_table(s["offenders"][:3])
    # A record with a hole in it does not show the run undisturbed: a watcher
    # that stalled or died saw nothing in the gap, and a long window averages
    # a burst away.
    max_gap = WATCH_GAP_FACTOR * bp.WATCH_INTERVAL_S
    for prev, cur in zip([None] + samples[:-1], samples):
        gap = max(cur.get("window_s", 0.0), (cur["t"] - prev["t"]) if prev and "t" in prev and "t" in cur else 0.0)
        if gap > max_gap:
            why = [f"no sample for {gap:.1f} s (more than {max_gap:g} s): the record does not cover the run"]
            bad.append((cur, why))
            lines.append(f"  at {time.strftime('%H:%M:%S', time.gmtime(cur.get('t', 0)))}Z: {why[0]}")
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
def _stat_line(pid, ppid, comm, ticks, child=0, start=100, flags=0, cpu=0):
    # fields 3..22: state ppid pgrp session tty tpgid flags minflt cminflt
    # majflt cmajflt utime stime cutime cstime priority nice threads itreal
    # starttime; then 23..38 (vsize .. exit_signal) and 39, processor
    return (f"{pid} ({comm}) S {ppid} 1 1 0 -1 {flags} 0 0 0 0 {ticks} 0 {child} 0 20 0 1 0 {start}"
            + " 0" * 16 + f" {cpu}\n")


def _write_proc(root: Path, cpus: dict[int, int], procs: list[tuple], uptime: float = 1000.0,
                irq: dict[int, tuple[int, int]] | None = None) -> None:
    """A synthetic `/proc`. A process is `(pid, ppid, comm, ticks, child, start)`,
    optionally followed by its `flags`, the CPU it last ran on and its cgroup
    path. `irq` gives a CPU's `(irq, softirq)` ticks, part of its busy time."""
    import shutil

    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    lines = ["cpu  0 0 0 0 0 0 0 0 0 0"]
    for cpu, busy in cpus.items():
        # irq and softirq as given, user = the rest of busy, idle = 100000 - busy
        hi, si = (irq or {}).get(cpu, (0, 0))
        lines.append(f"cpu{cpu} {busy - hi - si} 0 0 {100000 - busy} 0 {hi} {si} 0 0 0")
    (root / "stat").write_text("\n".join(lines) + "\n")
    (root / "uptime").write_text(f"{uptime} 0\n")
    (root / "loadavg").write_text("0.10 0.20 0.30 1/100 1\n")
    for pid, ppid, comm, ticks, child, start, *extra in procs:
        flags, cpu, cg = (list(extra) + [0, 0, None][len(extra):])[:3]
        d = root / str(pid)
        d.mkdir()
        (d / "stat").write_text(_stat_line(pid, ppid, comm, ticks, child, start, flags, cpu))
        if cg is None:
            cg = f"/system.slice/docker-{'ab' * 32}.scope" if comm == "kvbench" else "/user.slice"
        (d / "cgroup").write_text(f"0::{cg}\n")


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
        procs_a = base + [(50, 1, "kvbench", 360_000 * hz // 100, 0, 1_000, 0, 2)]
        procs_b = base[:2] + [(11, root, "bench", hz, 0, 600), (50, 1, "kvbench", 360_000 * hz // 100 + hz, 0, 1_000, 0, 2)]
        a = samp(t, 0.0, {0: 0, 1: 0, 2: 0, 3: 0}, procs_a, uptime=10_000_000.0)
        b = samp(t, 1.0, {0: hz, 1: 0, 2: hz, 3: 0}, procs_b, uptime=10_000_001.0)
        lifetime_pct = 100.0 * (360_000 * hz // 100) / hz / (10_000_000.0 - 1_000 / hz)
        assert lifetime_pct < 5.0, lifetime_pct  # what `ps` would have printed
        r = _with_cgroups(t, assess(a, b, pin, root))
        assert abs(r["foreign_busy_cpus"] - 1.0) < 1e-6 and abs(r["on_pin_foreign_busy_cpus"]) < 1e-6, r
        assert r["offenders"][0]["comm"] == "kvbench" and r["offenders"][0]["cpu_pct"] == 100.0, r
        assert r["offenders"][0]["cgroup"] == "ab" * 6, r
        why = verdict(r, bp.START_FOREIGN_MAX, bp.START_ON_PIN_MAX, bp.START_PROCESS_MAX_PCT)
        assert any("kvbench" in w for w in why) and any("foreign load 1.000 > 0.5 " in w for w in why), why
        # Off-pin, it is not yet on-pin contamination...
        assert not any("pinned CPUs" in w for w in why), why

        # 3. ...but the same process on a pinned CPU (an SMT sibling of the
        # benchmark) is, at a far lower level.
        procs_b3 = base[:2] + [(11, root, "bench", hz, 0, 600), (50, 1, "kvbench", 360_000 * hz // 100 + hz // 4, 0, 1_000, 0, 1)]
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

        # 4b. The motivating defect of the first version (run 36339992457): a
        # build's compilers are reaped by `cargo`, which is still running.
        # Their CPU is then only in cargo's cutime, and it is still the run's.
        cargo_a = base + [(12, root, "cargo", 0, 0, 610), (13, 12, "rustc", 0, 0, 620)]
        cargo_b = base + [(12, root, "cargo", 0, 4 * hz, 610)]
        a = samp(t, 0.0, {0: 0, 1: 0, 2: 0, 3: 0}, cargo_a)
        b = samp(t, 1.0, {0: 2 * hz, 1: 2 * hz, 2: 0, 3: 0}, cargo_b)
        r = assess(a, b, pin, root)
        assert abs(r["on_pin_foreign_busy_cpus"]) < 1e-6 and abs(r["own_busy_cpus"] - 4.0) < 1e-6, r

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

        # 9. The motivating defect of #1270: the benchmark used CPU between
        # the process scan and the /proc/stat read. The first version took
        # the own tree's ticks from the scan and so read that CPU as foreign
        # (1.0 here; 0.29-0.57 in the voided runs). The own tree is now read
        # after /proc/stat, and the window reads clean.
        bench = [(1, 0, "systemd", 0, 0, 1), (root, 1, "bash", 0, 0, 500)]
        a = samp(t, 0.0, {0: 0, 1: 0, 2: 0, 3: 0}, bench + [(11, root, "bench", 0, 0, 600)])

        def _bench_ran_on():
            _write_proc(t, {0: 2 * hz, 1: 0, 2: 0, 3: 0}, bench + [(11, root, "bench", 2 * hz, 0, 600)])

        _write_proc(t, {0: hz, 1: 0, 2: 0, 3: 0}, bench + [(11, root, "bench", hz, 0, 600)])
        b = sample(t, root, frozenset(), _after_scan=_bench_ran_on)
        b.mono = 1.0
        assert b.procs[11].ticks == 2 * hz and b.own_read_passes == 1, b  # re-read, not the scan's value
        r = assess(a, b, pin, root)
        assert abs(r["on_pin_foreign_busy_cpus"]) < 1e-6 and abs(r["own_busy_cpus"] - 2.0) < 1e-6, r
        assert r["own_read_lag_s"] >= 0 and r["own_read_passes"] == 1, r

        # 10. A child reaped between the scan and the re-read is counted once,
        # in its reaper's cutime: the re-read runs until the live set holds.
        a = samp(t, 0.0, {0: 0, 1: 0, 2: 0, 3: 0}, bench + [(11, root, "bench", 0, 0, 600)])

        def _reaped():
            _write_proc(t, {0: hz, 1: 0, 2: 0, 3: 0}, [(1, 0, "systemd", 0, 0, 1), (root, 1, "bash", 0, hz, 500)])

        _write_proc(t, {0: hz // 2, 1: 0, 2: 0, 3: 0}, bench + [(11, root, "bench", hz // 2, 0, 600)])
        b = sample(t, root, frozenset(), _after_scan=_reaped)
        b.mono = 1.0
        assert 11 not in b.procs and b.procs[root].child_ticks == hz and b.own_read_passes == 2, b
        r = assess(a, b, pin, root)
        assert abs(r["on_pin_foreign_busy_cpus"]) < 1e-6 and abs(r["own_busy_cpus"] - 1.0) < 1e-6, r

        # 11. Attribution: every class is summed and judged, and what the
        # pinned CPUs' counters hold beyond the own tree, the tasks and the
        # interrupts is `unattributed`: recorded, not judged.
        run_cg = "/system.slice/expanse-bench-runner.service"
        kw, us = hz // 10, hz // 20
        before = [(1, 0, "systemd", 0, 0, 1), (2, 0, "kthreadd", 0, 0, 1, PF_KTHREAD),
                  (root, 7, "bash", 0, 0, 500, 0, 0, run_cg), (7, 1, "Runner.Worker", 0, 0, 400, 0, 3, run_cg),
                  (11, root, "bench", 0, 0, 600, 0, 0, run_cg),
                  (40, 2, "kworker/1:2-events", 0, 0, 50, PF_KTHREAD, 1),
                  (41, 2, "ksoftirqd/3", 0, 0, 50, PF_KTHREAD, 3),
                  (60, 1, "sshd", 0, 0, 900, 0, 0)]
        after = [(1, 0, "systemd", 0, 0, 1), (2, 0, "kthreadd", 0, 0, 1, PF_KTHREAD),
                 (root, 7, "bash", 0, 0, 500, 0, 0, run_cg), (7, 1, "Runner.Worker", hz // 5, 0, 400, 0, 3, run_cg),
                 (11, root, "bench", hz, 0, 600, 0, 0, run_cg),
                 (40, 2, "kworker/1:2-events", kw, 0, 50, PF_KTHREAD, 1),
                 (41, 2, "ksoftirqd/3", kw, 0, 50, PF_KTHREAD, 3),
                 (60, 1, "sshd", us, 0, 900, 0, 0)]
        # CPU 0: the bench. CPU 1: the kworker, the sshd, an irq tick and a
        # softirq tick, and 5 ticks nothing accounts for. CPU 3: the runner
        # and ksoftirqd, off the pin.
        extra = 5
        _write_proc(t, {0: 0, 1: 0, 2: 0, 3: 0}, before)
        a = sample(t, root)
        a.mono = 0.0
        _write_proc(t, {0: hz, 1: kw + us + 2 + extra, 2: 0, 3: hz // 5 + kw}, after, irq={1: (1, 1)})
        b = sample(t, root)
        b.mono = 1.0
        r = assess(a, b, pin, root, frozenset(), run_cg, lambda pid: cgroup_path(t, pid))
        at = r["attribution"]
        def ce(ticks):
            return round(ticks / hz, 3)
        assert at["classes"] == {"kernel": ce(2 * kw), "runner": ce(hz // 5), "user": ce(us), "sampler": 0.0}, at
        assert at["classes_on_pin"] == {"kernel": ce(kw), "runner": 0.0, "user": ce(us), "sampler": 0.0}, at
        assert at["kernel_kinds"] == {"ksoftirqd": ce(kw), "kworker": ce(kw)}, at
        assert at["interrupts_on_pin"] == {"irq": ce(1), "softirq": ce(1), "steal": 0.0}, at
        assert at["unattributed_on_pin"] == ce(extra), at
        assert abs(r["on_pin_foreign_busy_cpus"] - ce(kw + us + 2)) < 1e-6, r
        assert abs(r["on_pin_busy_minus_own_cpus"] - ce(kw + us + 2 + extra)) < 1e-6, r
        # Host-wide: both kernel threads, the runner, the sshd and the two
        # interrupt ticks.
        assert abs(r["foreign_busy_cpus"] - ce(2 * kw + hz // 5 + us + 2)) < 1e-6, r
        text = "\n".join(attribution_lines(r))
        assert f"unattributed {ce(extra):.2f}" in text and "kernel threads: ksoftirqd" in text, text
        assert "recorded, not judged" in text, text

        # 12. THE MOTIVATING DEFECT (runs 37480277219 and 37481727076): a
        # 16-thread cell on a 16-CPU pin, 12.7 CPUs busy, its threads waking
        # each other. The CPU counters read 0.41 core-equivalents more than
        # the own tree's CPU time, with no foreign task and no interrupt time.
        # The two are different clocks, and the first version voided the run
        # on their difference. Nothing foreign ran: the window is quiet.
        pin16 = list(range(16))
        own16 = (127 * hz) // 10
        proc_a = [(1, 0, "systemd", 0, 0, 1, 0, 20), (root, 1, "bash", 0, 0, 500, 0, 20), (11, root, "bench", 0, 0, 600, 0, 0)]
        proc_b = proc_a[:2] + [(11, root, "bench", own16, 0, 600, 0, 0)]
        cpus_a = dict.fromkeys(range(24), 0)
        skew = (41 * hz) // 100
        cpus_b = {c: (own16 + skew) // 16 for c in range(16)} | dict.fromkeys(range(16, 24), 0)
        cpus_b[0] += (own16 + skew) - sum(cpus_b[c] for c in range(16))
        _write_proc(t, cpus_a, proc_a)
        a = sample(t, root)
        a.mono = 0.0
        _write_proc(t, cpus_b, proc_b)
        b = sample(t, root)
        b.mono = 1.0
        r = assess(a, b, pin16, root)
        assert abs(r["own_busy_cpus"] - 12.7) < 0.01 and abs(r["on_pin_busy_minus_own_cpus"] - 0.41) < 0.01, r
        assert r["on_pin_busy_minus_own_cpus"] > bp.RUN_ON_PIN_VOID, r  # what the first version voided on
        assert r["on_pin_foreign_busy_cpus"] == 0.0 and r["foreign_busy_cpus"] == 0.0, r
        assert verdict(r, bp.RUN_FOREIGN_VOID, bp.RUN_ON_PIN_VOID, bp.RUN_PROCESS_VOID_PCT) == [], r
        assert abs(r["attribution"]["unattributed_on_pin"] - 0.41) < 0.01, r

        # 12a. The other direction, which the first version passed: the
        # counters under-read the own tree by 0.2 while a foreign task takes
        # 0.3 of a pinned CPU. The difference reads 0.1, under the boundary;
        # the task is there all the same, and it voids.
        intr = (3 * hz) // 10
        under = (2 * hz) // 10
        proc_a2 = proc_a + [(70, 1, "intruder", 0, 0, 900, 0, 5)]
        proc_b2 = proc_b + [(70, 1, "intruder", intr, 0, 900, 0, 5)]
        cpus_b2 = {c: (own16 - under + intr) // 16 for c in range(16)} | dict.fromkeys(range(16, 24), 0)
        cpus_b2[0] += (own16 - under + intr) - sum(cpus_b2[c] for c in range(16))
        _write_proc(t, cpus_a, proc_a2)
        a = sample(t, root)
        a.mono = 0.0
        _write_proc(t, cpus_b2, proc_b2)
        b = sample(t, root)
        b.mono = 1.0
        r = assess(a, b, pin16, root)
        assert abs(r["on_pin_busy_minus_own_cpus"] - 0.1) < 0.01 and r["on_pin_busy_minus_own_cpus"] < bp.RUN_ON_PIN_VOID, r
        assert abs(r["on_pin_foreign_busy_cpus"] - 0.3) < 0.01, r
        why = verdict(r, bp.RUN_FOREIGN_VOID, bp.RUN_ON_PIN_VOID, bp.RUN_PROCESS_VOID_PCT)
        assert any(f"pinned CPUs 0.300 > {bp.RUN_ON_PIN_VOID:g}" in w for w in why), why
        assert r["offenders"] and r["offenders"][0]["comm"] == "intruder", r

        # 12c. A task that left the pinned CPUs inside the window is still
        # counted there: it is placed by where it ran at either end.
        proc_b3 = proc_b + [(70, 1, "intruder", intr, 0, 900, 0, 20)]
        _write_proc(t, cpus_a, proc_a2)
        a = sample(t, root)
        a.mono = 0.0
        _write_proc(t, cpus_b2, proc_b3)
        b = sample(t, root)
        b.mono = 1.0
        r = assess(a, b, pin16, root)
        assert abs(r["on_pin_foreign_busy_cpus"] - 0.3) < 0.01, r

        # 12b. A crossing prints the value it was judged on. Run 36466620921
        # voided on 0.252 and printed "0.25 > 0.25": the reading is stored at
        # three decimals, so it is printed at three.
        edge = {"foreign_busy_cpus": 0.0, "on_pin_foreign_busy_cpus": 0.252, "offenders": []}
        assert verdict(edge, 1.0, 0.25, 100.0) == ["foreign load on the pinned CPUs 0.252 > 0.25"], verdict(edge, 1.0, 0.25, 100.0)
        assert verdict(dict(edge, on_pin_foreign_busy_cpus=0.25), 1.0, 0.25, 100.0) == []

        # 13. summarize prints the breakdown under a void: for a record of
        # this rule, for one judged on the counters-less-own-tree figure
        # (it says which rule judged it), and for one from before the
        # attribution existed.
        rec = dict(r, t=0, governor=["performance"])
        counters_rule = {k: v for k, v in rec.items() if k not in ("busy_minus_own_cpus", "on_pin_busy_minus_own_cpus")}
        old_rec = {k: v for k, v in counters_rule.items() if k not in ("attribution", "own_read_lag_s", "own_read_passes")}
        for record, needle in ((rec, "tasks 0.30 (kernel 0.00, runner 0.00, user 0.30"),
                               (rec, "recorded, not judged"),
                               (counters_rule, "this record predates the task-sum rule"),
                               (old_rec, "no task-class breakdown")):
            jl.write_text(json.dumps(record) + "\n")
            buf = Path(td, "sum.md")
            with open(os.devnull, "w") as dn:
                old, sys.stdout = sys.stdout, dn
                try:
                    assert cmd_summarize(argparse.Namespace(inp=str(jl), md=str(buf))) == 1
                finally:
                    sys.stdout = old
            assert needle in buf.read_text(), buf.read_text()

        # 14. A long-lived foreign process that exits inside the window brings
        # its whole lifetime into its reaper's cutime. Only the part after the
        # first sample belongs to the window: an idle daemon with a large CPU
        # total behind it that exits is not that much load.
        def window(procs_a, procs_b, cpus_end, pin_cpus=None, self_pids=frozenset()):
            _write_proc(t, cpus_a, procs_a)
            x = sample(t, root)
            x.mono = 0.0
            _write_proc(t, cpus_end, procs_b)
            y = sample(t, root)
            y.mono = 1.0
            return assess(x, y, pin_cpus or pin16, root, self_pids)

        life = 36_000 * hz
        quiet_end = {c: own16 // 16 for c in range(16)} | dict.fromkeys(range(16, 24), 0)
        quiet_end[0] += own16 - sum(quiet_end[c] for c in range(16))
        r = window(proc_a + [(80, 1, "daemon", life, 0, 300, 0, 20)],
                   [(1, 0, "systemd", 0, life, 1, 0, 20)] + proc_b[1:], quiet_end)
        assert r["foreign_busy_cpus"] == 0.0 and r["on_pin_foreign_busy_cpus"] == 0.0, r
        assert verdict(r, bp.RUN_FOREIGN_VOID, bp.RUN_ON_PIN_VOID, bp.RUN_PROCESS_VOID_PCT) == [], r

        # 15. THE CASE THE GUARD EXISTS FOR (`cc1plus`, `rustc`): a foreign
        # process burns a pinned CPU and exits inside the window, and its
        # reaper sits off the pinned CPUs. Nothing records where it ran, so
        # its CPU counts on the pinned CPUs. Here it used 0.6 of a core, on
        # top of 5 s of earlier life that is not this window's.
        earlier, burst = 5 * hz, (6 * hz) // 10
        r = window(proc_a + [(81, 1, "cc1plus", earlier, 0, 900, 0, 5)],
                   [(1, 0, "systemd", 0, earlier + burst, 1, 0, 20)] + proc_b[1:], quiet_end)
        assert abs(r["foreign_busy_cpus"] - 0.6) < 0.01 and abs(r["on_pin_foreign_busy_cpus"] - 0.6) < 0.01, r
        assert r["attribution"]["reaped_children_on_pin"] == r["on_pin_foreign_busy_cpus"], r
        why = verdict(r, bp.RUN_FOREIGN_VOID, bp.RUN_ON_PIN_VOID, bp.RUN_PROCESS_VOID_PCT)
        assert any("pinned CPUs 0.600" in w for w in why), why
        assert "reaped children 0.60" in "\n".join(attribution_lines(r))

        # 15a. The same through a build driver that is still running: `make`,
        # parked off the pinned CPUs, reaps compilers that came and went
        # wholly inside the window.
        r = window(proc_a + [(82, 1, "make", 0, 0, 900, 0, 20)],
                   proc_b + [(82, 1, "make", 0, (9 * hz) // 10, 900, 0, 20)], quiet_end)
        assert abs(r["on_pin_foreign_busy_cpus"] - 0.9) < 0.01, r
        assert any("pinned CPUs" in w for w in verdict(r, bp.RUN_FOREIGN_VOID, bp.RUN_ON_PIN_VOID, bp.RUN_PROCESS_VOID_PCT)), r

        # 15b. A PID reused inside the window is an exit and a birth: the old
        # process's lifetime is not the new one's load.
        r = window(proc_a + [(83, 1, "old", life, 0, 300, 0, 5)],
                   [(1, 0, "systemd", 0, life, 1, 0, 20)] + proc_b[1:] + [(83, 1, "new", hz // 10, 0, 99_000, 0, 20)],
                   quiet_end)
        assert abs(r["foreign_busy_cpus"] - 0.1) < 0.01 and r["on_pin_foreign_busy_cpus"] == 0.0, r

        # 16. A multi-threaded intruder whose main thread sleeps off the
        # pinned CPUs while a worker burns one. Field 39 of the process line
        # is the main thread's CPU; the CPU time is the whole group's.
        def with_threads(pid, cpus_of_threads):
            for i, cpu in enumerate(cpus_of_threads):
                d = t / str(pid) / "task" / str(pid + i)
                d.mkdir(parents=True)
                (d / "stat").write_text(_stat_line(pid + i, 1, "worker", 0, 0, 900, 0, cpu))
            line = (t / str(pid) / "stat").read_text().split()
            line[19] = str(len(cpus_of_threads))  # num_threads
            (t / str(pid) / "stat").write_text(" ".join(line) + "\n")

        mt_a = proc_a + [(90, 1, "intruder", 0, 0, 900, 0, 20)]
        mt_b = proc_b + [(90, 1, "intruder", (4 * hz) // 10, 0, 900, 0, 20)]
        _write_proc(t, cpus_a, mt_a)
        with_threads(90, [20, 21])
        a = sample(t, root)
        a.mono = 0.0
        _write_proc(t, quiet_end, mt_b)
        with_threads(90, [20, 7])
        b = sample(t, root)
        b.mono = 1.0
        assert b.procs[90].thread_cpus == (7, 20) and b.procs[90].processor == 20, b.procs[90]
        r = assess(a, b, pin16, root)
        assert abs(r["on_pin_foreign_busy_cpus"] - 0.4) < 0.01, r

        # 17. The guard's own CPU is not host-wide foreign load.
        me = 95
        r = window(proc_a + [(me, 1, "python3", 0, 0, 900, 0, 20)],
                   proc_b + [(me, 1, "python3", hz // 2, 0, 900, 0, 20)], quiet_end, self_pids=frozenset({me}))
        assert r["foreign_busy_cpus"] == 0.0 and abs(r["self_busy_cpus"] - 0.5) < 0.01, r
        assert r["attribution"]["classes"]["sampler"] == 0.5, r

        # 18. A scan that cannot see every process is refused, not read as a
        # quiet host: an entry that exists and cannot be read, and a `/proc`
        # with no PID 1 (a PID namespace, `hidepid`).
        _write_proc(t, cpus_a, proc_a + [(96, 1, "hidden", 0, 0, 900)])
        (t / "96" / "stat").chmod(0o000)
        try:
            if os.geteuid() != 0:  # root reads through the mode bits
                try:
                    sample(t, root)
                except IncompleteScan as exc:
                    assert "cannot read" in str(exc), exc
                else:
                    raise AssertionError("an unreadable process entry was skipped")
        finally:
            (t / "96" / "stat").chmod(0o644)
        _write_proc(t, cpus_a, proc_a[1:])
        try:
            sample(t, root)
        except IncompleteScan as exc:
            assert "no PID 1" in str(exc), exc
        else:
            raise AssertionError("a process table without PID 1 was accepted")
        # A process that exits between the listing and the read is skipped.
        _write_proc(t, cpus_a, proc_a + [(97, 1, "gone", 0, 0, 900)])
        (t / "97" / "stat").unlink()
        assert 97 not in sample(t, root).procs

        # 19. summarize refuses a record with a hole in it: one long window,
        # and two clean samples far apart.
        calm = {"foreign_busy_cpus": 0.0, "on_pin_foreign_busy_cpus": 0.0, "offenders": [], "governor": ["performance"]}
        for records, expect in (
            ([dict(calm, t=100.0, window_s=2.0), dict(calm, t=102.0, window_s=2.0)], 0),
            ([dict(calm, t=100.0, window_s=600.0)], 1),
            ([dict(calm, t=100.0, window_s=2.0), dict(calm, t=3100.0, window_s=2.0)], 1),
        ):
            jl.write_text("".join(json.dumps(x) + "\n" for x in records))
            with open(os.devnull, "w") as dn:
                old, sys.stdout = sys.stdout, dn
                try:
                    got = cmd_summarize(argparse.Namespace(inp=str(jl), md=None))
                finally:
                    sys.stdout = old
            assert got == expect, (records, got)

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
