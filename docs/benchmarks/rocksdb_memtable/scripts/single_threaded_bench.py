#!/usr/bin/env python3
"""Driver for the RocksDB memtable single-threaded arm (#868).

Replaces the shell loop in `.github/workflows/bench_baremetal.yml` -- five whole
`make -C integrations/rocksdb bench` invocations piped to text files and parsed
afterwards -- for the reason #868 names: a shell loop around a C++ binary that
times every phase inside one process has no cell boundaries for
`scripts/bench_provenance.py` to wrap, so the artifact it produced carried a
host description and nothing else. No load snapshot, no busy-CPU delta, no raw
rows, and it is the source of four published wall-clock ratios.

Modelled on this suite's `concurrent_read_scaling.py`, deliberately: same
structure, same provenance module, same refusal to write a committed artifact
that cannot say what else was on the host. What it owns, and why the C++ binary
does not:

  * **The rounds and their order.** `bench_memtable --arm <phase>` times exactly
    one phase per invocation. This driver interleaves the phases *within* each
    round (AGENTS.md 8.20.2) -- five rounds of `fillrandom` followed by five of
    `readrandom` would confound thermal drift with the difference between them.
    The order is produced by `sweep_order()` and the self-test pins that it is
    round-major, because the loop nesting is the whole invariant.

  * **The load snapshots.** `load.foreign_busy_cpus` is the host's busy CPU over
    a cell minus the runner's own children's, so it needs a process boundary per
    cell to be attributable at all. One phase per invocation is what supplies
    it. The three implementations stay *inside* one invocation on purpose: a
    published ratio's two arms have to be timed under the same host state, and a
    boundary per implementation would make each ratio a comparison across two
    different contention windows.

  * **The intervals.** BCa 95% per arm, and **paired** BCa on the per-round
    ratio `A_round / B_round` -- paired because both arms of every ratio come
    from the same invocation, so the interval carries the round-to-round
    covariance instead of treating the two series as independent. This is a
    change of estimator from the superseded `scripts/rocksdb_bench_harvest.py`,
    which took a two-sample ratio interval over the same rounds; it is recorded
    in `provenance.estimators` and in `statistics`, and it is one reason the
    published section 12 figures cannot be swapped for these cell-for-cell.

  * **The symmetry checks.** Every implementation of a timed arm must report the
    same `consumed` count -- the same number of callback hits, the same number
    of entries walked. A differing count means the arms did not do the same
    work, and the number published as a ratio would be comparing two workloads.

What it does not own: the verdicts. Those are read against
`docs/benchmarks/rocksdb_memtable/METHODOLOGY.md` by a reviewer.

Usage:
    python3 docs/benchmarks/rocksdb_memtable/scripts/single_threaded_bench.py \
        --out docs/benchmarks/rocksdb_memtable/results/baseline_rocksdb.json
    python3 docs/benchmarks/rocksdb_memtable/scripts/single_threaded_bench.py --quick
    python3 docs/benchmarks/rocksdb_memtable/scripts/single_threaded_bench.py --self-test
"""

from __future__ import annotations

import argparse
import inspect
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
import bench_provenance as prov  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

BENCH = REPO_ROOT / "integrations" / "rocksdb" / "build" / "bench_memtable"
SUITE_DIR = REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable"
DEFAULT_OUT = SUITE_DIR / "results" / "baseline_rocksdb.json"

#: One phase per invocation. `memory` is a deterministic allocator census and
#: carries no interval (AGENTS.md 8.4); it is swept like the rest so that its
#: determinism is checked across rounds rather than assumed.
ARMS = ("fillrandom", "readrandom", "seekrandom", "prefixscan", "memory")
CENSUS_ARM = "memory"

#: The implementation prefix whose arms are the subject of a published ratio.
SUBJECT = "ExpanseMemTable"

CONFIDENCE = 0.95
RESAMPLES = 2000
SEED = 42

#: BCa needs n >= 3 (`scripts/bca_bootstrap.py`); fewer rounds cannot produce a
#: publishable interval, so a committed run refuses rather than emitting one.
MIN_ROUNDS = 3

CSV_FIELDS = ("arm", "implementation", "round", "ops", "elapsed_s", "mops",
              "consumed", "bytes_total", "bytes_per_entry")


