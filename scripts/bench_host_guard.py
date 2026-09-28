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
  The own tree is read immediately after `/proc/stat`, not with the process
  scan before it, so the two operands of that subtraction describe the same
  instant (`sample`, #1270).
- **Attribution** (#1270): the foreign load split into task classes —
  kernel threads (by kind: kworker, ksoftirqd, rcu, migration, other), the
  runner's own processes (its cgroup, outside the run's tree), the guard
  itself, and user processes — each with its sum, host-wide and on the pinned
  CPUs (by the CPU a task last ran on); interrupt, softirq and steal time from
  the per-CPU counters; and what none of those accounts for on the pinned
  CPUs. The void rule does not read the attribution: every class counts as
  foreign (`docs/BENCHMARKING.md` rule 8).
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
    processor: int = -1  # the CPU it last ran on


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
    # cutime=16 cstime=17 starttime=22 processor=39, i.e. rest indices 1, 6,
    # 11, 12, 13, 14, 19 and 36.
    return Proc(
        pid=int(pid_s), ppid=int(f[1]), comm=comm,
        ticks=int(f[11]) + int(f[12]), child_ticks=int(f[13]) + int(f[14]),
        start_ticks=int(f[19]), flags=int(f[6]),
        processor=int(f[36]) if len(f) > 36 else -1,
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
        "own_read_lag_s": b.own_read_lag_s,
        "own_read_passes": b.own_read_passes,
        "attribution": attribute(a, b, pin_set, own_all, set(self_pids), on_busy - own, intr, scale,
                                 run_cgroup, cgroup_of),
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
              on_pin_foreign_ticks: float, intr: dict, scale: float, run_cgroup: str | None,
              cgroup_of=None) -> dict:
    """Split the foreign load into task classes and interrupt time.

    A task's CPU over the window is `(utime + stime + cutime + cstime)` at the
    end less at the start, so a short-lived child that a foreign process
    reaped inside the window is counted in its reaper's class; a process that
    exited and was reaped is dropped from both ends and so appears only there.
    It is put on the pinned CPUs or off them by the CPU it last ran on, which
    for a process that moved in the window is an approximation, said so in
    the record.

    Interrupt, softirq and steal time come from the per-CPU counters, which
    the kernel charges to no task under IRQ time accounting.

    Whatever the tasks and interrupts on the pinned CPUs do not account for is
    `unattributed_on_pin`. It is not dropped: the void rule reads the pinned
    CPUs' busy time less the own tree's, and the attribution only explains it.
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
    for pid in set(a.procs) | set(b.procs):
        if pid in own_all and pid not in self_pids:
            continue
        pa, pb = a.procs.get(pid), b.procs.get(pid)
        if pa and pb and pa.start_ticks != pb.start_ticks:
            pa = None  # a reused PID: a new process
        if pb is None:
            continue  # exited: its CPU is in its reaper's cutime
        d = total(pb) - (total(pa) if pa else 0)
        if d == 0:
            continue
        cls = task_class(pb, self_pids, run_cgroup, cg)
        host[cls] += d
        if pb.processor in pin_set:
            on_pin[cls] += d
        if cls == "kernel":
            k = kernel_kind(pb.comm)
            kinds[k] = kinds.get(k, 0) + d

    def ce(ticks: float) -> float:
        return round(ticks / scale, 3)

    intr_on = sum(intr["on"].values())
    tasks_on = sum(on_pin.values())
    return {
        "classes": {k: ce(v) for k, v in host.items()},
        "classes_on_pin": {k: ce(v) for k, v in on_pin.items()},
        "kernel_kinds": {k: ce(v) for k, v in sorted(kinds.items())},
        "interrupts_on_pin": {k: ce(v) for k, v in intr["on"].items()},
        "interrupts_off_pin": {k: ce(v) for k, v in intr["off"].items()},
        "unattributed_on_pin": ce(on_pin_foreign_ticks - tasks_on - intr_on),
        "placement": "a task's CPU is placed on or off the pinned CPUs by the CPU it last ran on",
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
    intr_sum = sum(intr_on.values())
    lines = [
        f"  on the pinned CPUs {a['on_pin_foreign_busy_cpus']:.2f} = tasks {tasks_on:.2f} "
        f"({', '.join(f'{k} {v:.2f}' for k, v in on.items())}) + interrupts {intr_sum:.2f} "
        f"({', '.join(f'{k} {v:.2f}' for k, v in intr_on.items())}) + unattributed {at['unattributed_on_pin']:.2f}",
        f"  host-wide foreign tasks: {', '.join(f'{k} {v:.2f}' for k, v in at['classes'].items())}"
        + (f"; kernel threads: {', '.join(f'{k} {v:.2f}' for k, v in at['kernel_kinds'].items())}" if at["kernel_kinds"] else ""),
    ]
    lag = a.get("own_read_lag_s")
    if lag is not None:
        bound = a["own_busy_cpus"] * lag / a["window_s"] if a.get("window_s") else 0.0
        lines.append(f"  own tree read {lag * 1e3:.2f} ms after the CPU counters "
                     f"({a.get('own_read_passes', 0)} pass(es)); at its load that moves the reading by at most {bound:.3f}")
    if at["unattributed_on_pin"] > tasks_on + intr_sum:
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

        # 11. Attribution: every class is summed, and whatever tasks and
        # interrupts do not explain on the pinned CPUs is `unattributed`.
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
        ce = lambda ticks: round(ticks / hz, 3)  # noqa: E731
        assert at["classes"] == {"kernel": ce(2 * kw), "runner": ce(hz // 5), "user": ce(us), "sampler": 0.0}, at
        assert at["classes_on_pin"] == {"kernel": ce(kw), "runner": 0.0, "user": ce(us), "sampler": 0.0}, at
        assert at["kernel_kinds"] == {"ksoftirqd": ce(kw), "kworker": ce(kw)}, at
        assert at["interrupts_on_pin"] == {"irq": ce(1), "softirq": ce(1), "steal": 0.0}, at
        assert at["unattributed_on_pin"] == ce(extra), at
        assert abs(r["on_pin_foreign_busy_cpus"] - ce(kw + us + 2 + extra)) < 1e-6, r
        text = "\n".join(attribution_lines(r))
        assert f"unattributed {ce(extra):.2f}" in text and "kernel threads: ksoftirqd" in text, text
        assert "no task or interrupt time accounts for most of it" not in text, text

        # 12. Fail closed: load on the pinned CPUs that no task explains still
        # voids, and the report says so in task-class terms.
        _write_proc(t, {0: 0, 1: 0, 2: 0, 3: 0}, before)
        a = sample(t, root)
        a.mono = 0.0
        _write_proc(t, {0: hz, 1: hz // 2, 2: 0, 3: 0},
                    before[:4] + [(11, root, "bench", hz, 0, 600, 0, 0, run_cg)] + before[5:])
        b = sample(t, root)
        b.mono = 1.0
        r = assess(a, b, pin, root, frozenset(), run_cg, lambda pid: cgroup_path(t, pid))
        assert r["attribution"]["unattributed_on_pin"] == 0.5, r
        assert any("pinned CPUs 0.50" in w for w in verdict(r, bp.RUN_FOREIGN_VOID, bp.RUN_ON_PIN_VOID, bp.RUN_PROCESS_VOID_PCT)), r
        assert "no task or interrupt time accounts for most of it" in "\n".join(attribution_lines(r))

        # 13. summarize prints the breakdown under a void, for a record with
        # the attribution and for one from before it.
        rec = dict(r, t=0, governor=["performance"])
        old_rec = {k: v for k, v in rec.items() if k not in ("attribution", "own_read_lag_s", "own_read_passes")}
        for record, needle in ((rec, "unattributed 0.50"), (old_rec, "no task-class breakdown")):
            jl.write_text(json.dumps(record) + "\n")
            buf = Path(td, "sum.md")
            with open(os.devnull, "w") as dn:
                old, sys.stdout = sys.stdout, dn
                try:
                    assert cmd_summarize(argparse.Namespace(inp=str(jl), md=str(buf))) == 1
                finally:
                    sys.stdout = old
            assert needle in buf.read_text(), buf.read_text()

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
