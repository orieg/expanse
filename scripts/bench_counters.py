#!/usr/bin/env python3
"""Hardware counters for a named comparative-suite cell, so a mechanism paragraph
can stop ending in "unmeasured" (#737), and per-thread counters for the
concurrent cells so a writer's cost and a reader's cost stop being one number
(#568).

Every mechanism paragraph in `masstree_comparison/README.md` and
`hot_comparison/README.md` ends in "unmeasured" — the reader collapse under one
writer, the string-lookup gap, the sorted-order insert loss, the scan-start
effect, the memory cascade. AGENTS.md section 8.9 principle 1 forbids stating a
microarchitectural cause without counters, and none of the comparative harnesses
takes any. #724, #725 and #730 each schedule `perf stat` runs for one path by
hand; this is that, once, for every cell.

## Two modes

**Process mode** (the single-threaded cells): runs a cell's existing binary under

    perf stat -e page-faults,dTLB-load-misses,LLC-load-misses,cycles,instructions

one `perf` invocation per repeat, divides each event by the cell's own probe or
key count, and writes `counters_<cell>.json` beside the wall-clock artifact with
a BCa 95% bootstrap interval over the repeats (section 8.4).

**Per-thread mode** (the concurrent cells, #568): starts the concurrent binary
in its `--arm <arm> --rounds R --wait-stdin` counters mode, and on every round's
`threads_ready` line attaches

    perf stat --per-thread -p <pid> -x, -e <events>

to the live process, waits for the attach to settle, releases the round through
the binary's stdin, waits for its `counters` row, and stops perf. The CSV then
carries one row per (thread, event) keyed by the thread's `comm-tid`; the
harness names its threads `writer-N` / `reader-N`, so the rows group by role.
Writer-thread events are divided by that round's `write_ops`, reader-thread
events by its `read_ops` — both the harness's own counts — and the BCa interval
is over rounds. The main thread's rows (the join, the population walk) are kept
verbatim and attributed to neither role.

Why an attach and not a wrapper: `perf stat -- <binary>` counts the prefill and
every round into one number, and cannot tell a writer from a reader. The attach
counts exactly the threads that exist when it happens, which is why the harness
announces them after the spawn and before the barrier. What `perf stat
--per-thread -p` was observed to do on the reference host is recorded on the
harness's `threads_ready` (perf 6.8.12): it enumerates `/proc/<pid>/task` once
at attach, a thread created later has no row, and `SIGINT` writes the rows.

Two cells add a `perf c2c record -p <pid>` over one extra round and save
`perf c2c report --stdio` as `c2c_<cell>.txt` beside the JSON, with the sync
wrappers' field offsets (`--layout`, health build) in the JSON so a reader can
name which fields share a line. The report is text a reader inspects; nothing
here parses it into a number.

## What it deliberately does not do

**Process mode does not bracket the timed loop.** A cell's binary builds a
population and then probes it, and `perf stat` counts the whole process. So
every process-mode figure is *per process*, and the build is in it. Two
consequences, both stated in the artifact rather than left for a reader to
discover: a lookup cell's counts include the build that preceded the probes,
and a comparison between two arms is only as clean as the similarity of their
builds. `scripts/perf_counters.py` differences a `build` phase against a
`probe` phase to get around this; the comparative bins have no such phase
switch, and inventing one would change the binaries this suite measured.

**It does not attribute.** A counter is evidence for a mechanism, not the
mechanism. A cell whose intervals overlap decides nothing and says so.

## Hybrid hosts

The reference host is an Alder-Lake-class part whose kernel exposes two core
PMUs, so one requested event comes back as `cpu_core/<event>/` **and**
`cpu_atom/<event>/` and never as a bare `<event>`. Those two rows count two
different microarchitectures over two different sets of cores and are never
summed: this driver selects one PMU, confines the workload to that PMU's CPUs,
requests every hardware event qualified with that PMU's name, reads only that
PMU's rows and names it in the artifact. That logic already exists in
`scripts/perf_counters.py` and is imported rather than written twice.

Fail-loud (section 8.1): a missing `perf`, a kernel that refuses to open a
counter, a missing binary, a workload that escaped its pin, an attach that saw
fewer threads than the harness spawned, or a binary that exited non-zero exits
non-zero with the cause and the fix named. It never degrades into a report that
reads as complete. An event the host cannot open — `syscalls:sys_enter_futex`
needs tracepoint access — is recorded as `None` with the preflight that said
so, never as 0.

Usage:
  python3 scripts/bench_counters.py --list
  python3 scripts/bench_counters.py --cell hot_lookup_random_1m --repeats 7
  python3 scripts/bench_counters.py --cell masstree_conc_map_w1_r8 --repeats 5
  python3 scripts/bench_counters.py --all --out-dir docs/benchmarks/hot_comparison/results
  python3 scripts/bench_counters.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402
from bench_provenance import add_load, host_facts, load_snapshot  # noqa: E402
from perf_counters import (  # noqa: E402
    Preflight,
    NOT_COUNTED,
    paranoid_level,
    parse_perf_csv,
    pin_for,
    pmu_cpus,
    resolve_pmu,
    row_for,
    run_perf,
)

CRATE = REPO_ROOT / "crates" / "expanse-hot-bench" / "Cargo.toml"

# The five events #724, #725 and #730 name, in the order they name them.
BASE_EVENTS = [
    "page-faults",
    "dTLB-load-misses",
    "LLC-load-misses",
    "cycles",
    "instructions",
]
# Cross-core snoop hits: the counter #730 and #568 ask for on a concurrent cell,
# and the only one in this set that speaks to reader-reader line transfer.
CONCURRENT_EVENT = "mem_load_l3_hit_retired.xsnp_hitm"

# The per-thread events #568 names for a concurrent cell. `ref-cycles` beside
# `cycles` separates a frequency change from a work change; `task-clock` and
# `context-switches` say how much of a thread's wall time it was on a CPU;
# `l2_rqsts.rfo_miss` is the writer's line-ownership traffic and `xsnp_hitm`
# the cross-core line transfer a reader pays for. Every name was checked
# against `perf list` on the reference host before it was written here.
THREAD_EVENTS = [
    "cycles",
    "ref-cycles",
    "instructions",
    "task-clock",
    "context-switches",
    "LLC-load-misses",
    "l2_rqsts.rfo_miss",
    CONCURRENT_EVENT,
]
# A tracepoint, so it needs tracefs access the hardware events do not; it is
# requested only when its own preflight opens it, and its absence is recorded.
FUTEX_EVENT = "syscalls:sys_enter_futex"

# Events served by the kernel's software PMU rather than a core PMU: they are
# never PMU-qualified, and on a hybrid host perf returns them unqualified.
SOFTWARE_EVENTS = {"task-clock", "context-switches", "page-faults", "cpu-clock",
                   "cpu-migrations", "minor-faults", "major-faults"}

# How long the attach gets to settle before the round is released. The attach
# opens one counter set per thread; releasing under it would count the round
# from somewhere after its first instruction.
ATTACH_SETTLE_S = 0.2

# The thread-name prefixes the concurrent harnesses use (their `comm`).
ROLE_PREFIXES = (("writer", "writer-"), ("reader", "reader-"))
# Which harness count divides which role's events.
ROLE_OPS_KEY = {"writer": "write_ops", "reader": "read_ops"}


class Cell:
    """One named cell: which binary, which arguments, and what to divide by.

    `ops_key` names the field in the binary's own JSON rows that says how many
    operations a round did, so the per-operation figure is the harness's own
    count and not a second, independently-derived one that could disagree
    (section 8.2).

    A concurrent cell (`concurrent=True`) runs in per-thread mode: `arm` is
    the harness's `--arm` value, `c2c` adds the `perf c2c` round, and `layout`
    records the sync wrappers' field offsets from a health build.
    """

    def __init__(self, name: str, issue: int, suite: str, binary: str, args: list[str],
                 features: list[str], concurrent: bool = False, note: str = "",
                 blocked: str = "", arm: str = "", c2c: bool = False,
                 layout: bool = False):
        self.name = name
        self.issue = issue
        self.suite = suite
        self.binary = binary
        self.args = args
        self.features = features
        self.concurrent = concurrent
        self.note = note
        # Non-empty when the harness cannot yet produce what this cell needs.
        self.blocked = blocked
        self.arm = arm
        self.c2c = c2c
        self.layout = layout

    @property
    def mode(self) -> str:
        return "per-thread" if self.concurrent else "process"

    def events(self) -> list[str]:
        if self.concurrent:
            return list(THREAD_EVENTS)
        return list(BASE_EVENTS)


def _conc(name: str, issue: int, suite: str, binary: str, args: list[str],
          features: list[str], note: str, **kw) -> Cell:
    return Cell(name, issue, suite, binary, args, features, concurrent=True,
                arm="expanse", note=note, **kw)


# The cells #737's gate names (the four from #724 / #725 / #730 and the HOT
# `random` 1M lookup cell), the two #730 concurrent cells, and #568's thirteen.
CELLS = [
    # --- #724: a 12-byte string lookup costs four times a u64 lookup ---
    Cell("masstree_str_lookup_counter_1m", 724, "masstree_comparison",
         "masstree_string_latency", ["lookup_hit", "counter", "1000000"], ["masstree"],
         note="the `counter` 100%-hit string lookup, both arms in one process"),
    Cell("masstree_str_lookup_short_1m", 724, "masstree_comparison",
         "masstree_string_latency", ["lookup_hit", "short", "1000000"], ["masstree"],
         note="the `short` 100%-hit string lookup, both arms in one process"),
    # --- #725: sorted-order insertion loses to the append path ---
    Cell("masstree_insert_sparse_1m_sorted", 725, "masstree_comparison",
         "masstree_latency", ["insert", "sparse", "1000000", "sorted"], ["masstree"],
         note="sorted `sparse` insert, the widest of the three unpredicted losses"),
    Cell("masstree_insert_random_1m_sorted", 725, "masstree_comparison",
         "masstree_latency", ["insert", "random", "1000000", "sorted"], ["masstree"],
         note="sorted `random` insert"),
    # --- #730: eight readers with no writer ---
    #
    # Per-thread mode over the Expanse arm alone: the harness's `counters` rows
    # publish `read_ops`, which is the per-probe divisor these two cells were
    # blocked on until #568's harness work added it.
    _conc("masstree_conc_str_w0_r1", 730, "masstree_comparison",
          "masstree_concurrent", ["str", "0", "1"], ["masstree"],
          "one reader, no writer — the wrapper's own per-probe cost"),
    _conc("masstree_conc_str_w0_r8", 730, "masstree_comparison",
          "masstree_concurrent", ["str", "0", "8"], ["masstree"],
          "eight readers, no writer — reader-reader line traffic if it grows with R"),
    # The #725 order pair. A per-process count cannot separate the two arms,
    # but both arms are present in *both* cells, so the difference between them
    # isolates what changing the build order costs — which is the question #725
    # asks first ("whether the sorted-order loss is the append path or the
    # page-fault bill").
    Cell("masstree_insert_random_1m_shuffled", 725, "masstree_comparison",
         "masstree_latency", ["insert", "random", "1000000", "shuffled"], ["masstree"],
         note="the same cell shuffled; compare with masstree_insert_random_1m_sorted"),
    # --- #737's own gate cell ---
    Cell("hot_lookup_random_1m", 737, "hot_comparison",
         "hot_latency", ["map", "lookup_hit", "random", "1000000"], [],
         note="the HOT `random` 1M 100%-hit map lookup"),
    # --- #568: multi-writer scaling, attributed per thread, Expanse arm ---
    _conc("masstree_conc_map_w1_r8", 568, "masstree_comparison",
          "masstree_concurrent", ["map", "1", "8"], ["masstree"],
          "one writer, eight readers — the reader collapse under a writer; "
          "also the `perf c2c` cell", c2c=True, layout=True),
    _conc("masstree_conc_map_w1_r0", 568, "masstree_comparison",
          "masstree_concurrent", ["map", "1", "0"], ["masstree"],
          "one writer alone — the writer's uncontended per-insert cost"),
    _conc("masstree_conc_map_w4_r0", 568, "masstree_comparison",
          "masstree_concurrent", ["map", "4", "0"], ["masstree"],
          "four writers, map arm"),
    _conc("masstree_conc_map_w0_r8", 568, "masstree_comparison",
          "masstree_concurrent", ["map", "0", "8"], ["masstree"],
          "eight readers, no writer — the reader-side control for the C2 coherence columns"),
    _conc("masstree_conc_map_w8_r0", 568, "masstree_comparison",
          "masstree_concurrent", ["map", "8", "0"], ["masstree"],
          "eight writers — one thread per physical P-core"),
    _conc("masstree_conc_map_w16_r0", 568, "masstree_comparison",
          "masstree_concurrent", ["map", "16", "0"], ["masstree"],
          "sixteen writers — two threads per physical P-core (SMT siblings)"),
    _conc("masstree_conc_str_w8_r0", 568, "masstree_comparison",
          "masstree_concurrent", ["str", "8", "0"], ["masstree"],
          "eight string writers"),
    _conc("masstree_conc_str_w16_r0", 568, "masstree_comparison",
          "masstree_concurrent", ["str", "16", "0"], ["masstree"],
          "sixteen string writers — two threads per physical P-core"),
    _conc("hot_conc_set_w1_r8", 568, "hot_comparison",
          "hot_concurrent", ["set", "1", "8"], ["rowex"],
          "one writer, eight readers, set arm"),
    _conc("hot_conc_map_w1_r8", 568, "hot_comparison",
          "hot_concurrent", ["map", "1", "8"], ["rowex"],
          "one writer, eight readers, map arm; also the `perf c2c` cell",
          c2c=True, layout=True),
    _conc("hot_conc_set_w0_r8", 568, "hot_comparison",
          "hot_concurrent", ["set", "0", "8"], ["rowex"],
          "eight readers, no writer, set arm — the reader-side control"),
    _conc("hot_conc_map_w0_r8", 568, "hot_comparison",
          "hot_concurrent", ["map", "0", "8"], ["rowex"],
          "eight readers, no writer, map arm — the reader-side control"),
    _conc("hot_conc_set_w1_r0", 568, "hot_comparison",
          "hot_concurrent", ["set", "1", "0"], ["rowex"],
          "one writer alone, set arm"),
    _conc("hot_conc_map_w1_r0", 568, "hot_comparison",
          "hot_concurrent", ["map", "1", "0"], ["rowex"],
          "one writer alone, map arm"),
    _conc("hot_conc_map_w4_r0", 568, "hot_comparison",
          "hot_concurrent", ["map", "4", "0"], ["rowex"],
          "four writers, map arm"),
    _conc("hot_conc_map_w8_r0", 568, "hot_comparison",
          "hot_concurrent", ["map", "8", "0"], ["rowex"],
          "eight writers, map arm"),
]

BY_NAME = {c.name: c for c in CELLS}


# --------------------------------------------------------------------------
# event names on a hybrid host
# --------------------------------------------------------------------------
def qualify(event: str, pmu: str | None) -> str:
    """The name to *request* from perf for `event` on `pmu`.

    On the hybrid reference host an unqualified hardware event is served by
    both core PMUs, and `perf stat` has been seen to print non-zero counts on
    the sibling PMU for a process pinned away from it. Requesting
    `cpu_core/cycles/` explicitly is what makes the row unambiguous. Software
    events and tracepoints have no core PMU and stay bare.
    """
    if not pmu or event in SOFTWARE_EVENTS or ":" in event or "/" in event:
        return event
    return f"{pmu}/{event}/"


def role_of(comm: str) -> str:
    """`writer-3` -> `writer`, `reader-0` -> `reader`, anything else -> `other`."""
    for role, prefix in ROLE_PREFIXES:
        if comm.startswith(prefix):
            return role
    return "other"


# --------------------------------------------------------------------------
# per-thread CSV
# --------------------------------------------------------------------------
def parse_per_thread_csv(text: str) -> dict[str, dict]:
    """`{thread_key -> {comm, tid, role, rows}}` from `perf stat --per-thread -x,`.

    perf prefixes every row with the thread it counted, as `<comm>-<tid>`,
    then the usual `value,unit,event,runtime_ns,pct_running,...`. `comm` may
    itself contain `-` (`writer-0`), so the tid is the last `-`-separated
    field. `rows` is the same shape `parse_perf_csv` returns, so `row_for`
    selects the PMU row per thread exactly as it does per process.
    """
    per_thread: dict[str, list[str]] = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        key, sep, rest = line.partition(",")
        if not sep or not key or "-" not in key:
            continue
        per_thread.setdefault(key, []).append(rest)
    out: dict[str, dict] = {}
    for key, lines in per_thread.items():
        comm, _, tid = key.rpartition("-")
        if not tid.isdigit():
            continue
        out[key] = {
            "comm": comm,
            "tid": int(tid),
            "role": role_of(comm),
            "rows": parse_perf_csv("\n".join(lines) + "\n"),
        }
    return out


def role_totals(threads: dict[str, dict], events: list[str], pmu: str | None) -> dict:
    """`{role -> {event -> total or None}}` plus the thread count per role.

    A role's total is the sum over its threads of the selected PMU's row. If
    any thread of the role has no counted row for an event, the total is
    `None`: a partial sum would read as a smaller cost, not as a gap.
    """
    totals: dict[str, dict] = {}
    counts: dict[str, int] = {}
    for t in threads.values():
        role = t["role"]
        counts[role] = counts.get(role, 0) + 1
        acc = totals.setdefault(role, {ev: 0.0 for ev in events})
        for ev in events:
            if acc[ev] is None:
                continue
            row = row_for(t["rows"], ev, pmu)
            if row is None or row["value"] is None:
                acc[ev] = None
            else:
                acc[ev] += row["value"]
    return {"totals": totals, "threads": counts}


def per_op_for_role(totals: dict | None, ops: int) -> dict:
    """Each event total divided by `ops`; `None` when there is nothing to divide."""
    if not totals or not ops:
        return {ev: None for ev in (totals or {})}
    return {ev: (None if v is None else v / ops) for ev, v in totals.items()}


# --------------------------------------------------------------------------
# preflight
# --------------------------------------------------------------------------
def preflight(events: list[str]) -> tuple[str | None, str, list[str], list[str], list[dict]]:
    """Probe the real capability, pick a PMU, classify each requested event.

    The probe opens a counter over a real process, because "is `perf` on PATH"
    answers a weaker question and has answered it wrongly on hosts where
    `perf_event_paranoid` forbids the open.
    """
    if platform.system() != "Linux":
        raise Preflight(
            f"perf stat is Linux-only and this host reports {platform.system()}. "
            "Run this on the reference host. No counters were collected."
        )
    if shutil.which("perf") is None:
        raise Preflight(
            "`perf` is not on PATH. Install the kernel's matching tools package "
            "(`linux-tools-common` plus `linux-tools-$(uname -r)` on Debian/Ubuntu). "
            "No counters were collected."
        )

    rc, csv_text, _out, err = run_perf(["instructions"], ["true"], dict(os.environ))
    probed = parse_perf_csv(csv_text)
    rows = [e for e in probed.values() if e["base_event"] == "instructions"]
    if not rows or all(r["value"] is None for r in rows):
        raise Preflight(
            "`perf stat -e instructions` could not open a hardware counter for a "
            f"process on this host (perf_event_paranoid = {paranoid_level()}, rc = {rc}). "
            "Lower it to 2 or below, or grant CAP_PERFMON. No counters were "
            f"collected.\n{err.strip()}"
        )

    pmu, why = resolve_pmu([r["pmu"] for r in rows], "auto")
    pin = pin_for(pmu) if pmu else []

    # Which of the requested events this host can actually serve. An event the
    # kernel refuses is reported by name, never silently dropped and never
    # counted as zero — a zero would read as "measured, and there were none".
    rc, csv_text, _o, _e = run_perf(events, ["true"], dict(os.environ), pin)
    parsed = parse_perf_csv(csv_text)
    available, unavailable = [], []
    for ev in events:
        row = row_for(parsed, ev, pmu)
        if row is None:
            unavailable.append({"event": ev, "reason": "no row returned for the selected PMU"})
        elif row["status"] in NOT_COUNTED or row["value"] is None:
            unavailable.append({"event": ev, "reason": row["status"]})
        else:
            available.append(ev)
    if not available:
        raise Preflight(
            "none of the requested events could be counted on this host: "
            + "; ".join(f"{u['event']} ({u['reason']})" for u in unavailable)
        )
    return pmu, why, pin, available, unavailable


def futex_preflight() -> dict:
    """Whether `syscalls:sys_enter_futex` opens on this host, and why not if not.

    A tracepoint needs tracefs access that a hardware counter does not, and
    perf fails the *whole* invocation when one event cannot be opened — so the
    probe is on its own, and its outcome travels in the artifact's provenance.
    """
    rc, csv_text, _out, err = run_perf([FUTEX_EVENT], ["true"], dict(os.environ))
    row = row_for(parse_perf_csv(csv_text), FUTEX_EVENT, None)
    ok = rc == 0 and row is not None and row["value"] is not None
    reason = "counted" if ok else (
        (row["status"] if row is not None else "no row") + "; " + err.strip()[:300]
    )
    return {"event": FUTEX_EVENT, "available": ok, "rc": rc, "reason": reason}


# --------------------------------------------------------------------------
# running a cell
# --------------------------------------------------------------------------
def target_root(occ_stats: bool = False) -> Path:
    target = os.environ.get("CARGO_TARGET_DIR")
    root = Path(target) if target else (CRATE.parent / "target")
    # The health build is a different binary of the same name: its own target
    # dir keeps the two from overwriting each other (and from rebuilding the
    # C++ arms on every switch).
    return root / "occ-stats" if occ_stats else root


def binary_path(name: str, occ_stats: bool = False) -> Path:
    return target_root(occ_stats) / "release" / name


def build(cell: Cell, env: dict, occ_stats: bool = False) -> None:
    args = ["cargo", "build", "--release", "--manifest-path", str(CRATE), "--bin", cell.binary]
    features = list(cell.features) + (["occ-stats"] if occ_stats else [])
    if features:
        args += ["--features", ",".join(features)]
    env = dict(env)
    if occ_stats:
        env["CARGO_TARGET_DIR"] = str(target_root(True))
    proc = subprocess.run(args, env=env)
    if proc.returncode != 0:
        raise Preflight(f"building {cell.binary}{' (occ-stats)' if occ_stats else ''} "
                        "failed; no counters were collected")


def ops_of(stdout: str, cell: Cell) -> tuple[int, list[dict]]:
    """Total operations the binary reports over its own rounds, and the rows.

    The divisor is the harness's own count. Deriving a second one here would
    let the published per-operation figure disagree with the published cell.
    """
    rows = [json.loads(line) for line in stdout.splitlines() if line.startswith("{")]
    if not rows:
        raise Preflight(f"{cell.binary} emitted no JSON rows; the cell is void")
    if cell.concurrent:
        # A concurrent `counters` row carries `read_ops` (probes completed by
        # the readers) and `write_ops` (fresh keys inserted). Reader-side
        # figures divide by the first, writer-side by the second; this driver
        # never invents a third.
        rows = [r for r in rows if r.get("role") == "counters"]
        key = "read_ops" if any(r.get("readers") for r in rows) else "write_ops"
        total = sum(r.get(key) or 0 for r in rows)
        if not total:
            raise Preflight(
                f"{cell.binary} `counters` rows carry no non-zero `{key}`, so a "
                f"per-operation figure cannot be derived from the harness's own count"
            )
        return total, rows
    total = sum(r.get("ops") or 0 for r in rows)
    if not total:
        raise Preflight(f"{cell.binary} rows carry no `ops`; cannot divide")
    return total, rows


def one_repeat(cell: Cell, events: list[str], pin: list[str], pmu: str | None,
               env: dict) -> dict:
    """One `perf stat` over one whole process; returns per-op figures."""
    exe = binary_path(cell.binary)
    if not exe.is_file():
        raise Preflight(f"{exe} does not exist; build it before collecting counters")
    rc, csv_text, out, err = run_perf(events, [str(exe), *cell.args], env, pin)
    if rc != 0:
        raise Preflight(f"{cell.name}: the cell exited {rc}\n{err.strip()[:800]}")
    ops, rows = ops_of(out, cell)
    parsed = parse_perf_csv(csv_text)
    per_op, raw = {}, {}
    for ev in events:
        row = row_for(parsed, ev, pmu)
        if row is None or row["value"] is None:
            per_op[ev], raw[ev] = None, None
            continue
        # Section 8.9 principle 5: verbatim raw event row divided by the exact
        # probe count, both published.
        raw[ev] = row["value"]
        per_op[ev] = row["value"] / ops
    return {"ops": ops, "rounds": len(rows), "raw": raw, "per_op": per_op}


def interval_over(samples: list[float]) -> dict:
    """The per-op mean with its BCa 95% interval, or a note when there are too few."""
    if len(samples) < 3:
        return {"per_op_mean": (sum(samples) / len(samples)) if samples else None,
                "ci_lower": None, "ci_upper": None, "ci_method": None, "samples": samples,
                "note": "fewer than three samples counted; no interval (BCa needs a jackknife)"}
    mean, lo, hi, ci_method = bca_bootstrap_ci_with_method(samples, num_resamples=2000, seed=42)
    # `ci_method` names the construction that produced the interval
    # (`bca_bootstrap.CI_METHOD_*`, #880): anything but `bca` means one of BCa's
    # corrections degenerated on these repeats, which a counter cell states
    # rather than leaving a reader to assume (AGENTS.md §8.1).
    return {"per_op_mean": mean, "ci_lower": lo, "ci_upper": hi, "ci_method": ci_method,
            "samples": samples}


def collect(cell: Cell, repeats: int, events: list[str], pin: list[str],
            pmu: str | None, env: dict) -> dict:
    reps = [one_repeat(cell, events, pin, pmu, env) for _ in range(repeats)]
    per_event = {}
    for ev in events:
        samples = [r["per_op"][ev] for r in reps if r["per_op"].get(ev) is not None]
        per_event[ev] = interval_over(samples)
    return {
        "cell": cell.name, "issue": cell.issue, "suite": cell.suite,
        "binary": cell.binary, "args": cell.args, "note": cell.note,
        "concurrent": cell.concurrent, "mode": cell.mode,
        "repeats": repeats, "ops_per_repeat": reps[0]["ops"],
        "rounds_per_repeat": reps[0]["rounds"],
        "events": per_event,
        "repeats_raw": [{"ops": r["ops"], "raw": r["raw"], "per_op": r["per_op"]} for r in reps],
    }


# --------------------------------------------------------------------------
# per-thread mode
# --------------------------------------------------------------------------
def read_until(stream, pred, what: str, cell: Cell, stderr_path: Path) -> dict:
    """The next JSON line satisfying `pred`; EOF before it is a void cell."""
    for line in stream:
        line = line.strip()
        if not line.startswith("{"):
            continue
        obj = json.loads(line)
        if pred(obj):
            return obj
    err = stderr_path.read_text(errors="replace").strip()[:800] if stderr_path.exists() else ""
    raise Preflight(f"{cell.name}: {cell.binary} ended before it printed {what}; "
                    f"the cell is void\n{err}")


def stop_perf(perf: subprocess.Popen, what: str, cell: Cell) -> tuple[int, str]:
    """SIGINT ends an attached perf and makes it write its output."""
    if perf.poll() is None:
        perf.send_signal(signal.SIGINT)
    try:
        _out, err = perf.communicate(timeout=60)
    except subprocess.TimeoutExpired:
        perf.kill()
        raise Preflight(f"{cell.name}: {what} did not exit within 60 s of SIGINT")
    return perf.returncode, err or ""


def start_harness(cell: Cell, pin: list[str], env: dict, rounds: int,
                  stderr_path: Path) -> subprocess.Popen:
    exe = binary_path(cell.binary)
    if not exe.is_file():
        raise Preflight(f"{exe} does not exist; build it before collecting counters")
    argv = list(pin) + [str(exe), *cell.args, "--arm", cell.arm,
                        "--rounds", str(rounds), "--wait-stdin"]
    # stderr goes to a file, not a pipe: a pipe nobody drains would block the
    # harness on its first diagnostic while this driver waits on stdout.
    with open(stderr_path, "w") as err_fh:
        return subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=err_fh, text=True, bufsize=1, env=env,
        )


def finish_harness(child: subprocess.Popen, cell: Cell, stderr_path: Path) -> None:
    child.stdin.close()
    rc = child.wait(timeout=600)
    if rc != 0:
        err = stderr_path.read_text(errors="replace").strip()[:800]
        raise Preflight(f"{cell.name}: {cell.binary} exited {rc}; the cell is void\n{err}")


def one_attached_round(cell: Cell, child: subprocess.Popen, round_idx: int,
                       requested: list[str], events: list[str], pmu: str | None,
                       stderr_path: Path) -> dict:
    """Attach perf stat to the round's threads, release it, collect the rows."""
    ready = read_until(child.stdout, lambda o: o.get("event") == "threads_ready",
                       f"`threads_ready` for round {round_idx}", cell, stderr_path)
    if ready.get("round") != round_idx:
        raise Preflight(f"{cell.name}: expected threads_ready for round {round_idx}, "
                        f"got {ready}")
    pid = int(ready["pid"])
    with tempfile.NamedTemporaryFile("r+", suffix=".csv", delete=False) as fh:
        csv_path = fh.name
    perf = subprocess.Popen(
        ["perf", "stat", "--per-thread", "-p", str(pid), "-x,", "-o", csv_path,
         "-e", ",".join(requested)],
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
    )
    time.sleep(ATTACH_SETTLE_S)
    if perf.poll() is not None:
        _o, err = perf.communicate()
        raise Preflight(f"{cell.name}: perf stat --per-thread -p {pid} exited "
                        f"{perf.returncode} before the round was released; nothing "
                        f"was counted\n{err.strip()[:800]}")
    child.stdin.write("\n")
    child.stdin.flush()
    row = read_until(child.stdout, lambda o: o.get("role") == "counters",
                     f"the `counters` row for round {round_idx}", cell, stderr_path)
    rc, err = stop_perf(perf, "perf stat", cell)
    csv_text = Path(csv_path).read_text(encoding="utf-8", errors="replace")
    os.unlink(csv_path)
    threads = parse_per_thread_csv(csv_text)
    if not threads:
        raise Preflight(f"{cell.name}: perf stat --per-thread wrote no thread rows "
                        f"(rc {rc})\n{err.strip()[:800]}")
    grouped = role_totals(threads, events, pmu)
    counts = grouped["threads"]
    # The attach counts the threads that existed when it happened. Fewer
    # writer or reader rows than the harness spawned means it happened too
    # early or too late, and the round describes a subset of the work.
    for role, key in (("writer", "writers"), ("reader", "readers")):
        want, got = int(row.get(key, 0)), counts.get(role, 0)
        if want != got:
            raise Preflight(f"{cell.name} round {round_idx}: the harness spawned {want} "
                            f"{role} thread(s) but perf counted {got}; the attach did not "
                            f"see the round's threads, so the round is void")
    per_op = {}
    for role, key in ROLE_OPS_KEY.items():
        per_op[role] = per_op_for_role(grouped["totals"].get(role), int(row.get(key, 0)))
    return {
        "round": round_idx, "pid": pid,
        "read_ops": row.get("read_ops"), "write_ops": row.get("write_ops"),
        "reader_elapsed_s": row.get("reader_elapsed_s"),
        "writer_elapsed_s": row.get("writer_elapsed_s"),
        "harness_row": row,
        "threads": {k: {"comm": t["comm"], "tid": t["tid"], "role": t["role"],
                        "rows": t["rows"]} for k, t in threads.items()},
        "role_threads": counts,
        "role_raw": grouped["totals"],
        "per_op": per_op,
        "perf_rc": rc,
    }


