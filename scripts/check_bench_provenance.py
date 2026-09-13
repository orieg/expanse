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

Every committed `docs/benchmarks/*/results/` artifact whose name matches one of
`ARTIFACT_GLOBS` — the `baseline_*` sweeps and the `ablation*` interventional
arms measured against them — must carry `provenance.host`,
`provenance.estimators`, load snapshots with a busy-CPU delta and per-cell
`rounds_raw`, **unless it is grandfathered below**.

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

## Which construction produced an interval (#880, #882)

`scripts/bca_bootstrap.py` can now say whether a defensive clamp bound an
interval: `bca_bootstrap_ci_with_method` / `bca_bootstrap_ratio_ci_with_method`
return a fourth value from the `CI_METHOD_*` vocabulary. #882 landed the
capability and converted one harvester; the rest kept the three-value entry
point, so for their cells "this is a BCa interval" stayed an assumption a reader
makes rather than something the artifact states.

A fourth return value callers may ignore is exactly the shape that drifts, and
it drifted inside one PR: `scripts/fit_usl.py` still unpacked two values from
`_bca_from_distribution`, so after #882 its `try` raised `ValueError` on every
call and every interval silently became the plain-percentile fallback. So this
gate carries a producer census:

  - every module that imports `bca_bootstrap` is named in exactly one of
    `CI_METHOD_PRODUCERS` or `CI_METHOD_EXEMPT` — a new interval producer cannot
    appear without saying which it is;
  - a producer calls the `*_with_method` entry points and not the bare
    three-value ones (waivable per line, with a stated reason, for the one
    legitimate use: asserting that both paths agree), does not discard the
    fourth value, and writes the label out once per call site that binds it —
    counted per site, because a file-wide "does `ci_method` appear anywhere"
    check stays green when one site of three stops recording it;
  - and an artifact that records the label for one interval records it for all
    of them — partial adoption is how a field drops back out of half a schema.

The committed artifacts are NOT relabelled. Their intervals can be recomputed
from `rounds_raw`, but only after restoring CPython's pre-3.12 `sum()`
accumulation (3.12 gave it Neumaier compensation, which moves `theta_hat` by a
ULP and with it the selected percentile index). Backfilling would mean
committing a second accumulation path for the estimator — the duplication #880
removed — to add a field whose derived value is `bca` on every cell. Artifacts
gain the label at their next re-run, which the census is what guarantees.

Usage:
  python3 scripts/check_bench_provenance.py
  python3 scripts/check_bench_provenance.py --self-test
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BENCH = REPO_ROOT / "docs" / "benchmarks"

# --------------------------------------------------------------------------
# the construction-label producer census (#880)
# --------------------------------------------------------------------------

# Directories swept for modules that import `bca_bootstrap`. A producer that
# lives outside them would not be discovered, so the list is deliberately the
# whole of the tooling and suite-driver tree rather than the files known today.
CI_METHOD_ROOTS = ("scripts", "docs/benchmarks", "bindings", "integrations", "crates")

# Modules that turn samples into a published interval. Each must reach the
# shared estimator through `*_with_method`, keep the fourth value, and name
# `ci_method` in what it writes.
CI_METHOD_PRODUCERS = {
    "docs/benchmarks/art_comparison/scripts/recompute_and_patch_json.py",
    "docs/benchmarks/concurrency/scripts/ablations.py",
    "docs/benchmarks/concurrency/scripts/writer_scaling.py",
    "docs/benchmarks/hot_comparison/scripts/run_all.py",
    "docs/benchmarks/hot_comparison/scripts/run_strings.py",
    "docs/benchmarks/masstree_comparison/scripts/run_all.py",
    "docs/benchmarks/rocksdb_memtable/scripts/concurrent_read_scaling.py",
    "docs/benchmarks/rocksdb_memtable/scripts/single_threaded_bench.py",
    "docs/benchmarks/set_algebra/scripts/harvest_domain.py",
    "scripts/bench_baseline.py",
    "scripts/bench_counters.py",
    "scripts/esp32_bench_harvest.py",
    "scripts/line_transfer_matrix.py",
    "scripts/perf_counters.py",
    "scripts/pin_exposure.py",
}

