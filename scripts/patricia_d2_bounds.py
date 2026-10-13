#!/usr/bin/env python3
"""
scripts/patricia_d2_bounds.py — threshold derivations for Diagnostic D2 (Refs #1096).

Pre-registers the mathematical derivations behind Diagnostic D2 (Characterisation tier:
physical paging sensitivity and cache-line layout isolation for 1M prefix scan,
docs/benchmarks/patricia_comparison/METHODOLOGY.md §3 Amendment A4), as committed,
unit-tested code (AGENTS.md §8.8 commit 1, Rule 12 / §1.3).

Diagnostic D2 evaluates whether 2 MiB transparent huge pages
(GLIBC_TUNABLES=glibc.malloc.hugetlb=1) eliminate the 1M generator-order prefix scan
cycle gap between strmap_prefix_scan and strmap_prefix_scan_sorted, or whether
the gap is dominated by spatial cache-line dispersal across the 45 MB tree.

All clauses are evaluated on BCa 95% bootstrap confidence interval bounds (Rule 1 / §1.1):
  - Clause D2a (dTLB Elimination): BCa_upper(generator_2m.dtlb) <= dtlb_bound
  - Clause D2b (L3 Miss Invariance): BCa_lower(generator_2m.l3 - sorted_2m.l3) >= l3_bound
  - Clause D2c (Surviving Cycle Gap): BCa_lower(generator_2m.cycles - sorted_2m.cycles) >= cycle_bound

Reference values are pinned to committed D1 artifacts:
  - docs/benchmarks/patricia_comparison/results/counters_prefix_scan_d1.json (commit c3e54ec2)
  - docs/benchmarks/patricia_comparison/results/counters_prefix_scan_d1_stalls.json (commit c3e54ec2)

Usage:
  python3 scripts/patricia_d2_bounds.py            # run the pinned tests and print derived bounds
  python3 scripts/patricia_d2_bounds.py --self-test
  python3 scripts/patricia_d2_bounds.py --evaluate <d2_2m.json> <d2_4k.json>      # one A5 dispatch
  python3 scripts/patricia_d2_bounds.py --verdict <2m> <4k> [<2m> <4k> ...]       # the A5 pair, in dispatch order
  python3 scripts/patricia_d2_bounds.py --evaluate-a4 <d2_2m.json>                # the superseded A4 reading
"""

from __future__ import annotations

import json
import math
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

# Pinned baseline figures from D1 artifact counters_prefix_scan_d1.json
# (49,934,000 total ops = 249,670 entries * 200 passes)
D1_OPS = 49_934_000
D1_GEN_DTLB_POINT = 40_962_884.5 / D1_OPS       # 0.820341 misses/entry
D1_SORTED_DTLB_POINT = 486_625.4 / D1_OPS       # 0.009745 misses/entry
D1_GEN_L3_POINT = 11_784_514.5 / D1_OPS         # 0.236002 misses/entry
D1_SORTED_L3_POINT = -2_489.7 / D1_OPS          # -0.000050 misses/entry
D1_GEN_CYCLES_POINT = 7_911_328_314.9 / D1_OPS  # 158.4357 cycles/entry
D1_SORTED_CYCLES_POINT = 3_630_337_417.8 / D1_OPS # 72.7027 cycles/entry
D1_BASELINE_DELTA_CYCLES = D1_GEN_CYCLES_POINT - D1_SORTED_CYCLES_POINT # 85.7330 cycles/entry

# Pinned baseline figures from D1 stalls artifact counters_prefix_scan_d1_stalls.json
D1_STALLS_GEN_L3_CYCLES = 2_623_482_789.9 / D1_OPS # 52.5390 stall cycles/entry
D1_STALLS_SORTED_L3_CYCLES = 1_793_431.3 / D1_OPS  # 0.0359 stall cycles/entry
D1_STALLS_DELTA_L3_CYCLES = D1_STALLS_GEN_L3_CYCLES - D1_STALLS_SORTED_L3_CYCLES # 52.5031 cycles/entry
D1_STALLS_TOTAL_DELTA_CYCLES = (7_921_202_316.5 - 3_629_647_185.4) / D1_OPS # 85.9445 cycles/entry
D1_STALLS_L3_FRACTION = D1_STALLS_DELTA_L3_CYCLES / D1_STALLS_TOTAL_DELTA_CYCLES # 0.610895 (61.1%)


def dtlb_elimination_upper_bound(baseline_gen_dtlb: float, min_reduction: float = 0.95) -> float:
    """Computes the maximum allowable BCa upper bound for dTLB load misses under huge pages.

    A reduction of >= 95% indicates successful elimination of virtual page translation overhead,
    approaching the sorted order baseline (0.0097).
    """
    if baseline_gen_dtlb <= 0:
        raise ValueError(f"baseline_gen_dtlb must be positive, got {baseline_gen_dtlb}")
    if not (0 < min_reduction < 1):
        raise ValueError(f"min_reduction must be between 0 and 1, got {min_reduction}")
    return round(baseline_gen_dtlb * (1.0 - min_reduction), 4)


def l3_invariance_lower_bound(baseline_delta_l3: float, min_retention: float = 0.60) -> float:
    """Computes the minimum allowable BCa lower bound for L3 cache misses delta under huge pages.

    Retaining >= 60% of baseline delta-L3 indicates that spatial cache-line dispersal
    persists independently of page size.
    """
    if baseline_delta_l3 <= 0:
        raise ValueError(f"baseline_delta_l3 must be positive, got {baseline_delta_l3}")
    if not (0 < min_retention <= 1):
        raise ValueError(f"min_retention must be between 0 and 1, got {min_retention}")
    return round(baseline_delta_l3 * min_retention, 4)


def surviving_cycle_gap_lower_bound(baseline_delta_cycles: float, stall_fraction: float) -> float:
    """Computes the minimum allowable BCa lower bound for the surviving cycle gap under huge pages.

    Derived from the hardware stall attribution in D1 stalls, where L3 miss stalls accounted
    for 61.1% of the cycle gap.
    """
    if baseline_delta_cycles <= 0:
        raise ValueError(f"baseline_delta_cycles must be positive, got {baseline_delta_cycles}")
    if not (0 < stall_fraction <= 1):
        raise ValueError(f"stall_fraction must be between 0 and 1, got {stall_fraction}")
    return round(baseline_delta_cycles * stall_fraction, 2)


def registered_bounds() -> dict[str, float]:
    """The three bounds METHODOLOGY.md §3.4.3 registers for D2a, D2b and D2c.

    D2c pins the D1 L3-stall delta directly (52.50 cycles/entry), not the
    product `surviving_cycle_gap_lower_bound` computes from the cycle gap and
    the stall fraction (52.37): the registered table states 52.50.
    """
    return {
        "dtlb_upper": dtlb_elimination_upper_bound(D1_GEN_DTLB_POINT, 0.95),
        "l3_delta_lower": l3_invariance_lower_bound(D1_GEN_L3_POINT - D1_SORTED_L3_POINT, 0.60),
        "cycle_delta_lower": round(D1_STALLS_DELTA_L3_CYCLES, 2),
    }