def collect_per_thread(cell: Cell, rounds: int, requested: list[str], events: list[str],
                       pin: list[str], pmu: str | None, env: dict) -> dict:
    """R rounds of one arm, each with its own `perf stat --per-thread` attach."""
    with tempfile.NamedTemporaryFile(suffix=".stderr", delete=False) as fh:
        stderr_path = Path(fh.name)
    child = start_harness(cell, pin, env, rounds, stderr_path)
    out = []
    try:
        for i in range(rounds):
            out.append(one_attached_round(cell, child, i, requested, events, pmu, stderr_path))
        finish_harness(child, cell, stderr_path)
    finally:
        if child.poll() is None:
            child.kill()
        stderr_path.unlink(missing_ok=True)

    roles = {}
    for role, key in ROLE_OPS_KEY.items():
        n = out[0]["role_threads"].get(role, 0)
        per_event = {}
        for ev in events:
            samples = [r["per_op"][role].get(ev) for r in out
                       if r["per_op"].get(role, {}).get(ev) is not None]
            per_event[ev] = interval_over(samples)
        roles[role] = {"ops_key": key, "threads": n, "events": per_event}
    # The flat view `counter_tables.py` renders: one entry per role/event.
    flat = {f"{role}/{ev}": v for role, r in roles.items() if r["threads"]
            for ev, v in r["events"].items()}
    primary = "reader" if out[0]["role_threads"].get("reader") else "writer"
    return {
        "cell": cell.name, "issue": cell.issue, "suite": cell.suite,
        "binary": cell.binary, "args": cell.args, "arm": cell.arm, "note": cell.note,
        "concurrent": True, "mode": "per-thread",
        "repeats": rounds, "rounds": rounds,
        "ops_per_repeat": out[0][ROLE_OPS_KEY[primary]], "rounds_per_repeat": 1,
        "primary_role": primary,
        "roles": roles,
        "events": flat,
        "rounds_raw": out,
    }


