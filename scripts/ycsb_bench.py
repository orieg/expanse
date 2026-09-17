#!/usr/bin/env python3
"""Driver for the single-threaded YCSB suites (#1005).

Two suites, one driver, because what they need is the same thing:

  * `--suite hashbrown` runs `benches/hashbrown_ycsb.rs` (workload
    `hashbrown_ycsb`) and writes
    `docs/benchmarks/hashbrown_comparison/results/baseline_ycsb.json`.
  * `--suite core` runs `benches/ycsb.rs` and `benches/ycsb_dense.rs`
    (workloads `workload_ycsb` and `workload_ycsb_dense`) and writes
    `docs/benchmarks/ycsb/results/baseline_ycsb.json`.

Both harnesses used to be run straight from `cargo bench`. The hashbrown
artifact that produced is six `*_mops` objects and nothing else -- no host, no
commit, no rounds, no estimator, no load snapshot -- and each cell is one
`Instant` window over one pass, so no interval can be computed from it. The
core suite's table resolved to a CI run and to no committed artifact at all.

Modelled on `docs/benchmarks/rocksdb_memtable/scripts/single_threaded_bench.py`,
deliberately. What this driver owns, and why the Rust binaries do not:

  * **The rounds and their order.** One harness process per `(round, harness)`.
    The order comes from `sweep_order()` and is round-major, so drift across
    the sweep lands inside every cell's samples alike instead of separating two
    harnesses (AGENTS.md 8.20.2). Inside a process the arms of a cell run back
    to back, with the arm that goes first rotating by round and workload, so a
    ratio's two arms share one host state.

  * **The pin.** `scripts/bench_pin.py` is applied before anything is timed and
    the result is recorded in the artifact. On the hybrid reference host an arm
    that lands on an efficiency core measures 1.576x the P-core time (#639).

  * **The load snapshots.** One per process boundary, with the busy-CPU delta
    and the split of own and foreign CPU (AGENTS.md 8.17).

  * **The intervals.** BCa 95% per cell, and **paired** BCa on the per-round
    quotient `subject_round / baseline_round` for every ratio -- paired because
    both arms of a ratio come from the same process in the same round.

  * **The symmetry checks.** Every arm of a cell must report the same work
    checksum (`consumed`), the round stamp must be the one asked for, and the
    throughput column must agree with ops / seconds.

  * **The gate's judgement, before the artifact lands.** The artifact is judged
    by `scripts/check_bench_provenance.py`'s own `findings_for()` under the name
    it will be committed as, before it is written and again in the self-test on
    synthetic rows (AGENTS.md 8.20.7).

What it does not own: the verdicts, and the prose. A reviewer reads the
intervals; `takeaways()` only renders sentences from the ratio cells so that a
summary cannot disagree with the table it summarises.

Usage:
    python3 scripts/ycsb_bench.py --suite hashbrown
    python3 scripts/ycsb_bench.py --suite core --populations 100k,1m,10m
    python3 scripts/ycsb_bench.py --suite core --quick
    python3 scripts/ycsb_bench.py --self-test
"""

from __future__ import annotations

import argparse
import copy
import inspect
import json
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
import bench_provenance as prov  # noqa: E402
import check_bench_provenance  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

BENCH_DOCS = REPO_ROOT / "docs" / "benchmarks"
PACKAGE = "expanse-trie"
ISSUE = 1005

CONFIDENCE = 0.95
RESAMPLES = 2000
SEED = 42

#: BCa needs n >= 3 (`scripts/bca_bootstrap.py`); fewer rounds cannot produce a
#: publishable interval, so a committed run refuses rather than emitting one.
MIN_ROUNDS = 3
DEFAULT_ROUNDS = 8

DISQUALIFIED = "DISQUALIFIED"

LATENCY_PERCENTILES = ("p50_ns", "p95_ns", "p99_ns", "p999_ns")

SUITES = {
    "hashbrown": {
        "provenance_suite": "hashbrown_comparison",
        "harnesses": ("hashbrown_ycsb",),
        "out": BENCH_DOCS / "hashbrown_comparison" / "results" / "baseline_ycsb.json",
        "quick_dir": BENCH_DOCS / "hashbrown_comparison" / "results" / "quick",
        # Subject arms and the baselines each is divided by. Payload is `u64`
        # on every arm of this harness, so every pair is payload-symmetric.
        "ratios": (("expanse", "btree", True), ("expanse", "hashbrown", True)),
        "legacy_summary": True,
    },
    "core": {
        "provenance_suite": "ycsb",
        "harnesses": ("ycsb", "ycsb_dense"),
        "out": BENCH_DOCS / "ycsb" / "results" / "baseline_ycsb.json",
        "quick_dir": BENCH_DOCS / "ycsb" / "results" / "quick",
        # The blob arm against the two blob baselines is payload-symmetric
        # (128 B on all three). The `u64` trie against a 128 B baseline is not
        # (AGENTS.md 8.16), and the cell says so instead of leaving it out.
        "ratios": (
            ("ExpanseBlobMap (128B)", "BTreeMap (128B)", True),
            ("ExpanseBlobMap (128B)", "SkipMap (128B)", True),
            ("ExpanseMap (u64)", "BTreeMap (128B)", False),
            ("ExpanseMap (u64)", "SkipMap (128B)", False),
        ),
        "legacy_summary": False,
    },
}