# --- Host-condition bounds for the errata A5 to Amendment A4 (Refs #1096) -----------
# `a5_host_condition` and `evaluate_d2` read them.

LOAD1_TAU_S = 60.0   # time constant of the one-minute load average
FOREIGN_GRID = 0.01  # `bench_provenance.load_snapshot` rounds its busy-CPU figures to this


def expected_load1(load0: float, runnable: float, elapsed_s: float, tau_s: float = LOAD1_TAU_S) -> float:
    """The one-minute load average after `elapsed_s` with `runnable` tasks runnable throughout.

    Linux updates the figure every 5 s as `load = load * e + active * (1 - e)` with
    `e = 1/exp(5 s / 1 min)` (`include/linux/sched/loadavg.h`, v6.8: `calc_load`, `EXP_1`
    "1/exp(5sec/1min) as fixed-point", `LOAD_FREQ` "5 sec intervals";
    https://raw.githubusercontent.com/torvalds/linux/v6.8/include/linux/sched/loadavg.h).
    The closed form of that recurrence over a constant `active` is

        load(t) = runnable + (load0 - runnable) * exp(-t / tau)

    so the figure moves toward `runnable` and never crosses it. A harness that keeps
    one thread runnable therefore reads at least `min(load0, 1.0)` at every later
    instant, and exactly 1.0 only in the limit: `load1 <= 1.0` during such a run
    holds only if the run began below 1.0, and is then an upper bound on how long
    the run has lasted, not a statement about other load.
    """
    if load0 < 0 or runnable < 0:
        raise ValueError(f"load0 and runnable must be non-negative, got {load0}, {runnable}")
    if elapsed_s < 0 or tau_s <= 0:
        raise ValueError(f"elapsed_s must be non-negative and tau_s positive, got {elapsed_s}, {tau_s}")
    return runnable + (load0 - runnable) * math.exp(-elapsed_s / tau_s)


def foreign_load_ceiling(bound: float, alternative: float, arm_cycles: float, slowdown: float = 2.0) -> float:
    """The largest cell-mean foreign load, in core-equivalents, that cannot by itself
    carry a cycle difference at `alternative` over `bound`.

    Worst-case model, an assumption and not a measurement: every foreign CPU-second
    falls on the generator arm alone, and for that time the arm's cycles per entry
    are multiplied by at most `slowdown`. A foreign load `f` then inflates the arm,
    and so the generator-minus-sorted difference, by at most
    `f * (slowdown - 1) * arm_cycles` cycles per entry. Requiring that to stay below
    `bound - alternative` gives `f < (bound - alternative) / ((slowdown - 1) * arm_cycles)`,
    returned rounded down to the grid the load snapshot records (`FOREIGN_GRID`).
    """
    if not bound > alternative >= 0:
        raise ValueError(f"need bound > alternative >= 0, got {bound}, {alternative}")
    if arm_cycles <= 0:
        raise ValueError(f"arm_cycles must be positive, got {arm_cycles}")
    if slowdown <= 1:
        raise ValueError(f"slowdown must exceed 1, got {slowdown}")
    f = (bound - alternative) / ((slowdown - 1.0) * arm_cycles)
    return math.floor(f / FOREIGN_GRID + 1e-9) * FOREIGN_GRID


def foreign_reading_floor(roundings: int = 3, grid: float = FOREIGN_GRID) -> float:
    """The most negative foreign reading rounding alone can produce, as a grid value.

    `foreign = round(round(host) - round(own))`: three roundings of at most half a
    grid step each, 0.015 at the 0.01 grid, which the grid records as 0.02. A
    reading below the negative of this is the two clocks disagreeing, and a
    remainder of that size can hide a foreign task of the same size.
    """
    if roundings < 1 or grid <= 0:
        raise ValueError(f"need roundings >= 1 and grid > 0, got {roundings}, {grid}")
    return -math.ceil(roundings * grid / 2 / grid - 1e-9) * grid


A5_PHYSICAL_CORES = 16        # the reference host: 8 P-cores (CPUs 0-15, two threads each) and 8 E-cores (16-23)
A5_VOID_REPLACEMENTS_MAX = 3  # replacement dispatches across the pair, registered in A5.5


def a5_host_bounds() -> dict[str, float]:
    """The host bounds the errata A5 registers, from frozen D1 inputs only.

    `foreign_cell_max`: `foreign_load_ceiling` with the registered D2c bound
    (52.50), the alternative D2c's falsifier names ("paging alone resolved the
    majority of the cycle gap": half of D1's 85.9445), the generator arm's D1
    cycles per entry from the same artifact (158.6335), and `slowdown = 2`.
    No D2 run's reading enters it.

    `load1_max` and `load1_spread_max` are AGENTS.md section 8.17's floor: a load
    average above half the host's physical cores, or a shift of more than 2
    between arms, voids a comparison. `void_replacements_max` is the registered
    cap on replacement dispatches across the pair.
    """
    d1_gen_cycles = 7_921_202_316.5 / D1_OPS
    return {
        "foreign_cell_max": round(foreign_load_ceiling(
            registered_bounds()["cycle_delta_lower"], D1_STALLS_TOTAL_DELTA_CYCLES / 2, d1_gen_cycles, 2.0), 2),
        "foreign_cell_min": round(foreign_reading_floor(), 2),
        "load1_max": A5_PHYSICAL_CORES / 2,
        "load1_spread_max": 2.0,
        "void_replacements_max": A5_VOID_REPLACEMENTS_MAX,
    }


GENERATOR_ARM = "strmap_prefix_scan"
SORTED_ARM = "strmap_prefix_scan_sorted"


REGISTERED_RUNS = 10  # A4: `--runs 10` per cell
# A4's two-run protocol names the host condition: `loadavg <= 1.0`, `foreign_busy_cpus == 0`.
REGISTERED_LOAD1_MAX = 1.0
REGISTERED_FOREIGN_MAX = 0.0


def host_precondition(artifact: dict) -> dict:
    """A4's registered host condition, read from the artifact's load snapshots.

    Every snapshot's one-minute load average must be at most 1.0 and every
    recorded foreign-CPU figure must be 0. An artifact with no snapshot does
    not meet it.
    """
    loads = artifact.get("provenance", {}).get("loads") or []
    load1 = [float(x["load1"]) for x in loads if x.get("load1") is not None]
    foreign = [float(x["foreign_busy_cpus_since_prev"]) for x in loads
               if x.get("foreign_busy_cpus_since_prev") is not None]
    unmet = []
    if not load1:
        unmet.append("no load snapshot is recorded")
    elif max(load1) > REGISTERED_LOAD1_MAX:
        unmet.append(f"load1 reached {max(load1):g}, registered `loadavg <= {REGISTERED_LOAD1_MAX:g}`")
    if foreign and max(foreign) > REGISTERED_FOREIGN_MAX:
        unmet.append(f"foreign_busy_cpus reached {max(foreign):g}, registered `foreign_busy_cpus == 0`")
    return {"met": not unmet, "unmet": unmet,
            "load1_max": max(load1) if load1 else None, "foreign_busy_cpus_max": max(foreign) if foreign else None}