def sweep_order(rounds: int, arms: tuple) -> list[tuple[int, str]]:
    """The `(round, arm)` invocation order: round-major, arms interleaved.

    Round-major is the requirement, not a preference (AGENTS.md 8.20.2). An
    arm-major order -- every round of one phase, then every round of the next --
    puts a whole phase's samples in one stretch of the host's thermal and
    frequency history, so a difference between two phases and a drift across the
    sweep become the same number.
    """
    if rounds < 1:
        raise ValueError(f"rounds must be >= 1, got {rounds}")
    if not arms:
        raise ValueError("no arms to sweep")
    return [(rd, arm) for rd in range(rounds) for arm in arms]


def parse_rows(text: str, expect_arm: str, expect_round: int) -> list[dict]:
    """The CSV rows one invocation emitted, or raises.

    Fails loudly on anything unexpected rather than returning what it could
    parse: a phase that produced nothing must stop the sweep, not contribute a
    zero to a published mean (AGENTS.md 8.1).
    """
    lines = [ln.strip() for ln in text.splitlines() if ln.strip()]
    data = [ln for ln in lines
            if not ln.startswith("#") and not ln.startswith(CSV_FIELDS[0] + ",")]
    if not data:
        raise RuntimeError(f"{expect_arm}: no CSV data rows in:\n{text}")
    rows = []
    for ln in data:
        parts = ln.split(",")
        if len(parts) != len(CSV_FIELDS):
            raise RuntimeError(
                f"{expect_arm}: expected {len(CSV_FIELDS)} CSV fields, got "
                f"{len(parts)}: {ln!r}")
        d = dict(zip(CSV_FIELDS, parts))
        row = {
            "arm": d["arm"],
            "implementation": d["implementation"],
            "round": int(d["round"]),
            "ops": int(d["ops"]),
            "elapsed_s": float(d["elapsed_s"]),
            "mops": float(d["mops"]),
            "consumed": int(d["consumed"]),
            "bytes_total": int(d["bytes_total"]),
            "bytes_per_entry": float(d["bytes_per_entry"]),
        }
        if row["arm"] != expect_arm:
            raise RuntimeError(
                f"asked for arm {expect_arm!r} and the binary reported "
                f"{row['arm']!r} — the invocation measured a different phase "
                f"from the one the cell is labelled with")
        if row["round"] != expect_round:
            raise RuntimeError(
                f"{expect_arm}: asked for round {expect_round} and the binary "
                f"reported {row['round']} — the round stamp would mislabel the "
                f"raw rows a published mean is recomputed from")
        if row["arm"] == CENSUS_ARM:
            # A census row: exact bytes, no timed window.
            if row["bytes_total"] <= 0 or row["elapsed_s"] != 0.0:
                raise RuntimeError(
                    f"census row is malformed (bytes_total="
                    f"{row['bytes_total']}, elapsed_s={row['elapsed_s']}): "
                    f"{ln!r}")
        else:
            if row["bytes_total"] != 0:
                raise RuntimeError(f"timed row carries a byte count: {ln!r}")
            if row["ops"] <= 0 or row["elapsed_s"] <= 0.0:
                raise RuntimeError(
                    f"timed row has ops={row['ops']} elapsed_s="
                    f"{row['elapsed_s']}: a phase that measured nothing must "
                    f"not reach an artifact as a zero: {ln!r}")
            # The binary prints Mops/s as well as the two numbers it is computed
            # from. Checking them against each other catches a truncated or
            # mis-ordered column, which a parser that trusted one of them would
            # carry into every published mean.
            recomputed = (row["ops"] / row["elapsed_s"]) / 1e6
            if abs(recomputed - row["mops"]) > 1e-6 * max(1.0, row["mops"]):
                raise RuntimeError(
                    f"{expect_arm}/{row['implementation']}: mops column "
                    f"{row['mops']} disagrees with ops/elapsed_s "
                    f"{recomputed}: {ln!r}")
        rows.append(row)
    # Symmetry: every implementation of a timed arm did the same amount of work.
    # The scan phases make this load-bearing -- an iterator that terminates
    # early is fast and wrong, and the ratio would publish the shortfall as a
    # win (`ScanBatch` non-termination, fixed at 986425d0).
    if rows[0]["arm"] != CENSUS_ARM:
        consumed = {r["consumed"] for r in rows}
        if len(consumed) != 1:
            raise RuntimeError(
                f"{expect_arm}: implementations disagree on how much work they "
                f"did: " + ", ".join(f"{r['implementation']}={r['consumed']}"
                                     for r in rows) +
                " — the arms are not measuring the same thing, so a ratio over "
                "them would compare two workloads (AGENTS.md 8.3)")
    return rows