# Importers that are not producers, each with the reason. Exemption is by
# explicit entry and never inferred from a file looking test-shaped.
CI_METHOD_EXEMPT = {
    "scripts/test_bca_bootstrap.py":
        "the estimator's own unit tests: they call the bare entry points on "
        "purpose, to pin that the three-value signatures still return what six "
        "suites' committed intervals came from",
    "scripts/fit_usl.py":
        "reaches `_bca_from_distribution` directly, which returns its label as a "
        "third value; `_ci_bounds` forwards it and `test_ci_bounds_reaches_the_"
        "shared_bca_construction` pins that",
    "scripts/rocksdb_bench_harvest.py":
        "the single-threaded rocksdb runner is being rewritten under #868 and is "
        "not this change's to convert; its two call sites still take the bare "
        "entry point, so `baseline_rocksdb.json` stays unlabelled until then",
}

# A bare three-value call, module-qualified or not. `_with_method` spellings do
# not match: the `(` has to follow the name immediately.
BARE_CALL = re.compile(r"(?<![\w.])(?:\w+\.)?bca_bootstrap_(?:ci|ratio_ci)\s*\(")

# A producer may call the bare entry point where that IS the point — an
# equivalence assertion between the two paths. Waived per line, by an explicit
# comment carrying the reason, within this many lines above the call.
BARE_CALL_WAIVER = "# bare-entry-point:"
BARE_CALL_WAIVER_WINDOW = 3

# The label must be written out, not just bound to a local — and counted per
# binding, because a file-wide "does the string `ci_method` appear anywhere"
# check stays green when one call site of several stops recording it. That is
# the same defect shape as a helper-level assertion that survives the call site
# dropping the call: every site is counted, so N bindings need N recordings.
#
# A recording is a dict entry under a string key (`"ci_method": ci_method`,
# including the `.update({...})` form), an assignment into a subscript
# (`cell[f"{role}_ci_method"] = ci_method`), or a `return` that carries the name
# (`scripts/pin_exposure.py`'s `_interval`, whose caller writes the key).
#
# The naming convention for the key is `<prefix>ci_method` beside
# `<prefix>ci_lower`, which is what the artifact-side pairing check reads. A
# suite whose interval is a `[lo, hi]` pair rather than two keys has no
# `ci_lower` to prefix-match and names its own key instead
# (`set_algebra`'s `ci_pooled_bca` / `ci_pooled_bca_method`).
def _recording_patterns(name: str) -> tuple[re.Pattern[str], ...]:
    n = re.escape(name)
    return (
        re.compile(r"""["'][^"'\n]*["']\s*:\s*""" + n + r"\b"),   # {"k": name}
        re.compile(r"\]\s*=\s*" + n + r"\b"),                      # obj[k] = name
        re.compile(r"\breturn\b[^\n]*\b" + n + r"\b"),             # return …, name
    )

# An assignment taking a `*_with_method` result. The LHS is captured so the
# fourth target can be checked: a `_` there is the drift this census exists for,
# and it is the one shape the interpreter cannot catch (unpacking three from
# four already raises).
WITH_METHOD_ASSIGN = re.compile(
    r"^\s*(?P<lhs>[^=\n]+?)\s*=\s*(?:\w+\.)?"
    r"bca_bootstrap_(?:ci|ratio_ci)_with_method\s*\("
)
WITH_METHOD_CALL = re.compile(
    r"(?<![\w.])(?:\w+\.)?bca_bootstrap_(?:ci|ratio_ci)_with_method\s*\("
)
IMPORTS_BCA = re.compile(r"^\s*(?:from\s+bca_bootstrap\s+import|import\s+bca_bootstrap)\b",
                         re.MULTILINE)

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
    "rocksdb_memtable", "concurrency",
)

# The artifact filename families this gate governs, in a suite's `results/`
# directory and in its `multi_writer_olc/` subdirectory.
#
# `baseline_*` is a sweep. `ablation*` is the interventional arm measured
# against one (section 8.20): it publishes a wall-clock ratio of a variant
# against the default, so it is a comparative artifact under section 8.17
# exactly like the sweep it is compared to, and a published wall-clock result
# without its load snapshot is inadmissible. `baseline_*` alone missed every
# one of them.
#
# The `ablation*` glob is deliberately wide enough for both spellings on disk:
#   - `ablation_<mechanism>_writer_scaling[_run2].json` — the #568 writer-scaling
#     ablations, whose Hypothesis D verdicts are published in
#     `docs/benchmarks/concurrency/README.md`. Same shape as the `baseline_*`
#     sweeps: a `throughput` cell list plus `provenance`.
#   - `ablations.json` / `ablations_str.json` — the older #789 feature
#     ablations, a different and older shape (cells under `cells`).
# Both families carry host, estimators, busy-CPU deltas and per-cell
# `rounds_raw` today, so neither is grandfathered and the older shape is
# covered rather than excluded.
ARTIFACT_GLOBS = ("baseline_*.json", "ablation*.json")

