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

Fail-loud (AGENTS.md section 8.1): a missing `perf`, a kernel that refuses to
open a counter, or a missing workload binary exits non-zero with the cause and
the fix named. It never degrades into a report that reads as complete.

Usage:
  python3 scripts/perf_counters.py --out perf-counters.json
  python3 scripts/perf_counters.py --pops 262144,1048576,4194304 --runs 10
  python3 scripts/perf_counters.py --preflight-only
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
# rather than left as gaps in a table.
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


# --------------------------------------------------------------------------
# perf stat CSV
# --------------------------------------------------------------------------
def parse_perf_csv(text: str) -> dict[str, dict[str, float | str | None]]:
    """`{event -> {value, pct_running, status}}` from `perf stat -x,` output.

    perf's CSV is `value,unit,event,runtime_ns,pct_running[,metric,...]`.
    A counter the kernel could not open carries a placeholder in the value
    column; it is reported as such, never as zero — a zero would read as
    "measured, and there were none".
    """
    out: dict[str, dict[str, float | str | None]] = {}
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
        if value in NOT_COUNTED:
            out[event] = {"value": None, "pct_running": pct, "status": value}
            continue
        try:
            out[event] = {
                "value": float(value),
                "pct_running": pct,
                "status": "counted",
            }
        except ValueError:
            out[event] = {"value": None, "pct_running": pct, "status": f"unparsed:{value}"}
    return out