def preflight(bench: Path) -> None:
    """One throwaway invocation, so a broken binary reports before the sweep.

    The concurrent arm's first reference-host attempt died at cell 1 of 60
    because the binary could not load `libexpanse.so`, and the traceback named
    an exit code rather than the loader. Check once, up front, and say what is
    actually wrong (AGENTS.md 8.1).
    """
    cwd = str(Path(bench).resolve().parent.parent)
    res = subprocess.run([str(bench), "--arm", CENSUS_ARM, "--round", "0"],
                         capture_output=True, text=True, cwd=cwd)
    if res.returncode == 0:
        return
    hint = ""
    if ("shared object" in res.stderr or "image not found" in res.stderr
            or "dyld" in res.stderr or res.returncode == 127):
        hint = ("\nThe binary cannot load libexpanse. The Makefile links it against the "
                "relative path ../../target/release/libexpanse.{so,dylib}, so it only "
                "resolves when run from integrations/rocksdb/ -- this driver sets that as "
                "the working directory. Check the release library exists: "
                "cargo build --release -p expanse-capi")
    raise RuntimeError(
        f"preflight failed: {bench} exited {res.returncode} (cwd {cwd})\n"
        f"stderr:\n{res.stderr}{hint}")


def run_sweep(bench: Path, rounds: int, arms: tuple, provenance: dict) -> list[dict]:
    preflight(bench)
    rows: list[dict] = []
    # `new_provenance` already took the opening snapshot; a second one here
    # would leave two cells labelled `start` and make `since` ambiguous.
    for rd, arm in sweep_order(rounds, arms):
        label = f"cell:{arm}:round{rd}"
        start = prov.begin_cell(provenance, label)
        cmd = [str(bench), "--arm", arm, "--round", str(rd)]
        # Run from the integration directory, as `make -C integrations/rocksdb
        # bench` does. The Makefile links the binary against the RELATIVE path
        # `../../target/release/libexpanse.so`, and because a cargo cdylib
        # carries no SONAME the linker records that path verbatim as DT_NEEDED.
        # A DT_NEEDED containing a slash is resolved against the process's
        # working directory and ignores any rpath, so the binary only loads from
        # inside integrations/rocksdb/. Invoking it by absolute path from the
        # repo root failed with `cannot open shared object file` on Linux while
        # macOS resolved it anyway -- Mach-O records an absolute install name --
        # so a local smoke cannot reproduce it and the fix is copied
        # deliberately from `concurrent_read_scaling.py` rather than rediscovered
        # on the reference host.
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=cwd_for(bench))
        if res.returncode != 0:
            raise RuntimeError(
                f"{label}: {' '.join(cmd)} exited {res.returncode}\n"
                f"stdout:\n{res.stdout}\nstderr:\n{res.stderr}")
        load = prov.end_cell(start)
        for row in parse_rows(res.stdout, arm, rd):
            row["cell"] = label
            row["load"] = load
            rows.append(row)
    prov.add_load(provenance, "end")
    return rows


def cwd_for(bench: Path) -> str:
    """`integrations/rocksdb`, the only directory the binary's DT_NEEDED resolves from."""
    return str(Path(bench).resolve().parent.parent)


def _series(rows: list[dict], arm: str) -> dict[str, dict[int, dict]]:
    """`{implementation: {round: row}}` for one timed arm."""
    out: dict[str, dict[int, dict]] = {}
    for r in rows:
        if r["arm"] != arm:
            continue
        per_impl = out.setdefault(r["implementation"], {})
        if r["round"] in per_impl:
            raise RuntimeError(
                f"{arm}/{r['implementation']}: round {r['round']} measured twice")
        per_impl[r["round"]] = r
    return out


def arm_cells(rows: list[dict]) -> list[dict]:
    """One published cell per `(benchmark, implementation)`, with its rounds.

    `rounds_raw` carries each round's ops, seconds and throughput *and that
    invocation's load attribution*: the process boundary is `(arm, round)`, so
    every implementation of an arm in a round shares one snapshot, and the
    artifact says so here rather than leaving a reader to assume a per-arm one.
    """
    cells = []
    for arm in sorted({r["arm"] for r in rows if r["arm"] != CENSUS_ARM}):
        series = _series(rows, arm)
        for impl in sorted(series):
            by_round = series[impl]
            raw = [{"round": rd,
                    "mops": by_round[rd]["mops"],
                    "ops": by_round[rd]["ops"],
                    "elapsed_s": by_round[rd]["elapsed_s"],
                    "consumed": by_round[rd]["consumed"],
                    "load": by_round[rd]["load"]}
                   for rd in sorted(by_round)]
            samples = [e["mops"] for e in raw]
            cell = {
                "id": f"{arm}/{impl}",
                "benchmark": arm,
                "implementation": impl,
                "unit": "Mops_per_second",
                "n": len(samples),
                "rounds_raw": raw,
            }
            if len(samples) >= MIN_ROUNDS:
                point, lo, hi, ci_method = bca_bootstrap_ci_with_method(
                    samples, CONFIDENCE, RESAMPLES, SEED)
                cell.update({"point": round(point, 4), "ci_lower": round(lo, 4),
                             "ci_upper": round(hi, 4), "ci_method": ci_method})
            else:
                cell.update({"point": sum(samples) / len(samples),
                             "ci_lower": None, "ci_upper": None, "ci_method": None,
                             "why_no_interval":
                                 f"fewer than {MIN_ROUNDS} rounds; BCa needs n >= 3"})
            cells.append(cell)
    return cells


