#!/usr/bin/env python3
"""Hardware performance counters for the random point-lookup path (#455 R0).

This is a **diagnostic instrument, not a gate.** Nothing here fails a build on
a counter value. The `Ir` regression gate is unchanged and stays the gate for
insert / churn / strmap work; this exists because `Ir` is structurally blind to
the two mechanisms #455 documents on the random lookup path — they change cache
line fills and address-dependency chains and retire no extra instructions.

    crates/expanse/examples/perf_point_lookup.rs   the workload
      -> perf stat -x, over `build` and `probe` phases
      -> probe - build = counts attributed to the probe loop
      -> BCa 95% CI over repeated runs (scripts/bca_bootstrap.py)
      -> perf-counters-<suite>.json + a markdown table

What it can answer: whether a change moves cache line fills, translation
misses, stall cycles or branch mispredictions on this path, with an interval.

What it cannot answer: anything about a workload it did not run, and anything
on a microarchitecture whose counters it reports as unavailable. It does not
bracket a region inside one process — it differences two processes that do
identical work up to the probe loop, so process setup does not cancel exactly.
The interval is what makes that legible; a single run is not a measurement.

Hybrid CPUs: a host whose kernel exposes more than one core PMU — an Intel
P-core `cpu_core` alongside an E-core `cpu_atom`, or an ARM big.LITTLE pair —
answers one requested event with one row per PMU, named `cpu_core/instructions/`
and `cpu_atom/instructions/`, and with no row named `instructions`. Those two
rows count two different microarchitectures over two different sets of cores.
This driver never sums them: it selects one PMU, confines the workload to that
PMU's CPUs, reads only that PMU's rows, and names the PMU in the JSON and in
every rendered table. See `resolve_pmu` for why selecting beats summing.

Fail-loud (AGENTS.md section 8.1): a missing `perf`, a kernel that refuses to
open a counter, a workload that escapes its pin, or a missing workload binary
exits non-zero with the cause and the fix named. It never degrades into a
report that reads as complete.

Usage:
  python3 scripts/perf_counters.py --out perf-counters.json
  python3 scripts/perf_counters.py --pops 262144,1048576,4194304 --runs 10
  python3 scripts/perf_counters.py --preflight-only
  python3 scripts/perf_counters.py --pmu cpu_core
  python3 scripts/perf_counters.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bca_bootstrap import bca_bootstrap_ci  # noqa: E402

# The counters #455 R0 names. The first group is portable; the second is
# Intel-specific and simply does not exist on other microarchitectures, which
# is why availability is probed per event and the unavailable ones are printed
# rather than left as gaps in a table. On a hybrid Intel part the second group
# is further restricted to the P-core PMU, so availability is a property of the
# (event, PMU) pair and not of the host alone.
PORTABLE_EVENTS = [
    "cycles",
    "instructions",
    "L1-dcache-load-misses",
    "LLC-load-misses",
    "dTLB-load-misses",
    "branch-misses",
]
VENDOR_EVENTS = [
    "cycle_activity.stalls_l3_miss",
    "mem_load_retired.l3_miss",
    "br_misp_retired.all_branches",
]
DEFAULT_EVENTS = PORTABLE_EVENTS + VENDOR_EVENTS

WORKLOAD_REL = "target/release/examples/perf_point_lookup"
BUILD_CMD = "cargo build --release -p expanse-trie --example perf_point_lookup"

# BCa needs a jackknife, so n >= 3 (bca_bootstrap.py raises below that).
MIN_RUNS = 3
CONFIDENCE = 0.95
NUM_RESAMPLES = 2000
SEED = 42

# perf's own placeholders in the value column of `-x,` output.
NOT_COUNTED = {"<not counted>", "<not supported>", "<unsupported>"}

# Core PMUs in the order this driver picks between them when a host exposes
# more than one. These are kernel PMU names, not a finding about any run: Linux
# names the performance-core PMU of an Alder-Lake-class hybrid part `cpu_core`
# and its efficiency-core PMU `cpu_atom`, and a uniform x86 host's single core
# PMU `cpu`. A host whose PMU names are not in this list gets no arbitrary
# pick — it fails and asks for an explicit `--pmu`.
PREFERRED_CORE_PMUS = ("cpu_core", "cpu")

# Where the kernel publishes each PMU's CPU list (`/sys/devices/cpu_core/cpus`).
SYS_PMU_ROOT = Path("/sys/devices")


# --------------------------------------------------------------------------
# perf stat CSV
# --------------------------------------------------------------------------
def split_event_name(name: str) -> tuple[str | None, str]:
    """`cpu_core/instructions/` -> `("cpu_core", "instructions")`.

    perf qualifies an event with the PMU that served it whenever the host has
    more than one that could, so on a hybrid CPU the single requested event
    `instructions` comes back as `cpu_core/instructions/` plus
    `cpu_atom/instructions/` and never as a bare `instructions`. An
    unqualified name keeps a `None` PMU rather than inventing one, so a
    uniform host stays exactly as it was.
    """
    if "/" not in name:
        return None, name
    pmu, _, rest = name.partition("/")
    base = rest[:-1] if rest.endswith("/") else rest
    if not pmu or not base:
        return None, name
    return pmu, base


def parse_perf_csv(text: str) -> dict[str, dict]:
    """`{event -> {value, pct_running, status, event, pmu, base_event}}`.

    perf's CSV is `value,unit,event,runtime_ns,pct_running[,metric,...]`.
    Keyed on the event field exactly as perf wrote it, so the two rows a
    hybrid host emits for one requested event stay distinct instead of one
    overwriting the other; `pmu` and `base_event` carry the decomposition.
    A counter the kernel could not open carries a placeholder in the value
    column; it is reported as such, never as zero — a zero would read as
    "measured, and there were none".
    """
    out: dict[str, dict] = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(",")
        if len(parts) < 3:
            continue
        value, _unit, event = parts[0].strip(), parts[1].strip(), parts[2].strip()
        if not event:
            continue
        pct = None
        if len(parts) >= 5 and parts[4].strip():
            try:
                pct = float(parts[4].strip())
            except ValueError:
                pct = None
        pmu, base = split_event_name(event)
        common = {"event": event, "pmu": pmu, "base_event": base, "pct_running": pct}
        if value in NOT_COUNTED:
            out[event] = dict(common, value=None, status=value)
            continue
        try:
            out[event] = dict(common, value=float(value), status="counted")
        except ValueError:
            out[event] = dict(common, value=None, status=f"unparsed:{value}")
    return out


def select_rows(parsed: dict[str, dict], requested: str) -> dict[str | None, dict]:
    """Every parsed row that answers `requested`, keyed by the PMU that served it.

    A request `X` matches a row whose event field is `X`, `<pmu>/X/` or
    `<pmu>/X`. A request that already names a PMU matches only that PMU.
    """
    req_pmu, req_base = split_event_name(requested)
    out: dict[str | None, dict] = {}
    for name, entry in parsed.items():
        pmu, base = split_event_name(name)
        if base != req_base:
            continue
        if req_pmu is not None and pmu != req_pmu:
            continue
        out[pmu] = entry
    return out


def row_for(parsed: dict[str, dict], requested: str, pmu: str | None) -> dict | None:
    """The row for `requested` on `pmu` (`None` = this host's only core PMU)."""
    rows = select_rows(parsed, requested)
    if pmu in rows:
        return rows[pmu]
    if len(rows) == 1 and None in rows:
        # perf left the row unqualified. That is what a uniform host always
        # does, and what a hybrid host may do for an event only one of its
        # PMUs can serve. With the workload pinned to `pmu`, the count is
        # that PMU's either way.
        return rows[None]
    return None


def run_perf(
    events: list[str],
    cmd: list[str],
    env: dict[str, str],
    pin: list[str] | None = None,
) -> tuple[int, str, str, str]:
    """One `perf stat` invocation. Returns (rc, csv_text, stdout, stderr).

    `pin` prefixes the whole invocation (`taskset -c <cpus>`) rather than
    wrapping the workload, so the affinity is inherited by the process perf
    counts and `taskset`'s own instructions are not counted into it.
    """
    with tempfile.NamedTemporaryFile("r+", suffix=".csv", delete=False) as fh:
        stat_path = fh.name
    try:
        argv = (
            list(pin or [])
            + ["perf", "stat", "-x,", "-o", stat_path, "-e", ",".join(events), "--"]
            + cmd
        )
        proc = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )
        csv_text = Path(stat_path).read_text(encoding="utf-8", errors="replace")
    finally:
        os.unlink(stat_path)
    return proc.returncode, csv_text, proc.stdout, proc.stderr