# Keys under which an artifact holds its cells. `throughput_variant` is the
# ablation artifacts' variant arm — the half of the comparison that is not the
# default — and owes its rounds like the `throughput` arm it is divided by.
CELL_KEYS = ("cells", "results", "throughput", "throughput_variant",
             "health", "latency", "memory")

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
# `throughput_variant` is an ablation's interventional arm, measured under the
# same thread load as the default arm it is divided by.
ATTRIBUTED_CELL_KEYS = ("throughput", "throughput_variant", "health")

# Name parts that mark an artifact concurrent: its cells were measured with
# more than one thread running, so #568 Step 0's per-cell attribution applies
# — every timed cell says how much foreign CPU was on the host while it ran,
# and the host block says which governor each pinned CPU was under.
#
# Two families qualify, and matching only the first left the second unchecked:
#   - `baseline_concurrent*` — the FFI reader/writer suites.
#   - `*writer_scaling*` — the multi-writer sweep and its ablation arms, which
#     are concurrent by construction (W writers per cell) and carry every C(W)
#     verdict published in `docs/benchmarks/concurrency/README.md`.
CONCURRENT_NAME_PARTS = ("baseline_concurrent", "writer_scaling")


def is_concurrent(rel: str) -> bool:
    name = Path(rel).name
    return any(part in name for part in CONCURRENT_NAME_PARTS)


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

    if Path(rel).name.startswith("counters_"):
        if not obj.get("rounds_raw"):
            problems.append(f"{rel}: missing `rounds_raw`")
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
    out.extend(partial_label_problems(rel, obj))
    if is_concurrent(rel):
        exempt, note = attribution_status(rel, obj)
        if note:
            out.append(note)
        if not exempt:
            out.extend(check_attribution(rel, obj))
    return out


def bca_importers() -> list[str]:
    """Repo-relative paths of every `.py` that imports the shared estimator.

    Discovery, not a list: a new interval producer has to be classified rather
    than added to a table someone remembers to update. A file that only mentions
    `bca_bootstrap` in prose or a string is not an importer and is not swept in.
    """
    found: set[str] = set()
    for root in CI_METHOD_ROOTS:
        base = REPO_ROOT / root
        if not base.is_dir():
            continue
        for path in base.rglob("*.py"):
            try:
                text = path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            if IMPORTS_BCA.search(text):
                found.add(path.resolve().relative_to(REPO_ROOT).as_posix())
    return sorted(found)


def producer_problems(rel: str, text: str) -> list[str]:
    """Findings for one module declared a construction-label producer."""
    problems = []
    lines = text.splitlines()
    bare = []
    for i, line in enumerate(lines):
        if not BARE_CALL.search(line):
            continue
        window = lines[max(0, i - BARE_CALL_WAIVER_WINDOW):i + 1]
        if any(BARE_CALL_WAIVER in w for w in window):
            continue
        bare.append(i + 1)
    if bare:
        problems.append(
            f"{rel}: line(s) {bare} call the bare three-value entry point — a cell it "
            f"writes cannot say which construction produced its interval; use "
            f"bca_bootstrap_ci_with_method / bca_bootstrap_ratio_ci_with_method (#880), "
            f"or waive the line with a `{BARE_CALL_WAIVER} <reason>` comment above it"
        )
    if not WITH_METHOD_CALL.search(text):
        problems.append(
            f"{rel}: declared a construction-label producer but never calls a "
            f"`*_with_method` entry point — drop its CI_METHOD_PRODUCERS entry or "
            f"convert it"
        )
    bound: dict[str, list[int]] = {}
    for i, line in enumerate(lines, start=1):
        m = WITH_METHOD_ASSIGN.match(line)
        if not m:
            continue
        targets = [t.strip() for t in m.group("lhs").split(",")]
        if len(targets) != 4:
            problems.append(
                f"{rel}:{i}: a `*_with_method` result is bound to {len(targets)} name(s), "
                f"not 4 — the construction label is the fourth value"
            )
            continue
        if targets[3].startswith("_"):
            problems.append(
                f"{rel}:{i}: the construction label is discarded into {targets[3]!r} — "
                f"record it beside the interval instead (#880); a fourth value callers "
                f"throw away is how this capability drifts"
            )
            continue
        bound.setdefault(targets[3], []).append(i)
    if not bound:
        return problems
    for name, sites in sorted(bound.items()):
        pats = _recording_patterns(name)
        # Lines, not pattern hits: `return {"ci_method": ci_method}` is one
        # recording that two of the three patterns match, and counting hits
        # would let it cover a second site that records nothing.
        records = sum(1 for line in lines if any(p.search(line) for p in pats))
        if records < len(sites):
            problems.append(
                f"{rel}: `{name}` is bound at {len(sites)} `*_with_method` call site(s) "
                f"(line(s) {sites}) but written out {records} time(s) — a site whose label "
                f"is never recorded leaves that cell as unlabelled as before #880. Record "
                f"it under a `<prefix>ci_method` key beside the interval's "
                f"`<prefix>ci_lower`, or return it to a caller that does"
            )
    return problems


