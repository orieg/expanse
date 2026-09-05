#!/usr/bin/env python3
"""Why does criterion's warm-up move the E-core arm and not the P-core one? (#679)

#672 measured, on the hybrid reference host, that the same criterion arm pinned
to the efficiency cores reads 17.8% slower at a 2-second warm-up than at the
3-second default, while the performance-core arm does not move at all. The
obvious hypothesis is frequency ramp — the E-cores take longer to reach their
steady clock than the warm-up lasts — and AGENTS.md §8.9 forbids stating it
without a counter. This script is that counter.

It runs ONE criterion arm under `perf stat -I <ms>` on the core class's own
PMU (`cpu_core` or `cpu_atom`), reading `cycles`, `instructions` and
`ref-cycles` per interval, and concurrently samples the kernel's
`scaling_cur_freq` for the pinned CPUs at the same cadence. Two quantities
fall out of the timeline, and they separate the two candidate mechanisms:

  * **effective frequency** — `cycles / ref-cycles` (the core clock relative to
    the TSC-nominal clock, independent of how much of each interval the task
    was actually running), cross-checked against cpufreq's own report;
  * **instructions per cycle** — the per-cycle throughput.

If the warm-up effect is frequency ramp, the E-core frequency is still rising
when a short warm-up ends and the measured window averages a lower clock, with
IPC unchanged between the two warm-up settings. If it is something else, the
frequency is flat at both settings and IPC, or criterion's own iteration
plan, is what differs. The script reports both per window and does not pick.

    python3 scripts/warmup_ramp.py --bench domain -p expanse-trie \\
        --filter raw_expanse_set_intersection/100000 \\
        --warm-up-times 2,3 --reps 3 --json results/warmup_ramp_679.json
    python3 scripts/warmup_ramp.py --self-test

Cells (core class × warm-up) are interleaved with a rotating lead so a drift
over the session does not load onto one cell. Criterion's own report for each
run — the sample plan it chose and the time it measured — is parsed from its
stdout and stored beside the counters, so the wall-clock figure and the
counters that explain it come from the same process.

Windows are approximate by construction: criterion runs its warm-up in
doubling batches until at least the requested time has elapsed, so the warm-up
ends somewhere in [W, ~1.5 W] after the arm starts. The summary therefore
reports (a) the frequency ramp from the first busy interval, (b) the mean over
`[busy_start, busy_start + W]` — the interval the warm-up certainly covers —
and (c) the mean over the last `--tail-seconds` of the run, which is certainly
inside the measured region. Both windows are stated; neither is a claim about
where criterion drew its boundary.

Preflight is fail-loud (§8.1): a non-hybrid host, no `perf`, no `taskset`, an
inherited affinity mask, or an event the PMU refuses to count all stop the run.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import statistics
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from pin_exposure import (  # noqa: E402
    E_CORE_PMU,
    P_CORE_PMU,
    Preflight,
    parse_cpu_list,
    preflight,
    resolve_provenance,
    round_order,
    toolchain,
)

SCHEMA = "expanse.warmup_ramp.v1"
ISSUE_URL = "https://github.com/orieg/expanse/issues/679"
EVENTS = ("cycles", "instructions", "ref-cycles")
CPUFREQ_ROOT = Path("/sys/devices/system/cpu")
DEFAULT_MEASUREMENT_S = 5
CLASSES: Dict[str, str] = {"p_cores": P_CORE_PMU, "e_cores": E_CORE_PMU}

# `perf stat -x, -I` rows: time,count,unit,event,run_ns,pct[,metric,unit]
PERF_ROW = re.compile(r"^\s*(?P<t>\d+\.\d+),(?P<count>[^,]*),(?P<unit>[^,]*),(?P<event>[^,]+),")
# criterion's plan and result lines
CRIT_WARMUP = re.compile(r"Warming up for (?P<s>\d+(?:\.\d+)?) s")
CRIT_PLAN = re.compile(
    r"Collecting (?P<samples>\d+) samples in estimated (?P<s>\d+(?:\.\d+)?) s \((?P<iters>[\d.]+)(?P<mag>[kMB]?) iterations\)"
)
CRIT_TIME = re.compile(
    r"time:\s*\[(?P<lo>[\d.]+) (?P<lou>\S+) (?P<mid>[\d.]+) (?P<midu>\S+) (?P<hi>[\d.]+) (?P<hiu>\S+)\]"
)
UNIT_NS = {"ps": 1e-3, "ns": 1.0, "µs": 1e3, "us": 1e3, "ms": 1e6, "s": 1e9}
MAG = {"": 1.0, "k": 1e3, "M": 1e6, "B": 1e9}


def fail(msg: str) -> None:
    print(f"warmup_ramp.py: {msg}", file=sys.stderr)
    sys.exit(1)


# --------------------------------------------------------------------------- parsing
def parse_perf_csv(text: str) -> List[Dict[str, Any]]:
    """`perf stat -x, -I` stderr -> one row per interval with every event's count.

    A `<not counted>` or `<not supported>` cell is a refusal: the PMU did not
    answer, and an interval with a missing event cannot be read (§8.1).
    """
    by_t: Dict[float, Dict[str, float]] = {}
    for line in text.splitlines():
        m = PERF_ROW.match(line)
        if not m:
            continue
        count = m.group("count").strip()
        event = m.group("event").strip().strip("/")
        event = event.rsplit("/", 1)[-1] if "/" in event else event  # cpu_atom/cycles/ -> cycles
        if count.startswith("<"):
            raise ValueError(f"perf reported {count!r} for {event} at t={m.group('t')}s")
        by_t.setdefault(float(m.group("t")), {})[event] = float(count)
    rows: List[Dict[str, Any]] = []
    for t in sorted(by_t):
        ev = by_t[t]
        missing = [e for e in EVENTS if e not in ev]
        if missing:
            raise ValueError(f"perf interval at t={t}s lacks {missing}")
        cyc, ins, ref = ev["cycles"], ev["instructions"], ev["ref-cycles"]
        rows.append(
            {
                "t": t,
                "cycles": cyc,
                "instructions": ins,
                "ref_cycles": ref,
                "freq_ratio": (cyc / ref) if ref else None,
                "ipc": (ins / cyc) if cyc else None,
            }
        )
    return rows


def parse_criterion(text: str) -> Dict[str, Any]:
    """The plan criterion chose and the time it reported, from its stdout."""
    out: Dict[str, Any] = {}
    m = CRIT_WARMUP.search(text)
    if m:
        out["warm_up_s"] = float(m.group("s"))
    m = CRIT_PLAN.search(text)
    if m:
        out["samples"] = int(m.group("samples"))
        out["estimated_s"] = float(m.group("s"))
        out["iterations"] = float(m.group("iters")) * MAG[m.group("mag")]
    m = CRIT_TIME.search(text)
    if m:
        out["time_ns"] = [
            float(m.group("lo")) * UNIT_NS[m.group("lou")],
            float(m.group("mid")) * UNIT_NS[m.group("midu")],
            float(m.group("hi")) * UNIT_NS[m.group("hiu")],
        ]
    return out


def tsc_nominal_ghz() -> Optional[float]:
    """The `@ N.NNGHz` of the model name, which is the TSC-nominal clock ref-cycles counts at."""
    try:
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.startswith("model name"):
                m = re.search(r"@\s*(\d+(?:\.\d+)?)\s*GHz", line)
                return float(m.group(1)) if m else None
    except OSError:
        return None
    return None


# --------------------------------------------------------------------------- summary
def summarise(
    rows: Sequence[Dict[str, Any]], warm_up_s: float, tail_s: float, interval_s: float
) -> Dict[str, Any]:
    """Ramp and per-window means from one run's timeline.

    Phases are told apart by `ref-cycles`, which counts at the TSC-nominal
    rate whenever the task runs and so says "busy" independently of the clock
    the core is at — a `cycles` threshold would miss the early, low-clock part
    of exactly the ramp this script exists to see. The single-core reference
    is the *median* ref-cycles of the non-idle intervals: criterion ends every
    run with a multi-threaded bootstrap analysis whose ref-cycles run at
    several cores' worth, and a threshold taken from the *peak* would call
    only that burst busy (the first version of this function did). Intervals
    above 1.5× the single-core reference are that analysis phase and are
    excluded from every window; the tail window is the last `tail_s` of
    single-threaded busy intervals, which is inside criterion's measurement.
    The last interval is dropped because `perf` cuts it short at exit.
    """
    if len(rows) < 3:
        raise ValueError("timeline too short to summarise")
    body = list(rows[:-1])
    peak = max(r["ref_cycles"] for r in body)
    # The floor only has to exclude idle intervals. It must sit well below one
    # core's share of the peak: on the P-cores criterion's analysis burst runs
    # 16 threads, so a 10% floor put every single-threaded interval below it
    # and the median landed on the burst (the second defect this function had).
    active = [r["ref_cycles"] for r in body if r["ref_cycles"] >= 0.01 * peak]
    if not active:
        raise ValueError("no active interval")
    single_core_ref = statistics.median(active)
    busy = [r for r in body if 0.5 * single_core_ref <= r["ref_cycles"] <= 1.5 * single_core_ref]
    analysis = [r for r in body if r["ref_cycles"] > 1.5 * single_core_ref]
    if not busy:
        raise ValueError("no single-threaded busy interval")
    # criterion's analysis comes after measurement; anything single-threaded
    # after it (process teardown) is not the arm and is dropped too
    if analysis:
        first_analysis_t = analysis[0]["t"]
        busy = [r for r in busy if r["t"] < first_analysis_t]
        if not busy:
            raise ValueError("no single-threaded busy interval before the analysis phase")
    t0 = busy[0]["t"] - interval_s
    steady_window = [r for r in busy if r["t"] > busy[-1]["t"] - tail_s]
    steady = statistics.median(r["freq_ratio"] for r in steady_window if r["freq_ratio"])
    ramp_to_95 = None
    for r in busy:
        if r["freq_ratio"] is not None and r["freq_ratio"] >= 0.95 * steady:
            ramp_to_95 = round(r["t"] - t0, 3)
            break
    warm = [r for r in busy if r["t"] - t0 <= warm_up_s]
    tail = steady_window

    def mean_of(win: Sequence[Dict[str, Any]], key: str) -> Optional[float]:
        vals = [r[key] for r in win if r.get(key) is not None]
        return statistics.fmean(vals) if vals else None

    return {
        "busy_start_s": round(t0, 3),
        "busy_intervals": len(busy),
        "analysis_intervals_excluded": len(analysis),
        "single_core_ref_cycles_per_interval": single_core_ref,
        "steady_freq_ratio": steady,
        "ramp_to_95pct_of_steady_s": ramp_to_95,
        "warmup_window_s": [round(t0, 3), round(t0 + warm_up_s, 3)],
        "warmup_mean_freq_ratio": mean_of(warm, "freq_ratio"),
        "warmup_mean_ipc": mean_of(warm, "ipc"),
        "warmup_mean_instructions_per_interval": mean_of(warm, "instructions"),
        "tail_window_s": [round(busy[-1]["t"] - tail_s, 3), round(busy[-1]["t"], 3)],
        "tail_mean_freq_ratio": mean_of(tail, "freq_ratio"),
        "tail_mean_ipc": mean_of(tail, "ipc"),
        "tail_mean_instructions_per_interval": mean_of(tail, "instructions"),
        "total_cycles": sum(r["cycles"] for r in busy),
        "total_instructions": sum(r["instructions"] for r in busy),
    }


def cell_key(cls: str, warm_up_s: float) -> str:
    return f"{cls}@{warm_up_s:g}s"


def aggregate(runs: Sequence[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Per (class, warm-up): the mean over reps of each summary quantity and criterion's mid time."""
    cells: Dict[str, List[Dict[str, Any]]] = {}
    for r in runs:
        cells.setdefault(cell_key(r["core_class"], r["warm_up_s"]), []).append(r)
    out: List[Dict[str, Any]] = []
    for key in sorted(cells):
        rs = cells[key]

        def mean(path: Tuple[str, ...]) -> Optional[float]:
            vals = []
            for r in rs:
                v: Any = r
                for p in path:
                    v = v.get(p) if isinstance(v, dict) else None
                if isinstance(v, list):
                    v = v[1]
                if v is not None:
                    vals.append(float(v))
            return statistics.fmean(vals) if vals else None

        out.append(
            {
                "cell": key,
                "core_class": rs[0]["core_class"],
                "warm_up_s": rs[0]["warm_up_s"],
                "reps": len(rs),
                "criterion_time_ns_mid_mean": mean(("criterion", "time_ns")),
                "criterion_iterations_mean": mean(("criterion", "iterations")),
                "ramp_to_95pct_s_mean": mean(("summary", "ramp_to_95pct_of_steady_s")),
                "warmup_mean_freq_ratio": mean(("summary", "warmup_mean_freq_ratio")),
                "tail_mean_freq_ratio": mean(("summary", "tail_mean_freq_ratio")),
                "warmup_mean_ipc": mean(("summary", "warmup_mean_ipc")),
                "tail_mean_ipc": mean(("summary", "tail_mean_ipc")),
                "cpufreq_tail_mean_khz": mean(("cpufreq_summary", "tail_mean_khz")),
                "cpufreq_max_khz": mean(("cpufreq_summary", "max_khz")),
            }
        )
    return out