# --------------------------------------------------------------------------
# preflight
# --------------------------------------------------------------------------
class Preflight(Exception):
    """A named infrastructure cause, with the fix. Always fatal."""


def paranoid_level() -> str:
    try:
        return Path("/proc/sys/kernel/perf_event_paranoid").read_text().strip()
    except OSError:
        return "unreadable"


def classify_probe(rc: int, probed: dict[str, dict], err: str, paranoid: str) -> str | None:
    """`None` if the capability probe counted; otherwise the fatal message.

    Two failures look alike at a glance and have nothing in common:

    * **The kernel refused the counter.** perf exits non-zero, or emits the
      row with a `<not counted>` placeholder. Capability and
      `perf_event_paranoid` advice applies here and only here.
    * **The event is not in the output under the name we looked for.** perf
      exits 0 and counted correctly; the driver looked up the wrong key. On a
      hybrid CPU the rows are `cpu_core/instructions/` and
      `cpu_atom/instructions/`, and a lookup of `instructions` finds neither.
      No amount of `setcap` or `sysctl` changes that, and printing capability
      advice for this case sends the reader to reconfigure a host that was
      never misconfigured.

    Kept pure so both branches are covered by `--self-test` on a machine with
    no PMU at all.
    """
    rows = select_rows(probed, "instructions")
    tail = f"  perf said: {err.strip()[:400]}\nNo counters were collected and no numbers were produced."

    if rc != 0 or (rows and all(e.get("value") is None for e in rows.values())):
        statuses = (
            ", ".join(f"{e.get('event')}={e.get('status')}" for e in rows.values())
            or "no rows emitted"
        )
        return (
            "`perf stat -e instructions` could not open a hardware counter for a process "
            f"it started (exit {rc}, kernel.perf_event_paranoid={paranoid}, rows: {statuses}).\n"
            "  This driver only ever counts a process it starts itself. That is the\n"
            "  per-process path, which a paranoid level of 1 permits; the level that\n"
            "  matters for system-wide (`-a`, or `perf stat` with no command) counting is\n"
            "  a different gate and is not what this needs. If the per-process open is\n"
            "  refused anyway:\n"
            "    1. grant the runner CAP_PERFMON: "
            "`sudo setcap cap_perfmon,cap_sys_ptrace,cap_syslog=ep $(command -v perf)`\n"
            "    2. or lower the gate for all users: "
            "`sudo sysctl -w kernel.perf_event_paranoid=1` "
            "(persist it in /etc/sysctl.d/)\n"
            "  A virtualised host may expose no PMU at all, in which case neither fix "
            "applies and the counters have to be collected on bare metal.\n" + tail
        )

    if not rows:
        seen = ", ".join(sorted(probed)) or "(none)"
        return (
            "`perf stat -e instructions` exited 0 and emitted no row this driver could "
            f"match to `instructions` (kernel.perf_event_paranoid={paranoid}).\n"
            "  This is an event-naming mismatch, not a permissions problem. perf ran, the\n"
            "  counter opened, and the lookup missed. Changing `perf_event_paranoid` or\n"
            "  granting CAP_PERFMON will not fix it and is not the remedy here.\n"
            f"  Event names perf actually emitted: {seen}\n"
            "  Matched forms are `instructions`, `<pmu>/instructions/` and\n"
            "  `<pmu>/instructions`. If the emitted names are some other form,\n"
            "  `split_event_name` in this script has to learn it.\n" + tail
        )

    return None