def ci_method_census() -> list[str]:
    """Every finding from the producer census."""
    problems = []
    importers = bca_importers()
    declared = CI_METHOD_PRODUCERS | set(CI_METHOD_EXEMPT)
    for rel in importers:
        if rel == "scripts/bca_bootstrap.py":
            continue  # the module itself; it defines the entry points
        if rel not in declared:
            problems.append(
                f"{rel}: imports bca_bootstrap but is in neither CI_METHOD_PRODUCERS nor "
                f"CI_METHOD_EXEMPT — say whether it publishes an interval that must name "
                f"its construction (#880), with a reason if it does not"
            )
    for rel in sorted(declared):
        if rel not in importers:
            problems.append(
                f"{rel}: named in the construction-label census but does not import "
                f"bca_bootstrap — the entry is exempting or requiring nothing"
            )
    for rel in sorted(CI_METHOD_PRODUCERS & set(CI_METHOD_EXEMPT)):
        problems.append(f"{rel}: named as both a producer and exempt; it is one or the other")
    for rel in sorted(CI_METHOD_PRODUCERS):
        path = REPO_ROOT / rel
        if not path.is_file():
            continue  # reported above as not importing
        problems.extend(producer_problems(rel, path.read_text(encoding="utf-8")))
    return problems


def ci_method_keys(node, prefix: str = "") -> tuple[set[str], set[str]]:
    """`(interval prefixes, labelled prefixes)` found anywhere under `node`.

    A prefix is whatever precedes `ci_lower`, so `writer_ci_lower` pairs with
    `writer_ci_method` and a bare `ci_lower` with `ci_method`. Walks nested
    dicts, because several suites hold an interval in a sub-object
    (`ns_per_transfer`, a per-condition block) rather than flat on the cell.
    """
    intervals: set[str] = set()
    labelled: set[str] = set()
    if isinstance(node, dict):
        for k, v in node.items():
            if isinstance(k, str) and k.endswith("ci_lower"):
                intervals.add(prefix + k[: -len("ci_lower")])
            elif isinstance(k, str) and k.endswith("ci_method"):
                labelled.add(prefix + k[: -len("ci_method")])
            if isinstance(v, (dict, list)):
                sub_i, sub_l = ci_method_keys(v, f"{prefix}{k}.")
                intervals |= sub_i
                labelled |= sub_l
    elif isinstance(node, list):
        for i, v in enumerate(node):
            if isinstance(v, (dict, list)):
                # The index is part of the prefix, so each cell is paired on its
                # own. Without it one labelled cell would cover every unlabelled
                # cell beside it under the same key — which is precisely the
                # partial adoption this check exists to catch.
                sub_i, sub_l = ci_method_keys(v, f"{prefix}[{i}].")
                intervals |= sub_i
                labelled |= sub_l
    return intervals, labelled


