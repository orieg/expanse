#!/usr/bin/env python3
"""Hardware counters for a named comparative-suite cell, so a mechanism paragraph
can stop ending in "unmeasured" (#737).

Every mechanism paragraph in `masstree_comparison/README.md` and
`hot_comparison/README.md` ends in "unmeasured" — the reader collapse under one
writer, the string-lookup gap, the sorted-order insert loss, the scan-start
effect, the memory cascade. AGENTS.md section 8.9 principle 1 forbids stating a
microarchitectural cause without counters, and none of the comparative harnesses
takes any. #724, #725 and #730 each schedule `perf stat` runs for one path by
hand; this is that, once, for every cell.

## What it does

Runs a named cell's existing binary under

    perf stat -e page-faults,dTLB-load-misses,LLC-load-misses,cycles,instructions

plus `mem_load_l3_hit_retired.xsnp_hitm` where the cell is concurrent, one
`perf` invocation per repeat, divides each event by the cell's probe or key
count, and writes `counters_<cell>.json` beside the wall-clock artifact with a
BCa 95% bootstrap interval over the repeats (section 8.4).

## What it deliberately does not do

**It does not bracket the timed loop.** A cell's binary builds a population and
then probes it, and `perf stat` counts the whole process. So every figure here
is *per process*, and the build is in it. Two consequences, both stated in the
artifact rather than left for a reader to discover: a lookup cell's counts
include the build that preceded the probes, and a comparison between two arms
is only as clean as the similarity of their builds. `scripts/perf_counters.py`
differences a `build` phase against a `probe` phase to get around this; the
comparative bins have no such phase switch, and inventing one would change the
binaries this suite measured. What the counters can answer as they stand is
whether two arms differ in page faults, translation misses or last-level misses
*over the same work*, by a margin far larger than the build's share.

**It does not attribute.** A counter is evidence for a mechanism, not the
mechanism. A cell whose intervals overlap decides nothing and says so.

## Hybrid hosts

The reference host is an Alder-Lake-class part whose kernel exposes two core
PMUs, so one requested event comes back as `cpu_core/<event>/` **and**
`cpu_atom/<event>/` and never as a bare `<event>`. Those two rows count two
different microarchitectures over two different sets of cores and are never
summed: this driver selects one PMU, confines the workload to that PMU's CPUs,
reads only that PMU's rows and names it in the artifact. That logic already
exists in `scripts/perf_counters.py` and is imported rather than written twice.

Fail-loud (section 8.1): a missing `perf`, a kernel that refuses to open a
counter, a missing binary or a workload that escaped its pin exits non-zero
with the cause and the fix named. It never degrades into a report that reads as
complete.

Usage:
  python3 scripts/bench_counters.py --list
  python3 scripts/bench_counters.py --cell hot_lookup_random_1m --repeats 7
  python3 scripts/bench_counters.py --all --out-dir docs/benchmarks/hot_comparison/results
  python3 scripts/bench_counters.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

from bca_bootstrap import bca_bootstrap_ci  # noqa: E402
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
    split_event_name,
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


class Cell:
    """One named cell: which binary, which arguments, and what to divide by.

    `ops_key` names the field in the binary's own JSON rows that says how many
    operations a round did, so the per-operation figure is the harness's own
    count and not a second, independently-derived one that could disagree
    (section 8.2).
    """

    def __init__(self, name: str, issue: int, suite: str, binary: str, args: list[str],
                 features: list[str], concurrent: bool = False, note: str = "",
                 blocked: str = ""):
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

    def events(self) -> list[str]:
        return BASE_EVENTS + ([CONCURRENT_EVENT] if self.concurrent else [])


# The cells #737's gate names: the four from #724 / #725 / #730, and the HOT
# `random` 1M lookup cell.
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
    # BLOCKED, and the block is the finding. The concurrent binaries publish
    # throughput (`*_reader_mops`), not an operation count, so there is no
    # harness-owned divisor for a per-probe counter figure and this driver
    # refuses to invent one — it exits non-zero naming the missing field rather
    # than dividing by a number it derived itself (section 8.2). #730's own
    # scope item 1 adds the W = 0, R in {1,2,4,8} cells; publishing `read_ops`
    # alongside them is what unblocks these two, and until then they are
    # declared here and excluded from `--all` rather than quietly dropped.
    Cell("masstree_conc_str_w0_r1", 730, "masstree_comparison",
         "masstree_concurrent", ["str", "0", "1"], ["masstree"], concurrent=True,
         blocked="the concurrent harness publishes reader throughput, not `read_ops`; "
                 "no per-probe divisor exists until #730 adds one",
         note="one reader, no writer — the wrapper's own per-probe cost"),
    Cell("masstree_conc_str_w0_r8", 730, "masstree_comparison",
         "masstree_concurrent", ["str", "0", "8"], ["masstree"], concurrent=True,
         blocked="the concurrent harness publishes reader throughput, not `read_ops`; "
                 "no per-probe divisor exists until #730 adds one",
         note="eight readers, no writer — reader-reader line traffic if it grows with R"),
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
]

BY_NAME = {c.name: c for c in CELLS}


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


# --------------------------------------------------------------------------
# running a cell
# --------------------------------------------------------------------------
def binary_path(name: str) -> Path:
    target = os.environ.get("CARGO_TARGET_DIR")
    root = Path(target) if target else (CRATE.parent / "target")
    return root / "release" / name


def build(cell: Cell, env: dict) -> None:
    args = ["cargo", "build", "--release", "--manifest-path", str(CRATE), "--bin", cell.binary]
    if cell.features:
        args += ["--features", ",".join(cell.features)]
    proc = subprocess.run(args, env=env)
    if proc.returncode != 0:
        raise Preflight(f"building {cell.binary} failed; no counters were collected")


def ops_of(stdout: str, cell: Cell) -> tuple[int, list[dict]]:
    """Total operations the binary reports over its own rounds, and the rows.

    The divisor is the harness's own count. Deriving a second one here would
    let the published per-operation figure disagree with the published cell.
    """
    rows = [json.loads(line) for line in stdout.splitlines() if line.startswith("{")]
    if not rows:
        raise Preflight(f"{cell.binary} emitted no JSON rows; the cell is void")
    if cell.concurrent:
        # A concurrent row reports throughput, not an op count; use the reader
        # op total the harness publishes if it has one, else refuse rather than
        # divide by a number this driver invented.
        total = sum(r.get("read_ops") or 0 for r in rows)
        if not total:
            raise Preflight(
                f"{cell.binary} rows carry no `read_ops`, so a per-probe figure "
                f"cannot be derived from the harness's own count"
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


def collect(cell: Cell, repeats: int, events: list[str], pin: list[str],
            pmu: str | None, env: dict) -> dict:
    reps = [one_repeat(cell, events, pin, pmu, env) for _ in range(repeats)]
    per_event = {}
    for ev in events:
        samples = [r["per_op"][ev] for r in reps if r["per_op"].get(ev) is not None]
        if len(samples) < 2:
            per_event[ev] = {"per_op_mean": samples[0] if samples else None,
                             "ci_lower": None, "ci_upper": None,
                             "note": "fewer than two repeats counted; no interval"}
            continue
        mean, lo, hi = bca_bootstrap_ci(samples, num_resamples=2000, seed=42)
        per_event[ev] = {"per_op_mean": mean, "ci_lower": lo, "ci_upper": hi,
                         "samples": samples}
    return {
        "cell": cell.name, "issue": cell.issue, "suite": cell.suite,
        "binary": cell.binary, "args": cell.args, "note": cell.note,
        "concurrent": cell.concurrent,
        "repeats": repeats, "ops_per_repeat": reps[0]["ops"],
        "rounds_per_repeat": reps[0]["rounds"],
        "events": per_event,
        "repeats_raw": [{"ops": r["ops"], "raw": r["raw"], "per_op": r["per_op"]} for r in reps],
    }


def provenance(pmu, why, pin, available, unavailable, repeats) -> dict:
    return {
        "instrument": "perf stat, one invocation per repeat, whole process",
        "commit": os.environ.get("EXPANSE_BENCH_COMMIT", "unknown"),
        "perf_event_paranoid": paranoid_level(),
        "pmu": pmu, "pmu_reason": why, "pmu_cpus": pmu_cpus(pmu), "pin": pin,
        "events_available": available, "events_unavailable": unavailable,
        "repeats": repeats,
        "estimators": {
            "per_op": "the raw event row from perf's CSV divided by the harness's own "
                      "operation count for that process (AGENTS.md section 8.9 "
                      "principle 5); both are published",
            "interval": "BCa 95% bootstrap over the repeats (scripts/bca_bootstrap.py), "
                        "2000 resamples",
        },
        "scope": "counts are per PROCESS and include the population build; this driver "
                 "does not bracket the timed loop, so a per-op figure is not a "
                 "probe-loop figure and two arms are comparable only over the same work",
        "attribution": "a counter is evidence for a mechanism, not the mechanism; a cell "
                       "whose intervals overlap decides nothing",
    }


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------
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

    # Every declared cell must name a binary that exists in the crate.
    for cell in CELLS:
        src = CRATE.parent / "src" / "bin" / f"{cell.binary}.rs"
        if not src.is_file():
            failures.append(f"cell {cell.name} names a missing binary source {src}")
    if len(BY_NAME) != len(CELLS):
        failures.append("two cells share a name")
    # The gate names four cells from #724/#725/#730 plus the HOT lookup cell.
    for issue, want in ((724, 2), (725, 3), (730, 2), (737, 1)):
        got = sum(1 for c in CELLS if c.issue == issue)
        if got != want:
            failures.append(f"expected {want} cell(s) for #{issue}, found {got}")
    conc = [c for c in CELLS if c.concurrent]
    if not conc or any(CONCURRENT_EVENT not in c.events() for c in conc):
        failures.append("a concurrent cell does not request the snoop-hit counter")
    if any(c.concurrent and not c.blocked for c in CELLS):
        failures.append("a concurrent cell is not marked blocked; it would hang or "
                        "divide by an invented count")
    if CONCURRENT_EVENT in BY_NAME["hot_lookup_random_1m"].events():
        failures.append("a single-threaded cell requested the snoop-hit counter")

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
    ap.add_argument("--repeats", type=int, default=7)
    ap.add_argument("--out-dir", default=None)
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--skip-build", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return _self_test()
    if args.list:
        for c in CELLS:
            print(f"  {c.name:36} #{c.issue}  {c.suite:22} {c.binary} {' '.join(c.args)}")
        return 0

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

    events = sorted({e for c in cells for e in c.events()},
                    key=lambda e: (BASE_EVENTS + [CONCURRENT_EVENT]).index(e))
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

    prov = provenance(pmu, why, pin, available, unavailable, args.repeats)
    results = []
    for cell in cells:
        try:
            if not args.skip_build:
                build(cell, env)
            evs = [e for e in cell.events() if e in available]
            print(f"\n[{cell.name}] {cell.binary} {' '.join(cell.args)}")
            res = collect(cell, args.repeats, evs, pin, pmu, env)
        except Preflight as exc:
            print(f"::error::bench_counters.py: {cell.name}: {exc}", file=sys.stderr)
            return 1
        for ev, v in res["events"].items():
            if v["per_op_mean"] is None:
                continue
            ci = ("" if v["ci_lower"] is None
                  else f" [{v['ci_lower']:.4g}, {v['ci_upper']:.4g}]")
            print(f"  {ev:34} {v['per_op_mean']:>12.4g} /op{ci}")
        results.append(res)
        # Written now, not at the end: a later cell that fails loud must not
        # discard the measurements already taken.
        suite_dir = REPO_ROOT / "docs" / "benchmarks" / res["suite"] / "results"
        out_dir = Path(args.out_dir) if args.out_dir else suite_dir
        out_dir.mkdir(parents=True, exist_ok=True)
        path = out_dir / f"counters_{res['cell']}.json"
        path.write_text(json.dumps({"provenance": prov, **res}, indent=2) + "\n")
        print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