def resolve_pmu(observed: list[str | None], override: str) -> tuple[str | None, str]:
    """Pick the one core PMU whose rows this run reads, and say why.

    Why one PMU rather than a sum, on a host that reports several:

    * Summing `cpu_core` + `cpu_atom` answers a question nobody asked. The two
      PMUs count different microarchitectures — different cache hierarchies,
      different branch predictors, different pipeline widths. A sum of a
      P-core cache-miss count and an E-core cache-miss count is not a
      cache-miss count for anything that exists.
    * The workload here is a single-threaded point-lookup loop. It runs on one
      core type at a time; whichever type it lands on, the other PMU's
      contribution to a sum is either zero (and the sum is a mislabel) or
      non-zero because the scheduler migrated it mid-run (and the sum blends
      two machines, silently, run to run).
    * Selecting one PMU *and pinning the workload to that PMU's CPUs* makes
      the number comparable between runs, which is the whole point of a
      diagnostic that gets re-run against a change.
    * Reporting both PMUs side by side was the alternative. It doubles every
      table for a workload that only ever occupies one core type, and it still
      needs the pin to be reproducible — so it buys nothing this instrument
      needs. `--pmu cpu_atom` measures the other core type deliberately, as a
      separate run with its own labelled artifact.

    The chosen PMU is recorded in the artifact and rendered in the table
    header, so no number leaves here without the core type it came from
    attached (AGENTS.md section 8).
    """
    names = ", ".join(sorted(str(p) for p in observed))
    if override and override != "auto":
        if override not in observed:
            raise Preflight(
                f"--pmu {override} was requested but perf served this event from: {names}. "
                "Pass one of those, or `auto`. No counters were collected."
            )
        return override, f"requested explicitly with --pmu {override}"

    real = [p for p in observed if p is not None]
    if not real:
        return None, "this host reports a single, unqualified core PMU"
    if len(real) == 1:
        return real[0], f"the only core PMU on this host is `{real[0]}`"
    for candidate in PREFERRED_CORE_PMUS:
        if candidate in real:
            return (
                candidate,
                f"`{candidate}` is the performance-core PMU among the {len(real)} "
                f"this host exposes ({names})",
            )
    raise Preflight(
        f"this host exposes {len(real)} core PMUs ({names}) and none of them is one this "
        f"driver knows how to rank ({', '.join(PREFERRED_CORE_PMUS)}). Summing counters "
        "across core types would produce a number that describes no machine, so nothing "
        "was collected. Re-run with `--pmu <name>` naming the one to measure."
    )


def pmu_cpus(pmu: str | None) -> str | None:
    """The CPU list the kernel publishes for `pmu`, e.g. `0-15` for `cpu_core`."""
    if not pmu:
        return None
    try:
        return (SYS_PMU_ROOT / pmu / "cpus").read_text().strip() or None
    except OSError:
        return None


def pin_for(pmu: str) -> list[str]:
    """`taskset` prefix confining the workload to `pmu`'s CPUs.

    Without it the scheduler is free to move a single-threaded workload
    between a P-core and an E-core mid-run. The selected PMU stops counting
    while the task is off its own cores, so the counts would silently
    under-report by however long the task spent elsewhere — a number that
    looks measured and is not.
    """
    cpus = pmu_cpus(pmu)
    if cpus is None:
        raise Preflight(
            f"the kernel publishes no CPU list at {SYS_PMU_ROOT / pmu / 'cpus'}, so the "
            f"workload cannot be confined to the `{pmu}` cores. Unpinned, it can migrate "
            "between core types mid-run and the counts would silently under-report. "
            "No counters were collected."
        )
    if shutil.which("taskset") is None:
        raise Preflight(
            "`taskset` is not on PATH, so the workload cannot be confined to the "
            f"`{pmu}` cores on this hybrid host (install `util-linux`). Unpinned counts "
            "on a hybrid CPU are not comparable run to run. No counters were collected."
        )
    return ["taskset", "-c", cpus]


def pin_violations(parsed: dict[str, dict], events: list[str], pmu: str | None) -> list[str]:
    """Counted, non-zero rows on a core PMU this run is pinned away from.

    With the workload confined to `pmu`'s CPUs, no sibling PMU can retire
    anything on its behalf. A non-zero sibling count therefore means the pin
    did not hold and the run straddled two core types, which makes every
    number in it a blend of two microarchitectures rather than a measurement.
    A `<not counted>` or `<not supported>` sibling row is the expected shape
    and is not a violation.
    """
    bad: list[str] = []
    for event in events:
        for other, entry in select_rows(parsed, event).items():
            if other is None or other == pmu:
                continue
            value = entry.get("value")
            if isinstance(value, (int, float)) and value > 0:
                bad.append(f"{entry.get('event', event)}={value:,.0f}")
    return bad


def _pin_failed(pin: list[str], pmu: str | None, bad: list[str]) -> Preflight:
    return Preflight(
        f"the workload was pinned to the `{pmu}` cores (`{' '.join(pin)}`) but a sibling "
        f"core PMU still counted work for it: {', '.join(bad)}. The pin did not hold, so "
        "these counts straddle two microarchitectures and describe neither. Nothing was "
        "published. Check that the CPU list in /sys matches the running kernel's topology "
        "and that no cgroup or affinity policy is overriding taskset."
    )