def partial_label_problems(rel: str, obj) -> list[str]:
    """An artifact labels every interval it publishes, or none of them.

    None is the pre-#880 state and is reported by name in the summary, never
    enforced: the committed intervals were not relabelled, because deriving the
    label needs CPython's pre-3.12 `sum()` restored (see the module docstring).
    *Some* is the failure — a converted runner that dropped the label at one of
    its several interval sites, which is how a field falls out of half a schema.
    """
    intervals, labelled = ci_method_keys(obj)
    if not labelled:
        return []
    missing = sorted(intervals - labelled)
    if missing:
        return [
            f"{rel}: records a construction label for some intervals but not for "
            f"{len(missing)} other(s) ({', '.join(f'{p}ci_lower' for p in missing[:4])}"
            f"{', …' if len(missing) > 4 else ''}) — a runner that names the "
            f"construction names it for every interval it publishes (#880)"
        ]
    return []


def artifacts() -> list[Path]:
    out: list[Path] = []
    for suite in SUITES:
        res = BENCH / suite / "results"
        if not res.is_dir():
            continue
        for d in (res, res / "multi_writer_olc"):
            if not d.is_dir():
                continue
            found: set[Path] = set()
            for pattern in ARTIFACT_GLOBS:
                found.update(d.glob(pattern))
            if d != res:
                found.update(d.glob("counters_*.json"))
            out.extend(sorted(found))
    return out