def ratio_cells(rows: list[dict]) -> list[dict]:
    """Paired BCa on `subject_round / baseline_round`, per benchmark.

    Paired, not two-sample: both arms of every ratio were timed back to back
    inside one invocation, so the per-round quotient removes the round's own
    drift instead of leaving it in both marginals.
    """
    out = []
    for arm in sorted({r["arm"] for r in rows if r["arm"] != CENSUS_ARM}):
        series = _series(rows, arm)
        subjects = sorted(i for i in series if i.startswith(SUBJECT))
        baselines = sorted(i for i in series if not i.startswith(SUBJECT))
        for subj in subjects:
            for base in baselines:
                shared = sorted(set(series[subj]) & set(series[base]))
                per_round = []
                for rd in shared:
                    denom = series[base][rd]["mops"]
                    if denom <= 0.0:
                        raise RuntimeError(
                            f"{arm}/{base} round {rd} reported {denom} Mops/s; "
                            f"a ratio cannot be formed against it")
                    per_round.append(series[subj][rd]["mops"] / denom)
                entry = {
                    "id": f"{arm}/{subj}_vs_{base}",
                    "benchmark": arm,
                    "subject": subj,
                    "baseline": base,
                    "unit": "ratio_of_paired_per_round_Mops_higher_is_better",
                    "n": len(per_round),
                    "rounds_raw": [{"round": rd, "ratio": v}
                                   for rd, v in zip(shared, per_round)],
                }
                if len(per_round) >= MIN_ROUNDS:
                    r, lo, hi, ci_method = bca_bootstrap_ci_with_method(
                        per_round, CONFIDENCE, RESAMPLES, SEED)
                    entry.update({"ratio": round(r, 4), "ci_lower": round(lo, 4),
                                  "ci_upper": round(hi, 4), "ci_method": ci_method,
                                  # AGENTS.md 8.4: the LOWER bound clears the
                                  # floor, never the point estimate.
                                  "beats_baseline": lo > 1.0})
                else:
                    entry.update({"ratio": sum(per_round) / len(per_round),
                                  "ci_lower": None, "ci_upper": None, "ci_method": None,
                                  "beats_baseline": None,
                                  "why_no_interval":
                                      f"fewer than {MIN_ROUNDS} rounds; BCa needs n >= 3"})
                out.append(entry)
    return out


def census_cells(rows: list[dict]) -> list[dict]:
    """The memory census, and a refusal if it moved between rounds.

    Deterministic seeded allocator accounting: identical in every round by
    construction, so a difference is a defect and not a sample. Carries no
    interval -- one on an exact count would be wrong, not missing (8.4).
    """
    by_impl: dict[str, dict[int, dict]] = {}
    for r in rows:
        if r["arm"] != CENSUS_ARM:
            continue
        by_impl.setdefault(r["implementation"], {})[r["round"]] = r
    out = []
    for impl in sorted(by_impl):
        per_round = by_impl[impl]
        totals = {r["bytes_total"] for r in per_round.values()}
        if len(totals) != 1:
            raise RuntimeError(
                f"memory/{impl}: bytes_total differed across rounds ({sorted(totals)}) "
                f"— it is deterministic allocator accounting and must not vary")
        row = per_round[min(per_round)]
        out.append({
            "id": f"memory/{impl}",
            "implementation": impl,
            "unit": "bytes_per_entry",
            "bytes_total": row["bytes_total"],
            "bytes_per_entry": row["bytes_per_entry"],
            "entries": row["bytes_total"] / row["bytes_per_entry"],
            "rounds_observed": len(per_round),
            "interval": None,
            "why_no_interval": (
                "Deterministic allocator accounting — identical in every round, and "
                "verified so here. AGENTS.md 8.4 scopes interval requirements to "
                "continuous and sampling metrics; an interval on an exact count would "
                "be wrong, not missing."
            ),
        })
    return out