def layout_rows(cell: Cell, env: dict, skip_build: bool) -> list[dict]:
    """`sync::layout_report()` from the health build, as the harness prints it."""
    if not skip_build:
        build(cell, env, occ_stats=True)
    exe = binary_path(cell.binary, occ_stats=True)
    if not exe.is_file():
        raise Preflight(f"{exe} (occ-stats build) does not exist; build it or drop --skip-build")
    proc = subprocess.run([str(exe), "--layout"], capture_output=True, text=True, env=env)
    if proc.returncode != 0:
        raise Preflight(f"{cell.name}: `{exe.name} --layout` exited {proc.returncode}\n"
                        f"{proc.stderr.strip()[:800]}")
    rows = [json.loads(l) for l in proc.stdout.splitlines() if l.startswith("{")]
    rows = [r for r in rows if r.get("role") == "layout"]
    if not rows:
        raise Preflight(f"{cell.name}: `--layout` printed no layout rows")
    return rows


def c2c_round(cell: Cell, pin: list[str], env: dict, out_dir: Path) -> dict:
    """`perf c2c record -p <pid>` over one extra round, then the text report."""
    with tempfile.NamedTemporaryFile(suffix=".stderr", delete=False) as fh:
        stderr_path = Path(fh.name)
    data = out_dir / f"c2c_{cell.name}.data"
    report = out_dir / f"c2c_{cell.name}.txt"
    child = start_harness(cell, pin, env, 1, stderr_path)
    try:
        ready = read_until(child.stdout, lambda o: o.get("event") == "threads_ready",
                           "`threads_ready` for the c2c round", cell, stderr_path)
        pid = int(ready["pid"])
        perf = subprocess.Popen(
            ["perf", "c2c", "record", "-p", str(pid), "-o", str(data)],
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
        )
        time.sleep(ATTACH_SETTLE_S)
        if perf.poll() is not None:
            _o, err = perf.communicate()
            raise Preflight(f"{cell.name}: perf c2c record -p {pid} exited "
                            f"{perf.returncode} before the round was released\n"
                            f"{err.strip()[:800]}")
        child.stdin.write("\n")
        child.stdin.flush()
        row = read_until(child.stdout, lambda o: o.get("role") == "counters",
                         "the `counters` row for the c2c round", cell, stderr_path)
        record_rc, record_err = stop_perf(perf, "perf c2c record", cell)
        finish_harness(child, cell, stderr_path)
    finally:
        if child.poll() is None:
            child.kill()
        stderr_path.unlink(missing_ok=True)
    if not data.is_file():
        raise Preflight(f"{cell.name}: perf c2c record wrote no {data} (rc {record_rc})\n"
                        f"{record_err.strip()[:800]}")
    rep = subprocess.run(["perf", "c2c", "report", "--stdio", "--full-symbols", "-i", str(data)],
                         capture_output=True, text=True)
    if rep.returncode != 0 or not rep.stdout.strip():
        raise Preflight(f"{cell.name}: perf c2c report exited {rep.returncode} or was empty\n"
                        f"{rep.stderr.strip()[:800]}")
    report.write_text(rep.stdout)
    # The raw sample file is large and machine-specific; the text report is
    # the artifact a reader inspects.
    data.unlink(missing_ok=True)
    return {
        "report": str(report.relative_to(REPO_ROOT)) if report.is_relative_to(REPO_ROOT)
        else str(report),
        "record_rc": record_rc, "report_rc": rep.returncode,
        "round": {"read_ops": row.get("read_ops"), "write_ops": row.get("write_ops"),
                  "reader_elapsed_s": row.get("reader_elapsed_s"),
                  "writer_elapsed_s": row.get("writer_elapsed_s")},
        "note": "text for a reader to inspect; nothing here is parsed into a number",
    }