A5_SNAPSHOTS = 3  # `perf_counters.py`: one before each of the two cells, one at the end


def _is_number(v) -> bool:
    return isinstance(v, (int, float)) and not isinstance(v, bool)


def a5_host_condition(artifact: dict) -> dict:
    """The errata A5's host condition on one artifact of a dispatch (METHODOLOGY.md section A5.3).

    H0: `provenance.host_guard` records a quiet start gate, a complete in-run
    record with at least one sample, and no void. H1: each of the two cell
    windows (snapshots 2 and 3) carries a numeric `foreign_busy_cpus_since_prev`
    within `[foreign_cell_min, foreign_cell_max]`. H2: every snapshot's `load1`
    is at most `load1_max`, and the largest minus the smallest at most
    `load1_spread_max`. Anything not recorded is not met: a missing snapshot,
    reading or guard record is never read as quiet.
    """
    b = a5_host_bounds()
    prov = artifact.get("provenance", {})
    loads = prov.get("loads") or []
    unmet = []
    foreign = []
    if len(loads) != A5_SNAPSHOTS:
        unmet.append(f"{len(loads)} load snapshot(s), registered {A5_SNAPSHOTS}")
    else:
        for snap in loads[1:]:
            f = snap.get("foreign_busy_cpus_since_prev")
            if not _is_number(f):
                unmet.append(f"H1: the window ending at {snap.get('label')!r} has no foreign reading")
                continue
            foreign.append(float(f))
            if f > b["foreign_cell_max"]:
                unmet.append(f"H1: foreign_busy_cpus {f:g} > {b['foreign_cell_max']:g} in the window ending at {snap.get('label')!r}")
            if f < b["foreign_cell_min"]:
                unmet.append(f"H1: foreign_busy_cpus {f:g} < {b['foreign_cell_min']:g} in the window ending at {snap.get('label')!r}")
    load1 = [float(x["load1"]) for x in loads if _is_number(x.get("load1"))]
    if len(load1) != len(loads) or not load1:
        unmet.append("H2: a load snapshot has no load1")
    else:
        if max(load1) > b["load1_max"]:
            unmet.append(f"H2: load1 reached {max(load1):g} > {b['load1_max']:g}")
        if max(load1) - min(load1) > b["load1_spread_max"]:
            unmet.append(f"H2: load1 moved by {max(load1) - min(load1):g} > {b['load1_spread_max']:g} within the artifact")
    guard = prov.get("host_guard")
    if not isinstance(guard, dict):
        unmet.append("H0: no host-guard record in provenance")
    else:
        start, run = guard.get("start") or {}, guard.get("run") or {}
        if start.get("verdict") != "quiet":
            unmet.append(f"H0: the start gate read {start.get('verdict')!r}, not 'quiet'")
        if guard.get("watcher_ok") is not True:
            unmet.append("H0: the host watcher's record is incomplete")
        if not _is_number(run.get("samples")) or run["samples"] < 1:
            unmet.append("H0: the in-run record holds no sample")
        if run.get("voids") != 0:
            unmet.append(f"H0: {run.get('voids')!r} in-run void(s): {'; '.join(run.get('reasons') or []) or 'no reason recorded'}")
    return {"registration": "A5", "met": not unmet, "unmet": unmet,
            "load1_max": max(load1) if load1 else None,
            "foreign_busy_cpus_max": max(foreign) if foreign else None,
            "foreign_busy_cpus_min": min(foreign) if foreign else None}


def evaluate_d2(artifact_2m: dict, artifact_4k: dict | None = None, *, registration: str = "A5",
                expected_runs: int = REGISTERED_RUNS, expected_entries: int = D1_OPS) -> dict:
    """Evaluates D2a, D2b and D2c on one dispatch's `counters_prefix_scan_d2_2m.json` artifact.

    `registration` selects the host condition. `"A5"` (the default, and the
    only one a verdict is read under) needs the dispatch's 4 KiB artifact too:
    `a5_host_condition` must hold on both, the two must name one commit and one
    run, and a dispatch that does not meet it is VOID. `"A4"` is the superseded
    condition (`host_precondition`), kept so the record of the artifacts
    committed under it stays reproducible; under it an unmet host condition
    with every clause met is INTERMEDIATE.

    Counts are per entry: each run's `probe - build` count divided by
    `distinct_probes * passes`. D2a reads the generator arm alone. D2b and D2c
    read the difference of the two arms, taken run by run in the order the
    artifact records them. The arms are separate processes run as two blocks,
    so that order pairs nothing: the interval is one of many a re-ordering
    would give. Every clause is decided on a BCa 95% bound (2,000 resamples,
    seed 42), never on the point.

    The run is VOID, with no clause evaluated, when it is not the measurement
    A4 registered: `hugetlb` not recorded, an arm that faulted in no huge page
    or fell back from one, an arm recorded twice, an arm without
    `expected_runs` runs, a per-entry denominator other than
    `expected_entries`, or a counter a clause reads whose status is not `ok`.

    The verdict is FAIL when a clause is not met and PASS when every clause is
    met (under `"A4"`: PASS only if `host_precondition` is met, INTERMEDIATE
    otherwise).

    One call is one dispatch. The registration needs two: `d2_verdict`.
    """
    if registration not in ("A4", "A5"):
        raise ValueError(f"registration must be 'A4' or 'A5', got {registration!r}")
    if registration == "A5" and artifact_4k is None:
        raise ValueError("A5 judges both artifacts of a dispatch: the 4 KiB artifact is required")
    arms = [c["arm"] for c in artifact_2m["cells"]]
    cells = {c["arm"]: c for c in artifact_2m["cells"]}
    missing = [a for a in (GENERATOR_ARM, SORTED_ARM) if a not in cells]
    if missing:
        raise ValueError(f"artifact has no cell for arm(s) {missing}")
    voids = []
    if artifact_2m.get("provenance", {}).get("hugetlb") is not True:
        voids.append("provenance.hugetlb is not true")
    read = ("dTLB-load-misses", "mem_load_retired.l3_miss", "cycles")
    for arm in (GENERATOR_ARM, SORTED_ARM):
        cell = cells[arm]
        if arms.count(arm) != 1:
            voids.append(f"{arm}: recorded {arms.count(arm)} times")
        thp = cell.get("thp_delta", {})
        if thp.get("thp_fault_alloc", 0) <= 0:
            voids.append(f"{arm}: thp_fault_alloc did not increase")
        if thp.get("thp_fault_fallback", 0) > 0:
            voids.append(f"{arm}: {thp['thp_fault_fallback']} huge-page fault(s) fell back to small pages")
        entries = cell["distinct_probes"] * cell["passes"]
        if entries != expected_entries:
            voids.append(f"{arm}: {entries} entries per run, registered {expected_entries}")
        for counter in read:
            c = cell["counters"].get(counter, {})
            if c.get("status") != "ok":
                voids.append(f"{arm}: counter {counter} status is {c.get('status')!r}")
            elif len(c["samples"]) != expected_runs:
                voids.append(f"{arm}: {len(c['samples'])} runs of {counter}, registered {expected_runs}")
    prov = artifact_2m.get("provenance", {})
    ident = {"registration": registration, "commit": prov.get("commit"), "run_id": prov.get("run_id")}
    if registration == "A4":
        host = host_precondition(artifact_2m)
    else:
        prov_4k = artifact_4k.get("provenance", {})
        if prov_4k.get("hugetlb") is not False:
            voids.append("4 KiB artifact: provenance.hugetlb is not false")
        for key in ("commit", "run_id"):
            if not prov.get(key) or prov.get(key) != prov_4k.get(key):
                voids.append(f"the two artifacts do not name one {key}: {prov.get(key)!r} and {prov_4k.get(key)!r}")
        by_artifact = {"2 MiB": a5_host_condition(artifact_2m), "4 KiB": a5_host_condition(artifact_4k)}
        unmet = [f"{name} artifact: {u}" for name, c in by_artifact.items() for u in c["unmet"]]
        host = {"registration": "A5", "met": not unmet, "unmet": unmet, "artifacts": by_artifact}
        # A5.3: a dispatch that is not admissible is VOID, decided before any clause is read.
        voids.extend(unmet)
    if voids:
        return {"verdict": "VOID", "voids": voids, "clauses": {}, "host_precondition": host, **ident}

    def per_entry(arm: str, counter: str) -> list[float]:
        cell = cells[arm]
        return [x / expected_entries for x in cell["counters"][counter]["samples"]]

    def diff(counter: str) -> list[float]:
        return [a - b for a, b in zip(per_entry(GENERATOR_ARM, counter), per_entry(SORTED_ARM, counter), strict=True)]

    bounds = registered_bounds()
    clauses = {}
    for name, data, key, side in (
        ("D2a", per_entry(GENERATOR_ARM, "dTLB-load-misses"), "dtlb_upper", "upper"),
        ("D2b", diff("mem_load_retired.l3_miss"), "l3_delta_lower", "lower"),
        ("D2c", diff("cycles"), "cycle_delta_lower", "lower"),
    ):
        point, lo, hi, method = bca_bootstrap_ci_with_method(data, confidence=0.95)
        bound = bounds[key]
        met = hi <= bound if side == "upper" else lo >= bound
        clauses[name] = {
            "n": len(data), "point": point, "ci": [lo, hi], "ci_method": method,
            "bound": bound, "decided_on": side, "met": met,
        }
    if not all(c["met"] for c in clauses.values()):
        verdict = "FAIL"
    else:
        verdict = "PASS" if host["met"] else "INTERMEDIATE"
    return {"verdict": verdict, "voids": [], "clauses": clauses, "host_precondition": host, **ident}