def preflight(
    root: Path, events: list[str], pmu_override: str
) -> tuple[str | None, str, list[str], list[str], list[dict[str, str]]]:
    """Probe the real capability, pick a PMU, then classify each requested event.

    Deliberately not a proxy: the question is "can this kernel open a counter
    for us", so the probe opens one. `perf` being on PATH answers a weaker
    question and has answered it wrongly before on hosts where
    `perf_event_paranoid` forbids the open.

    Returns `(pmu, why, pin, available, unavailable)`.
    """
    if platform.system() != "Linux":
        raise Preflight(
            f"perf stat is Linux-only and this host reports {platform.system()}. "
            "Run this on the bare-metal reference host (`/benchmark point_lookup_counters` "
            "from a PR comment, or the `point_lookup_counters` suite in the Bare-Metal "
            "Benchmarks workflow). No counters were collected."
        )
    if shutil.which("perf") is None:
        raise Preflight(
            "`perf` is not on PATH. Install the kernel's matching tools package "
            "(`linux-tools-common` plus `linux-tools-$(uname -r)` on Debian/Ubuntu, "
            "`perf` on Fedora/RHEL) before re-triggering. No counters were collected."
        )

    workload = root / WORKLOAD_REL
    if not workload.is_file():
        raise Preflight(
            f"the workload binary {WORKLOAD_REL} does not exist. Build it with "
            f"`{BUILD_CMD}` before re-triggering. No counters were collected."
        )

    probe_env = _workload_env({"EXPANSE_PERF_POP": "1", "EXPANSE_PERF_PHASE": "build"})

    # The capability probe: one real counter over one real process.
    rc, csv_text, _out, err = run_perf(["instructions"], [str(workload)], probe_env)
    probed = parse_perf_csv(csv_text)
    problem = classify_probe(rc, probed, err, paranoid_level())
    if problem:
        raise Preflight(problem)

    rows = select_rows(probed, "instructions")
    pmu, why = resolve_pmu(list(rows), pmu_override)
    # Pin only where there is something to pin away from: a uniform host has
    # one core PMU and behaves exactly as it did before this existed.
    pin = pin_for(pmu) if pmu and len([p for p in rows if p is not None]) > 1 else []

    # Per-event classification, on the selected PMU and under the same pin the
    # sweep will use. Probed one at a time so a single unsupported event cannot
    # take the whole set down with it, and so the reason is attributable to the
    # event that caused it. An event that exists on one core type and not the
    # other — `cycle_activity.stalls_l3_miss` and `mem_load_retired.l3_miss` are
    # P-core-only on Alder Lake — is judged on the selected PMU alone, so it is
    # available when the selected PMU serves it and is never written off as
    # globally unavailable because a sibling PMU said `<not supported>`.
    available: list[str] = []
    unavailable: list[dict[str, str]] = []
    for event in events:
        rc, csv_text, _out, err = run_perf([event], [str(workload)], probe_env, pin=pin)
        parsed = parse_perf_csv(csv_text)
        served = select_rows(parsed, event)
        entry = row_for(parsed, event, pmu)
        if rc == 0 and entry and entry.get("value") is not None:
            if pin:
                bad = pin_violations(parsed, [event], pmu)
                if bad:
                    raise _pin_failed(pin, pmu, bad)
            available.append(event)
            continue
        if rc != 0:
            reason = "perf refused the event (unknown on this microarchitecture)"
        elif not served:
            reason = "perf emitted no row for this event"
        elif entry is None:
            others = ", ".join(f"`{p}`" for p in sorted(str(k) for k in served))
            reason = (
                f"served by {others}, not by the selected PMU `{pmu}`; this run is pinned "
                f"to `{pmu}`, so it is not collected here"
            )
        else:
            reason = str(entry.get("status") or "no value returned")
            if pmu:
                reason = f"{reason} on `{pmu}`"
            # Name the sibling that did count it, so an event which exists on
            # the other core type reads as "elsewhere", not as "nowhere".
            elsewhere = [
                f"`{p}`"
                for p, e in served.items()
                if p is not None and p != pmu and e.get("value") is not None
            ]
            if elsewhere:
                reason = (
                    f"{reason}; counted on {', '.join(sorted(elsewhere))}, which this run "
                    "is pinned away from"
                )
        unavailable.append(
            {
                "event": event,
                "pmu": str(pmu) if pmu else "(single core PMU)",
                "reason": reason,
                "perf_stderr": err.strip()[:200],
            }
        )
    if not available:
        where = f" on the `{pmu}` PMU" if pmu else ""
        raise Preflight(
            f"no requested counter is available{where} — every one of "
            f"{', '.join(events)} was refused. A table with no counters in it is not "
            "a measurement. No numbers were produced."
        )
    return pmu, why, pin, available, unavailable


def _workload_env(extra: dict[str, str]) -> dict[str, str]:
    env = dict(os.environ)
    env.update(extra)
    return env


# --------------------------------------------------------------------------
# the sweep
# --------------------------------------------------------------------------
def cell_id(arm: str, pop: int, hit_pct: int) -> str:
    return f"{arm}/pop={pop}/hit={hit_pct}"