def provenance(pmu, why, pin, available, unavailable, repeats, futex: dict | None) -> dict:
    prov = {
        "instrument": {
            "process": "perf stat, one invocation per repeat, whole process",
            "per-thread": "perf stat --per-thread -p <pid>, attached per round after the "
                          "harness spawned the round's threads and before its barrier "
                          f"released them ({ATTACH_SETTLE_S:.1f} s settle), stopped by "
                          "SIGINT after the harness's `counters` row",
        },
        "commit": os.environ.get("EXPANSE_BENCH_COMMIT", "unknown"),
        "perf_event_paranoid": paranoid_level(),
        "pmu": pmu, "pmu_reason": why, "pmu_cpus": pmu_cpus(pmu), "pin": pin,
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "host": host_facts(),
        "events_available": available, "events_unavailable": unavailable,
        "futex_preflight": futex,
        "repeats": repeats,
        "estimators": {
            "per_op": "the raw event row from perf's CSV divided by the harness's own "
                      "operation count (AGENTS.md section 8.9 principle 5); both are "
                      "published. Per-thread mode: writer-thread rows summed and divided "
                      "by that round's `write_ops`, reader-thread rows by its `read_ops`; "
                      "the main thread's rows are kept and attributed to neither",
            "interval": "BCa 95% bootstrap over the repeats (process mode) or rounds "
                        "(per-thread mode) (scripts/bca_bootstrap.py), 2000 resamples",
        },
        "scope": "process mode: counts are per PROCESS and include the population build; "
                 "two arms are comparable only over the same work. Per-thread mode: counts "
                 "are per THREAD over one round, from the attach to the harness's "
                 "`counters` row; the prefill is outside them",
        "attribution": "a counter is evidence for a mechanism, not the mechanism; a cell "
                       "whose intervals overlap decides nothing",
        "loads": [load_snapshot("start")],
    }
    return prov


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------
# Verbatim `perf stat --per-thread -p <pid> -x, -o <csv>
# -e cpu_core/cycles/,cpu_core/instructions/,task-clock` output from the
# reference host (perf 6.8.12, Linux 6.8) against a pinned C process whose two
# spinning threads were named `writer-0` and `reader-0` through
# `pthread_setname_np`; `perthread-<pid>` is the main thread. The process then
# spawned a third thread, `late-0`, 1.5 s after perf attached: it has no row
# here, which is the attach-time enumeration the harness handshake exists for.
# Rows are `<comm>-<tid>,<value>,<unit>,<event>,<runtime>,<pct>,<metric>,<unit>`.
PER_THREAD_FIXTURE = (
    "# started on Tue Sep  8 06:48:46 2026\n"
    "\n"
    "perthread-3515545,8054099575,,cpu_core/cycles/,1604452556,100.00,5.020,GHz\n"
    "writer-0-3515547,8271740307,,cpu_core/cycles/,1692085641,100.00,4.889,GHz\n"
    "reader-0-3515548,8271632107,,cpu_core/cycles/,1692019932,100.00,4.889,GHz\n"
    "perthread-3515545,6491391887,,cpu_core/instructions/,1604457908,100.00,4.046,G/sec\n"
    "writer-0-3515547,6378556346,,cpu_core/instructions/,1692077311,100.00,3.770,G/sec\n"
    "reader-0-3515548,6356945597,,cpu_core/instructions/,1692012158,100.00,3.757,G/sec\n"
    "perthread-3515545,1604.47,msec,task-clock,1604466757,100.00,0.574,CPUs utilized\n"
    "writer-0-3515547,1692.07,msec,task-clock,1692066734,100.00,0.605,CPUs utilized\n"
    "reader-0-3515548,1692.00,msec,task-clock,1692001865,100.00,0.605,CPUs utilized\n"
)
# The same program with perf attached *after* `late-0` existed: every thread
# has a row, and `late-0` is neither a writer nor a reader.
PER_THREAD_LATE_FIXTURE = (
    "# started on Tue Sep  8 06:48:51 2026\n"
    "\n"
    "perthread-3515568,15393,,cpu_core/cycles/,27892,100.00,,\n"
    "writer-0-3515570,874026758,,cpu_core/cycles/,178791860,100.00,,\n"
    "reader-0-3515571,874233862,,cpu_core/cycles/,178828690,100.00,,\n"
    "late-0-3515585,4951107719,,cpu_core/cycles/,982189028,100.00,,\n"
)