def render(cells: Sequence[Dict[str, Any]], meta: Dict[str, Any]) -> str:
    ghz = meta.get("tsc_nominal_ghz")

    def freq(ratio: Optional[float]) -> str:
        if ratio is None:
            return "—"
        return f"{ratio:.3f}" + (f" ({ratio * ghz:.2f} GHz)" if ghz else "")

    def num(v: Optional[float], fmt: str) -> str:
        return "—" if v is None else format(v, fmt)

    out = [
        f"### Warm-up ramp on {meta['host']} — `{meta['bench']}` `{meta['filter']}`, {meta['reps']} interleaved reps",
        "",
        "Per cell: criterion's own measured time (mid of its interval, mean over reps), the sample plan it chose, "
        "the effective core clock as `cycles / ref-cycles` (relative to TSC-nominal) averaged over the warm-up "
        "window and over the run's tail, the IPC over the same two windows, the time from first busy interval to "
        "95% of the steady clock, and cpufreq's own `scaling_cur_freq` over the tail.",
        "",
        "| cell | criterion ns | iterations | ramp→95% s | freq warm-up | freq tail | IPC warm-up | IPC tail | cpufreq tail |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for c in cells:
        out.append(
            f"| `{c['cell']}` | {num(c['criterion_time_ns_mid_mean'], ',.1f')} | "
            f"{num(c['criterion_iterations_mean'], ',.0f')} | {num(c['ramp_to_95pct_s_mean'], '.2f')} | "
            f"{freq(c['warmup_mean_freq_ratio'])} | {freq(c['tail_mean_freq_ratio'])} | "
            f"{num(c['warmup_mean_ipc'], '.3f')} | {num(c['tail_mean_ipc'], '.3f')} | "
            f"{num(c['cpufreq_tail_mean_khz'], ',.0f')} kHz |"
        )
    out += [
        "",
        "Reading: if the short warm-up's `freq warm-up` is below its `freq tail` while `IPC` matches across "
        "warm-ups, the clock was still ramping when measurement began. If `freq` is flat and `IPC` or the "
        "iteration plan differs, the effect is not the clock.",
    ]
    return "\n".join(out) + "\n"


# --------------------------------------------------------------------------- running
class CpufreqSampler:
    """Samples `scaling_cur_freq` for a CPU set at a fixed cadence on a thread."""

    def __init__(self, cpus: Sequence[int], interval_s: float) -> None:
        self.paths = [CPUFREQ_ROOT / f"cpu{c}" / "cpufreq" / "scaling_cur_freq" for c in cpus]
        self.interval_s = interval_s
        self.samples: List[Tuple[float, int]] = []
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, daemon=True)
        self.t0 = time.monotonic()

    def _read_max(self) -> int:
        best = 0
        for p in self.paths:
            try:
                best = max(best, int(p.read_text().strip()))
            except (OSError, ValueError):
                continue
        return best

    def _run(self) -> None:
        while not self._stop.is_set():
            self.samples.append((round(time.monotonic() - self.t0, 3), self._read_max()))
            self._stop.wait(self.interval_s)

    def __enter__(self) -> "CpufreqSampler":
        self._thread.start()
        return self

    def __exit__(self, *exc: Any) -> None:
        self._stop.set()
        self._thread.join(timeout=2)

    def summary(self, tail_s: float) -> Dict[str, Any]:
        if not self.samples:
            return {"n": 0}
        t_end = self.samples[-1][0]
        tail = [k for t, k in self.samples if t > t_end - tail_s and k > 0]
        return {
            "n": len(self.samples),
            "max_khz": max(k for _, k in self.samples),
            "tail_mean_khz": statistics.fmean(tail) if tail else None,
        }