def build_artifact(rows: list[dict], provenance: dict, rounds: int, arms: tuple) -> dict:
    payload = {
        "schema": "expanse.baseline.v1",
        "kind": "wall_clock_bca",
        "suite": "rocksdb",
        "fixture": "integrations/rocksdb/benches/bench_memtable.cc",
        "workload_id": "rocksdb_memtable_single_threaded",
        "settings": {
            "rounds": rounds,
            "arms": list(arms),
            "invocation": "one `bench_memtable --arm <phase> --round N` per cell",
            "cell_boundary": (
                "(arm, round) — one process per phase per round, which is what makes "
                "load.foreign_busy_cpus attributable to a cell; the three "
                "implementations are timed inside one invocation so a ratio's two arms "
                "share one host state"
            ),
        },
        "statistics": {
            "estimator": "mean of per-round throughput (Mops/s)",
            "ratio_estimator": (
                "mean of the per-round paired quotient subject/baseline, with a "
                "one-sample BCa interval over those quotients"
            ),
            "method": "BCa bootstrap",
            "confidence": CONFIDENCE,
            "num_resamples": RESAMPLES,
            "seed": SEED,
            "point_and_interval_share_one_definition": True,
        },
        "cells": arm_cells(rows),
        "ratios": ratio_cells(rows),
        "memory": census_cells(rows),
        "verdicts": None,
        "why_no_verdicts": (
            "Verdicts are read against docs/benchmarks/rocksdb_memtable/METHODOLOGY.md "
            "by a reviewer; this driver emits the intervals and does not decide them."
        ),
    }
    # `attach` RETURNS the carrying dict; it does not mutate in place.
    return prov.attach(payload, provenance)


def output_path(out: Path, quick: bool) -> Path:
    """Where the artifact is written, with `--quick` confined to scratch.

    A smoke sweep must not be able to land on a committed baseline (AGENTS.md
    8.5), so `--quick` is redirected under `results/quick/` -- gitignored -- no
    matter what `--out` named.
    """
    if not quick:
        return out
    return SUITE_DIR / "results" / "quick" / out.name


def blind_cells(rows: list[dict]) -> list[str]:
    """Cells whose load snapshot could not attribute foreign CPU, by label."""
    bad = []
    for r in rows:
        fb = r.get("load", {}).get("foreign_busy_cpus")
        if not isinstance(fb, (int, float)) or isinstance(fb, bool):
            bad.append(r["cell"])
    return sorted(set(bad))


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------
def _csv(arm: str, rnd: int, rows: list[tuple]) -> str:
    head = ("# rocksdb_memtable_single_threaded\n" + ",".join(CSV_FIELDS) + "\n")
    body = ""
    for impl, ops, secs, consumed, bytes_total, bpe in rows:
        mops = 0.0 if secs == 0 else (ops / secs) / 1e6
        body += (f"{arm},{impl},{rnd},{ops},{secs:.12f},{mops:.9f},"
                 f"{consumed},{bytes_total},{bpe:.6f}\n")
    return head + body


def _timed(arm: str, rnd: int, per_impl: dict[str, float], consumed: int = 7) -> str:
    return _csv(arm, rnd, [(impl, 50000, 50000 / (m * 1e6), consumed, 0, 0.0)
                           for impl, m in per_impl.items()])


def _fake_load() -> dict:
    return {"since": "start", "wall_s": 1.0, "busy_cpus_since_prev": 1.2,
            "own_busy_cpus": 1.0, "foreign_busy_cpus": 0.2}


