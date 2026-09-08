#!/usr/bin/env python3
"""Committed benchmark artifacts must carry the fields that make them recomputable (#732).

The defect this pins: `hot_comparison`'s and `art_comparison`'s runners took one
load average per phase and kept no raw rows, and `hashbrown_comparison`,
`redis_zset_engine` and `search_inverted_index` took no load snapshot at all. A
reader of those artifacts cannot recompute a published median or ratio, cannot
tell what the ratio column *is*, cannot see the host's frequency governor or
huge-page mode, and cannot tell whether another process was resident while the
numbers were taken. Only `masstree_comparison` published all of it.

`scripts/bench_provenance.py` is that code, shared. This gate is what stops the
fields dropping back out (AGENTS.md section 8.12).

## What is required, and of which artifacts

Every committed `docs/benchmarks/*/results/baseline_*.json` must carry
`provenance.host`, `provenance.estimators` and per-cell `rounds_raw`, **unless
it is grandfathered below**.

Grandfathering is by explicit entry, not by a date or a commit comparison: an
artifact is listed with the commit it was measured at, and the gate fails if
the file on disk carries a *different* commit — that is, if it was re-measured
and not brought up to standard. So an old artifact stays legal until someone
re-runs its suite, and the re-run cannot land without the fields. Removing an
entry is a deliberate, reviewable edit; adding one requires saying why.

A `rounds_raw` requirement is only meaningful where a cell has rounds. Memory
and census artifacts are exact byte counts with no rounds and no interval
(section 8.4), so they are required to carry `host` and `estimators` and are
exempt from `rounds_raw`; the exemption is per artifact and stated, never
inferred from the file being empty of them.

## The concurrent artifacts owe attribution as well (#568 Step 0)

A concurrent sweep's busy-CPU delta is the sweep's own threads — 5.7
core-equivalents on the reference host — so it cannot say whether anything else
was resident, and `scaling_governor` read from `cpu0` cannot say what governor
the other fifteen pinned CPUs ran under. Every `baseline_concurrent*.json` must
therefore carry, per throughput and health cell, `load.foreign_busy_cpus` — the
host's busy CPU over that cell minus the runner's own children's, a number and
never `None` — and `provenance.host.scaling_governor_by_cpu` with the pin set it
was read for. The four artifacts measured before the runners recorded these are
grandfathered by the same commit-pinned mechanism: re-measure at another commit
and the fields are required.

Usage:
  python3 scripts/check_bench_provenance.py
  python3 scripts/check_bench_provenance.py --self-test
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BENCH = REPO_ROOT / "docs" / "benchmarks"

# Artifacts measured before the shared module existed. Key: path relative to
# `docs/benchmarks/`. Value: the `provenance.commit`(s) they were measured at —
# a tuple where more than one is legal — or None where the artifact carries no
# commit at all. Re-measuring at any *other* commit makes the gate require the
# fields.
#
# The `art_comparison` artifacts left this table when the suite was re-run at
# the scan-start fix (#745) with the shared module in the tree, and the
# `hot_comparison` single-threaded and concurrent artifacts when it was re-run
# at `0f4fd40c`:
# they now carry `host`, `estimators`, busy-CPU deltas and per-cell
# `rounds_raw`, so the gate enforces them like any other. Only the instrument
# bridge remains, and it is not re-measured by that runner.
GRANDFATHERED = {
    "hot_comparison/results/baseline_instrument_bridge.json": ("86daaddf",),
    # rocksdb_memtable — a `expanse.baseline.v1` artifact from
    # `scripts/bench_baseline.py`, whose provenance block names the host and
    # the run but carries no load snapshot and no per-cell rounds. It publishes
    # wall-clock throughput ratios against RocksDB's SkipMap
    # (`docs/BENCHMARKING.md` §12), so it is named here rather than left out of
    # the gate's scope: the entry is what makes its next re-run land the fields.
    "rocksdb_memtable/results/baseline_rocksdb.json": ("6cb64b459e753c73b305cddd56fedef1fe31a0e1",),
    # hashbrown_comparison, redis_zset_engine, search_inverted_index — these
    # runners took no load snapshot at all before this change, and several of
    # their artifacts are bare JSON arrays.
    "hashbrown_comparison/results/baseline_native.json": None,
    "hashbrown_comparison/results/baseline_ycsb.json": None,
    "hashbrown_comparison/results/baseline_tail_latency.json": None,
    "hashbrown_comparison/results/baseline_distributions.json": None,
    "hashbrown_comparison/results/baseline_memory.json": None,
    "redis_zset_engine/results/baseline_zadd.json": None,
    "redis_zset_engine/results/baseline_range.json": None,
    "redis_zset_engine/results/baseline_rank.json": None,
    "redis_zset_engine/results/baseline_memory.json": None,
    "search_inverted_index/results/baseline_boolean.json": None,
    "search_inverted_index/results/baseline_wand.json": None,
    "search_inverted_index/results/baseline_memory.json": None,
}

# A cell list under this key is a memory census: exact byte counts, no rounds
# and no interval (section 8.4). Required to carry `host` and `estimators`;
# exempt from `rounds_raw`, by a stated rule rather than inferred from the cells
# happening to lack them.
CENSUS_KEYS = {"memory"}

# Whole artifacts that are censuses, for the same reason.
NO_ROUNDS = {
    "art_comparison/results/baseline_memory.json",
    "hot_comparison/results/baseline_memory_curve.json",
    "hot_comparison/results/baseline_string_memory.json",
    "masstree_comparison/results/baseline_memory.json",
    "masstree_comparison/results/baseline_string_memory.json",
}

# The comparative suites this gate governs: the ones whose runners drive an
# Expanse arm against a competitor and publish a ratio. Other suites under
# `docs/benchmarks/` (on-device, fuel-counted, single-arm) have their own
# instruments and are not in scope for #732.
SUITES = (
    "art_comparison", "hot_comparison", "hashbrown_comparison",
    "redis_zset_engine", "search_inverted_index", "masstree_comparison",
    "rocksdb_memtable",
)

# Keys under which an artifact holds its cells.
CELL_KEYS = ("cells", "results", "throughput", "health", "latency", "memory")

# Concurrent artifacts measured before the runners took a load snapshot per
# cell with the runner's own child CPU split out, and read the governor of
# every pinned CPU (#568 Step 0). Same mechanism as GRANDFATHERED: the entry
# names the commit, and a re-measurement at any other commit must carry
# `load.foreign_busy_cpus` on every throughput and health cell and
# `host.scaling_governor_by_cpu`. Both runs of each suite are listed because
# rule 18 commits both.
# Empty since the four concurrent artifacts were re-measured at a1982ff2 with
# per-cell attribution (#568 Step 0); the mechanism stays for the next artifact
# that predates a field.
ATTRIBUTION_GRANDFATHERED: dict[str, tuple[str, ...]] = {}

# The cell lists in a concurrent artifact that are timed or counted under
# thread load, and so owe a per-cell attribution. `memory` there is a
# single-writer build-only census (section 8.4) and does not.
ATTRIBUTED_CELL_KEYS = ("throughput", "health")


def is_concurrent(rel: str) -> bool:
    return Path(rel).name.startswith("baseline_concurrent")


def cell_lists(obj: dict) -> list[tuple[str, list]]:
    out = []
    for k in CELL_KEYS:
        v = obj.get(k)
        if isinstance(v, list) and v and isinstance(v[0], dict):
            out.append((k, v))
    return out


def has_rounds(cell: dict) -> bool:
    """Whether a cell carries its rounds.

    Directly, or in the phase objects it groups: `art_comparison`'s
    small-payload cell is one population holding a memory census and three
    timed phases, and the rounds belong to the phases. A cell that groups
    phases and carries nothing anywhere still fails.
    """
    if not isinstance(cell, dict):
        return False
    if cell.get("rounds_raw"):
        return True
    return any(isinstance(v, dict) and v.get("rounds_raw") for v in cell.values())


def check_artifact(rel: str, obj) -> list[str]:
    """Findings for one artifact, already known to be non-grandfathered."""
    problems = []
    if not isinstance(obj, dict):
        return [f"{rel}: is a bare JSON array — wrap it with bench_provenance.attach()"]
    prov = obj.get("provenance")
    if not isinstance(prov, dict):
        return [f"{rel}: no `provenance` block"]
    if not isinstance(prov.get("host"), dict):
        problems.append(f"{rel}: `provenance.host` missing — bench_provenance.host_facts()")
    if not isinstance(prov.get("estimators"), dict):
        problems.append(f"{rel}: `provenance.estimators` missing — "
                        f"say what the ratio column is, not what a reader guesses")
    loads = prov.get("loads")
    if not isinstance(loads, list) or not loads:
        problems.append(f"{rel}: `provenance.loads` missing or empty")
    elif not any("busy_cpus_since_prev" in s for s in loads if isinstance(s, dict)):
        problems.append(f"{rel}: load snapshots carry no `busy_cpus_since_prev` — "
                        f"the load average lags a heavy process by about thirty seconds")

    if rel in NO_ROUNDS:
        return problems

    lists = cell_lists(obj)
    if not lists:
        problems.append(f"{rel}: no cell list found under any of {CELL_KEYS}")
        return problems
    for key, cells in lists:
        if key in CENSUS_KEYS:
            continue
        without = [i for i, c in enumerate(cells) if not has_rounds(c)]
        if without:
            problems.append(
                f"{rel}: {len(without)} of {len(cells)} cells under `{key}` carry no "
                f"`rounds_raw` (first at index {without[0]}) — a published median and "
                f"ratio cannot be recomputed without the rounds they summarise"
            )
    return problems


def check_attribution(rel: str, obj) -> list[str]:
    """Findings for a concurrent artifact that is not attribution-grandfathered.

    `foreign_busy_cpus` must be a number on every timed or counted cell: a
    `None` there is a snapshot that could not attribute, which is not a
    smaller finding than a missing one (section 8.1). The governor map must
    be present with the pin set it was read for.
    """
    problems = []
    if not isinstance(obj, dict) or not isinstance(obj.get("provenance"), dict):
        return problems  # check_artifact already reported the block
    host = obj["provenance"].get("host")
    if isinstance(host, dict):
        if "scaling_governor_by_cpu" not in host or "scaling_governor_pin_set" not in host:
            problems.append(
                f"{rel}: `provenance.host` carries no `scaling_governor_by_cpu` / "
                f"`scaling_governor_pin_set` — the governor of cpu0 does not say what the "
                f"other pinned CPUs ran under; bench_provenance.host_facts() records both"
            )
    for key in ATTRIBUTED_CELL_KEYS:
        cells = obj.get(key)
        if not isinstance(cells, list):
            continue
        bad = [i for i, c in enumerate(cells)
               if not (isinstance(c, dict) and isinstance(c.get("load"), dict)
                       and isinstance(c["load"].get("foreign_busy_cpus"), (int, float))
                       and not isinstance(c["load"].get("foreign_busy_cpus"), bool))]
        if bad:
            problems.append(
                f"{rel}: {len(bad)} of {len(cells)} cells under `{key}` carry no numeric "
                f"`load.foreign_busy_cpus` (first at index {bad[0]}) — the sweep's busy-CPU "
                f"delta is its own threads, so a per-cell split of own and foreign CPU is "
                f"what says whether anything else was resident (bench_provenance.begin_cell "
                f"/ end_cell)"
            )
    return problems


def attribution_status(rel: str, obj) -> tuple[bool, str | None]:
    """`(exempt, finding)` for the concurrent-attribution grandfather table.

    Exempt only when the artifact is listed *and* carries a listed commit; a
    listed artifact at another commit was re-measured, is not exempt, and
    says so.
    """
    if rel not in ATTRIBUTION_GRANDFATHERED:
        return False, None
    allowed = ATTRIBUTION_GRANDFATHERED[rel]
    got = obj.get("provenance", {}).get("commit") if isinstance(obj, dict) else None
    if got in allowed:
        return True, None
    return False, (
        f"{rel}: attribution-grandfathered at commit(s) {allowed!r} but carries {got!r} — "
        f"it was re-measured, so it must now carry per-cell load.foreign_busy_cpus and "
        f"host.scaling_governor_by_cpu; drop its ATTRIBUTION_GRANDFATHERED entry"
    )


def findings_for(rel: str, obj) -> list[str]:
    """Every finding for one non-grandfathered artifact, both requirement sets."""
    out = check_artifact(rel, obj)
    if is_concurrent(rel):
        exempt, note = attribution_status(rel, obj)
        if note:
            out.append(note)
        if not exempt:
            out.extend(check_attribution(rel, obj))
    return out


def artifacts() -> list[Path]:
    out = []
    for suite in SUITES:
        out.extend(sorted((BENCH / suite / "results").glob("baseline_*.json")))
    return out


def run() -> int:
    findings, checked, grandfathered, attribution_grandfathered = [], 0, 0, 0
    for path in artifacts():
        rel = str(path.relative_to(BENCH))
        try:
            obj = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError) as exc:
            findings.append(f"{rel}: cannot be read as JSON ({exc})")
            continue
        if rel in GRANDFATHERED:
            want = GRANDFATHERED[rel]
            allowed = want if isinstance(want, tuple) else (want,)
            got = obj.get("provenance", {}).get("commit") if isinstance(obj, dict) else None
            if got in allowed:
                grandfathered += 1
                continue
            findings.append(
                f"{rel}: grandfathered at commit(s) {allowed!r} but carries {got!r} — it "
                f"was re-measured, so it must now carry provenance.host, "
                f"provenance.estimators and per-cell rounds_raw; drop its "
                f"GRANDFATHERED entry"
            )
            # Fall through and report exactly what it is missing.
        checked += 1
        if is_concurrent(rel) and attribution_status(rel, obj)[0]:
            attribution_grandfathered += 1
        findings.extend(findings_for(rel, obj))

    if findings:
        for f in findings:
            print(f"::error::check_bench_provenance.py: {f}")
        print(f"check_bench_provenance.py: {len(findings)} finding(s) over "
              f"{checked} enforced artifact(s)")
        return 1
    print(f"check_bench_provenance.py: {checked} artifact(s) carry host, estimators, "
          f"busy-CPU deltas and per-cell rounds_raw; {grandfathered} grandfathered; "
          f"{attribution_grandfathered} concurrent artifact(s) grandfathered from per-cell "
          f"attribution and the per-CPU governor map (#568 Step 0)")
    return 0


# --------------------------------------------------------------------------
# self-test: the motivating defect, pinned (AGENTS.md section 8.12)
# --------------------------------------------------------------------------

_GOOD = {
    "provenance": {
        "commit": "abc1234",
        "host": {"cpu_model": "x", "scaling_governor": "performance"},
        "estimators": {"ratio": "mean(A)/mean(B) with a two-sample BCa 95% interval",
                       "columns": "medians", "raw": "rounds_raw"},
        "loads": [{"label": "start", "load1": 0.0, "busy_cpus_since_prev": None},
                  {"label": "end", "load1": 1.0, "busy_cpus_since_prev": 1.02}],
    },
    "cells": [{"pillar": "lookup_hit",
               "rounds_raw": [{"round": 0, "first_arm": "hot", "hot_ns_per_op": 1.0}]}],
}

# A concurrent artifact as the runners write it from #568 Step 0 on: the
# governor map over the pin set, and a per-cell load with the runner's own
# child CPU split from the host's.
_GOOD_CONC = {
    "provenance": {
        "commit": "fedcba9",
        "host": {"cpu_model": "x", "scaling_governor": "powersave",
                 "scaling_governor_by_cpu": {"0": "powersave", "1": "powersave"},
                 "scaling_governor_pin_set": "0-1",
                 "scaling_governor_pin_source": "EXPANSE_BENCH_PIN_APPLIED"},
        "estimators": {"ratio": "mean(A)/mean(B)", "columns": "medians", "raw": "rounds_raw"},
        "loads": [{"label": "start", "busy_cpus_since_prev": None},
                  {"label": "cell:set:W1:R0", "since": "start", "busy_cpus_since_prev": 1.01,
                   "own_busy_cpus_since_prev": 0.0, "foreign_busy_cpus_since_prev": 1.01},
                  {"label": "after-concurrent", "since": "cell:set:W1:R0",
                   "busy_cpus_since_prev": 1.98, "own_busy_cpus_since_prev": 0.97,
                   "foreign_busy_cpus_since_prev": 1.01}],
    },
    "throughput": [{"arm": "set", "writers": 1, "readers": 0,
                    "rounds_raw": [{"round": 0, "expanse_writer_mops": 8.6}],
                    "load": {"since": "cell:set:W1:R0", "wall_s": 12.0, "busy_cpus_since_prev": 1.98,
                             "own_busy_cpus": 0.97, "foreign_busy_cpus": 1.01}}],
    "health": [{"arm": "set", "writers": 1, "readers": 8,
                "rounds_raw": [{"round": 0, "read_ops": 100}],
                "load": {"since": "cell:set:W1:R8", "wall_s": 6.0, "busy_cpus_since_prev": 9.1,
                         "own_busy_cpus": 8.9, "foreign_busy_cpus": 0.2}}],
    "memory": [{"lambda_target": 1.0, "expanse_alloc_bytes_per_key": 14.1}],
}
# A listed path, for the grandfather cases, and an unlisted concurrent path,
# for the field cases — a listed path at a foreign commit is itself a finding.
_CONC_OLD = "hot_comparison/results/baseline_concurrent.json"
_CONC_REL = "hot_comparison/results/baseline_concurrent_run3.json"


def _self_test() -> int:
    import copy
    failures = []

    def expect(name, obj, want_substr, rel="fixture.json"):
        got = findings_for(rel, obj)
        if want_substr is None:
            if got:
                failures.append(f"{name}: expected no finding, got {got}")
            return
        if not any(want_substr in g for g in got):
            failures.append(f"{name}: expected a finding mentioning {want_substr!r}, got {got}")

    expect("a complete artifact passes", copy.deepcopy(_GOOD), None)

    # THE MOTIVATING DEFECT, both halves. "One loadavg per phase and no raw
    # rows" must fail — a gate that passes here measures the wrong invariant.
    no_raw = copy.deepcopy(_GOOD)
    del no_raw["cells"][0]["rounds_raw"]
    expect("no raw rows", no_raw, "rounds_raw")

    one_loadavg = copy.deepcopy(_GOOD)
    one_loadavg["provenance"]["loads"] = [{"label": "start", "load1": 0.0},
                                          {"label": "end", "load1": 1.0}]
    expect("load averages with no jiffy delta", one_loadavg, "busy_cpus_since_prev")

    no_host = copy.deepcopy(_GOOD)
    del no_host["provenance"]["host"]
    expect("no host facts", no_host, "provenance.host")

    no_est = copy.deepcopy(_GOOD)
    del no_est["provenance"]["estimators"]
    expect("no estimators block", no_est, "provenance.estimators")

    no_prov = {"cells": []}
    expect("no provenance block", no_prov, "no `provenance` block")

    expect("a bare array", [1, 2, 3], "bare JSON array")

    # A partially-compliant artifact is a finding, not a pass: one cell without
    # rounds_raw among many is exactly how the field drops out again.
    partial = copy.deepcopy(_GOOD)
    partial["cells"].append({"pillar": "insert"})
    expect("one cell of two without raw rows", partial, "1 of 2 cells")

    # A cell may hold its rounds in the phase objects it groups, and a cell
    # that groups phases and carries rounds nowhere is still a finding
    # (art_comparison's small-payload cell, #745).
    phased = copy.deepcopy(_GOOD)
    phased["cells"] = [{
        "population": 7,
        "memory": {"expanse_bpk": 24.0},
        "lookup_hit": {"rounds_raw": [{"round": 0, "expanse_ns": 1.0}]},
    }]
    expect("a cell whose rounds live in its phases", phased, None)

    phased_empty = copy.deepcopy(_GOOD)
    phased_empty["cells"] = [{"population": 7, "lookup_hit": {"expanse_ns_op": 1.0}}]
    expect("a cell that groups phases and carries no rounds", phased_empty, "rounds_raw")

    # `results` is a cell key: art_comparison's harnesses publish under it, and
    # dropping it from CELL_KEYS would report the suite as having no cells at
    # all rather than checking the cells it has.
    under_results = copy.deepcopy(_GOOD)
    under_results["results"] = under_results.pop("cells")
    expect("cells published under `results`", under_results, None)

    under_results_bad = copy.deepcopy(_GOOD)
    under_results_bad["results"] = under_results_bad.pop("cells")
    del under_results_bad["results"][0]["rounds_raw"]
    expect("`results` cells without raw rows", under_results_bad, "rounds_raw")

    # Every grandfathered path must actually exist, or the list is silently
    # exempting nothing and rotting.
    # Every grandfathered path must exist, or the list is exempting nothing and
    # rotting. The two sensitivity artifacts and the second concurrent run
    # arrive with #731 / #733 / #735, so they are allowed to be absent until
    # then and are named rather than silently skipped.
    pending = {
        "hot_comparison/results/baseline_sensitivity.json",
        "hot_comparison/results/baseline_string_sensitivity.json",
        "hot_comparison/results/baseline_concurrent_run2.json",
    }
    for rel in GRANDFATHERED:
        if (BENCH / rel).is_file():
            continue
        if rel in pending:
            print(f"  note: {rel} not present yet — arrives with #731 / #733 / #735")
            continue
        failures.append(f"GRANDFATHERED names a missing artifact: {rel}")
    for rel in NO_ROUNDS:
        if not (BENCH / rel).is_file():
            failures.append(f"NO_ROUNDS names a missing artifact: {rel}")

    # --- #568 Step 0: per-cell attribution and the per-CPU governor map -----
    # THE MOTIVATING DEFECT: a concurrent artifact whose busy-CPU delta is
    # its own sixteen threads and whose governor is cpu0's. Complete passes;
    # each half missing is a finding; a non-concurrent artifact owes neither.
    expect("a complete concurrent artifact passes", copy.deepcopy(_GOOD_CONC), None, _CONC_REL)

    no_split = copy.deepcopy(_GOOD_CONC)
    del no_split["throughput"][0]["load"]
    expect("a throughput cell with no per-cell load", no_split, "foreign_busy_cpus", _CONC_REL)

    none_split = copy.deepcopy(_GOOD_CONC)
    none_split["health"][0]["load"]["foreign_busy_cpus"] = None
    expect("a health cell whose foreign_busy_cpus is None", none_split,
           "foreign_busy_cpus", _CONC_REL)

    no_map = copy.deepcopy(_GOOD_CONC)
    del no_map["provenance"]["host"]["scaling_governor_by_cpu"]
    expect("cpu0's governor only", no_map, "scaling_governor_by_cpu", _CONC_REL)

    no_set = copy.deepcopy(_GOOD_CONC)
    del no_set["provenance"]["host"]["scaling_governor_pin_set"]
    expect("a governor map with no pin set named", no_set, "scaling_governor_pin_set", _CONC_REL)

    # The requirement is scoped to concurrent artifacts: the same shape under a
    # latency name owes host/estimators/rounds_raw and nothing more.
    expect("a non-concurrent artifact is not asked for attribution",
           copy.deepcopy(_GOOD), None, "hot_comparison/results/baseline_latency.json")
    lat_shaped = copy.deepcopy(_GOOD_CONC)
    del lat_shaped["throughput"][0]["load"]
    expect("a non-concurrent artifact without per-cell load passes", lat_shaped, None,
           "hot_comparison/results/baseline_latency.json")

    # The grandfather mechanism, both directions: the pinned commit is exempt,
    # any other commit is enforced and told why.
    # The list may be empty (every committed concurrent artifact attributed);
    # pin the mechanism on a synthetic entry rather than on a real one.
    old = copy.deepcopy(_GOOD_CONC)
    del old["throughput"][0]["load"]
    del old["provenance"]["host"]["scaling_governor_by_cpu"]
    saved = dict(ATTRIBUTION_GRANDFATHERED)
    ATTRIBUTION_GRANDFATHERED[_CONC_OLD] = ("64f8a3af",)
    try:
        old["provenance"]["commit"] = ATTRIBUTION_GRANDFATHERED[_CONC_OLD][0]
        expect("a pre-attribution run at its pinned commit is exempt", old, None, _CONC_OLD)
        old["provenance"]["commit"] = "0000000"
        expect("the same artifact at another commit is enforced", old, "foreign_busy_cpus", _CONC_OLD)
        expect("... and says the entry must go", old, "ATTRIBUTION_GRANDFATHERED", _CONC_OLD)
    finally:
        ATTRIBUTION_GRANDFATHERED.clear()
        ATTRIBUTION_GRANDFATHERED.update(saved)

    for rel in ATTRIBUTION_GRANDFATHERED:
        if not (BENCH / rel).is_file():
            failures.append(f"ATTRIBUTION_GRANDFATHERED names a missing artifact: {rel}")
        elif not is_concurrent(rel):
            failures.append(f"ATTRIBUTION_GRANDFATHERED lists a non-concurrent artifact: {rel}")

    for msg in failures:
        print(f"  FAIL {msg}")
    if failures:
        print(f"check_bench_provenance.py --self-test: {len(failures)} failure(s)")
        return 1
    print("check_bench_provenance.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    return _self_test() if args.self_test else run()


if __name__ == "__main__":
    sys.exit(main())