def _self_test() -> int:
    failures = []

    # The CSV a hybrid host actually returns, from a real run on the reference
    # host: one requested event, two rows, no bare row.
    hybrid = (
        "62,,page-faults,173176999,100.00,,\n"
        "100166,,cpu_atom/dTLB-load-misses/,527314,0.00,,\n"
        "654,,cpu_core/dTLB-load-misses/,172649685,99.00,,\n"
        "133119427,,cpu_atom/cycles/,527314,0.00,,\n"
        "812254473,,cpu_core/cycles/,172649685,99.00,,\n"
    )
    parsed = parse_perf_csv(hybrid)
    if row_for(parsed, "cycles", "cpu_core")["value"] != 812254473.0:
        failures.append("the cpu_core row was not selected for `cycles`")
    if row_for(parsed, "cycles", "cpu_atom")["value"] != 133119427.0:
        failures.append("the cpu_atom row was not selected for `cycles`")
    # Never summed: the two PMUs count different microarchitectures.
    if row_for(parsed, "cycles", "cpu_core")["value"] == 812254473.0 + 133119427.0:
        failures.append("the two PMU rows were summed")
    # An unqualified row on a host with one core PMU still resolves.
    if row_for(parsed, "page-faults", "cpu_core")["value"] != 62.0:
        failures.append("an unqualified row did not resolve for the selected PMU")

    # An unsupported counter is None, never 0 — a zero reads as "measured, and
    # there were none".
    unsup = parse_perf_csv("<not supported>,,mem_load_l3_hit_retired.xsnp_hitm,0,0.00,,\n")
    row = row_for(unsup, CONCURRENT_EVENT, None)
    if row is None or row["value"] is not None or row["status"] != "<not supported>":
        failures.append(f"an unsupported counter did not stay None: {row}")

    # ---- event names on a hybrid host ----------------------------------
    if qualify("cycles", "cpu_core") != "cpu_core/cycles/":
        failures.append("a hardware event was not qualified with the pinned PMU")
    if qualify("task-clock", "cpu_core") != "task-clock":
        failures.append("a software event was PMU-qualified")
    if qualify(FUTEX_EVENT, "cpu_core") != FUTEX_EVENT:
        failures.append("a tracepoint was PMU-qualified")
    if qualify("cycles", None) != "cycles":
        failures.append("a uniform host's event was qualified")

    # ---- per-thread CSV: parse, group by role, divide by the right count --
    threads = parse_per_thread_csv(PER_THREAD_FIXTURE)
    if set(threads) != {"writer-0-3515547", "reader-0-3515548", "perthread-3515545"}:
        failures.append(f"per-thread rows were not keyed by comm-tid: {sorted(threads)}")
    else:
        w0 = threads["writer-0-3515547"]
        if w0["role"] != "writer" or w0["tid"] != 3515547 or w0["comm"] != "writer-0":
            failures.append("writer-0 was not grouped as a writer with its comm and tid")
        if threads["reader-0-3515548"]["role"] != "reader":
            failures.append("reader-0 was not grouped as a reader")
        if threads["perthread-3515545"]["role"] != "other":
            failures.append("the main thread was attributed to a role")
    late = parse_per_thread_csv(PER_THREAD_LATE_FIXTURE)
    if len(late) != 4 or late.get("late-0-3515585", {}).get("role") != "other":
        failures.append(f"a late, unnamed-role thread was not kept as `other`: {sorted(late)}")
    events = ["cycles", "instructions", "task-clock"]
    grouped = role_totals(threads, events, "cpu_core")
    totals = grouped["totals"]
    if grouped["threads"] != {"writer": 1, "reader": 1, "other": 1}:
        failures.append(f"role thread counts wrong: {grouped['threads']}")
    if totals.get("writer", {}).get("cycles") != 8271740307.0:
        failures.append(f"writer cycles should be the writer row alone: {totals.get('writer')}")
    if totals.get("reader", {}).get("cycles") != 8271632107.0:
        failures.append(f"reader cycles should be the reader row alone: {totals.get('reader')}")
    # Fail-then-pass for the grouping: a driver that bypassed the role split
    # would hand every thread's rows to one role, and the writer total would
    # then be the process total. Pin that this total is NOT what a role gets.
    bypass = role_totals({k: dict(t, role="writer") for k, t in threads.items()},
                         events, "cpu_core")["totals"]["writer"]["cycles"]
    if bypass != 8054099575.0 + 8271740307.0 + 8271632107.0:
        failures.append(f"the bypass control did not sum every thread: {bypass}")
    if totals["writer"]["cycles"] == bypass:
        failures.append("the writer total equals the process total: the role grouping "
                        "was bypassed")
    # Software rows are unqualified and still resolve per thread.
    if totals["reader"]["task-clock"] != 1692.00:
        failures.append(f"an unqualified software row did not resolve per thread: "
                        f"{totals['reader']}")
    # The divisor is the role's own count: writers by write_ops, readers by
    # read_ops, and a role with no operations gets None, never a zero figure.
    w = per_op_for_role(totals["writer"], 1024)
    r = per_op_for_role(totals["reader"], 512)
    if w["cycles"] != 8271740307.0 / 1024 or r["cycles"] != 8271632107.0 / 512:
        failures.append(f"per-op division used the wrong count: {w} {r}")
    if any(v is not None for v in per_op_for_role(totals["writer"], 0).values()):
        failures.append("a role with zero operations produced a per-op figure")
    # A thread with an uncounted row makes the role total None, not a partial sum.
    partial = parse_per_thread_csv(PER_THREAD_FIXTURE +
                                   "reader-1-3515549,<not counted>,,cpu_core/cycles/,0,0.00,,\n")
    if role_totals(partial, ["cycles"], "cpu_core")["totals"]["reader"]["cycles"] is not None:
        failures.append("a partially-counted role summed to a number")

    # ---- the cell registry --------------------------------------------
    for cell in CELLS:
        src = CRATE.parent / "src" / "bin" / f"{cell.binary}.rs"
        if not src.is_file():
            failures.append(f"cell {cell.name} names a missing binary source {src}")
    if len(BY_NAME) != len(CELLS):
        failures.append("two cells share a name")
    # The gate names four cells from #724/#725/#730 plus the HOT lookup cell,
    # and #568 adds sixteen per-thread cells (ten attribution cells, three
    # readers-alone controls, and three PR 5 multi-writer mechanism cells).
    for issue, want in ((724, 2), (725, 3), (730, 2), (737, 1), (568, 16)):
        got = sum(1 for c in CELLS if c.issue == issue)
        if got != want:
            failures.append(f"expected {want} cell(s) for #{issue}, found {got}")
    conc = [c for c in CELLS if c.concurrent]
    if not conc or any(CONCURRENT_EVENT not in c.events() for c in conc):
        failures.append("a concurrent cell does not request the snoop-hit counter")
    for c in conc:
        if c.blocked:
            failures.append(f"{c.name} is still marked blocked; `read_ops` exists now")
        if c.mode != "per-thread" or not c.arm:
            failures.append(f"{c.name} is concurrent but has no per-thread arm")
        if c.events() != THREAD_EVENTS:
            failures.append(f"{c.name} does not request the per-thread event set")
    for name in ("masstree_conc_map_w1_r8", "hot_conc_map_w1_r8"):
        if not (BY_NAME[name].c2c and BY_NAME[name].layout):
            failures.append(f"{name} is not the c2c + layout cell")
    if sum(1 for c in CELLS if c.c2c) != 2:
        failures.append("exactly two cells carry the perf c2c round")
    if CONCURRENT_EVENT in BY_NAME["hot_lookup_random_1m"].events():
        failures.append("a single-threaded cell requested the snoop-hit counter")
    if BY_NAME["hot_lookup_random_1m"].mode != "process":
        failures.append("a single-threaded cell is not in process mode")
    if FUTEX_EVENT in THREAD_EVENTS:
        failures.append("the futex tracepoint must be added by its own preflight, not by default")

    for m in failures:
        print(f"  FAIL {m}")
    if failures:
        print(f"bench_counters.py --self-test: {len(failures)} failure(s)")
        return 1
    print("bench_counters.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--cell", action="append", default=[])
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--repeats", type=int, default=7,
                    help="process mode: perf invocations; per-thread mode: harness rounds")
    ap.add_argument("--out-dir", default=None)
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--skip-build", action="store_true")
    ap.add_argument("--no-c2c", action="store_true",
                    help="skip the perf c2c round on the cells that carry one")
    args = ap.parse_args()

    if args.self_test:
        return _self_test()
    if args.list:
        for c in CELLS:
            extra = f" [{c.mode}, --arm {c.arm}{', c2c' if c.c2c else ''}]" if c.concurrent else ""
            print(f"  {c.name:36} #{c.issue}  {c.suite:22} {c.binary} {' '.join(c.args)}{extra}")
        return 0

    # After --list and --self-test, which measure nothing, and before any cell
    # runs. This script is invoked directly, so no runner pinned it (#779).
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import bench_pin  # noqa: PLC0415

    bench_pin.apply("bench_counters.py")

    if args.all:
        names = [c.name for c in CELLS if not c.blocked]
        for c in CELLS:
            if c.blocked:
                print(f"::warning::bench_counters.py: {c.name} (#{c.issue}) is BLOCKED and "
                      f"was not run: {c.blocked}")
    else:
        names = args.cell
    if not names:
        print("nothing to do: pass --cell <name>, --all, or --list", file=sys.stderr)
        return 2
    unknown = [n for n in names if n not in BY_NAME]
    if unknown:
        print(f"unknown cell(s): {', '.join(unknown)}", file=sys.stderr)
        return 2

    cells = [BY_NAME[n] for n in names]
    for c in cells:
        if c.blocked and not args.all:
            print(f"::error::bench_counters.py: {c.name} is blocked: {c.blocked}",
                  file=sys.stderr)
            return 1
    env = dict(os.environ)
    env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-cpu=haswell"

    order = BASE_EVENTS + [e for e in THREAD_EVENTS if e not in BASE_EVENTS]
    events = sorted({e for c in cells for e in c.events()}, key=order.index)
    try:
        pmu, why, pin, available, unavailable = preflight(events)
    except Preflight as exc:
        print(f"::error::bench_counters.py: {exc}", file=sys.stderr)
        return 1
    print(f"PMU {pmu} ({why}); pin {' '.join(pin) or 'none'}; "
          f"paranoid={paranoid_level()}")
    if unavailable:
        for u in unavailable:
            print(f"::warning::event unavailable on this host: {u['event']} ({u['reason']})")
    futex = None
    if any(c.concurrent for c in cells):
        futex = futex_preflight()
        print(f"futex tracepoint: {'available' if futex['available'] else 'UNAVAILABLE'} "
              f"({futex['reason']})")
        if futex["available"]:
            available = available + [FUTEX_EVENT]
        else:
            unavailable = unavailable + [{"event": FUTEX_EVENT, "reason": futex["reason"]}]

    prov = provenance(pmu, why, pin, available, unavailable, args.repeats, futex)
    results = []
    for cell in cells:
        try:
            if not args.skip_build:
                build(cell, env)
            evs = [e for e in cell.events() if e in available]
            if cell.concurrent and FUTEX_EVENT in available:
                evs.append(FUTEX_EVENT)
            print(f"\n[{cell.name}] {cell.binary} {' '.join(cell.args)}"
                  + (f" --arm {cell.arm} ({cell.mode})" if cell.concurrent else ""))
            add_load(prov, f"before {cell.name}")
            if cell.concurrent:
                requested = [qualify(e, pmu) for e in evs]
                res = collect_per_thread(cell, args.repeats, requested, evs, pin, pmu, env)
            else:
                res = collect(cell, args.repeats, evs, pin, pmu, env)
            add_load(prov, f"after {cell.name}")
            suite_dir = REPO_ROOT / "docs" / "benchmarks" / res["suite"] / "results"
            out_dir = Path(args.out_dir) if args.out_dir else suite_dir
            out_dir.mkdir(parents=True, exist_ok=True)
            if cell.layout:
                res["layout"] = layout_rows(cell, env, args.skip_build)
            if cell.c2c and not args.no_c2c:
                res["c2c"] = c2c_round(cell, pin, env, out_dir)
                add_load(prov, f"after {cell.name} c2c")
        except Preflight as exc:
            print(f"::error::bench_counters.py: {cell.name}: {exc}", file=sys.stderr)
            return 1
        for ev, v in res["events"].items():
            if v["per_op_mean"] is None:
                continue
            ci = ("" if v["ci_lower"] is None
                  else f" [{v['ci_lower']:.4g}, {v['ci_upper']:.4g}]")
            print(f"  {ev:40} {v['per_op_mean']:>12.4g} /op{ci}")
        results.append(res)
        # Written now, not at the end: a later cell that fails loud must not
        # discard the measurements already taken.
        path = out_dir / f"counters_{res['cell']}.json"
        path.write_text(json.dumps({"provenance": prov, **res}, indent=2) + "\n")
        print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