def run_cell(
    workload: Path,
    events: list[str],
    arm: str,
    pop: int,
    hit_pct: int,
    passes: int,
    runs: int,
    pmu: str | None = None,
    pin: list[str] | None = None,
) -> dict:
    """Both phases, `runs` times each, interleaved.

    Interleaved per methodology rule 1: build and probe alternate rather than
    running as two blocks, so thermal or frequency drift lands on both and
    cancels in the difference instead of being attributed to the probe loop.
    """
    phases: dict[str, list[dict]] = {"build": [], "probe": []}
    for _ in range(runs):
        for phase in ("build", "probe"):
            env = _workload_env(
                {
                    "EXPANSE_PERF_ARM": arm,
                    "EXPANSE_PERF_PHASE": phase,
                    "EXPANSE_PERF_POP": str(pop),
                    "EXPANSE_PERF_HIT_PCT": str(hit_pct),
                    "EXPANSE_PERF_PASSES": str(passes),
                }
            )
            rc, csv_text, out, err = run_perf(events, [str(workload)], env, pin=pin)
            if rc != 0:
                raise Preflight(
                    f"the workload exited {rc} for {cell_id(arm, pop, hit_pct)} phase={phase}. "
                    "A partial sweep is not a result.\n"
                    f"  stdout: {out.strip()[:300]}\n  stderr: {err.strip()[:300]}"
                )
            parsed = parse_perf_csv(csv_text)
            if pin:
                bad = pin_violations(parsed, events, pmu)
                if bad:
                    raise _pin_failed(pin, pmu, bad)
            phases[phase].append({"counters": parsed, "shape": out.strip()})
    return phases


def attributed(phases: dict[str, list[dict]], event: str, pmu: str | None = None) -> list[float]:
    """Per-run `probe - build` for one event on one PMU, over the paired runs."""
    out: list[float] = []
    for build, probe in zip(phases["build"], phases["probe"]):
        b_row = row_for(build["counters"], event, pmu)
        p_row = row_for(probe["counters"], event, pmu)
        b = None if b_row is None else b_row.get("value")
        p = None if p_row is None else p_row.get("value")
        if b is None or p is None:
            continue
        out.append(float(p) - float(b))
    return out


def min_pct_running(
    phases: dict[str, list[dict]], event: str, pmu: str | None = None
) -> float | None:
    """Lowest multiplexing fraction seen for one event across every run.

    Below 100 the kernel time-shared the counter and perf scaled the value up.
    That is a real property of the number and is printed, not hidden.
    """
    seen = []
    for phase in phases.values():
        for r in phase:
            row = row_for(r["counters"], event, pmu)
            if row is not None:
                seen.append(row.get("pct_running"))
    vals = [float(v) for v in seen if isinstance(v, (int, float))]
    return min(vals) if vals else None


def perf_event_name(phases: dict[str, list[dict]], event: str, pmu: str | None) -> str:
    """The event name perf itself used, so a table cell carries its PMU."""
    for phase in phases.values():
        for r in phase:
            row = row_for(r["counters"], event, pmu)
            if row is not None:
                return str(row.get("event") or event)
    return event


def summarise(samples: list[float]) -> dict:
    if len(samples) < MIN_RUNS:
        return {
            "n": len(samples),
            "point": (sum(samples) / len(samples)) if samples else None,
            "ci_lower": None,
            "ci_upper": None,
            "status": f"n<{MIN_RUNS}: no interval",
        }
    point, lo, hi = bca_bootstrap_ci(
        samples, confidence=CONFIDENCE, num_resamples=NUM_RESAMPLES, seed=SEED
    )
    return {"n": len(samples), "point": point, "ci_lower": lo, "ci_upper": hi, "status": "ok"}


# --------------------------------------------------------------------------
# rendering
# --------------------------------------------------------------------------
def render(doc: dict) -> list[str]:
    """The markdown block. Every cell is derived from `doc` (rule 10)."""
    lines = [
        "### Hardware counters — random point lookup "
        "([#455](https://github.com/orieg/expanse/issues/455) R0)",
        "",
        "Diagnostic only: **no counter here gates anything.** Counts are "
        "`probe - build` over paired runs of "
        "`crates/expanse/examples/perf_point_lookup.rs`, with a BCa 95% interval "
        "over the per-run differences.",
        "",
    ]

    pmu = doc.get("pmu") or {}
    selected = pmu.get("selected")
    pin_cmd = pmu.get("pin_command")
    if selected and pin_cmd:
        lines += [
            f"**Counted on the `{selected}` PMU only** — {pmu.get('selection_reason')}. "
            f"The workload is confined to that PMU's CPUs (`{pmu.get('cpus')}`) with "
            f"`{pin_cmd}`. This host exposes more than one core PMU; the other one counts "
            "a different microarchitecture, and its rows are neither added to these "
            "numbers nor mixed into them.",
            "",
        ]
    elif selected:
        lines += [
            f"Counted on the `{selected}` PMU — {pmu.get('selection_reason')}. "
            "No pinning was applied and none is needed.",
            "",
        ]

    unavailable = doc.get("counters_unavailable") or []
    if unavailable:
        lines += [
            f"**{len(unavailable)} requested counter(s) unavailable on this host** "
            "— listed rather than left as a gap in the table:",
            "",
            "| Counter | Why |",
            "|---|---|",
        ]
        lines += [f"| `{u['event']}` | {u['reason']} |" for u in unavailable]
        lines.append("")
    else:
        where = f" on `{selected}`" if selected else " on this host"
        lines += [f"Every requested counter was available{where}.", ""]

    for cell in doc["cells"]:
        lines += [
            f"<details><summary><b>{cell['id']}</b> — "
            f"{cell['distinct_probes']:,} distinct probes, "
            f"{cell['passes']} pass(es), {cell['hit_pct']}% hit</summary>",
            "",
            "| Counter | Attributed to probe (mean) | BCa 95% CI | n | Min % counter running |",
            "|:---|---:|:---|---:|---:|",
        ]
        for event, stat in cell["counters"].items():
            pct = stat.get("min_pct_running")
            pct_s = "—" if pct is None else f"{pct:.0f}%"
            if stat["ci_lower"] is None:
                ci = stat["status"]
            else:
                ci = f"[{stat['ci_lower']:,.0f}, {stat['ci_upper']:,.0f}]"
            point = "—" if stat["point"] is None else f"{stat['point']:,.0f}"
            # perf's own name for the row, so a hybrid host's cells read
            # `cpu_core/instructions/` and cannot be mistaken for a merge.
            label = stat.get("perf_event_name") or event
            lines.append(f"| `{label}` | {point} | {ci} | {stat['n']} | {pct_s} |")
        lines += ["", "</details>", ""]

    lines += [
        "Any counter whose *min % running* is below 100 was time-shared by the "
        "kernel across the event set and scaled up by `perf`; treat it as an "
        "estimate. Per-run values are in the JSON artifact, so every interval "
        "here is recomputable with `scripts/bca_bootstrap.py`.",
        "",
    ]
    return lines