def perf_argv(pmu: str, cpus: str, interval_ms: int, bench_argv: Sequence[str]) -> List[str]:
    events = ",".join(f"{pmu}/{e}/" for e in EVENTS)
    return ["perf", "stat", "-I", str(interval_ms), "-x", ",", "-e", events, "--", "taskset", "-c", cpus] + list(bench_argv)


def bench_argv(bench: str, package: str, filter_: str, warm_up_s: float, measurement_s: float) -> List[str]:
    return [
        "cargo", "bench", "--bench", bench, "-p", package, "--",
        filter_, "--warm-up-time", f"{warm_up_s:g}", "--measurement-time", f"{measurement_s:g}",
    ]


def perf_can_count(pmu: str, cpus: str) -> None:
    argv = ["perf", "stat", "-x", ",", "-e", ",".join(f"{pmu}/{e}/" for e in EVENTS), "--", "taskset", "-c", cpus, "true"]
    proc = subprocess.run(argv, capture_output=True, text=True)
    if proc.returncode != 0:
        raise Preflight(f"`{' '.join(argv)}` exited {proc.returncode}: {proc.stderr.strip()[:300]}")
    if "<not" in proc.stderr:
        raise Preflight(f"the `{pmu}` PMU refuses one of {EVENTS}: {proc.stderr.strip()[:300]}")