def run_perf(events: list[str], cmd: list[str], env: dict[str, str]) -> tuple[int, str, str, str]:
    """One `perf stat` invocation. Returns (rc, csv_text, stdout, stderr)."""
    with tempfile.NamedTemporaryFile("r+", suffix=".csv", delete=False) as fh:
        stat_path = fh.name
    try:
        proc = subprocess.run(
            ["perf", "stat", "-x,", "-o", stat_path, "-e", ",".join(events), "--"]
            + cmd,
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


def preflight(root: Path, events: list[str]) -> tuple[list[str], list[dict[str, str]]]:
    """Probe the real capability, then classify each requested event.

    Deliberately not a proxy: the question is "can this kernel open a counter
    for us", so the probe opens one. `perf` being on PATH answers a weaker
    question and has answered it wrongly before on hosts where
    `perf_event_paranoid` forbids the open.
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

    # The capability probe: one real counter over one real process.
    rc, csv_text, _out, err = run_perf(
        ["instructions"], [str(workload)], _workload_env({"EXPANSE_PERF_POP": "1", "EXPANSE_PERF_PHASE": "build"})
    )
    probed = parse_perf_csv(csv_text)
    got = probed.get("instructions", {}).get("value")
    if rc != 0 or got is None:
        raise Preflight(
            "`perf stat -e instructions` could not open a hardware counter on this host "
            f"(exit {rc}, kernel.perf_event_paranoid={paranoid_level()}).\n"
            "  Fix, in order of preference:\n"
            "    1. grant the runner CAP_PERFMON: "
            "`sudo setcap cap_perfmon,cap_sys_ptrace,cap_syslog=ep $(command -v perf)`\n"
            "    2. or lower the gate for all users: "
            "`sudo sysctl -w kernel.perf_event_paranoid=1` "
            "(persist it in /etc/sysctl.d/)\n"
            "  A virtualised host may expose no PMU at all, in which case neither fix "
            "applies and the counters have to be collected on bare metal.\n"
            f"  perf said: {err.strip()[:400]}\n"
            "No counters were collected and no numbers were produced."
        )

    # Per-event classification. Probed one at a time so a single unsupported
    # event cannot take the whole set down with it, and so the reason is
    # attributable to the event that caused it.
    available: list[str] = []
    unavailable: list[dict[str, str]] = []
    for event in events:
        rc, csv_text, _out, err = run_perf(
            [event], [str(workload)], _workload_env({"EXPANSE_PERF_POP": "1", "EXPANSE_PERF_PHASE": "build"})
        )
        parsed = parse_perf_csv(csv_text)
        entry = parsed.get(event)
        if rc == 0 and entry and entry.get("value") is not None:
            available.append(event)
            continue
        if rc != 0:
            reason = "perf refused the event (unknown on this microarchitecture)"
        else:
            reason = str((entry or {}).get("status") or "no value returned")
        unavailable.append({"event": event, "reason": reason, "perf_stderr": err.strip()[:200]})
    if not available:
        raise Preflight(
            "no requested counter is available on this host — every one of "
            f"{', '.join(events)} was refused. A table with no counters in it is not "
            "a measurement. No numbers were produced."
        )
    return available, unavailable


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
            rc, csv_text, out, err = run_perf(events, [str(workload)], env)
            if rc != 0:
                raise Preflight(
                    f"the workload exited {rc} for {cell_id(arm, pop, hit_pct)} phase={phase}. "
                    "A partial sweep is not a result.\n"
                    f"  stdout: {out.strip()[:300]}\n  stderr: {err.strip()[:300]}"
                )
            parsed = parse_perf_csv(csv_text)
            phases[phase].append({"counters": parsed, "shape": out.strip()})
    return phases


def attributed(phases: dict[str, list[dict]], event: str) -> list[float]:
    """Per-run `probe - build` for one event, over the paired runs."""
    out: list[float] = []
    for build, probe in zip(phases["build"], phases["probe"]):
        b = build["counters"].get(event, {}).get("value")
        p = probe["counters"].get(event, {}).get("value")
        if b is None or p is None:
            continue
        out.append(float(p) - float(b))
    return out


def min_pct_running(phases: dict[str, list[dict]], event: str) -> float | None:
    """Lowest multiplexing fraction seen for one event across every run.

    Below 100 the kernel time-shared the counter and perf scaled the value up.
    That is a real property of the number and is printed, not hidden.
    """
    seen = [
        r["counters"].get(event, {}).get("pct_running")
        for phase in phases.values()
        for r in phase
    ]
    vals = [float(v) for v in seen if isinstance(v, (int, float))]
    return min(vals) if vals else None


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
        lines += ["Every requested counter was available on this host.", ""]

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
            lines.append(f"| `{event}` | {point} | {ci} | {stat['n']} | {pct_s} |")
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
def build_doc(args, root: Path, available: list[str], unavailable: list[dict], cells: list[dict]) -> dict:
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
        available, unavailable = preflight(root, args.events)
    except Preflight as exc:
        print(f"::error::{exc}", file=sys.stderr)
        return 1

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
                        workload, available, arm, pop, hit_pct, args.passes, args.runs
                    )
                    counters = {}
                    for event in available:
                        samples = attributed(phases, event)
                        stat = summarise(samples)
                        stat["min_pct_running"] = min_pct_running(phases, event)
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

    doc = build_doc(args, root, available, unavailable, cells)
    Path(args.out).write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    print("\n".join(render(doc)))
    return 0


# --------------------------------------------------------------------------
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

    phases = {
        "build": [{"counters": {"cycles": {"value": 100.0, "pct_running": 100.0}}, "shape": "b"}],
        "probe": [{"counters": {"cycles": {"value": 175.0, "pct_running": 50.0}}, "shape": "p"}],
    }
    assert attributed(phases, "cycles") == [75.0], attributed(phases, "cycles")
    # An event with no value in either phase yields no sample rather than a
    # fabricated zero difference.
    assert attributed(phases, "absent") == []
    assert min_pct_running(phases, "cycles") == 50.0

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

    empty = "\n".join(render({"counters_unavailable": [], "cells": []}))
    assert "Every requested counter was available" in empty, empty

    assert MIN_RUNS >= 3, "BCa needs a jackknife"
    assert set(VENDOR_EVENTS) <= set(DEFAULT_EVENTS)
    print("perf_counters.py --self-test: all checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