# --------------------------------------------------------------------------
def build_doc(
    args,
    root: Path,
    pmu: str | None,
    why: str,
    pin: list[str],
    available: list[str],
    unavailable: list[dict],
    cells: list[dict],
) -> dict:
    return {
        "schema": "expanse.perfcounters.v1",
        "generated_by": "scripts/perf_counters.py",
        "gates": False,
        "provenance": {
            "host_description": args.host_desc,
            "commit": args.commit or _git_commit(root),
            "run_id": args.run_id,
            "kernel_perf_event_paranoid": paranoid_level(),
        },
        "pmu": {
            "selected": pmu,
            "selection_reason": why,
            "requested": args.pmu,
            "cpus": pmu_cpus(pmu),
            "pin_command": " ".join(pin) if pin else None,
            "merge_policy": (
                "one core PMU per run; rows from any other core PMU are never summed "
                "into these counts"
            ),
        },
        "statistics": {
            "estimator": "mean of per-run (probe - build)",
            "method": "BCa bootstrap",
            "confidence": CONFIDENCE,
            "num_resamples": NUM_RESAMPLES,
            "seed": SEED,
            "min_n": MIN_RUNS,
        },
        "counters_requested": args.events,
        "counters_available": available,
        "counters_unavailable": unavailable,
        "cells": cells,
    }