def run_cell(
    cls: str, cpus: str, warm_up_s: float, args: argparse.Namespace, rep: int
) -> Dict[str, Any]:
    pmu = CLASSES[cls]
    interval_s = args.interval_ms / 1000.0
    argv = perf_argv(pmu, cpus, args.interval_ms, bench_argv(args.bench, args.package, args.filter, warm_up_s, args.measurement_time))
    print(f"rep {rep + 1}/{args.reps}: {cell_key(cls, warm_up_s)}  {' '.join(argv)}", file=sys.stderr)
    with CpufreqSampler(sorted(parse_cpu_list(cpus)), interval_s) as sampler:
        proc = subprocess.run(argv, cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr[-4000:])
        fail(f"cell {cell_key(cls, warm_up_s)} exited {proc.returncode}")
    try:
        rows = parse_perf_csv(proc.stderr)
        summary = summarise(rows, warm_up_s, args.tail_seconds, interval_s)
    except ValueError as e:
        fail(f"cell {cell_key(cls, warm_up_s)}: {e}")
        raise
    # criterion prints `time:` on stdout but its progress lines ("Warming up",
    # "Collecting … samples") on stderr, where perf's CSV also lands.
    crit = parse_criterion(proc.stdout + "\n" + proc.stderr)
    if "time_ns" not in crit:
        fail(f"cell {cell_key(cls, warm_up_s)}: criterion reported no `time:` line — was the filter `{args.filter}` matched by exactly one arm?")
    return {
        "core_class": cls,
        "pmu": pmu,
        "cpus": cpus,
        "warm_up_s": warm_up_s,
        "rep": rep,
        "argv": argv,
        "criterion": crit,
        "criterion_output": [
            ln for ln in (proc.stdout + "\n" + proc.stderr).splitlines()
            if ln.strip() and not PERF_ROW.match(ln)
        ][-16:],
        "summary": summary,
        "cpufreq_summary": sampler.summary(args.tail_seconds),
        "perf_timeline": rows,
        "cpufreq_timeline": sampler.samples,
    }