def self_test() -> int:  # noqa: C901 - a checklist, read top to bottom
    fails: list[str] = []

    def check(name, got, want):
        if got != want:
            fails.append(f"{name}: got {got!r}, want {want!r}")

    def raises(name, fn, needle=""):
        try:
            fn()
        except RuntimeError as exc:
            if needle and needle not in str(exc):
                fails.append(f"{name}: raised {exc!r}, expected it to mention {needle!r}")
        except Exception as exc:  # noqa: BLE001
            fails.append(f"{name}: raised {type(exc).__name__}, expected RuntimeError")
        else:
            fails.append(f"{name}: did not raise")

    # --- the interleaving invariant (AGENTS.md 8.20.2) ---------------------
    order = sweep_order(3, ("a", "b"))
    check("sweep_order", order, [(0, "a"), (0, "b"), (1, "a"), (1, "b"),
                                 (2, "a"), (2, "b")])
    # Round-major: the round index never decreases, and every arm appears once
    # per round. An arm-major order passes neither.
    if [rd for rd, _ in order] != sorted(rd for rd, _ in order):
        fails.append("sweep_order is not round-major")
    # And the call site must actually use it. A helper asserted in isolation
    # stays green when the loop it was written for is re-nested by hand, which
    # is the mutation this check exists to fail on.
    src = inspect.getsource(run_sweep)
    if "for rd, arm in sweep_order(" not in src:
        fails.append("run_sweep does not drive its loop from sweep_order(); the "
                     "interleaving invariant is unpinned at the call site")
    if sum(1 for ln in src.splitlines() if ln.strip().startswith("for ")) != 2:
        fails.append("run_sweep has gained or lost a loop; an arm-major nesting "
                     "would break the interleaving sweep_order() exists to fix")

    # --- CSV parsing ------------------------------------------------------
    rows = parse_rows(_timed("readrandom", 2, {"ExpanseMemTable": 3.8,
                                               "SkipListRep": 2.6}), "readrandom", 2)
    check("row count", len(rows), 2)
    check("round", rows[0]["round"], 2)
    check("implementation", rows[1]["implementation"], "SkipListRep")
    if abs(rows[0]["mops"] - 3.8) > 1e-6:
        fails.append(f"mops: {rows[0]['mops']}")

    raises("no data rows", lambda: parse_rows("# only a comment\n", "readrandom", 0),
           "no CSV data rows")
    raises("short row", lambda: parse_rows("readrandom,Expanse,0,1\n", "readrandom", 0),
           "CSV fields")
    raises("wrong arm",
           lambda: parse_rows(_timed("seekrandom", 0, {"A": 1.0}), "readrandom", 0),
           "measured a different phase")
    raises("wrong round",
           lambda: parse_rows(_timed("readrandom", 4, {"A": 1.0}), "readrandom", 0),
           "mislabel the raw rows")
    # A timed row that measured nothing must not arrive as a zero.
    raises("zero elapsed",
           lambda: parse_rows("readrandom,A,0,50000,0.000000000000,0.0,7,0,0.0\n",
                              "readrandom", 0),
           "measured nothing")
    # A mops column that disagrees with ops/elapsed_s is a mis-read table.
    raises("mops disagrees",
           lambda: parse_rows("readrandom,A,0,50000,0.010000000000,99.0,7,0,0.0\n",
                              "readrandom", 0),
           "disagrees with ops/elapsed_s")
    # Arm symmetry: the implementations must have done the same work.
    asym = _timed("prefixscan", 0, {"ExpanseMemTable (Iterator)": 100.0})
    asym += _csv("prefixscan", 0, [("SkipListRep", 50000, 0.001, 40000, 0, 0.0)]).splitlines(
        keepends=True)[-1]
    raises("asymmetric consumed", lambda: parse_rows(asym, "prefixscan", 0),
           "not measuring the same thing")
    # The census row's own shape.
    cen = parse_rows(_csv("memory", 0, [("ExpanseMemTable", 0, 0.0, 0, 1320704, 13.20704)]),
                     "memory", 0)
    check("census bytes", cen[0]["bytes_total"], 1320704)
    raises("census with no bytes",
           lambda: parse_rows("memory,A,0,0,0.000000000000,0.0,0,0,0.0\n", "memory", 0),
           "census row is malformed")
    raises("timed row carrying bytes",
           lambda: parse_rows("readrandom,A,0,50000,0.01,5.0,7,99,1.0\n", "readrandom", 0),
           "timed row carries a byte count")

    # --- paired ratio -----------------------------------------------------
    # Strong round-to-round drift with a constant per-round ratio of 1.5. A
    # paired estimator returns 1.5; an unpaired one smears it, which is why the
    # estimator is stated in the artifact rather than left to a reader.
    drifting: list[dict] = []
    for rd in range(5):
        base = 2.0 + 0.7 * rd
        for impl, m in (("ExpanseMemTable", 1.5 * base), ("SkipListRep", base)):
            drifting.append({"arm": "readrandom", "implementation": impl, "round": rd,
                             "ops": 50000, "elapsed_s": 1.0, "mops": m, "consumed": 7,
                             "bytes_total": 0, "bytes_per_entry": 0.0,
                             "cell": f"cell:readrandom:round{rd}", "load": _fake_load()})
    r = ratio_cells(drifting)
    check("one ratio", len(r), 1)
    if abs(r[0]["ratio"] - 1.5) > 1e-3:
        fails.append(f"paired ratio under drift: {r[0]['ratio']} (pairing lost?)")
    lo, hi = r[0]["ci_lower"], r[0]["ci_upper"]
    if not (lo <= r[0]["ratio"] <= hi):
        fails.append(f"ratio {r[0]['ratio']} outside its interval [{lo}, {hi}]")
    # The ratio cell must name its construction (AGENTS.md 8.1, #880). Which
    # label is not pinned on this fixture: (1.5 * base) / base is one ULP off
    # 1.5 in one round, so the distribution is two values and the estimator
    # honestly reports a clamped bias correction. Pinning that would pin float
    # noise. The varying fixture below pins the clean `bca` case instead.
    if r[0].get("ci_method") not in ("bca", "bc", "clamped", "degenerate"):
        fails.append(f"ratio cell names no construction: {r[0].get('ci_method')!r}")
    varying: list[dict] = []
    for rd, q in enumerate((1.4, 1.6, 1.45, 1.55, 1.5)):
        for impl, m in (("ExpanseMemTable", q), ("SkipListRep", 1.0)):
            varying.append({"arm": "readrandom", "implementation": impl, "round": rd,
                            "ops": 50000, "elapsed_s": 1.0, "mops": m, "consumed": 7,
                            "bytes_total": 0, "bytes_per_entry": 0.0,
                            "cell": f"cell:readrandom:round{rd}", "load": _fake_load()})
    check("a ratio that varies between rounds is a clean BCa interval",
          ratio_cells(varying)[0].get("ci_method"), "bca")
    check("beats_baseline is the LOWER bound clearing 1.0",
          r[0]["beats_baseline"], lo > 1.0)
    if r[0]["n"] != 5 or len(r[0]["rounds_raw"]) != 5:
        fails.append(f"ratio rounds_raw: {r[0]}")

    # --- arm cells --------------------------------------------------------
    cells = arm_cells(drifting)
    check("two cells", len(cells), 2)
    for c in cells:
        if not c["rounds_raw"]:
            fails.append(f"{c['id']} carries no rounds_raw")
        if not (c["ci_lower"] <= c["point"] <= c["ci_upper"]):
            fails.append(f"{c['id']}: point outside interval")
        # Throughput drifts round to round here, so this is the clean case; a
        # list where every label read `degenerate` would pass the ratio check.
        if c.get("ci_method") != "bca":
            fails.append(f"{c['id']}: varying samples should be a BCa interval, "
                         f"got {c.get('ci_method')!r}")
        if any("load" not in e for e in c["rounds_raw"]):
            fails.append(f"{c['id']}: a round row carries no load attribution")

    # --- census determinism ----------------------------------------------
    census = []
    for rd in range(3):
        census.append({"arm": "memory", "implementation": "ExpanseMemTable", "round": rd,
                       "ops": 0, "elapsed_s": 0.0, "mops": 0.0, "consumed": 0,
                       "bytes_total": 1320704, "bytes_per_entry": 13.20704,
                       "cell": f"cell:memory:round{rd}", "load": _fake_load()})
    check("census cell count", len(census_cells(census)), 1)
    check("census carries no interval", census_cells(census)[0]["interval"], None)
    drifted = [dict(c) for c in census]
    drifted[1]["bytes_total"] = 1320705
    raises("census drift", lambda: census_cells(drifted),
           "must not vary")

    # --- artifact shape the provenance gate reads -------------------------
    p = prov.new_provenance("rocksdb", 868, "paired per-round quotient",
                            repo_root=REPO_ROOT)
    art = build_artifact(drifting + census, p, 5, ARMS)
    for key in ("schema", "cells", "ratios", "memory", "provenance", "statistics"):
        if key not in art:
            fails.append(f"artifact missing {key}")
    if not isinstance(art["provenance"].get("host"), dict):
        fails.append("artifact provenance carries no host block")
    if not isinstance(art["provenance"].get("estimators"), dict):
        fails.append("artifact provenance carries no estimators block")
    if "core_pin" not in art["provenance"]:
        fails.append("artifact provenance does not record the core pin")
    loads = art["provenance"].get("loads") or []
    if not any("busy_cpus_since_prev" in s for s in loads):
        fails.append("artifact load snapshots carry no busy-CPU delta")
    for c in art["cells"]:
        if not c.get("rounds_raw"):
            fails.append(f"{c['id']}: no rounds_raw in the artifact")
            break

    # --- blindness refusal ------------------------------------------------
    check("a numeric foreign delta is not blind", blind_cells(drifting), [])
    blind = [dict(drifting[0])]
    blind[0]["load"] = dict(_fake_load(), foreign_busy_cpus=None)
    check("a None foreign delta is blind", blind_cells(blind),
          ["cell:readrandom:round0"])

    # --- --quick cannot overwrite a committed baseline (8.5) --------------
    q = output_path(DEFAULT_OUT, quick=True)
    if q.parent != SUITE_DIR / "results" / "quick":
        fails.append(f"--quick writes outside results/quick/: {q}")
    if output_path(DEFAULT_OUT, quick=False) != DEFAULT_OUT:
        fails.append("a non-quick run must write where --out said")

    if fails:
        print("single_threaded_bench.py --self-test: FAILED")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("single_threaded_bench.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--bench", type=Path, default=BENCH)
    ap.add_argument("--rounds", type=int, default=None,
                    help="default 5, or 1 under --quick")
    ap.add_argument("--quick", action="store_true",
                    help="smoke shape: 1 round unless --rounds says otherwise, written "
                         "under results/quick/ so it cannot overwrite a committed "
                         "baseline (AGENTS.md 8.5)")
    # Kept from the superseded harvester's interface: `host_facts()` records what
    # the machine is, and these two record which run it was and the anonymised
    # hardware description the suite README quotes (never a hostname, section 7).
    ap.add_argument("--host-desc", default="")
    ap.add_argument("--run-id", default="")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    if not args.bench.is_file():
        print(f"::error::benchmark binary not found: {args.bench}\n"
              f"build it with: make -C integrations/rocksdb bench", file=sys.stderr)
        return 1

    rounds = args.rounds if args.rounds is not None else (1 if args.quick else 5)
    if not args.quick and rounds < MIN_ROUNDS:
        print(f"::error::--rounds {rounds} cannot produce a BCa interval (n >= "
              f"{MIN_ROUNDS}); a point estimate is not publishable (AGENTS.md 8.4). "
              f"Use --quick for a shape smoke.", file=sys.stderr)
        return 1

    # The core pin, before anything is timed. `bench_pin.sh` pins the shell a
    # suite runner spawns this from and this call then verifies the affinity
    # actually arrived; run by hand with no such shell, it applies the pin
    # itself. On the hybrid reference host an arm that lands on an efficiency
    # core measures 1.576x the P-core time and no interval says so (#639).
    pin = bench_pin.apply("single_threaded_bench.py")

    provenance = prov.new_provenance(
        "rocksdb", 868, "paired per-round quotient subject/baseline, BCa 95%",
        repo_root=REPO_ROOT,
        pre_registration="docs/benchmarks/rocksdb_memtable/METHODOLOGY.md",
        host_description=args.host_desc or None,
        run_id=args.run_id or None,
        generated_by="docs/benchmarks/rocksdb_memtable/scripts/single_threaded_bench.py",
    )
    provenance["host"] = prov.host_facts(pin)
    provenance["estimators"] = prov.estimators(
        ratio=("mean over rounds of (subject Mops / baseline Mops) measured in the same "
               "invocation, with a one-sample BCa 95% interval over those paired "
               "quotients — not a two-sample ratio of the two columns beside it"),
        columns="per-arm columns are means of the same rounds' Mops/s",
        raw="every cell carries rounds_raw, the per-round samples and that cell's load",
    )

    rows = run_sweep(args.bench, rounds, ARMS, provenance)

    # A committed artifact must be able to say what else was on the host while
    # it was measured (AGENTS.md 8.17). The busy-CPU delta is read from
    # /proc/stat, which the Linux reference host has and a macOS dev box does
    # not -- where every cell would carry null and the artifact would be
    # rejected later, after the sweep had been paid for. Refuse now and name the
    # reason (8.1). `--quick` is exempt: it writes under results/quick/ and is a
    # shape smoke, never a baseline.
    if not args.quick:
        blind = blind_cells(rows)
        if blind:
            print(f"::error::{len(blind)} cell(s) carry no busy-CPU delta (first: "
                  f"{blind[0]}). The host exposes no /proc/stat, so this run cannot say "
                  f"whether anything else was resident while it was taken, and a "
                  f"committed artifact must (AGENTS.md 8.17). Run it on the reference "
                  f"host, or use --quick for a shape smoke under results/quick/.",
                  file=sys.stderr)
            return 1

    art = build_artifact(rows, provenance, rounds, ARMS)
    out = output_path(args.out, args.quick)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(art, indent=2) + "\n")
    print(f"wrote {out} ({len(art['cells'])} arm cell(s), {len(art['ratios'])} ratio(s), "
          f"{len(art['memory'])} census cell(s), {rounds} round(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