def d2_verdict(dispatches: list[dict]) -> dict:
    """The D2 verdict over A5 dispatches in the order they ran (METHODOLOGY.md section A5.5).

    Each item is an `evaluate_d2` result under `"A5"`. PASS: two admissible
    dispatches, each PASS. FAIL: an admissible dispatch that misses a clause;
    it is final and is never replaced. A VOID dispatch is replaced, at most
    `void_replacements_max` times across the pair: one more void ends D2 as
    INTERMEDIATE. Until one of those is reached the verdict is INCOMPLETE.

    Raises when the list is not the registered protocol: a result under
    another registration, dispatches at different commits, one run listed
    twice, or a dispatch made after the verdict was already decided.
    """
    cap = a5_host_bounds()["void_replacements_max"]
    commits = {d.get("commit") for d in dispatches}
    runs = [d.get("run_id") for d in dispatches]
    if any(d.get("registration") != "A5" for d in dispatches):
        raise ValueError("every dispatch must be evaluated under A5")
    if dispatches and (len(commits) != 1 or None in commits):
        raise ValueError(f"the dispatches do not share one commit: {sorted(map(str, commits))}")
    if None in runs or len(set(runs)) != len(runs):
        raise ValueError("each dispatch must be its own workflow run")
    admissible = voids = 0
    verdict, why = "INCOMPLETE", ""
    for i, d in enumerate(dispatches):
        if verdict != "INCOMPLETE":
            raise ValueError(f"dispatch {i + 1} was made after the verdict was decided ({verdict})")
        if d["verdict"] == "VOID":
            voids += 1
            if voids > cap:
                verdict = "INTERMEDIATE"
                why = (f"{voids} dispatches were void and at most {cap} replacements are registered; "
                       f"{admissible} admissible dispatch(es) of the 2 required")
        elif d["verdict"] == "FAIL":
            verdict, why = "FAIL", f"dispatch {i + 1} is admissible and does not meet every clause"
        elif d["verdict"] == "PASS":
            admissible += 1
            if admissible == 2:
                verdict, why = "PASS", "two admissible dispatches each meet D2a, D2b and D2c"
        else:
            raise ValueError(f"dispatch {i + 1} has verdict {d['verdict']!r}")
    if verdict == "INCOMPLETE":
        why = f"{admissible} admissible dispatch(es) of the 2 required; {voids} void, {cap - voids} replacement(s) left"
    return {"verdict": verdict, "why": why, "admissible": admissible, "voids": voids,
            "void_replacements_max": cap, "commit": next(iter(commits), None),
            "void_reasons": [d["voids"] for d in dispatches if d["verdict"] == "VOID"]}


def load_d1_reference_values(repo_root: Path) -> dict[str, float]:
    """Loads and computes the reference figures directly from committed D1 JSON artifacts."""
    d1_path = repo_root / "docs/benchmarks/patricia_comparison/results/counters_prefix_scan_d1.json"
    stalls_path = repo_root / "docs/benchmarks/patricia_comparison/results/counters_prefix_scan_d1_stalls.json"
    if not d1_path.exists() or not stalls_path.exists():
        raise FileNotFoundError(f"D1 artifacts not found under {repo_root}")

    with open(d1_path, "r", encoding="utf-8") as f:
        d1 = json.load(f)
    c0, c1 = d1["cells"][0], d1["cells"][1]
    ops = c0["distinct_probes"] * c0["passes"]

    with open(stalls_path, "r", encoding="utf-8") as f:
        stalls = json.load(f)
    s0, s1 = stalls["cells"][0], stalls["cells"][1]

    return {
        "ops": ops,
        "gen_dtlb": c0["counters"]["dTLB-load-misses"]["point"] / ops,
        "sorted_dtlb": c1["counters"]["dTLB-load-misses"]["point"] / ops,
        "gen_l3": c0["counters"]["mem_load_retired.l3_miss"]["point"] / ops,
        "sorted_l3": c1["counters"]["mem_load_retired.l3_miss"]["point"] / ops,
        "gen_cycles": c0["counters"]["cycles"]["point"] / ops,
        "sorted_cycles": c1["counters"]["cycles"]["point"] / ops,
        "stalls_delta_l3": (
            s0["counters"]["cycle_activity.stalls_l3_miss"]["point"]
            - s1["counters"]["cycle_activity.stalls_l3_miss"]["point"]
        ) / ops,
        "stalls_delta_cycles": (
            s0["counters"]["cycles"]["point"]
            - s1["counters"]["cycles"]["point"]
        ) / ops,
    }