def gate_rel(suite: str) -> str:
    """The name the provenance gate checks this suite's artifact under.

    `--out` says where one run writes, but the gate keys its rules on the
    committed path, so the judgement is taken under that path whatever `--out`
    named. Derived from the default output, never spelled a second time.
    """
    return str(SUITES[suite]["out"].relative_to(BENCH_DOCS))


def sweep_order(rounds: int, harnesses: tuple) -> list[tuple[int, str]]:
    """The `(round, harness)` invocation order: round-major, harnesses interleaved."""
    if rounds < 1:
        raise ValueError(f"rounds must be >= 1, got {rounds}")
    if not harnesses:
        raise ValueError("no harnesses to sweep")
    return [(rd, h) for rd in range(rounds) for h in harnesses]


def invocation(harness: str, rd: int, quick: bool, populations: str | None,
               ops: int | None) -> tuple[list[str], dict[str, str]]:
    """The command and the extra environment for one `(round, harness)` process."""
    cmd = ["cargo", "bench", "-q", "-p", PACKAGE, "--bench", harness, "--"]
    env: dict[str, str] = {}
    if harness == "hashbrown_ycsb":
        cmd += ["--json", "--rounds", "1", "--round-index", str(rd)]
        if quick:
            cmd.append("--quick")
        if populations:
            if "," in populations:
                raise ValueError("hashbrown_ycsb takes one population per run")
            cmd += ["--population", str(parse_population(populations))]
        if ops:
            cmd += ["--ops", str(ops)]
    else:
        env["YCSB_ROUNDS_JSON"] = "1"
        env["YCSB_ROUND"] = str(rd)
        if populations:
            env["YCSB_POPULATIONS"] = populations
        if ops:
            env["YCSB_OPS"] = str(ops)
        elif quick:
            env["YCSB_OPS"] = "20000"
    return cmd, env


def parse_population(token: str) -> int:
    """`100k` / `1m` / `10m` / a bare integer, as the Rust harness reads them."""
    t = token.strip().lower()
    mult = 1
    if t.endswith("k"):
        t, mult = t[:-1], 1_000
    elif t.endswith("m"):
        t, mult = t[:-1], 1_000_000
    try:
        n = int(t) * mult
    except ValueError:
        raise ValueError(f"cannot parse population {token!r}") from None
    if n < 2:
        raise ValueError(f"population {token!r} is below 2")
    return n


def _check_throughput(where: str, row: dict) -> None:
    if row["ops"] <= 0 or row["elapsed_s"] <= 0.0:
        raise RuntimeError(
            f"{where}: ops={row['ops']} elapsed_s={row['elapsed_s']}: a cell that "
            f"measured nothing must not reach an artifact as a zero")
    recomputed = (row["ops"] / row["elapsed_s"]) / 1e6
    if abs(recomputed - row["mops"]) > 1e-6 * max(1.0, row["mops"]):
        raise RuntimeError(
            f"{where}: mops column {row['mops']} disagrees with ops/elapsed_s "
            f"{recomputed}")


def parse_rows(harness: str, text: str, expect_round: int) -> list[dict]:
    """The rows one harness process emitted, normalised, or raises.

    Fails loudly on anything unexpected rather than returning what it could
    parse: a process that produced nothing must stop the sweep, not contribute
    a hole to a published mean (AGENTS.md 8.1).
    """
    rows: list[dict] = []
    if harness == "hashbrown_ycsb":
        start, end = text.find("{"), text.rfind("}")
        if start < 0 or end < start:
            raise RuntimeError(f"{harness}: no JSON object in:\n{text}")
        doc = json.loads(text[start:end + 1])
        if doc.get("workload_id") != "hashbrown_ycsb":
            raise RuntimeError(f"{harness}: emitted workload_id {doc.get('workload_id')!r}")
        if doc.get("settings", {}).get("smoke"):
            raise RuntimeError(
                f"{harness}: the binary ran its smoke path (no --bench flag reached it); "
                f"a smoke run is not a measurement")
        for r in doc.get("rows", []):
            row = {
                "workload_id": "hashbrown_ycsb",
                "key_shape": "dense_sequential",
                "workload": r["workload"],
                "arm": r["arm"],
                "population": int(r["population"]),
                "insertion_order": r["insertion_order"],
                "round": int(r["round"]),
                "status": r.get("status", "COMPLETED"),
            }
            if not row["status"].startswith(DISQUALIFIED):
                row.update({"ops": int(r["ops"]), "elapsed_s": float(r["elapsed_s"]),
                            "mops": float(r["mops"]), "consumed": int(r["consumed"])})
            rows.append(row)
    else:
        for ln in text.splitlines():
            ln = ln.strip()
            if not ln.startswith("{"):
                continue
            r = json.loads(ln)
            rows.append({
                "workload_id": r["suite_workload"],
                "key_shape": r["key_shape"],
                "workload": r["workload"],
                "arm": r["engine"],
                "population": int(r["population"]),
                "insertion_order": r["insertion_order"],
                "round": int(r["round"]),
                "status": "COMPLETED",
                "ops": int(r["ops"]),
                "elapsed_s": float(r["elapsed_s"]),
                "mops": float(r["mops"]),
                "consumed": int(r["consumed"]),
                "mem_bytes": int(r["mem_bytes"]),
                "mem_is_estimate": bool(r["mem_is_estimate"]),
                "latency": r["latency"],
            })
    if not rows:
        raise RuntimeError(f"{harness}: no rows in:\n{text}")

    by_cell: dict[tuple, set[int]] = {}
    for row in rows:
        where = (f"{row['workload_id']}/{row['workload']}/{row['arm']}/"
                 f"n{row['population']}/{row['insertion_order']}")
        if row["round"] != expect_round:
            raise RuntimeError(
                f"{where}: asked for round {expect_round} and the binary reported "
                f"{row['round']} — the round stamp would mislabel the raw rows a "
                f"published mean is recomputed from")
        if row["insertion_order"] not in ("sorted", "shuffled"):
            raise RuntimeError(f"{where}: unknown insertion order")
        if row["status"].startswith(DISQUALIFIED):
            continue
        _check_throughput(where, row)
        key = (row["workload_id"], row["workload"], row["population"],
               row["insertion_order"])
        by_cell.setdefault(key, set()).add(row["consumed"])
    # Symmetry: every arm of a cell did the same amount of work. A scan that
    # terminates early, or a write an arm silently rejects, is fast and wrong,
    # and a ratio would publish the shortfall as a win (AGENTS.md 8.3).
    for key, consumed in sorted(by_cell.items()):
        if len(consumed) != 1:
            raise RuntimeError(
                f"{'/'.join(str(k) for k in key)}: arms disagree on how much work they "
                f"did ({sorted(consumed)}) — not measuring the same thing, so a ratio "
                f"over them would compare two workloads (AGENTS.md 8.3)")
    return rows