def run() -> int:
    findings, checked, grandfathered, attribution_grandfathered = [], 0, 0, 0
    findings.extend(ci_method_census())
    unlabelled: list[str] = []
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
        intervals, labelled = ci_method_keys(obj)
        if intervals and not labelled:
            unlabelled.append(rel)
        findings.extend(findings_for(rel, obj))

    if unlabelled:
        # Named, never silently skipped (section 8.1). These predate #880's
        # construction label and gain it when their suite is next re-run; the
        # producer census above is what stops a re-run landing without it.
        print(f"check_bench_provenance.py: {len(unlabelled)} artifact(s) publish intervals "
              f"that name no construction (pre-#880, relabelled only by re-measurement): "
              f"{', '.join(unlabelled)}")

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
# An ablation artifact as `concurrency`'s runner writes it: the default arm
# under `throughput`, the interventional arm under `throughput_variant`, and
# the ratio between the two. Both arms are timed wall-clock cells and both owe
# their rounds. Not `baseline_concurrent*` by name, so it owes host,
# estimators, a busy-CPU delta and per-cell rounds, and not per-cell
# attribution.
_GOOD_ABL = {
    "provenance": {
        "suite": "concurrency",
        "commit": "1a2b3c4",
        # A writer-scaling name is concurrent (`is_concurrent`), so this
        # fixture owes the per-cell attribution and the per-CPU governor map
        # that #568 Step 0 requires of a concurrent artifact.
        "host": {"cpu_model": "x", "scaling_governor": "performance",
                 "scaling_governor_by_cpu": {"0": "performance", "1": "performance"},
                 "scaling_governor_pin_set": "0-1"},
        "estimators": {"ratio": "median(variant)/median(default)", "raw": "rounds_raw"},
        "loads": [{"label": "start", "load1": 0.4, "busy_cpus_since_prev": None},
                  {"label": "end", "load1": 1.2, "busy_cpus_since_prev": 0.31}],
    },
    "throughput": [{"arm": "map", "writers": 4,
                    "load": {"foreign_busy_cpus": 0.0},
                    "rounds_raw": [{"round": 0, "expanse_writer_mops": 8.6}]}],
    "throughput_variant": [{"arm": "map", "writers": 4,
                            "load": {"foreign_busy_cpus": 0.0},
                            "rounds_raw": [{"round": 0, "expanse_writer_mops": 8.9}]}],
    "comparison": [{"arm": "map", "variant_name": "freelist", "rounds": 8}],
}
_ABL_REL = "concurrency/results/ablation_synthetic_writer_scaling.json"

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

    # --- the ablation artifacts (section 8.17) -----------------------------
    # THE HOLE THIS CLOSED: `artifacts()` globbed `baseline_*.json` only, so
    # the committed `ablation*.json` artifacts — which publish the Hypothesis D
    # verdicts in `docs/benchmarks/concurrency/README.md` — were never read at
    # all. Field cases on their own do not pin that: `findings_for` is
    # path-agnostic and would have passed them before the fix too. The
    # selection is what has to be pinned, so this asserts both.
    selected = {str(p.relative_to(BENCH)) for p in artifacts()}
    for name in ("ablation_alloc_writer_scaling.json",
                 "ablation_epoch_writer_scaling.json",
                 "ablation_freelist_writer_scaling.json",
                 "ablation_freelist_writer_scaling_run2.json",
                 # the older #789 feature ablations, a different shape that the
                 # same glob covers and that passes the gate as committed
                 "ablations.json",
                 "ablations_str.json"):
        rel = f"concurrency/results/{name}"
        if not (BENCH / rel).is_file():
            failures.append(f"a named ablation artifact is missing: {rel}")
        elif rel not in selected:
            failures.append(f"artifacts() does not select {rel} — ARTIFACT_GLOBS is too narrow")

    # The concurrent predicate, pinned the same way and for the same reason:
    # `findings_for` is name-agnostic, so a fixture cannot catch a predicate
    # that never selects the artifact. Every writer-scaling sweep and ablation
    # arm is concurrent by construction.
    for name in ("baseline_concurrent.json",
                 "baseline_concurrent_ab.json",
                 "baseline_writer_scaling.json",
                 "baseline_writer_scaling_run2.json",
                 "ablation_alloc_writer_scaling.json",
                 "ablation_epoch_writer_scaling.json",
                 "ablation_freelist_writer_scaling.json",
                 "ablation_freelist_writer_scaling_run2.json"):
        if not is_concurrent(f"concurrency/results/{name}"):
            failures.append(
                f"is_concurrent() does not cover {name} — CONCURRENT_NAME_PARTS is too narrow"
            )
    # And a census must not be swept in by the widened match.
    if is_concurrent("masstree_comparison/results/baseline_memory.json"):
        failures.append("is_concurrent() matches a memory census — CONCURRENT_NAME_PARTS is too wide")

    expect("a complete ablation artifact passes", copy.deepcopy(_GOOD_ABL), None, _ABL_REL)

    abl_no_raw = copy.deepcopy(_GOOD_ABL)
    del abl_no_raw["throughput"][0]["rounds_raw"]
    expect("an ablation default arm without raw rows", abl_no_raw, "rounds_raw", _ABL_REL)

    # The variant arm is half the published ratio and is checked as such: a
    # gate that reads only `throughput` reads only the denominator.
    abl_var_no_raw = copy.deepcopy(_GOOD_ABL)
    del abl_var_no_raw["throughput_variant"][0]["rounds_raw"]
    expect("an ablation variant arm without raw rows", abl_var_no_raw,
           "throughput_variant", _ABL_REL)

    abl_no_delta = copy.deepcopy(_GOOD_ABL)
    abl_no_delta["provenance"]["loads"] = [{"label": "start", "load1": 0.4},
                                           {"label": "end", "load1": 1.2}]
    expect("an ablation artifact whose loads carry no jiffy delta", abl_no_delta,
           "busy_cpus_since_prev", _ABL_REL)

    abl_no_host = copy.deepcopy(_GOOD_ABL)
    del abl_no_host["provenance"]["host"]
    expect("an ablation artifact with no host facts", abl_no_host,
           "provenance.host", _ABL_REL)

    abl_no_est = copy.deepcopy(_GOOD_ABL)
    del abl_no_est["provenance"]["estimators"]
    expect("an ablation artifact with no estimators block", abl_no_est,
           "provenance.estimators", _ABL_REL)

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

    # --- #880: which construction produced an interval --------------------
    # THE DEFECT THIS PINS: #882 added a fourth return value naming the
    # construction, and `scripts/fit_usl.py` — inside the same PR — kept
    # unpacking two, so its `try` raised and every interval silently became the
    # percentile fallback. A value callers may ignore drifts, so the census
    # reads the real producer files rather than a fixture: THE CALL SITES ARE
    # THE ASSERTION. Revert any one of them to the bare entry point, or discard
    # the fourth value there, and this goes red.
    real = ci_method_census()
    if real:
        failures.extend(f"the committed census is not clean: {f}" for f in real)

    # The discovery half. A fixture cannot catch a sweep that stops finding the
    # producers — then every rule above would be checking an empty set.
    importers = set(bca_importers())
    for rel in sorted(CI_METHOD_PRODUCERS | set(CI_METHOD_EXEMPT)):
        if rel not in importers:
            failures.append(f"bca_importers() does not find {rel} — CI_METHOD_ROOTS or "
                            f"IMPORTS_BCA is too narrow")
    if "scripts/bca_bootstrap.py" in (CI_METHOD_PRODUCERS | set(CI_METHOD_EXEMPT)):
        failures.append("the estimator module itself must not be in the census tables")
    # And not so wide that prose counts: `bindings/python/bench_concurrency.py`
    # names the module in a disclosure string and imports nothing.
    if "bindings/python/bench_concurrency.py" in importers:
        failures.append("IMPORTS_BCA matched a file that only mentions the module in prose")

    def census_expect(name, source, want_substr):
        got = producer_problems("fixture.py", source)
        if want_substr is None:
            if got:
                failures.append(f"{name}: expected no finding, got {got}")
        elif not any(want_substr in g for g in got):
            failures.append(f"{name}: expected a finding mentioning {want_substr!r}, got {got}")

    good_src = (
        "from bca_bootstrap import bca_bootstrap_ci_with_method\n"
        "def cell(samples):\n"
        "    mean, lo, hi, ci_method = bca_bootstrap_ci_with_method(samples)\n"
        '    return {"ci_lower": lo, "ci_upper": hi, "ci_method": ci_method}\n'
    )
    census_expect("a converted producer passes", good_src, None)

    census_expect(
        "a bare three-value call",
        good_src.replace("mean, lo, hi, ci_method = bca_bootstrap_ci_with_method(samples)",
                         "mean, lo, hi = bca_bootstrap_ci(samples)"),
        "bare three-value entry point")
    census_expect(
        "a bare ratio call",
        good_src.replace("mean, lo, hi, ci_method = bca_bootstrap_ci_with_method(samples)",
                         "mean, lo, hi = bca_bootstrap_ratio_ci(samples, samples)"),
        "bare three-value entry point")
    # A module-qualified bare call is the same defect and was the spelling the
    # esp32 harvester uses.
    census_expect(
        "a module-qualified bare call",
        good_src.replace("mean, lo, hi, ci_method = bca_bootstrap_ci_with_method(samples)",
                         "mean, lo, hi = bca_bootstrap.bca_bootstrap_ci(samples)"),
        "bare three-value entry point")
    # ... and is waivable per line, with a stated reason, for the one legitimate
    # use: asserting the two paths agree.
    census_expect(
        "a waived bare call",
        good_src.replace(
            "    mean, lo, hi, ci_method = bca_bootstrap_ci_with_method(samples)\n",
            "    mean, lo, hi, ci_method = bca_bootstrap_ci_with_method(samples)\n"
            "    # bare-entry-point: the assertion is that both paths agree\n"
            "    assert (mean, lo, hi) == bca_bootstrap_ci(samples)\n"),
        None)

    # THE DRIFT SHAPE the interpreter cannot catch: unpacking three from four
    # raises, but discarding the fourth is silent and compiles forever.
    census_expect(
        "the construction label discarded",
        good_src.replace("mean, lo, hi, ci_method =", "mean, lo, hi, _method ="),
        "discarded")
    census_expect(
        "the label bound but never recorded",
        good_src.replace('"ci_method": ci_method', '"n": 1'),
        "written out 0 time(s)")
    # THE WEAKNESS THIS COUNTS PER SITE: a file-wide "does `ci_method` appear"
    # check stays green when one call site of three stops recording it, because
    # the others still mention the name. Three bindings need three recordings.
    three_sites = (
        "from bca_bootstrap import bca_bootstrap_ci_with_method\n"
        "def a(s):\n"
        "    m, lo, hi, ci_method = bca_bootstrap_ci_with_method(s)\n"
        '    return {"ci_lower": lo, "ci_method": ci_method}\n'
        "def b(s):\n"
        "    m, lo, hi, ci_method = bca_bootstrap_ci_with_method(s)\n"
        '    return {"ci_lower": lo, "ci_method": ci_method}\n'
        "def c(s, cell):\n"
        "    m, lo, hi, ci_method = bca_bootstrap_ci_with_method(s)\n"
        '    cell["x_ci_method"] = ci_method\n'
    )
    census_expect("three sites, three recordings", three_sites, None)
    census_expect(
        "one of three sites stops recording",
        three_sites.replace('    cell["x_ci_method"] = ci_method\n', "    cell\n"),
        "bound at 3 `*_with_method` call site(s)")
    census_expect(
        "a producer that calls nothing",
        "from bca_bootstrap import bca_bootstrap_ci_with_method\n"
        'X = {"ci_method": None}\n',
        "never calls a `*_with_method` entry point")
    # A suite whose interval is a [lo, hi] pair has no `ci_lower` to prefix, and
    # its `<name>_method` key satisfies the source-side rule (set_algebra).
    census_expect(
        "a pair-shaped interval with its own label key",
        "from bca_bootstrap import bca_bootstrap_ratio_ci_with_method\n"
        "def cell(a, b):\n"
        "    r, lo, hi, m = bca_bootstrap_ratio_ci_with_method(a, b)\n"
        '    return {"ci_pooled_bca": [lo, hi], "ci_pooled_bca_method": m}\n',
        None)

    # The classification tables, both directions.
    saved_p, saved_e = set(CI_METHOD_PRODUCERS), dict(CI_METHOD_EXEMPT)
    try:
        CI_METHOD_PRODUCERS.discard("scripts/bench_baseline.py")
        dropped = ci_method_census()
        if not any("neither CI_METHOD_PRODUCERS nor CI_METHOD_EXEMPT" in f
                   and "bench_baseline" in f for f in dropped):
            failures.append(f"an unclassified importer must be a finding, got {dropped}")
        CI_METHOD_PRODUCERS.add("scripts/bench_baseline.py")
        CI_METHOD_PRODUCERS.add("scripts/does_not_exist.py")
        stale = ci_method_census()
        if not any("does not import" in f for f in stale):
            failures.append(f"a stale census entry must be a finding, got {stale}")
        CI_METHOD_PRODUCERS.discard("scripts/does_not_exist.py")
        CI_METHOD_EXEMPT["scripts/bench_baseline.py"] = "both, which is not a state"
        both = ci_method_census()
        if not any("both a producer and exempt" in f for f in both):
            failures.append(f"a doubly-listed module must be a finding, got {both}")
    finally:
        CI_METHOD_PRODUCERS.clear()
        CI_METHOD_PRODUCERS.update(saved_p)
        CI_METHOD_EXEMPT.clear()
        CI_METHOD_EXEMPT.update(saved_e)
    for rel, reason in CI_METHOD_EXEMPT.items():
        if not reason or len(reason) < 20:
            failures.append(f"CI_METHOD_EXEMPT[{rel}] carries no usable reason")

    # --- the artifact side: all intervals labelled, or none ---------------
    # None is the pre-#880 state and is reported by name, not enforced; SOME is
    # the failure, because that is a converted runner dropping the label at one
    # of its several interval sites.
    none_labelled = {"throughput": [{"writer_ci_lower": 1.0, "writer_ci_upper": 2.0,
                                     "scaling_factor_c_n_ci_lower": 0.9,
                                     "scaling_factor_c_n_ci_upper": 1.1}]}
    if partial_label_problems("x.json", none_labelled):
        failures.append("an artifact that labels nothing must not be enforced")
    all_labelled = {"throughput": [{"writer_ci_lower": 1.0, "writer_ci_upper": 2.0,
                                    "writer_ci_method": "bca",
                                    "scaling_factor_c_n_ci_lower": 0.9,
                                    "scaling_factor_c_n_ci_upper": 1.1,
                                    "scaling_factor_c_n_ci_method": "bca"}]}
    if partial_label_problems("x.json", all_labelled):
        failures.append(f"a fully labelled artifact must pass, got "
                        f"{partial_label_problems('x.json', all_labelled)}")
    half = {"throughput": [{"writer_ci_lower": 1.0, "writer_ci_upper": 2.0,
                            "writer_ci_method": "bca",
                            "scaling_factor_c_n_ci_lower": 0.9,
                            "scaling_factor_c_n_ci_upper": 1.1}]}
    got = partial_label_problems("x.json", half)
    if not any("not for 1 other" in g for g in got):
        failures.append(f"a half-labelled artifact must be a finding, got {got}")
    # An interval nested in a sub-object is reached too (`ns_per_transfer`).
    nested = {"cells": [{"ns_per_transfer": {"ci_lower": 1.0, "ci_upper": 2.0}},
                        {"ns_per_transfer": {"ci_lower": 1.0, "ci_upper": 2.0,
                                             "ci_method": "bca"}}]}
    got = partial_label_problems("x.json", nested)
    if not got:
        failures.append("a nested interval must be reached by the pairing check")

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