class TestPatriciaD2Bounds(unittest.TestCase):
    def test_pinned_d1_constants(self):
        self.assertAlmostEqual(D1_GEN_DTLB_POINT, 0.820341, places=5)
        self.assertAlmostEqual(D1_SORTED_DTLB_POINT, 0.009745, places=5)
        self.assertAlmostEqual(D1_GEN_L3_POINT, 0.236002, places=5)
        self.assertAlmostEqual(D1_SORTED_L3_POINT, -0.000050, places=5)
        self.assertAlmostEqual(D1_BASELINE_DELTA_CYCLES, 85.7330, places=3)
        self.assertAlmostEqual(D1_STALLS_DELTA_L3_CYCLES, 52.5031, places=3)
        self.assertAlmostEqual(D1_STALLS_L3_FRACTION, 0.610895, places=5)

    def test_derived_bounds(self):
        # D2a: 95% reduction from 0.8203 -> 0.0410
        dtlb_bound = dtlb_elimination_upper_bound(D1_GEN_DTLB_POINT, 0.95)
        self.assertEqual(dtlb_bound, 0.0410)

        # D2b: 60% retention of delta L3 (0.2360 - (-0.0000)) -> 0.1416
        delta_l3 = D1_GEN_L3_POINT - D1_SORTED_L3_POINT
        l3_bound = l3_invariance_lower_bound(delta_l3, 0.60)
        self.assertEqual(l3_bound, 0.1416)

        # D2c: cycle gap lower bound matching L3 stalls attribution
        cycle_bound = surviving_cycle_gap_lower_bound(D1_BASELINE_DELTA_CYCLES, D1_STALLS_L3_FRACTION)
        self.assertEqual(cycle_bound, 52.37)
        # The registered D2c bound is the direct stall delta, not that product.
        self.assertEqual(
            registered_bounds(),
            {"dtlb_upper": 0.0410, "l3_delta_lower": 0.1416, "cycle_delta_lower": 52.50},
        )

    def test_invalid_inputs(self):
        with self.assertRaises(ValueError):
            dtlb_elimination_upper_bound(-1.0)
        with self.assertRaises(ValueError):
            dtlb_elimination_upper_bound(0.82, min_reduction=1.5)
        with self.assertRaises(ValueError):
            l3_invariance_lower_bound(0.0)
        with self.assertRaises(ValueError):
            l3_invariance_lower_bound(0.23, min_retention=-0.1)
        with self.assertRaises(ValueError):
            surviving_cycle_gap_lower_bound(-10.0, 0.5)
        with self.assertRaises(ValueError):
            surviving_cycle_gap_lower_bound(85.0, 0.0)

    def test_against_committed_artifacts(self):
        root = Path(__file__).resolve().parent.parent
        d = load_d1_reference_values(root)
        self.assertEqual(d["ops"], D1_OPS)
        self.assertAlmostEqual(d["gen_dtlb"], D1_GEN_DTLB_POINT, places=5)
        self.assertAlmostEqual(d["sorted_dtlb"], D1_SORTED_DTLB_POINT, places=5)
        self.assertAlmostEqual(d["gen_l3"], D1_GEN_L3_POINT, places=5)
        self.assertAlmostEqual(d["sorted_l3"], D1_SORTED_L3_POINT, places=5)
        self.assertAlmostEqual(d["gen_cycles"], D1_GEN_CYCLES_POINT, places=3)
        self.assertAlmostEqual(d["sorted_cycles"], D1_SORTED_CYCLES_POINT, places=3)
        self.assertAlmostEqual(d["stalls_delta_l3"], D1_STALLS_DELTA_L3_CYCLES, places=3)
        self.assertAlmostEqual(d["stalls_delta_cycles"], D1_STALLS_TOTAL_DELTA_CYCLES, places=3)

    QUIET_GUARD = {"start": {"verdict": "quiet", "windows": 3}, "watcher_ok": True,
                   "run": {"verdict": "undisturbed", "samples": 60, "voids": 0, "reasons": []}}

    @staticmethod
    def _artifact(gen: dict, srt: dict, hugetlb=True, thp=(100, 100), fallback=(0, 0),
                  loads=((0.4, None), (0.6, 0.01), (0.7, 0.02)), guard="quiet",
                  commit="c" * 40, run_id="run-1") -> dict:
        def cell(arm, counters, faults, fell_back):
            return {
                "arm": arm, "distinct_probes": 10, "passes": 10,
                "thp_delta": {"thp_fault_alloc": faults, "thp_fault_fallback": fell_back},
                "counters": {k: {"status": "ok", "samples": v} for k, v in counters.items()},
            }
        prov = {"hugetlb": hugetlb, "commit": commit, "run_id": run_id,
                "loads": [{"label": f"snapshot {i + 1}", "load1": l, "foreign_busy_cpus_since_prev": f}
                          for i, (l, f) in enumerate(loads)]}
        if guard is not None:
            prov["host_guard"] = json.loads(json.dumps(TestPatriciaD2Bounds.QUIET_GUARD)) if guard == "quiet" else guard
        return {"provenance": prov,
                "cells": [cell(GENERATOR_ARM, gen, thp[0], fallback[0]), cell(SORTED_ARM, srt, thp[1], fallback[1])]}

    JITTER = [0.0, 1.0, -1.0, 2.0, -2.0, 0.5, -0.5, 1.5, -1.5, 0.0]

    def _good(self):
        # Per entry (100 entries): dTLB 0.02, L3 delta 0.20, cycle delta 60.
        gen = {"dTLB-load-misses": [2.0 + j / 10 for j in self.JITTER],
               "mem_load_retired.l3_miss": [21.0 + j / 10 for j in self.JITTER],
               "cycles": [13000.0 + 10 * j for j in self.JITTER]}
        srt = {"dTLB-load-misses": [1.0] * 10,
               "mem_load_retired.l3_miss": [1.0] * 10,
               "cycles": [7000.0] * 10}
        return gen, srt

    def _control(self, **kw):
        """The 4 KiB artifact of the same dispatch."""
        return self._artifact(*self._good(), hugetlb=False, **kw)

    def _eval(self, artifact, control=None):
        return evaluate_d2(artifact, self._control() if control is None else control, expected_entries=100)

    def _eval_a4(self, artifact):
        return evaluate_d2(artifact, registration="A4", expected_entries=100)

    def test_evaluate_d2(self):
        gen, srt = self._good()
        jitter = self.JITTER
        r = self._eval(self._artifact(gen, srt))
        self.assertEqual(r["verdict"], "PASS")
        self.assertTrue(r["host_precondition"]["met"])
        self.assertEqual({k: v["met"] for k, v in r["clauses"].items()},
                         {"D2a": True, "D2b": True, "D2c": True})
        self.assertAlmostEqual(r["clauses"]["D2c"]["point"], 60.0, places=4)
        # The dispatch is identified, and both of its artifacts were judged under A5.
        self.assertEqual(r["registration"], "A5")
        self.assertEqual(r["commit"], "c" * 40)
        self.assertEqual(r["run_id"], "run-1")
        self.assertEqual(r["voids"], [])
        self.assertEqual(r["host_precondition"]["registration"], "A5")
        self.assertEqual(sorted(r["host_precondition"]["artifacts"]), ["2 MiB", "4 KiB"])
        self.assertTrue(all(a["met"] for a in r["host_precondition"]["artifacts"].values()))
        for c in r["clauses"].values():
            self.assertEqual(c["n"], REGISTERED_RUNS)
            self.assertLessEqual(c["ci"][0], c["point"])
            self.assertLessEqual(c["point"], c["ci"][1])
            self.assertTrue(c["ci_method"])

        # Each clause fails alone: one counter moved across its bound.
        for clause, counter, values in (
            ("D2a", "dTLB-load-misses", [5.0 + j / 10 for j in jitter]),       # 0.05 > 0.0410
            ("D2b", "mem_load_retired.l3_miss", [11.0 + j / 10 for j in jitter]),  # 0.10 < 0.1416
            ("D2c", "cycles", [12000.0 + 10 * j for j in jitter]),             # 50 < 52.50
        ):
            bad = dict(gen)
            bad[counter] = values
            r = self._eval(self._artifact(bad, srt))
            self.assertEqual(r["verdict"], "FAIL", clause)
            self.assertEqual([k for k, v in r["clauses"].items() if not v["met"]], [clause])
            # A missed clause on an admissible dispatch is FAIL, not VOID.
            self.assertEqual(r["voids"], [], clause)
            self.assertTrue(r["host_precondition"]["met"], clause)

        # A point on the right side of its bound with an interval across it fails:
        # the clause is decided on the bound.
        wide = dict(gen)
        wide["cycles"] = [12600.0] * 8 + [11000.0, 14200.0]
        r = self._eval(self._artifact(wide, srt))
        self.assertGreater(r["clauses"]["D2c"]["point"], 52.50)
        self.assertFalse(r["clauses"]["D2c"]["met"])
        self.assertEqual(r["verdict"], "FAIL")

        # D2a is decided on the UPPER bound: a point under the bound whose
        # interval reaches over it fails.
        over = dict(gen)
        over["dTLB-load-misses"] = [3.0] * 8 + [9.0, 9.0]   # mean 0.042 > 0.0410 at the top
        r = self._eval(self._artifact(over, srt))
        self.assertEqual(r["clauses"]["D2a"]["decided_on"], "upper")
        self.assertFalse(r["clauses"]["D2a"]["met"])
        self.assertEqual(r["verdict"], "FAIL")

    def test_evaluate_d2_void(self):
        gen, srt = self._good()

        def void(artifact, needle):
            r = self._eval(artifact)
            self.assertEqual(r["verdict"], "VOID", needle)
            self.assertEqual(r["clauses"], {})
            self.assertTrue(any(needle in v for v in r["voids"]), (needle, r["voids"]))

        void(self._artifact(gen, srt, hugetlb=False), "hugetlb is not true")
        void(self._artifact(gen, srt, thp=(0, 100)), "thp_fault_alloc did not increase")
        void(self._artifact(gen, srt, thp=(100, 0)), "thp_fault_alloc did not increase")
        void(self._artifact(gen, srt, fallback=(0, 7)), "fell back to small pages")
        no_key = self._artifact(gen, srt)
        del no_key["provenance"]["hugetlb"]
        void(no_key, "hugetlb is not true")
        no_thp = self._artifact(gen, srt)
        del no_thp["cells"][0]["thp_delta"]
        void(no_thp, "thp_fault_alloc did not increase")

        # Not the registered measurement: too few runs, arms of unequal
        # length, a different denominator on one arm, an arm recorded twice,
        # a counter that did not count.
        few = {k: v[:5] for k, v in gen.items()}
        void(self._artifact(few, {k: v[:5] for k, v in srt.items()}), "5 runs of")
        void(self._artifact(few, srt), "5 runs of")
        halved = self._artifact(gen, srt)
        halved["cells"][1]["passes"] = 20
        void(halved, "200 entries per run, registered 100")
        twice = self._artifact(gen, srt)
        twice["cells"].append(dict(twice["cells"][1]))
        void(twice, "recorded 2 times")
        dead = self._artifact(gen, srt)
        dead["cells"][1]["counters"]["cycles"]["status"] = "multiplexed"
        void(dead, "status is 'multiplexed'")
        # The registered denominator is D1's: the fixture's 100 is not it.
        self.assertEqual(evaluate_d2(self._artifact(gen, srt), self._control())["verdict"], "VOID")

        with self.assertRaises(ValueError):
            evaluate_d2({"provenance": {"hugetlb": True}, "cells": []}, self._control())
        # A5 judges both artifacts: a dispatch without its 4 KiB artifact is not evaluated.
        with self.assertRaises(ValueError):
            evaluate_d2(self._artifact(gen, srt), expected_entries=100)
        with self.assertRaises(ValueError):
            evaluate_d2(self._artifact(gen, srt), self._control(), registration="A6", expected_entries=100)

    def test_host_precondition(self):
        gen, srt = self._good()
        # Clauses met on a host the registration did not admit as written.
        for loads, needle in (
            (((1.47, None), (1.37, 0.0)), "load1 reached 1.47"),
            (((0.5, None), (0.6, 0.02)), "foreign_busy_cpus reached 0.02"),
            ((), "no load snapshot"),
        ):
            r = self._eval_a4(self._artifact(gen, srt, loads=loads))
            self.assertEqual(r["verdict"], "INTERMEDIATE", needle)
            self.assertTrue(all(c["met"] for c in r["clauses"].values()))
            self.assertTrue(any(needle in u for u in r["host_precondition"]["unmet"]), r["host_precondition"])
        # A failed clause is FAIL whatever the host read.
        bad = dict(gen)
        bad["cycles"] = [12000.0 + 10 * j for j in self.JITTER]
        self.assertEqual(self._eval_a4(self._artifact(bad, srt, loads=((1.9, None),)))["verdict"], "FAIL")
        # Exactly at the registered values is met.
        self.assertTrue(self._eval_a4(self._artifact(gen, srt, loads=((1.0, None), (1.0, 0.0))))["host_precondition"]["met"])

    def test_a5_host_condition(self):
        gen, srt = self._good()
        r = self._eval(self._artifact(gen, srt))
        self.assertEqual((r["verdict"], r["registration"], r["commit"], r["run_id"]), ("PASS", "A5", "c" * 40, "run-1"))
        self.assertEqual(r["host_precondition"]["unmet"], [])

        # Exactly at each bound is met: 0.06 and -0.02 foreign, load1 8.0, a spread of 2.0.
        for loads in (((0.4, None), (0.6, 0.06), (0.7, -0.02)), ((6.0, None), (8.0, 0.0), (7.0, 0.0))):
            self.assertEqual(self._eval(self._artifact(gen, srt, loads=loads))["verdict"], "PASS", loads)

        def void(needle, artifact=None, control=None, host=True):
            r = self._eval(self._artifact(gen, srt) if artifact is None else artifact, control)
            self.assertEqual(r["verdict"], "VOID", needle)
            self.assertEqual(r["clauses"], {}, needle)
            self.assertTrue(any(needle in v for v in r["voids"]), (needle, r["voids"]))
            self.assertEqual(r["host_precondition"]["met"], not host, needle)

        # One clause each, on the 2 MiB artifact.
        void("H1: foreign_busy_cpus 0.07 > 0.06", self._artifact(gen, srt, loads=((0.4, None), (0.6, 0.07), (0.7, 0.0))))
        void("H1: foreign_busy_cpus -0.03 < -0.02", self._artifact(gen, srt, loads=((0.4, None), (0.6, 0.0), (0.7, -0.03))))
        void("H2: load1 reached 8.01 > 8", self._artifact(gen, srt, loads=((7.0, None), (8.01, 0.0), (7.5, 0.0))))
        void("H2: load1 moved by 2.5 > 2", self._artifact(gen, srt, loads=((1.0, None), (3.5, 0.0), (2.0, 0.0))))
        void("2 load snapshot(s), registered 3", self._artifact(gen, srt, loads=((0.4, None), (0.6, 0.0))))
        void("has no foreign reading", self._artifact(gen, srt, loads=((0.4, None), (0.6, None), (0.7, 0.0))))
        void("has no foreign reading", self._artifact(gen, srt, loads=((0.4, None), (0.6, True), (0.7, 0.0))))
        void("H2: a load snapshot has no load1", self._artifact(gen, srt, loads=((None, None), (0.6, 0.0), (0.7, 0.0))))
        # H0: the guard's record.
        void("H0: no host-guard record", self._artifact(gen, srt, guard=None))
        for mutate, needle in (
            (lambda g: g["start"].update(verdict="busy"), "H0: the start gate read 'busy'"),
            (lambda g: g.update(watcher_ok=False), "H0: the host watcher's record is incomplete"),
            (lambda g: g["run"].update(samples=0), "H0: the in-run record holds no sample"),
            (lambda g: g["run"].update(voids=2, reasons=["foreign load on the pinned CPUs 0.389 > 0.25"]),
             "H0: 2 in-run void(s): foreign load on the pinned CPUs 0.389 > 0.25"),
            (lambda g: g["run"].pop("voids"), "H0: None in-run void(s)"),
        ):
            g = json.loads(json.dumps(self.QUIET_GUARD))
            mutate(g)
            void(needle, self._artifact(gen, srt, guard=g))
        # The same clauses on the 4 KiB control void the dispatch too.
        void("4 KiB artifact: H1: foreign_busy_cpus 0.07 > 0.06",
             control=self._control(loads=((0.4, None), (0.6, 0.07), (0.7, 0.0))))
        void("4 KiB artifact: H0: no host-guard record", control=self._control(guard=None))
        void("4 KiB artifact: provenance.hugetlb is not false", control=self._artifact(gen, srt), host=False)
        void("do not name one commit", control=self._control(commit="d" * 40), host=False)
        void("do not name one run_id", control=self._control(run_id="run-2"), host=False)
        # An inadmissible dispatch is VOID even when a clause would fail: no clause is read.
        bad = dict(gen)
        bad["cycles"] = [12000.0 + 10 * j for j in self.JITTER]
        self.assertEqual(self._eval(self._artifact(bad, srt))["verdict"], "FAIL")
        void("H1: foreign_busy_cpus 0.07", self._artifact(bad, srt, loads=((0.4, None), (0.6, 0.07), (0.7, 0.0))))

    def test_d2_verdict(self):
        def d(verdict, run):
            return {"verdict": verdict, "registration": "A5", "commit": "c" * 40, "run_id": f"run-{run}",
                    "voids": ["H1: x"] if verdict == "VOID" else []}

        def seq(*verdicts):
            return d2_verdict([d(v, i) for i, v in enumerate(verdicts)])

        self.assertEqual(seq("PASS", "PASS")["verdict"], "PASS")
        self.assertEqual(seq()["verdict"], "INCOMPLETE")
        self.assertEqual(seq("PASS")["verdict"], "INCOMPLETE")
        self.assertEqual(seq("PASS", "VOID")["verdict"], "INCOMPLETE")
        # A FAIL in an admissible dispatch is final, wherever it falls.
        self.assertEqual(seq("FAIL")["verdict"], "FAIL")
        self.assertEqual(seq("PASS", "VOID", "FAIL")["verdict"], "FAIL")
        # The cap: three voids are replaced, a fourth ends D2 as INTERMEDIATE.
        self.assertEqual(seq("VOID", "VOID", "VOID", "PASS", "PASS")["verdict"], "PASS")
        self.assertEqual(seq("VOID", "PASS", "VOID", "VOID", "PASS")["verdict"], "PASS")
        r = seq("VOID", "VOID", "VOID", "PASS")
        self.assertEqual((r["verdict"], r["voids"], r["admissible"]), ("INCOMPLETE", 3, 1))
        self.assertIn("0 replacement(s) left", r["why"])
        r = seq("VOID", "VOID", "VOID", "PASS", "VOID")
        self.assertEqual((r["verdict"], r["voids"], r["admissible"]), ("INTERMEDIATE", 4, 1))
        self.assertIn("at most 3 replacements", r["why"])
        self.assertEqual(len(r["void_reasons"]), 4)
        self.assertEqual(seq("VOID", "VOID", "VOID", "VOID")["verdict"], "INTERMEDIATE")
        # Not the registered protocol.
        for bad in (
            [d("PASS", 1), d("PASS", 1)],                                   # one run listed twice
            [d("PASS", 1), dict(d("PASS", 2), commit="d" * 40)],            # two commits
            [d("PASS", 1), dict(d("PASS", 2), registration="A4")],          # an A4 reading
            [d("PASS", 1), d("PASS", 2), d("PASS", 3)],                     # a dispatch after the verdict
            [d("FAIL", 1), d("PASS", 2)],                                   # a replaced FAIL
            [d("PASS", 1), d("INTERMEDIATE", 2)],
            [dict(d("PASS", 1), commit=None)],
        ):
            with self.assertRaises(ValueError):
                d2_verdict(bad)

    def test_expected_load1(self):
        # Reference values of the closed form.
        self.assertAlmostEqual(expected_load1(0.0, 1.0, 60.0), 1 - 0.36787944, places=6)
        self.assertAlmostEqual(expected_load1(2.0, 1.0, 60.0), 1 + 0.36787944, places=6)
        self.assertEqual(expected_load1(1.0, 1.0, 1e6), 1.0)
        self.assertEqual(expected_load1(1.7, 1.0, 0.0), 1.7)
        # One 5 s kernel step of the recurrence: e = EXP_1 / FIXED_1 = 1884 / 2048.
        self.assertAlmostEqual(expected_load1(2.0, 1.0, 5.0), 1 + 1884 / 2048, places=3)
        # Never crosses `runnable`, from either side.
        for load0 in (0.0, 0.5, 1.0, 1.5, 3.0):
            for t in (0.0, 1.0, 20.0, 600.0):
                got = expected_load1(load0, 1.0, t)
                self.assertGreaterEqual(got, min(load0, 1.0))
                self.assertLessEqual(got, max(load0, 1.0))
        for bad in ((-1.0, 1.0, 1.0), (1.0, -1.0, 1.0), (1.0, 1.0, -1.0)):
            with self.assertRaises(ValueError):
                expected_load1(*bad)
        with self.assertRaises(ValueError):
            expected_load1(1.0, 1.0, 1.0, tau_s=0.0)

    def test_expected_load1_against_committed_snapshots(self):
        # Every consecutive pair of load snapshots in the committed D1 and D2
        # counter artifacts, each a window in which the driver ran one
        # single-threaded workload process at a time: the recorded `load1` at
        # the end of the window against the closed form from the one at its
        # start with one task runnable. Observational, 12 windows.
        results = Path(__file__).resolve().parent.parent / "docs/benchmarks/patricia_comparison/results"
        names = ["counters_prefix_scan_d1.json", "counters_prefix_scan_d1_stalls.json"] + [
            f"counters_prefix_scan_d2_{pages}_run{run}.json" for run in (1, 2) for pages in ("4k", "2m")]
        windows, worst = 0, 0.0
        for name in names:
            with open(results / name, encoding="utf-8") as fh:
                loads = json.load(fh)["provenance"]["loads"]
            for prev, cur in zip(loads, loads[1:]):
                predicted = expected_load1(prev["load1"], 1.0, cur["monotonic_s"] - prev["monotonic_s"])
                worst = max(worst, abs(predicted - cur["load1"]))
                windows += 1
        self.assertEqual(windows, 12)
        self.assertLess(worst, 0.05)  # worst recorded residual is 0.03

    def test_a5_host_bounds(self):
        # (52.50 - 85.9445 / 2) / ((2 - 1) * 158.6335) = 9.5278 / 158.6335 = 0.06006
        self.assertAlmostEqual(D1_STALLS_TOTAL_DELTA_CYCLES / 2, 42.9722, places=3)
        self.assertAlmostEqual(7_921_202_316.5 / D1_OPS, 158.6335, places=3)
        self.assertEqual(a5_host_bounds(), {
            "foreign_cell_max": 0.06, "foreign_cell_min": -0.02,
            "load1_max": 8.0, "load1_spread_max": 2.0, "void_replacements_max": 3})
        self.assertAlmostEqual(foreign_load_ceiling(52.50, 42.9722, 158.6335, 2.0), 0.06, places=9)
        # The ceiling's dependence on the assumed slowdown.
        self.assertAlmostEqual(foreign_load_ceiling(52.50, 42.9722, 158.6335, 3.0), 0.03, places=9)
        self.assertAlmostEqual(foreign_load_ceiling(52.50, 42.9722, 158.6335, 4.0), 0.02, places=9)
        self.assertAlmostEqual(foreign_reading_floor(), -0.02, places=9)
        for bad in ((52.5, 52.5, 158.0, 2.0), (52.5, -1.0, 158.0, 2.0), (52.5, 42.0, 0.0, 2.0), (52.5, 42.0, 158.0, 1.0)):
            with self.assertRaises(ValueError):
                foreign_load_ceiling(*bad)
        with self.assertRaises(ValueError):
            foreign_reading_floor(0)

    def test_committed_runs(self):
        # The two runs as committed, read under A4 as they were dispatched:
        # every clause met, no void, and A4's host condition not met as written
        # (load1 above 1.0, foreign above 0). They are not re-read under A5:
        # an artifact dispatched before A5 carries no guard record and is VOID
        # there, whatever its snapshots read.
        results = Path(__file__).resolve().parent.parent / "docs/benchmarks/patricia_comparison/results"
        for run, d2c_lower in ((1, 65.28), (2, 66.10)):
            with open(results / f"counters_prefix_scan_d2_2m_run{run}.json", encoding="utf-8") as fh:
                artifact = json.load(fh)
            with open(results / f"counters_prefix_scan_d2_4k_run{run}.json", encoding="utf-8") as fh:
                a5 = evaluate_d2(artifact, json.load(fh))
            self.assertEqual(a5["verdict"], "VOID", run)
            self.assertEqual(a5["voids"], ["2 MiB artifact: H0: no host-guard record in provenance",
                                           "4 KiB artifact: H0: no host-guard record in provenance"])
            r = evaluate_d2(artifact, registration="A4")
            self.assertEqual(r["verdict"], "INTERMEDIATE", run)
            self.assertEqual(r["voids"], [])
            self.assertTrue(all(c["met"] for c in r["clauses"].values()))
            self.assertAlmostEqual(r["clauses"]["D2c"]["ci"][0], d2c_lower, places=2)
            self.assertFalse(r["host_precondition"]["met"])
            self.assertGreater(r["host_precondition"]["load1_max"], 1.0)