def preflight(harnesses: tuple) -> None:
    """Build every harness once, so a compile failure reports as one, up front."""
    for h in harnesses:
        res = subprocess.run(["cargo", "bench", "-p", PACKAGE, "--bench", h, "--no-run"],
                             capture_output=True, text=True, cwd=REPO_ROOT)
        if res.returncode != 0:
            raise RuntimeError(f"preflight: `cargo bench --bench {h} --no-run` exited "
                               f"{res.returncode}\nstderr:\n{res.stderr}")


def run_sweep(suite: str, rounds: int, quick: bool, populations: str | None,
              ops: int | None, provenance: dict) -> list[dict]:
    harnesses = SUITES[suite]["harnesses"]
    preflight(harnesses)
    rows: list[dict] = []
    # `new_provenance` already took the opening snapshot; a second one here
    # would leave two cells labelled `start` and make `since` ambiguous.
    for rd, harness in sweep_order(rounds, harnesses):
        label = f"cell:{harness}:round{rd}"
        start = prov.begin_cell(provenance, label)
        cmd, extra = invocation(harness, rd, quick, populations, ops)
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=REPO_ROOT,
                             env={**os.environ, **extra})
        if res.returncode != 0:
            raise RuntimeError(
                f"{label}: {' '.join(cmd)} exited {res.returncode}\n"
                f"stdout:\n{res.stdout}\nstderr:\n{res.stderr}")
        load = prov.end_cell(start)
        for row in parse_rows(harness, res.stdout, rd):
            row["cell"] = label
            row["load"] = load
            rows.append(row)
    prov.add_load(provenance, "end")
    return rows


def _cell_key(r: dict) -> tuple:
    return (r["workload_id"], r["workload"], r["arm"], r["population"],
            r["insertion_order"])


def _series(rows: list[dict]) -> dict[tuple, dict[int, dict]]:
    """`{cell key: {round: row}}` over the timed rows."""
    out: dict[tuple, dict[int, dict]] = {}
    for r in rows:
        if r["status"].startswith(DISQUALIFIED):
            continue
        per = out.setdefault(_cell_key(r), {})
        if r["round"] in per:
            raise RuntimeError(f"{'/'.join(map(str, _cell_key(r)))}: round "
                               f"{r['round']} measured twice")
        per[r["round"]] = r
    return out


def _interval(samples: list[float]) -> dict:
    """Point, BCa 95% interval and the construction that produced it."""
    if len(samples) < MIN_ROUNDS:
        return {"point": sum(samples) / len(samples), "ci_lower": None,
                "ci_upper": None, "ci_method": None,
                "why_no_interval": f"fewer than {MIN_ROUNDS} rounds; BCa needs n >= 3"}
    point, lo, hi, ci_method = bca_bootstrap_ci_with_method(
        samples, CONFIDENCE, RESAMPLES, SEED)
    return {"point": point, "ci_lower": lo, "ci_upper": hi, "ci_method": ci_method}


def _cell_id(key: tuple) -> str:
    wid, workload, arm, pop, order = key
    return f"{wid}/{workload}/{arm}/n{pop}/{order}"


def arm_cells(rows: list[dict]) -> list[dict]:
    """One published cell per `(workload id, workload, arm, population, order)`.

    `rounds_raw` carries each round's ops, seconds, throughput, work checksum
    *and that process's load attribution*: the process boundary is
    `(round, harness)`, so every cell of a harness in a round shares one
    snapshot, and the artifact says so rather than implying a per-cell one.
    """
    cells = []
    for key, by_round in sorted(_series(rows).items()):
        wid, workload, arm, pop, order = key
        raw = []
        for rd in sorted(by_round):
            r = by_round[rd]
            entry = {"round": rd, "mops": r["mops"], "ops": r["ops"],
                     "elapsed_s": r["elapsed_s"], "consumed": r["consumed"],
                     "load": r["load"]}
            if "latency" in r:
                entry["latency"] = r["latency"]
            raw.append(entry)
        first = by_round[min(by_round)]
        cell = {
            "id": _cell_id(key),
            "workload_id": wid,
            "key_shape": first["key_shape"],
            "workload": workload,
            "arm": arm,
            "population": pop,
            "insertion_order": order,
            "unit": "Mops_per_second",
            "n": len(raw),
            "rounds_raw": raw,
        }
        cell.update(_interval([e["mops"] for e in raw]))
        if "latency" in first:
            lat = first["latency"]
            cell["latency_window_means"] = {
                "method": lat["method"],
                "window_ops": lat["window_ops"],
                "what_a_percentile_is": (
                    "a percentile of per-window mean ns/op, NOT of single-op latency: "
                    "one stalled op inside a window moves that sample by 1/window_ops "
                    "of the stall"),
                "residual_ns_per_op": _interval(
                    [e["latency"]["residual_ns_per_op"] for e in raw]),
                "slow_windows_per_round": [e["latency"]["slow_windows"] for e in raw],
                **{p: _interval([e["latency"][p] for e in raw])
                   for p in LATENCY_PERCENTILES},
            }
        if "mem_bytes" in first:
            cell["mem_bytes"] = first["mem_bytes"]
            cell["mem_is_estimate"] = first["mem_is_estimate"]
        cells.append(cell)
    return cells