def build_first(bench: str, package: str) -> None:
    """Compile outside the measured window, so the timeline starts at the arm."""
    proc = subprocess.run(["cargo", "bench", "--no-run", "--bench", bench, "-p", package], cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr[-4000:])
        fail(f"`cargo bench --no-run --bench {bench}` exited {proc.returncode}")


# --------------------------------------------------------------------------- self-test
def self_test() -> None:
    # 1. perf CSV: hybrid PMU event names reduce to bare names, three events per interval
    csv = (
        "     0.100102378,314159425,,cpu_atom/cycles/,99231621,100.00,,\n"
        "     0.100102378,240010029,,cpu_atom/ref-cycles/,99235643,100.00,,\n"
        "     0.100102378,120000000,,cpu_atom/instructions/,99235643,100.00,,\n"
        "     0.200274244,379765008,,cpu_atom/cycles/,100170307,100.00,,\n"
        "     0.200274244,242332461,,cpu_atom/ref-cycles/,100170547,100.00,,\n"
        "     0.200274244,150000000,,cpu_atom/instructions/,100170547,100.00,,\n"
    )
    rows = parse_perf_csv(csv)
    assert [r["t"] for r in rows] == [0.100102378, 0.200274244], rows
    assert abs(rows[1]["freq_ratio"] - 379765008 / 242332461) < 1e-9
    assert abs(rows[0]["ipc"] - 120000000 / 314159425) < 1e-9
    for bad in ("0.1,<not counted>,,cpu_atom/cycles/,0,0,,\n", "0.1,5,,cpu_atom/cycles/,1,100,,\n"):
        try:
            parse_perf_csv(bad)
            raise AssertionError(f"{bad!r} must refuse")
        except ValueError:
            pass

    # 2. criterion stdout: plan and result, with unit conversion
    out = (
        "Benchmarking domain_set_algebra_overhead/raw_expanse_set_intersection/100000: Warming up for 2.0000 s\n"
        "Benchmarking ...: Collecting 100 samples in estimated 5.0631 s (355k iterations)\n"
        "domain_set_algebra_overhead/raw_expanse_set_intersection/100000\n"
        "                        time:   [14.021 µs 14.049 µs 14.078 µs]\n"
    )
    c = parse_criterion(out)
    assert c["warm_up_s"] == 2.0 and c["samples"] == 100 and c["iterations"] == 355_000, c
    assert [round(v) for v in c["time_ns"]] == [14021, 14049, 14078], c
    assert parse_criterion("time:   [1.2 ms 1.3 ms 1.4 ms]")["time_ns"][1] == 1.3e6
    assert "time_ns" not in parse_criterion("nothing here")

    # 3. summary on a synthetic ramp: idle, then busy with the clock rising
    #    from 0.30 to 1.00 over 1.5 s, then steady. The ramp lands where drawn,
    #    the warm-up window averages below the tail, and IPC is flat.
    rows = []
    t = 0.0
    for i in range(80):
        t = round(t + 0.1, 3)
        if i < 5:
            rows.append({"t": t, "cycles": 1e6, "instructions": 1e6, "ref_cycles": 1e6, "freq_ratio": 0.33, "ipc": 1.0})
            continue
        since = (i - 5) * 0.1
        ratio = min(1.0, 0.30 + 0.70 * since / 1.5)
        rows.append({"t": t, "cycles": 3.8e8 * ratio, "instructions": 3.8e8 * ratio * 1.2, "ref_cycles": 2.4e8, "freq_ratio": ratio, "ipc": 1.2})
    # ... then criterion's multi-threaded analysis: 6 intervals at ~8 cores'
    # worth of ref-cycles. A peak-relative threshold would call ONLY these
    # busy (the defect the first version had); they must be excluded instead.
    for _ in range(6):
        t = round(t + 0.1, 3)
        rows.append({"t": t, "cycles": 2.8e9, "instructions": 3.8e9, "ref_cycles": 1.9e9, "freq_ratio": 1.47, "ipc": 1.36})
    s = summarise(rows, warm_up_s=2.0, tail_s=3.0, interval_s=0.1)
    assert s["busy_start_s"] == 0.5, s
    assert s["analysis_intervals_excluded"] == 5, s  # the 6th is the dropped final interval
    assert s["tail_window_s"][1] == 8.0 and s["busy_intervals"] == 75, s
    # ... and a burst at 15x one core (the P-cores' 16-thread analysis) is
    # excluded just the same; with a 10%-of-peak floor it was the only "busy" phase
    wide = [dict(r, ref_cycles=3.6e9, cycles=7.0e9) if r["ref_cycles"] > 1e9 else r for r in rows]
    s_wide = summarise(wide, 2.0, 3.0, 0.1)
    assert s_wide["busy_start_s"] == 0.5 and s_wide["analysis_intervals_excluded"] == 5, s_wide
    assert 1.3 <= s["ramp_to_95pct_of_steady_s"] <= 1.5, s
    assert s["warmup_mean_freq_ratio"] < s["tail_mean_freq_ratio"] == 1.0, s
    assert abs(s["warmup_mean_ipc"] - 1.2) < 1e-9 and abs(s["tail_mean_ipc"] - 1.2) < 1e-9, s
    # ... and a flat clock reports no ramp beyond the first busy interval
    flat = [dict(r, freq_ratio=1.0, cycles=3.8e8) if 0.5 < r["t"] <= 8.0 else r for r in rows]
    assert summarise(flat, 2.0, 3.0, 0.1)["ramp_to_95pct_of_steady_s"] == 0.1

    # 4. aggregation averages reps per cell and the render carries every column
    runs = [
        {"core_class": "e_cores", "warm_up_s": 2.0, "criterion": {"time_ns": [1, 22948.0, 3], "iterations": 200e3},
         "summary": {**s, "ramp_to_95pct_of_steady_s": 1.4}, "cpufreq_summary": {"tail_mean_khz": 3.8e6, "max_khz": 3.8e6}},
        {"core_class": "e_cores", "warm_up_s": 2.0, "criterion": {"time_ns": [1, 22950.0, 3], "iterations": 200e3},
         "summary": {**s, "ramp_to_95pct_of_steady_s": 1.6}, "cpufreq_summary": {"tail_mean_khz": 3.8e6, "max_khz": 3.8e6}},
        {"core_class": "p_cores", "warm_up_s": 3.0, "criterion": {"time_ns": [1, 14049.0, 3], "iterations": 355e3},
         "summary": {**s}, "cpufreq_summary": {"tail_mean_khz": 5.0e6, "max_khz": 5.1e6}},
    ]
    cells = aggregate(runs)
    assert [c["cell"] for c in cells] == ["e_cores@2s", "p_cores@3s"], cells
    assert cells[0]["reps"] == 2 and abs(cells[0]["criterion_time_ns_mid_mean"] - 22949.0) < 1e-9
    assert abs(cells[0]["ramp_to_95pct_s_mean"] - 1.5) < 1e-9
    md = render(cells, {"host": "h", "bench": "domain", "filter": "f", "reps": 2, "tsc_nominal_ghz": 2.4})
    header = next(ln for ln in md.splitlines() if ln.startswith("| cell |"))
    body = [ln for ln in md.splitlines() if ln.startswith("| `")]
    assert len(body) == 2 and all(b.count("|") == header.count("|") for b in body), (header, body)
    assert "2.40 GHz" in md and "22,949.0" in md, md

    # 5. the assembled command: perf outside taskset outside cargo, criterion flags after `--`
    argv = perf_argv("cpu_atom", "16-23", 100, bench_argv("domain", "expanse-trie", "arm/1", 2, 5))
    assert argv[:2] == ["perf", "stat"] and "-I" in argv and argv[argv.index("--") + 1 :][:3] == ["taskset", "-c", "16-23"]
    assert argv[-4:] == ["--warm-up-time", "2", "--measurement-time", "5"], argv
    assert "cpu_atom/cycles/,cpu_atom/instructions/,cpu_atom/ref-cycles/" in argv

    # 6. cells rotate their lead like pin_exposure's conditions do
    order = round_order(1, ("a", "b", "c", "d"))
    assert order == ["b", "c", "d", "a"], order
    print("warmup_ramp.py --self-test: all checks passed")