def main() -> int:
    def load(path: str) -> dict:
        with open(path, "r", encoding="utf-8") as fh:
            return json.load(fh)

    mode, paths = sys.argv[1:2], sys.argv[2:]
    if mode == ["--evaluate-a4"]:
        if len(paths) != 1:
            print("usage: patricia_d2_bounds.py --evaluate-a4 <counters_prefix_scan_d2_2m.json>", file=sys.stderr)
            return 2
        print(json.dumps(evaluate_d2(load(paths[0]), registration="A4"), indent=2))
        return 0
    if mode == ["--evaluate"]:
        if len(paths) != 2:
            print("usage: patricia_d2_bounds.py --evaluate <d2_2m.json> <d2_4k.json>", file=sys.stderr)
            return 2
        print(json.dumps(evaluate_d2(load(paths[0]), load(paths[1])), indent=2))
        return 0
    if mode == ["--verdict"]:
        if not paths or len(paths) % 2:
            print("usage: patricia_d2_bounds.py --verdict <d2_2m.json> <d2_4k.json> [<d2_2m.json> <d2_4k.json> ...]",
                  file=sys.stderr)
            return 2
        dispatches = [evaluate_d2(load(a), load(b)) for a, b in zip(paths[::2], paths[1::2])]
        print(json.dumps({"verdict": d2_verdict(dispatches), "dispatches": dispatches}, indent=2))
        return 0
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestPatriciaD2Bounds)
    runner = unittest.TextTestRunner(stream=sys.stdout, verbosity=2)
    result = runner.run(suite)
    if not result.wasSuccessful():
        return 1

    bounds = registered_bounds()
    dtlb_bound = bounds["dtlb_upper"]
    delta_l3 = D1_GEN_L3_POINT - D1_SORTED_L3_POINT
    l3_bound = bounds["l3_delta_lower"]
    cycle_bound = bounds["cycle_delta_lower"]

    print("\n--- Diagnostic D2 Pre-Registered Bounds ---")
    print(f"Clause D2a (dTLB Elimination): BCa_upper(generator_2m.dtlb) <= {dtlb_bound} misses/entry (95% reduction from {D1_GEN_DTLB_POINT:.4f})")
    print(f"Clause D2b (L3 Miss Invariance): BCa_lower(generator_2m.l3 - sorted_2m.l3) >= {l3_bound} misses/entry (60% retention of {delta_l3:.4f})")
    print(f"Clause D2c (Surviving Cycle Gap): BCa_lower(generator_2m.cycles - sorted_2m.cycles) >= {cycle_bound} cycles/entry (pinning L3 stalls delta {D1_STALLS_DELTA_L3_CYCLES:.2f})")
    print("All derivations verified.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