def ratio_cells(rows: list[dict], pairs: tuple) -> list[dict]:
    """Paired BCa on `subject_round / baseline_round`, per cell coordinate.

    Paired, not two-sample: both arms of every ratio were timed in the same
    process in the same round, so the per-round quotient removes the round's
    own drift instead of leaving it in both marginals.
    """
    series = _series(rows)
    coords = sorted({(k[0], k[1], k[3], k[4]) for k in series})
    out = []
    for wid, workload, pop, order in coords:
        for subj, base, symmetric in pairs:
            s = series.get((wid, workload, subj, pop, order))
            b = series.get((wid, workload, base, pop, order))
            if not s or not b:
                continue  # hashbrown has no E cell; the absence is the status row's
            shared = sorted(set(s) & set(b))
            per_round = []
            for rd in shared:
                denom = b[rd]["mops"]
                if denom <= 0.0:
                    raise RuntimeError(f"{wid}/{workload}/{base} round {rd} reported "
                                       f"{denom} Mops/s; no ratio can be formed")
                per_round.append(s[rd]["mops"] / denom)
            entry = {
                "id": f"{wid}/{workload}/{subj}_vs_{base}/n{pop}/{order}",
                "workload_id": wid,
                "workload": workload,
                "population": pop,
                "insertion_order": order,
                "subject": subj,
                "baseline": base,
                "payload_symmetric": symmetric,
                "unit": "ratio_of_paired_per_round_Mops_higher_is_better",
                "n": len(per_round),
                "rounds_raw": [{"round": rd, "ratio": v}
                               for rd, v in zip(shared, per_round)],
            }
            entry.update(_interval(per_round))
            lo = entry["ci_lower"]
            # AGENTS.md 8.4: the LOWER bound clears the floor, never the point.
            entry["subject_leads"] = None if lo is None else lo > 1.0
            hi = entry["ci_upper"]
            entry["baseline_leads"] = None if hi is None else hi < 1.0
            out.append(entry)
    return out


def takeaways(ratios: list[dict]) -> list[str]:
    """One sentence per ratio cell, rendered from the cell and nothing else.

    The published takeaway ratios disagreed with the table above them because
    they were typed. A sentence produced here cannot: every number in it is a
    field of the cell it describes, interval and workload id included.
    """
    out = []
    for r in ratios:
        if r["ci_lower"] is None:
            continue
        if r["subject_leads"]:
            verdict = "leads"
        elif r["baseline_leads"]:
            verdict = "trails"
        else:
            verdict = "is not separated from"
        note = "" if r["payload_symmetric"] else "; payloads differ between the two arms"
        out.append(
            f"{r['workload']} n={r['population']} {r['insertion_order']}: "
            f"{r['subject']} {verdict} {r['baseline']}, "
            f"{r['point']:.3f}x [{r['ci_lower']:.3f}, {r['ci_upper']:.3f}] "
            f"({r['ci_method']}, n={r['n']} paired rounds; workload: `{r['workload_id']}`"
            f"{note})")
    return out


def status_rows(rows: list[dict]) -> list[dict]:
    """Arms that did not run a workload, and why -- never rendered as a zero."""
    seen, out = set(), []
    for r in rows:
        if not r["status"].startswith(DISQUALIFIED):
            continue
        key = (r["workload_id"], r["workload"], r["arm"])
        if key in seen:
            continue
        seen.add(key)
        out.append({"workload_id": r["workload_id"], "workload": r["workload"],
                    "arm": r["arm"], "status": r["status"]})
    return out


def legacy_summary(cells: list[dict], statuses: list[dict]) -> dict:
    """The `workload_x: {expanse_mops, ...}` objects `generate_charts.py` reads.

    Taken from the `sorted` cells, which is the order the pre-#1005 artifact was
    measured in, and labelled as such: a chart of one order is a chart of one
    regime, and the key says which.
    """
    out: dict[str, dict] = {}
    for c in cells:
        if c["insertion_order"] != "sorted":
            continue
        slot = out.setdefault(c["workload"], {
            "workload": "Workload " + c["workload"][-1].upper(),
            "summary_insertion_order": "sorted",
            "summary_estimator": "mean over rounds of the sorted-order cell",
            "hashbrown_status": "COMPLETED",
        })
        slot[f"{c['arm']}_mops"] = c["point"]
    for s in statuses:
        slot = out.setdefault(s["workload"], {})
        slot[f"{s['arm']}_mops"] = None
        slot[f"{s['arm']}_status"] = s["status"]
    return out