# --------------------------------------------------------------------------- main
def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bench", help="criterion bench target, e.g. `domain`")
    ap.add_argument("-p", "--package", default="expanse-trie")
    ap.add_argument("--filter", help="criterion filter matching exactly one arm")
    ap.add_argument("--warm-up-times", default="2,3", help="comma-separated criterion warm-up seconds (default 2,3)")
    ap.add_argument("--measurement-time", type=float, default=DEFAULT_MEASUREMENT_S)
    ap.add_argument("--classes", default="e_cores,p_cores", help="core classes to run (default both)")
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--interval-ms", type=int, default=100)
    ap.add_argument("--tail-seconds", type=float, default=3.0, help="length of the end-of-run window (default 3)")
    ap.add_argument("--commit", help="commit being measured; required outside a git checkout")
    ap.add_argument("--host-desc", help="anonymised hardware description; derived when omitted")
    ap.add_argument("--json", type=Path)
    ap.add_argument("--markdown", type=Path)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        self_test()
        return
    if not args.bench or not args.filter:
        ap.error("--bench and --filter are required")
    warm_ups = [float(w) for w in args.warm_up_times.split(",") if w.strip()]
    classes = [c.strip() for c in args.classes.split(",") if c.strip()]
    for c in classes:
        if c not in CLASSES:
            ap.error(f"unknown class {c!r}; choose from {sorted(CLASSES)}")
    try:
        p_cpus, e_cpus = preflight()
        cpus_of = {"p_cores": p_cpus, "e_cores": e_cpus}
        if not shutil_which("perf"):
            raise Preflight("`perf` is not on PATH; the counters cannot be read")
        for c in classes:
            perf_can_count(CLASSES[c], cpus_of[c])
        commit, commit_source, host_desc = resolve_provenance(args.commit, args.host_desc)
    except (Preflight, ValueError, RuntimeError) as e:
        fail(str(e))
        return

    build_first(args.bench, args.package)
    start_load = os.getloadavg()
    cells = [(c, w) for c in classes for w in warm_ups]
    labels = [cell_key(c, w) for c, w in cells]
    runs: List[Dict[str, Any]] = []
    for rep in range(args.reps):
        for label in round_order(rep, labels):
            c, w = cells[labels.index(label)]
            runs.append(run_cell(c, cpus_of[c], w, args, rep))
    agg = aggregate(runs)
    meta = {
        "host": host_desc,
        "provenance": {
            "host_description": host_desc,
            "commit": commit,
            "commit_source": commit_source,
            "toolchain": toolchain(),
            "perf_version": subprocess.run(["perf", "--version"], capture_output=True, text=True).stdout.strip(),
            "load_average_at_start": start_load,
            "load_average_at_end": os.getloadavg(),
        },
        "bench": args.bench,
        "package": args.package,
        "filter": args.filter,
        "reps": args.reps,
        "warm_up_times_s": warm_ups,
        "measurement_time_s": args.measurement_time,
        "interval_ms": args.interval_ms,
        "tail_seconds": args.tail_seconds,
        "cpus": cpus_of,
        "pmus": CLASSES,
        "events": list(EVENTS),
        "tsc_nominal_ghz": tsc_nominal_ghz(),
        "design": f"{len(cells)} cells ({'/'.join(labels)}) interleaved within each of {args.reps} reps, rotating which runs first",
    }
    md = render(agg, meta)
    print(md)
    if args.markdown:
        args.markdown.write_text(md, encoding="utf-8")
    if args.json:
        args.json.write_text(
            json.dumps({"schema": SCHEMA, "kind": "criterion_warmup_frequency_ipc_timeline", "issue": ISSUE_URL, "meta": meta, "cells": agg, "runs": runs}, indent=1) + "\n",
            encoding="utf-8",
        )


def shutil_which(name: str) -> Optional[str]:
    import shutil

    return shutil.which(name)


if __name__ == "__main__":
    main()