def _git_commit(root: Path) -> str:
    try:
        return subprocess.run(
            ["git", "-C", str(root), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--pops", default="1048576", help="comma-separated populations")
    ap.add_argument("--arms", default="map_get,set_contains")
    ap.add_argument("--hit-pcts", default="100,50")
    ap.add_argument("--passes", type=int, default=1)
    ap.add_argument("--runs", type=int, default=10, help=f"paired runs per cell (>= {MIN_RUNS})")
    ap.add_argument("--events", default=",".join(DEFAULT_EVENTS))
    ap.add_argument("--out", default="perf-counters.json")
    ap.add_argument("--host-desc", default="unspecified")
    ap.add_argument("--commit", default="")
    ap.add_argument("--run-id", default="")
    ap.add_argument(
        "--pmu",
        default="auto",
        help="core PMU to count on, for a host that exposes several "
        "(e.g. cpu_core, cpu_atom). `auto` prefers the performance-core PMU.",
    )
    ap.add_argument("--preflight-only", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    args.events = [e.strip() for e in args.events.split(",") if e.strip()]
    pops = [int(p) for p in args.pops.split(",") if p.strip()]
    arms = [a.strip() for a in args.arms.split(",") if a.strip()]
    hit_pcts = [int(h) for h in args.hit_pcts.split(",") if h.strip()]
    if args.runs < MIN_RUNS:
        print(
            f"::error::--runs={args.runs} is below {MIN_RUNS}; BCa needs a jackknife and "
            "an interval is the whole point of a non-deterministic counter",
            file=sys.stderr,
        )
        return 1

    root = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True
        ).stdout.strip()
    )

    try:
        pmu, why, pin, available, unavailable = preflight(root, args.events, args.pmu)
    except Preflight as exc:
        print(f"::error::{exc}", file=sys.stderr)
        return 1

    if pmu:
        pinned = f", workload pinned with `{' '.join(pin)}`" if pin else ""
        print(f"perf_counters.py: counting on PMU {pmu} ({why}){pinned}")
    for u in unavailable:
        print(f"::warning::counter unavailable on this host: {u['event']} — {u['reason']}")
    if args.preflight_only:
        print(f"perf_counters.py: {len(available)} counter(s) available: {', '.join(available)}")
        return 0

    workload = root / WORKLOAD_REL
    cells: list[dict] = []
    try:
        for arm in arms:
            for pop in pops:
                for hit_pct in hit_pcts:
                    phases = run_cell(
                        workload,
                        available,
                        arm,
                        pop,
                        hit_pct,
                        args.passes,
                        args.runs,
                        pmu=pmu,
                        pin=pin,
                    )
                    counters = {}
                    for event in available:
                        samples = attributed(phases, event, pmu)
                        stat = summarise(samples)
                        stat["min_pct_running"] = min_pct_running(phases, event, pmu)
                        stat["pmu"] = pmu
                        stat["perf_event_name"] = perf_event_name(phases, event, pmu)
                        stat["samples"] = samples
                        counters[event] = stat
                    cells.append(
                        {
                            "id": cell_id(arm, pop, hit_pct),
                            "arm": arm,
                            "population": pop,
                            "distinct_probes": pop,
                            "hit_pct": hit_pct,
                            "passes": args.passes,
                            "reuse_factor": args.passes,
                            "workload_echo": phases["probe"][0]["shape"],
                            "counters": counters,
                        }
                    )
    except Preflight as exc:
        print(f"::error::{exc}", file=sys.stderr)
        return 1

    doc = build_doc(args, root, pmu, why, pin, available, unavailable, cells)
    Path(args.out).write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    print("\n".join(render(doc)))
    return 0


# --------------------------------------------------------------------------
def _hybrid_csv() -> str:
    """`perf stat -x, -e instructions,cycle_activity.stalls_l3_miss` on a hybrid part.

    Transcribed from the reference host (12th Gen Intel Core i9-12900F, Alder
    Lake: `cpu_core` P-cores + `cpu_atom` E-cores) into the `-x,` CSV this
    driver actually parses. The shape that matters: one requested event yields
    one row per PMU, the rows carry materially different counts, and **no row
    is named `instructions`** — which is exactly why a `.get("instructions")`
    lookup returned None and the driver reported a permissions problem that
    did not exist. `cycle_activity.stalls_l3_miss` is P-core-only there, so it
    is counted on `cpu_core` and `<not supported>` on `cpu_atom`.
    """
    return (
        "# started on Thu Jan  1 00:00:00 2026\n"
        "\n"
        "51754185,,cpu_core/instructions/,1000000,100.00,,\n"
        "35287145,,cpu_atom/instructions/,1000000,100.00,,\n"
        "4096,,cpu_core/cycle_activity.stalls_l3_miss/,1000000,100.00,,\n"
        "<not supported>,,cpu_atom/cycle_activity.stalls_l3_miss/,0,0.00,,\n"
    )


def self_test() -> int:
    """Everything that does not need a PMU. Run in the `lint` CI job."""
    csv = (
        "# started on Thu Jan  1 00:00:00 2026\n"
        "\n"
        "1234567,,cycles,1000000,100.00,,\n"
        "89012,,instructions,1000000,52.31,,\n"
        "<not supported>,,cycle_activity.stalls_l3_miss,0,0.00,,\n"
    )
    parsed = parse_perf_csv(csv)
    assert parsed["cycles"]["value"] == 1234567.0, parsed
    assert parsed["cycles"]["pct_running"] == 100.0, parsed
    assert parsed["instructions"]["pct_running"] == 52.31, parsed
    # An unsupported counter is None, never 0 — a 0 would read as "measured,
    # and there were none", which is the silent-gap failure this tool exists
    # to avoid.
    assert parsed["cycle_activity.stalls_l3_miss"]["value"] is None, parsed
    assert parsed["cycle_activity.stalls_l3_miss"]["status"] == "<not supported>", parsed
    # An unqualified host keeps a None PMU rather than inventing one.
    assert parsed["cycles"]["pmu"] is None, parsed

    # ---- PMU-qualified event names (hybrid CPUs) -------------------------
    assert split_event_name("instructions") == (None, "instructions")
    assert split_event_name("cpu_core/instructions/") == ("cpu_core", "instructions")
    assert split_event_name("cpu_atom/instructions") == ("cpu_atom", "instructions")
    assert split_event_name("cpu_core/mem_load_retired.l3_miss/") == (
        "cpu_core",
        "mem_load_retired.l3_miss",
    )

    hybrid = parse_perf_csv(_hybrid_csv())
    # The regression this fixture exists for: the bare name is absent, so the
    # old `probed.get("instructions")` lookup found nothing on a host whose
    # counters were working perfectly.
    assert "instructions" not in hybrid, hybrid
    rows = select_rows(hybrid, "instructions")
    assert set(rows) == {"cpu_core", "cpu_atom"}, rows
    assert rows["cpu_core"]["value"] == 51754185.0, rows
    assert rows["cpu_atom"]["value"] == 35287145.0, rows
    assert row_for(hybrid, "instructions", "cpu_core")["value"] == 51754185.0
    assert row_for(hybrid, "instructions", "cpu_atom")["value"] == 35287145.0
    # Exact matching still works, and a PMU-qualified request stays scoped.
    assert select_rows(parsed, "cycles")[None]["value"] == 1234567.0
    assert set(select_rows(hybrid, "cpu_core/instructions/")) == {"cpu_core"}

    # A P-core-only event: counted on cpu_core, <not supported> on cpu_atom.
    # It is available on the selected PMU and must not read as globally absent.
    core_only = row_for(hybrid, "cycle_activity.stalls_l3_miss", "cpu_core")
    atom_only = row_for(hybrid, "cycle_activity.stalls_l3_miss", "cpu_atom")
    assert core_only is not None and core_only["value"] == 4096.0, core_only
    assert atom_only is not None and atom_only["value"] is None, atom_only
    assert atom_only["status"] == "<not supported>", atom_only

    # ---- PMU selection ---------------------------------------------------
    sel, why = resolve_pmu(["cpu_atom", "cpu_core"], "auto")
    assert sel == "cpu_core" and "cpu_core" in why, (sel, why)
    sel, why = resolve_pmu(["cpu_atom", "cpu_core"], "cpu_atom")
    assert sel == "cpu_atom" and "--pmu" in why, (sel, why)
    assert resolve_pmu([None], "auto")[0] is None
    assert resolve_pmu(["cpu"], "auto")[0] == "cpu"
    try:
        resolve_pmu(["cpu_atom", "cpu_core"], "cpu_nope")
        raise AssertionError("an unserved --pmu must be fatal")
    except Preflight as exc:
        assert "cpu_nope" in str(exc), exc
    try:
        resolve_pmu(["big_odd_pmu", "little_odd_pmu"], "auto")
        raise AssertionError("an unrankable PMU set must be fatal, not an arbitrary pick")
    except Preflight as exc:
        assert "--pmu" in str(exc), exc

    # ---- the pin holds ---------------------------------------------------
    # Pinned to cpu_core, a non-zero cpu_atom count means the workload
    # straddled both core types and the numbers blend two machines.
    assert pin_violations(hybrid, ["instructions"], "cpu_core") == [
        "cpu_atom/instructions/=35,287,145"
    ], pin_violations(hybrid, ["instructions"], "cpu_core")
    # A <not supported> sibling row is the expected shape, not a violation.
    assert pin_violations(hybrid, ["cycle_activity.stalls_l3_miss"], "cpu_core") == []
    # A uniform host has no sibling to violate anything.
    assert pin_violations(parsed, ["cycles"], None) == []

    # ---- probe classification: permissions vs naming ---------------------
    # Working hybrid host, rc == 0: no error at all. This is the failure the
    # fix removes — perf counted, and the driver called it a permissions
    # problem because it looked up the wrong key.
    assert classify_probe(0, hybrid, "", "1") is None
    assert classify_probe(0, parsed, "", "1") is None
    # rc == 0 and no matching row: a naming problem. It must say so, must list
    # what perf emitted, and must NOT prescribe capability or paranoid changes.
    naming = classify_probe(0, parse_perf_csv("42,,some_other_event,1000,100.00,,\n"), "", "1")
    assert naming is not None
    assert "naming mismatch" in naming and "some_other_event" in naming, naming
    assert "setcap" not in naming and "sysctl" not in naming, naming
    # rc != 0: the kernel refused. Capability advice belongs here and only here.
    refused = classify_probe(1, {}, "Access to performance monitoring ... is limited", "3")
    assert refused is not None and "setcap" in refused and "CAP_PERFMON" in refused, refused
    # rc == 0 but the row is a placeholder: also a refusal, not a naming issue.
    placeheld = classify_probe(
        0, parse_perf_csv("<not counted>,,instructions,0,0.00,,\n"), "", "2"
    )
    assert placeheld is not None and "setcap" in placeheld, placeheld

    # ---- attribution is per PMU -----------------------------------------
    phases = {
        "build": [{"counters": {"cycles": {"value": 100.0, "pct_running": 100.0}}, "shape": "b"}],
        "probe": [{"counters": {"cycles": {"value": 175.0, "pct_running": 50.0}}, "shape": "p"}],
    }
    assert attributed(phases, "cycles") == [75.0], attributed(phases, "cycles")
    # An event with no value in either phase yields no sample rather than a
    # fabricated zero difference.
    assert attributed(phases, "absent") == []
    assert min_pct_running(phases, "cycles") == 50.0

    hybrid_phases = {
        "build": [{"counters": parse_perf_csv(
            "10,,cpu_core/instructions/,1000,100.00,,\n"
            "7,,cpu_atom/instructions/,1000,80.00,,\n"
        ), "shape": "b"}],
        "probe": [{"counters": parse_perf_csv(
            "40,,cpu_core/instructions/,1000,90.00,,\n"
            "9,,cpu_atom/instructions/,1000,70.00,,\n"
        ), "shape": "p"}],
    }
    # Differencing happens within one PMU; the two core types never mix, and
    # the sum of the two (30 + 2 = 32) is never what comes out.
    assert attributed(hybrid_phases, "instructions", "cpu_core") == [30.0]
    assert attributed(hybrid_phases, "instructions", "cpu_atom") == [2.0]
    assert min_pct_running(hybrid_phases, "instructions", "cpu_core") == 90.0
    assert perf_event_name(hybrid_phases, "instructions", "cpu_core") == "cpu_core/instructions/"

    thin = summarise([1.0, 2.0])
    assert thin["ci_lower"] is None and "no interval" in thin["status"], thin
    wide = summarise([100.0, 104.0, 96.0, 101.0, 99.0])
    assert wide["ci_lower"] is not None and wide["ci_lower"] <= wide["point"] <= wide["ci_upper"], wide

    doc = {
        "counters_unavailable": [{"event": "mem_load_retired.l3_miss", "reason": "not Intel"}],
        "cells": [
            {
                "id": "map_get/pop=8/hit=100",
                "distinct_probes": 8,
                "passes": 1,
                "hit_pct": 100,
                "counters": {"cycles": dict(wide, min_pct_running=100.0)},
            }
        ],
    }
    out = "\n".join(render(doc))
    # Rendering is derived, not stamped (rule 10): the unavailable counter and
    # the measured one both have to reach the table from `doc`.
    assert "mem_load_retired.l3_miss" in out and "not Intel" in out, out
    assert "`cycles`" in out and "map_get/pop=8/hit=100" in out, out
    assert "no counter here gates anything" in out.lower(), out

    # A hybrid run says which PMU it counted, where it was pinned, and labels
    # every row with perf's own PMU-qualified event name.
    hybrid_doc = {
        "pmu": {
            "selected": "cpu_core",
            "selection_reason": "the performance-core PMU",
            "cpus": "0-15",
            "pin_command": "taskset -c 0-15",
        },
        "counters_unavailable": [],
        "cells": [
            {
                "id": "map_get/pop=8/hit=100",
                "distinct_probes": 8,
                "passes": 1,
                "hit_pct": 100,
                "counters": {
                    "instructions": dict(
                        wide,
                        min_pct_running=100.0,
                        pmu="cpu_core",
                        perf_event_name="cpu_core/instructions/",
                    )
                },
            }
        ],
    }
    hybrid_out = "\n".join(render(hybrid_doc))
    assert "`cpu_core` PMU only" in hybrid_out, hybrid_out
    assert "taskset -c 0-15" in hybrid_out and "0-15" in hybrid_out, hybrid_out
    assert "`cpu_core/instructions/`" in hybrid_out, hybrid_out

    empty = "\n".join(render({"counters_unavailable": [], "cells": []}))
    assert "Every requested counter was available" in empty, empty

    assert MIN_RUNS >= 3, "BCa needs a jackknife"
    assert set(VENDOR_EVENTS) <= set(DEFAULT_EVENTS)
    print("perf_counters.py --self-test: all checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