def build_artifact(suite: str, rows: list[dict], provenance: dict, rounds: int,
                   settings: dict) -> dict:
    spec = SUITES[suite]
    cells = arm_cells(rows)
    ratios = ratio_cells(rows, spec["ratios"])
    statuses = status_rows(rows)
    payload = {
        "schema": "expanse.ycsb.v1",
        "kind": "wall_clock_bca",
        "suite": spec["provenance_suite"],
        "workload_ids": sorted({r["workload_id"] for r in rows}),
        "settings": {
            **settings,
            "rounds": rounds,
            "harnesses": list(spec["harnesses"]),
            "invocation": "one `cargo bench --bench <harness>` process per (round, harness)",
            "cell_boundary": (
                "(round, harness) — every cell of a harness in a round shares one "
                "process and one load snapshot; the arms of a cell run back to back "
                "inside it, so a ratio's two arms share one host state"
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
        "cells": cells,
        "ratios": ratios,
        "arm_status": statuses,
        "takeaways": takeaways(ratios),
        "verdicts": None,
        "why_no_verdicts": (
            "No gate is pre-registered on this suite; the driver emits the intervals "
            "and a reviewer reads them."
        ),
    }
    if spec["legacy_summary"]:
        payload.update(legacy_summary(cells, statuses))
    # `attach` RETURNS the carrying dict; it does not mutate in place.
    return prov.attach(payload, provenance)


def gate_findings(suite: str, artifact: dict) -> list[str]:
    """`check_bench_provenance.py`'s findings for this artifact, as CI will check it.

    The gate's own code, not a restatement of its rules: a field the gate
    starts requiring is refused here -- before a sweep's artifact is written,
    and in the self-test -- rather than by the lint after a dispatched host run
    has already been paid for (AGENTS.md 8.20.7).
    """
    return check_bench_provenance.findings_for(gate_rel(suite), artifact)


def output_path(suite: str, out: Path, quick: bool) -> Path:
    """Where the artifact is written, with `--quick` confined to scratch (8.5)."""
    if not quick:
        return out
    return SUITES[suite]["quick_dir"] / out.name


def blind_cells(rows: list[dict]) -> list[str]:
    """Process cells whose load snapshot could not attribute foreign CPU."""
    bad = []
    for r in rows:
        fb = r.get("load", {}).get("foreign_busy_cpus")
        if not isinstance(fb, (int, float)) or isinstance(fb, bool):
            bad.append(r["cell"])
    return sorted(set(bad))


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------
def _fake_load() -> dict:
    return {"since": "start", "wall_s": 1.0, "busy_cpus_since_prev": 1.2,
            "own_busy_cpus": 1.0, "foreign_busy_cpus": 0.2}


def _hashbrown_doc(rd: int, mops: dict[str, float], consumed: int = 9,
                   smoke: bool = False, workloads=("workload_a", "workload_e")) -> str:
    rows = []
    for order in ("sorted", "shuffled"):
        for wl in workloads:
            for arm, m in mops.items():
                if arm == "hashbrown" and wl == "workload_e":
                    rows.append({"workload": wl, "arm": arm, "insertion_order": order,
                                 "population": 500000, "round": rd,
                                 "status": "DISQUALIFIED: no ordered scan"})
                    continue
                rows.append({"workload": wl, "arm": arm, "insertion_order": order,
                             "population": 500000, "round": rd, "ops": 100000,
                             "elapsed_s": 100000 / (m * 1e6), "mops": m,
                             "consumed": consumed, "status": "COMPLETED"})
    return "cargo noise\n" + json.dumps({"workload_id": "hashbrown_ycsb",
                                         "settings": {"smoke": smoke}, "rows": rows})


def _core_line(wid: str, shape: str, wl: str, engine: str, rd: int, mops: float,
               consumed: int = 9) -> str:
    return json.dumps({
        "suite_workload": wid, "key_shape": shape, "workload": wl, "engine": engine,
        "population": 100000, "insertion_order": "sorted", "round": rd, "ops": 200000,
        "elapsed_s": 200000 / (mops * 1e6), "mops": mops, "consumed": consumed,
        "mem_bytes": 1000, "mem_is_estimate": engine.startswith(("BTree", "Skip")),
        "latency": {"method": "window_mean", "window_ops": 64, "windows": 3125,
                    "bracket_ns": 26.0, "residual_ns_per_op": 26.0 / 64 + rd * 0.001,
                    "p50_ns": 40.0 + rd, "p90_ns": 50.0 + rd, "p95_ns": 60.0 + rd * 2,
                    "p99_ns": 90.0 + rd * 3, "p999_ns": 400.0 + rd * 7,
                    "max_ns": 900.0, "slow_windows": rd % 2,
                    "slow_window_factor": 8.0}})


def self_test() -> int:  # noqa: C901 - a checklist, read top to bottom
    fails: list[str] = []

    def check(name, got, want):
        if got != want:
            fails.append(f"{name}: got {got!r}, want {want!r}")

    def raises(name, fn, needle="", kind=RuntimeError):
        try:
            fn()
        except kind as exc:
            if needle and needle not in str(exc):
                fails.append(f"{name}: raised {exc!r}, expected it to mention {needle!r}")
        except Exception as exc:  # noqa: BLE001
            fails.append(f"{name}: raised {type(exc).__name__}, expected {kind.__name__}")
        else:
            fails.append(f"{name}: did not raise")

    # --- the interleaving invariant (AGENTS.md 8.20.2) ---------------------
    order = sweep_order(3, ("a", "b"))
    check("sweep_order", order, [(0, "a"), (0, "b"), (1, "a"), (1, "b"),
                                 (2, "a"), (2, "b")])
    src = inspect.getsource(run_sweep)
    if "for rd, harness in sweep_order(" not in src:
        fails.append("run_sweep does not drive its loop from sweep_order(); the "
                     "interleaving invariant is unpinned at the call site")
    raises("zero rounds", lambda: sweep_order(0, ("a",)), "rounds", ValueError)

    # --- invocations -------------------------------------------------------
    cmd, env = invocation("hashbrown_ycsb", 4, False, None, None)
    check("hashbrown passes one round per process",
          cmd[cmd.index("--") + 1:], ["--json", "--rounds", "1", "--round-index", "4"])
    check("hashbrown takes no env", env, {})
    cmd, env = invocation("ycsb_dense", 2, False, "100k,1m,10m", None)
    check("core selects the report mode by environment",
          env, {"YCSB_ROUNDS_JSON": "1", "YCSB_ROUND": "2",
                "YCSB_POPULATIONS": "100k,1m,10m"})
    check("core passes nothing after --", cmd[-1], "--")
    check("population tokens", [parse_population(t) for t in ("100k", "1M", "10m", "4096")],
          [100_000, 1_000_000, 10_000_000, 4096])
    raises("bad population", lambda: parse_population("lots"), "cannot parse", ValueError)

    # --- parsing -----------------------------------------------------------
    rows = parse_rows("hashbrown_ycsb",
                      _hashbrown_doc(2, {"expanse": 40.0, "btree": 14.0, "hashbrown": 160.0}),
                      2)
    check("hashbrown row count", len(rows), 12)
    check("disqualified rows are kept as statuses",
          sum(1 for r in rows if r["status"].startswith(DISQUALIFIED)), 2)
    raises("wrong round",
           lambda: parse_rows("hashbrown_ycsb", _hashbrown_doc(1, {"expanse": 1.0}), 0),
           "mislabel the raw rows")
    raises("smoke output is refused",
           lambda: parse_rows("hashbrown_ycsb",
                              _hashbrown_doc(0, {"expanse": 1.0}, smoke=True), 0),
           "not a measurement")
    raises("no JSON", lambda: parse_rows("hashbrown_ycsb", "nothing here", 0),
           "no JSON object")
    raises("no core rows", lambda: parse_rows("ycsb", "# only a comment\n", 0), "no rows")
    asym = json.loads(_hashbrown_doc(0, {"expanse": 40.0, "btree": 14.0})[len("cargo noise\n"):])
    asym["rows"][0]["consumed"] = 8
    raises("asymmetric work", lambda: parse_rows("hashbrown_ycsb", json.dumps(asym), 0),
           "not measuring the same thing")
    lying = json.loads(_hashbrown_doc(0, {"expanse": 40.0})[len("cargo noise\n"):])
    lying["rows"][0]["mops"] = 99.0
    raises("mops disagrees", lambda: parse_rows("hashbrown_ycsb", json.dumps(lying), 0),
           "disagrees with ops/elapsed_s")
    core = parse_rows("ycsb", "# comment\n" + _core_line(
        "workload_ycsb", "uniform_random", "workload_e", "BTreeMap (128B)", 3, 1.3), 3)
    check("core row carries its latency block", core[0]["latency"]["window_ops"], 64)
    check("core row carries its workload id", core[0]["workload_id"], "workload_ycsb")

    # --- a synthetic sweep, both suites -------------------------------------
    def sweep(suite: str) -> list[dict]:
        out = []
        for rd in range(5):
            drift = 1.0 + 0.2 * rd
            if suite == "hashbrown":
                q = (2.9, 3.1, 2.95, 3.05, 3.0)[rd]
                got = parse_rows("hashbrown_ycsb", _hashbrown_doc(
                    rd, {"expanse": 14.0 * drift * q, "btree": 14.0 * drift,
                         "hashbrown": 160.0 * drift}), rd)
            else:
                q = (0.62, 0.66, 0.63, 0.65, 0.64)[rd]
                text = ""
                for wid, shape in (("workload_ycsb", "uniform_random"),
                                   ("workload_ycsb_dense", "dense_clustered")):
                    for engine, m in (("ExpanseMap (u64)", 0.8 * drift),
                                      ("ExpanseBlobMap (128B)", 1.3 * drift * q),
                                      ("BTreeMap (128B)", 1.3 * drift),
                                      ("SkipMap (128B)", 0.3 * drift)):
                        text += _core_line(wid, shape, "workload_e", engine, rd, m) + "\n"
                got = parse_rows("ycsb", text, rd)
            for r in got:
                r["cell"] = f"cell:x:round{rd}"
                r["load"] = _fake_load()
            out.extend(got)
        return out

    for suite in SUITES:
        rows = sweep(suite)
        p = prov.new_provenance(SUITES[suite]["provenance_suite"], ISSUE,
                                "paired per-round quotient", repo_root=REPO_ROOT)
        art = build_artifact(suite, rows, p, 5, {"quick": False})
        for key in ("schema", "cells", "ratios", "provenance", "statistics", "takeaways"):
            if key not in art:
                fails.append(f"{suite}: artifact missing {key}")
        if "core_pin" not in art["provenance"]:
            fails.append(f"{suite}: artifact provenance does not record the core pin")
        check(f"{suite}: a well-formed artifact has no gate findings",
              gate_findings(suite, art), [])
        for c in art["cells"]:
            if not (c["ci_lower"] <= c["point"] <= c["ci_upper"]):
                fails.append(f"{c['id']}: point outside its interval")
            if c.get("ci_method") != "bca":
                fails.append(f"{c['id']}: varying samples should be BCa, got "
                             f"{c.get('ci_method')!r}")
            if any("load" not in e for e in c["rounds_raw"]):
                fails.append(f"{c['id']}: a round row carries no load attribution")
            for key in ("population", "insertion_order", "workload_id"):
                if key not in c:
                    fails.append(f"{c['id']}: cell does not record {key}")

        # The ratio is paired: strong round-to-round drift, a quotient that
        # barely moves. An unpaired estimator would smear it.
        want = 3.0 if suite == "hashbrown" else 0.64
        subj = SUITES[suite]["ratios"][0]
        r0 = [r for r in art["ratios"] if (r["subject"], r["baseline"]) == subj[:2]][0]
        if abs(r0["point"] - want) > 1e-6:
            fails.append(f"{suite}: paired ratio under drift is {r0['point']}, want {want}")
        if r0["ci_upper"] - r0["ci_lower"] > 0.2:
            fails.append(f"{suite}: ratio interval {r0['ci_lower']}..{r0['ci_upper']} is "
                         f"as wide as the drift — pairing lost?")
        check(f"{suite}: subject_leads is the LOWER bound clearing 1.0",
              r0["subject_leads"], r0["ci_lower"] > 1.0)
        check(f"{suite}: baseline_leads is the UPPER bound under 1.0",
              r0["baseline_leads"], r0["ci_upper"] < 1.0)
        # A takeaway is rendered from its cell: the numbers in the sentence are
        # the cell's, to the digit, with the interval and the workload id.
        sentence = [t for t in art["takeaways"]
                    if f"{subj[0]} " in t and f" {subj[1]}," in t][0]
        for needle in (f"{r0['point']:.3f}x", f"[{r0['ci_lower']:.3f}, {r0['ci_upper']:.3f}]",
                       f"workload: `{r0['workload_id']}`",
                       "leads" if suite == "hashbrown" else "trails"):
            if needle not in sentence:
                fails.append(f"{suite}: takeaway {sentence!r} lacks {needle!r}")

        def gate_names(name: str, mutate, needle: str, art=art, suite=suite) -> None:
            broken = copy.deepcopy(art)
            mutate(broken)
            found = gate_findings(suite, broken)
            if not any(needle in f for f in found):
                fails.append(f"{suite}/{name}: the gate did not name {needle!r}; "
                             f"findings: {found!r}")

        # `pop(key, None)`: a site that already dropped the field must be NAMED
        # by the gate, not turn this check into a KeyError of its own.
        gate_names("a cell without its rounds",
                   lambda a: a["cells"][0].pop("rounds_raw", None), "rounds_raw")
        gate_names("a header without host facts",
                   lambda a: a["provenance"].pop("host", None), "provenance.host")
        gate_names("a header without estimators",
                   lambda a: a["provenance"].pop("estimators", None),
                   "provenance.estimators")
        gate_names("load snapshots without a busy-CPU delta",
                   lambda a: a["provenance"].__setitem__(
                       "loads", [{k: v for k, v in s.items() if k != "busy_cpus_since_prev"}
                                 for s in a["provenance"]["loads"]]),
                   "busy_cpus_since_prev")
        gate_names("a ratio cell that names no construction",
                   lambda a: a["ratios"][0].pop("ci_method", None), "construction label")

        if suite == "hashbrown":
            # hashbrown has no E cell and no E ratio, and says why.
            if any(c["arm"] == "hashbrown" and c["workload"] == "workload_e"
                   for c in art["cells"]):
                fails.append("a disqualified arm was published as a cell")
            check("the disqualification is recorded",
                  [s["arm"] for s in art["arm_status"]], ["hashbrown"])
            legacy = art["workload_a"]
            check("legacy summary names its order", legacy["summary_insertion_order"],
                  "sorted")
            for k in ("expanse_mops", "btree_mops", "hashbrown_mops"):
                if not isinstance(legacy.get(k), float):
                    fails.append(f"legacy summary lacks {k}")
            check("a disqualified arm is null in the chart summary, never zero",
                  art["workload_e"]["hashbrown_mops"], None)
        else:
            check("both key shapes land in one artifact", art["workload_ids"],
                  ["workload_ycsb", "workload_ycsb_dense"])
            lat = art["cells"][0]["latency_window_means"]
            for pname in LATENCY_PERCENTILES:
                if lat[pname].get("ci_method") != "bca":
                    fails.append(f"latency {pname} carries no BCa interval: {lat[pname]!r}")
            if "NOT of single-op latency" not in lat["what_a_percentile_is"]:
                fails.append("the latency block does not say what its percentiles are")
            flags = {(r["subject"], r["payload_symmetric"]) for r in art["ratios"]}
            check("payload symmetry is recorded per ratio", flags,
                  {("ExpanseBlobMap (128B)", True), ("ExpanseMap (u64)", False)})

    check("gate name: hashbrown", gate_rel("hashbrown"),
          "hashbrown_comparison/results/baseline_ycsb.json")
    check("gate name: core", gate_rel("core"), "ycsb/results/baseline_ycsb.json")
    # The core artifact's suite directory is one the gate sweeps; otherwise the
    # committed artifact would be judged by nothing.
    if "ycsb" not in check_bench_provenance.SUITES:
        fails.append("check_bench_provenance.SUITES does not include `ycsb`")

    # Too few rounds: a point with no interval, and the reason.
    few = _interval([1.0, 2.0])
    check("two rounds carry no interval", (few["ci_lower"], few["ci_method"]), (None, None))
    if "why_no_interval" not in few:
        fails.append("a cell with no interval does not say why")

    # The driver's own source passes the gate's producer census.
    check("producer census",
          check_bench_provenance.producer_problems(
              "scripts/ycsb_bench.py", Path(__file__).read_text()), [])

    # --- blindness refusal ------------------------------------------------
    rows = sweep("hashbrown")
    check("a numeric foreign delta is not blind", blind_cells(rows), [])
    blind = [dict(rows[0])]
    blind[0]["load"] = dict(_fake_load(), foreign_busy_cpus=None)
    check("a None foreign delta is blind", blind_cells(blind), ["cell:x:round0"])

    # --- --quick cannot overwrite a committed baseline (8.5) --------------
    for suite, spec in SUITES.items():
        q = output_path(suite, spec["out"], quick=True)
        if q.parent != spec["quick_dir"] or q.parent.name != "quick":
            fails.append(f"{suite}: --quick writes outside results/quick/: {q}")
        if output_path(suite, spec["out"], quick=False) != spec["out"]:
            fails.append(f"{suite}: a non-quick run must write where --out said")

    if fails:
        print("ycsb_bench.py --self-test: FAILED")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("ycsb_bench.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--suite", choices=sorted(SUITES))
    ap.add_argument("--out", type=Path, default=None)
    ap.add_argument("--rounds", type=int, default=None,
                    help=f"default {DEFAULT_ROUNDS}, or 1 under --quick")
    ap.add_argument("--populations", default=None,
                    help="core: comma-separated, e.g. 100k,1m,10m (default 100k); "
                         "hashbrown: one population (default 500k)")
    ap.add_argument("--ops", type=int, default=None, help="ops per cell (harness default)")
    ap.add_argument("--quick", action="store_true",
                    help="smoke shape: 1 round unless --rounds says otherwise, written "
                         "under results/quick/ so it cannot overwrite a committed "
                         "baseline (AGENTS.md 8.5)")
    ap.add_argument("--host-desc", default="")
    ap.add_argument("--run-id", default="")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if not args.suite:
        ap.error("--suite is required")

    rounds = args.rounds if args.rounds is not None else (1 if args.quick else DEFAULT_ROUNDS)
    if not args.quick and rounds < MIN_ROUNDS:
        print(f"::error::--rounds {rounds} cannot produce a BCa interval (n >= "
              f"{MIN_ROUNDS}); a point estimate is not publishable (AGENTS.md 8.4). "
              f"Use --quick for a shape smoke.", file=sys.stderr)
        return 1
    if args.populations:
        for token in args.populations.split(","):
            parse_population(token)

    # The core pin, before anything is timed. A suite runner that sourced
    # `bench_pin.sh` has already pinned the shell and this call verifies the
    # affinity arrived; run by hand with no such shell, it applies the pin.
    pin = bench_pin.apply("ycsb_bench.py")

    spec = SUITES[args.suite]
    provenance = prov.new_provenance(
        spec["provenance_suite"], ISSUE,
        "paired per-round quotient subject/baseline, BCa 95%",
        repo_root=REPO_ROOT,
        host_description=args.host_desc or None,
        run_id=args.run_id or None,
        generated_by="scripts/ycsb_bench.py",
        quick=args.quick,
    )
    provenance["host"] = prov.host_facts(pin)
    provenance["estimators"] = prov.estimators(
        ratio=("mean over rounds of (subject Mops / baseline Mops) measured in the same "
               "process in the same round, with a one-sample BCa 95% interval over those "
               "paired quotients — not a two-sample ratio of the two columns beside it"),
        columns=("per-arm columns are means of the same rounds' Mops/s; latency figures "
                 "are percentiles of per-window mean ns/op, not of single-op latency"),
        raw="every cell carries rounds_raw, the per-round samples and that process's load",
    )

    rows = run_sweep(args.suite, rounds, args.quick, args.populations, args.ops, provenance)

    # A committed artifact must be able to say what else was on the host while
    # it was measured (AGENTS.md 8.17). The busy-CPU delta is read from
    # /proc/stat, which the Linux reference host has and a macOS dev box does
    # not. Refuse now and name the reason (8.1); `--quick` is exempt because it
    # writes under results/quick/ and is a shape smoke, never a baseline.
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

    art = build_artifact(args.suite, rows, provenance, rounds,
                         {"quick": args.quick, "populations": args.populations,
                          "ops_override": args.ops})
    if not args.quick:
        problems = gate_findings(args.suite, art)
        if problems:
            # Nothing is written: a file that looks like a result and fails the
            # gate is worse than no file (AGENTS.md 8.1).
            print("::error::the artifact would fail scripts/check_bench_provenance.py "
                  "and is not written:\n  " + "\n  ".join(problems), file=sys.stderr)
            return 1
    out = output_path(args.suite, args.out or spec["out"], args.quick)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(art, indent=2) + "\n")
    print(f"wrote {out} ({len(art['cells'])} cell(s), {len(art['ratios'])} ratio(s), "
          f"{rounds} round(s))")
    for line in art["takeaways"]:
        print("  " + line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
