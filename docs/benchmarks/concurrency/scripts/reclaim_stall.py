#!/usr/bin/env python3
"""METHODOLOGY §29: the stall and the memory peak of reclamation at the arena cap (#1300 item 1).

Runs `crates/expanse/benches/reclaim_stall.rs` once per (cell, round), each in
its own process, with the cell order rotated per round, takes a load window
around every process, and writes one artifact whose `throughput` cells carry
their rounds verbatim. It then reads §29.3's predictions P1-P5 from the rounds
and records each verdict beside the cells.

`--wait-counters` adds one pass of the `occ-stats` build over the sync cells
after the timed rounds and records, per stall window, the existing wait
counters (`gate_wait_cycles`, `quiesce_drain_cycles`, `sample_spins`,
`sample_spin_cycles`) beside the CPU time the window's worker threads consumed
(#1479: do the waits take a material share of time?). A share is a counter in
TSC ticks over the summed per-thread CPU time (`CLOCK_THREAD_CPUTIME_ID`) in
the same ticks, never wall time times threads. The counters are TSC deltas and
include time a waiting thread was descheduled, which CPU time does not, so a
share can exceed the waits' true CPU cost. No verdict reads these figures.

`--free-gate --base-ref REF` evaluates METHODOLOGY §42.3's G2 and G3
(#1481): the `SyncExpanseBlobMap` overwrite cells at W = 1, R = 2, at
200,000 and 3,600,000 live, on two builds in one run, the head (this tree)
and the base (`REF`, normally the merge base on main). Both builds run the
head's harness file, so both record `insert_top_noncompact_ns`, the largest
latencies of the inserts that ran no compaction; the engine is each build's
own. One process per (build, cell, round), interleaved, the order rotated per
round. G2 and G3 are decided at 200,000 live; the 3,600,000 cell is reported
and decides nothing. `--quick` runs the same code on a 64 MiB arena with small
live sets for three rounds, as a smoke test whose output is not a measurement.

Usage:
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --out ARTIFACT.json
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --free-gate --base-ref REF --out ARTIFACT.json
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --free-gate --base-ref REF --quick --out SCRATCH.json
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --out ARTIFACT.json --wait-counters
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
import blob_reclaim_bounds as bounds  # noqa: E402
import check_bench_provenance  # noqa: E402
from bca_bootstrap import CI_METHOD_BCA, bca_bootstrap_ci_with_method  # noqa: E402
from bench_provenance import begin_cell, end_cell, new_provenance  # noqa: E402

ROUNDS = 5
FULL_SECONDS = 3
SMALL, LARGE = bounds.STALL_LIVE_SETS
# §29.2's cells.
CELLS: tuple[dict[str, Any], ...] = (
    *({"map": "plain", "mode": "overwrite", "live": live, "writers": 1, "readers": 0}
      for live in (SMALL, LARGE)),
    *({"map": "sync", "mode": "overwrite", "live": live, "writers": w, "readers": 2}
      for w in (1, 4, 12) for live in (SMALL, LARGE)),
    *({"map": "sync", "mode": "full", "live": None, "writers": w, "readers": 2, "seconds": FULL_SECONDS}
      for w in (4, 12)),
)
# §29.3's thresholds.
STALL_MATERIAL_NS = 100_000_000
LINEAR_BAND = (0.8, 1.25)
READ_WAIT_SHARE = 0.5
PEAK_BAND = (0.9, 1.1)
REFUSAL_P99_NS = 100_000
EXPECTED_COMPACTIONS = 2


class DriverError(Exception):
    pass


def cell_id(c: dict[str, Any]) -> str:
    live = "fill" if c["live"] is None else str(c["live"])
    return f"{c['map']}:{c['mode']}:L{live}:W{c['writers']}:R{c['readers']}"


def rotated(cells: tuple, r: int) -> list:
    k = r % len(cells)
    return list(cells[k:]) + list(cells[:k])


def build(features: tuple[str, ...] = (), tree: Path | None = None,
          target_dir: Path | None = None) -> Path:
    env = dict(os.environ)
    if target_dir is not None:
        env["CARGO_TARGET_DIR"] = str(target_dir)
    out = subprocess.run(
        ["cargo", "build", "--release", "-p", "expanse-trie", "--bench", "reclaim_stall",
         *(["--features", ",".join(features)] if features else []),
         "--message-format=json-render-diagnostics"],
        cwd=tree or REPO_ROOT, check=True, stdout=subprocess.PIPE, text=True, env=env,
    ).stdout
    exes = [json.loads(ln).get("executable") for ln in out.splitlines() if ln.startswith("{")]
    exes = [e for e in exes if e and Path(e).name.startswith("reclaim_stall")]
    if len(exes) != 1:
        raise DriverError(f"expected one reclaim_stall executable, found {exes}")
    return Path(exes[0])


def probe(exe: Path, counters: bool = False) -> dict[str, Any]:
    """Refuses a binary that lacks `--mode identify` or is not the build expected (§8.1).

    `occ_stats` is the one build property the driver distinguishes: a timed
    pass runs the default build, and a binary built with `occ-stats` times
    something else.
    """
    proc = subprocess.run([str(exe), "--map", "sync", "--mode", "identify"],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
    lines = [ln for ln in proc.stdout.splitlines() if ln.startswith("{")]
    if proc.returncode != 0 or len(lines) != 1:
        raise DriverError(
            f"the reclaim_stall harness at {exe} has no `--mode identify` (exit {proc.returncode}): "
            f"it predates the probe, so no cell can run on it.\n{proc.stderr.strip()[-400:]}")
    row = json.loads(lines[0])
    if row.get("occ_stats") is not counters:
        raise DriverError(f"the build at {exe} reports occ_stats={row.get('occ_stats')!r}, expected {counters!r}")
    return row


def child_pin(applied: str) -> str | None:
    """The CPU list to confine each child to, or None where the driver holds none.

    `applied` is what `bench_pin.apply` returned: a list, or `none`/`off` where
    no pin was taken. Platforms without `sched_setaffinity` cannot confine a child.
    """
    if applied in ("none", "off") or not hasattr(os, "sched_setaffinity"):
        return None
    return applied


def check_affinity(row: dict[str, Any], pin: str | None, where: str) -> None:
    """Refuses a row whose process ran on CPUs other than the pin the driver asked for (§8.1)."""
    if pin is None:
        return
    got = row.get("cpu_affinity")
    if got is None or bench_pin.expand(str(got)) != bench_pin.expand(pin):
        raise DriverError(f"{where}: the process ran on CPUs {got!r}, not the requested pin {pin}")


def run_one(exe: Path, c: dict[str, Any], r: int, pin: str | None = None,
            extra: tuple[str, ...] = (), seed: int | None = None) -> dict[str, Any]:
    if seed is None:
        seed = 0x1300_0000 + 97 * r + CELLS.index(c)
    argv = [str(exe), "--map", c["map"], "--mode", c["mode"], "--writers", str(c["writers"]),
            "--readers", str(c["readers"]), "--round", str(r), "--seed", str(seed), *extra]
    if c["live"] is not None:
        argv += ["--live", str(c["live"])]
    if c["mode"] == "full":
        argv += ["--seconds", str(c["seconds"])]
    preexec = (bench_pin.child_affinity(pin)
               if pin is not None and hasattr(os, "sched_setaffinity") else None)
    out = subprocess.run(argv, check=True, stdout=subprocess.PIPE, text=True,
                         preexec_fn=preexec).stdout.strip().splitlines()
    if len(out) != 1:
        raise DriverError(f"{cell_id(c)} round {r}: expected one JSON row, got {len(out)} lines")
    row = json.loads(out[0])
    for k in ("map", "mode", "writers", "readers"):
        if row[k] != c[k]:
            raise DriverError(f"{cell_id(c)} round {r}: the row reports {k}={row[k]!r}")
    check_affinity(row, pin, f"{cell_id(c)} round {r}")
    return row


# The wait counters that accumulate TSC ticks; `sample_spins` is a count and has no share.
WAIT_CYCLE_COUNTERS = ("gate_wait_cycles", "quiesce_drain_cycles", "sample_spin_cycles")
WAIT_COUNTERS = ("gate_wait_cycles", "gate_blocked_entries", "quiesce_drain_cycles", "quiesce_calls",
                 "sample_spins", "sample_spin_cycles")
# Cells whose window the counters pass records: the sync cells, the only ones with waits.
WAIT_CELLS = tuple(c for c in CELLS if c["map"] == "sync")


def wait_report(row: dict[str, Any]) -> dict[str, Any]:
    """One stall window's wait counters and their shares of the window's thread CPU time.

    The denominator is `thread_cpu_cycles`: the sum over the window's worker
    threads of their measured CPU time, in TSC ticks. It is cross-checked
    against `thread_cpu_ns` and `tsc_hz`, so a figure derived from wall time
    times threads (which overstates CPU time whenever the threads outnumber
    the CPUs they may use) is refused rather than divided by (§8.1).
    """
    c = row.get("counters")
    if not isinstance(c, dict):
        raise DriverError("the row has no `counters`: the binary was not built with occ-stats")
    cpu_ns, cpu_cyc, hz = c.get("thread_cpu_ns"), c.get("thread_cpu_cycles"), c.get("tsc_hz")
    if not cpu_ns or not cpu_cyc or not hz:
        raise DriverError(f"the row's thread CPU time is missing or zero (ns={cpu_ns!r}, ticks={cpu_cyc!r}, "
                          f"tsc_hz={hz!r}): this platform has no per-thread CPU clock, so no share is defined")
    if abs(cpu_cyc - cpu_ns * hz / 1e9) > 1.0:
        raise DriverError(f"thread_cpu_cycles={cpu_cyc} is not thread_cpu_ns={cpu_ns} at tsc_hz={hz}")
    return {
        "denominator": "thread_cpu_cycles (sum of per-thread CLOCK_THREAD_CPUTIME_ID, in TSC ticks)",
        "thread_cpu_cycles": cpu_cyc,
        "thread_cpu_ns": cpu_ns,
        "tsc_hz": hz,
        "wall_s": row["elapsed_s"],
        "counters": {k: c[k] for k in WAIT_COUNTERS},
        "share_of_thread_cpu": {k: c[k] / cpu_cyc for k in WAIT_CYCLE_COUNTERS},
        "note": "the counters are TSC deltas and include time a waiting thread was descheduled; "
                "thread CPU time does not, so a share can exceed the waits' CPU cost",
    }


def stall_ns(row: dict[str, Any]) -> int:
    """§29.3: a window's stall is its largest insert latency."""
    return int(row["insert"]["max_ns"])


def interval(values: list[float]) -> dict[str, Any]:
    point, lo, hi, method = bca_bootstrap_ci_with_method(values)
    return {"mean": point, "ci_lower": lo, "ci_upper": hi, "ci_method": method,
            "median": statistics.median(values), "n": len(values)}


def peak_prediction(row: dict[str, Any]) -> int:
    live = row["live"]
    return row["index_mem_used"] + bounds.compaction_peak_bytes(
        bounds.MAX_ARENA_CAPACITY, live, bounds.HARNESS_LEN, bounds.DEFAULT_CHUNK_SIZE, live)


def by_round(cell: dict[str, Any]) -> dict[int, dict[str, Any]]:
    return {row["round"]: row for row in cell["rounds_raw"]}


def find(cells: list, **want) -> dict[str, Any]:
    hits = [c for c in cells if all(c[k] == v for k, v in want.items())]
    if len(hits) != 1:
        raise DriverError(f"expected one cell matching {want}, found {len(hits)}")
    return hits[0]


def evaluate(cells: list[dict[str, Any]]) -> dict[str, Any]:
    out: dict[str, Any] = {"P1": [], "P2": [], "P3": [], "P4": [], "P5": [], "checks": []}
    lo_ns, hi_ns = min(bounds.s27_ns_per_record()), max(bounds.s27_ns_per_record())
    for c in cells:
        if c["mode"] != "overwrite":
            continue
        rows = c["rounds_raw"]
        bad = [row["round"] for row in rows if row["compactions"] != EXPECTED_COMPACTIONS
               or row["inserts_refused"] != 0]
        out["checks"].append({"cell": c["cell"], "rounds_without_two_compactions_or_with_refusals": bad})
        s = [float(stall_ns(row)) for row in rows]
        iv = interval(s)
        out["P1"].append({"cell": c["cell"], "stall_ns": iv,
                          "predicted_linear_ns": [bounds.predicted_stall_ns(c["live"], lo_ns),
                                                  bounds.predicted_stall_ns(c["live"], hi_ns)],
                          "material": iv["median"] >= STALL_MATERIAL_NS if c["live"] == LARGE else None})
        if c["map"] == "sync":
            shares = [row["read"]["max_ns"] / stall_ns(row) for row in rows]
            verdict = ("HOLDS" if all(x >= READ_WAIT_SHARE for x in shares)
                       else "FAILS" if all(x < READ_WAIT_SHARE for x in shares) else "MIXED")
            out["P3"].append({"cell": c["cell"], "read_max_over_stall": shares, "verdict": verdict})
        ratios = [row["heap_peak"] / peak_prediction(row) for row in rows]
        entry = {"cell": c["cell"], "peak_over_prediction": ratios}
        if c["map"] == "plain":
            entry["verdict"] = ("HOLDS" if all(PEAK_BAND[0] <= x <= PEAK_BAND[1] for x in ratios)
                                else "FAILS")
        out["P4"].append(entry)
    for mp, w in (("plain", 1), ("sync", 1), ("sync", 4), ("sync", 12)):
        small = by_round(find(cells, map=mp, mode="overwrite", live=SMALL, writers=w))
        large = by_round(find(cells, map=mp, mode="overwrite", live=LARGE, writers=w))
        paired = sorted(set(small) & set(large))
        ratios = [(stall_ns(large[r]) / LARGE) / (stall_ns(small[r]) / SMALL) for r in paired]
        iv = interval(ratios)
        verdict = ("LINEAR" if LINEAR_BAND[0] <= iv["ci_lower"] and iv["ci_upper"] <= LINEAR_BAND[1]
                   else "SUPERLINEAR" if iv["ci_lower"] > LINEAR_BAND[1] else "INCONCLUSIVE")
        out["P2"].append({"map": mp, "writers": w, "per_record_ratio": iv, "verdict": verdict})
    for c in cells:
        if c["mode"] != "full":
            continue
        p99s = [row["insert"]["p99_ns"] for row in c["rounds_raw"]]
        all_refused = all(row["inserts_ok"] == 0 for row in c["rounds_raw"])
        out["P5"].append({"cell": c["cell"], "refused_p99_ns": p99s,
                          "refused_max_ns": [row["insert"]["max_ns"] for row in c["rounds_raw"]],
                          "every_insert_refused": all_refused,
                          "verdict": "HOLDS" if all(x <= REFUSAL_P99_NS for x in p99s) else "FAILS"})
    out["stall_material"] = any(e["material"] for e in out["P1"] if e["material"] is not None)
    return out


def measure(rounds: int, out_path: Path, wait_counters: bool = False) -> dict[str, Any]:
    applied = bench_pin.apply("reclaim_stall.py")
    prov = new_provenance("concurrency", 1300, "§29 reclaim stall (no ratio gate)", REPO_ROOT,
                          cell_isolation="process", rounds=rounds, core_pin_applied=applied)
    exe = build()
    probe(exe)
    pin = child_pin(applied)
    by_cell: dict[str, dict[str, Any]] = {}
    for r in range(rounds):
        for c in rotated(CELLS, r):
            cid = cell_id(c)
            start = begin_cell(prov, f"cell:{cid}:r{r}")
            row = run_one(exe, c, r, pin)
            row["load"] = end_cell(start)
            cell = by_cell.setdefault(cid, {"cell": cid, **{k: c[k] for k in ("map", "mode", "live", "writers", "readers")},
                                            "rounds_raw": []})
            cell["rounds_raw"].append(row)
    cells = [by_cell[cell_id(c)] for c in CELLS]
    for cell in cells:
        loads = [row["load"] for row in cell["rounds_raw"]]
        cell["load"] = max(loads, key=lambda ld: ld.get("foreign_busy_cpus") or 0.0)
    art = {"provenance": prov, "throughput": cells}
    art["predictions"] = evaluate(cells)
    if wait_counters:
        # A counters build times nothing the predictions read; its rows are kept apart from `throughput`.
        cexe = build(("occ-stats",))
        probe(cexe, counters=True)
        windows = []
        for c in WAIT_CELLS:
            cid = cell_id(c)
            start = begin_cell(prov, f"wait_counters:{cid}")
            row = run_one(cexe, c, 0, pin)
            load = end_cell(start)
            windows.append({"cell": cid, "round": 0, **wait_report(row), "load": load})
        art["wait_counters"] = windows
    rel = check_bench_provenance.rel_path(out_path.resolve())
    findings = check_bench_provenance.findings_for(rel, json.loads(json.dumps(art)))
    if findings:
        raise DriverError("the artifact fails the provenance gate: " + "; ".join(findings))
    out_path.write_text(json.dumps(art, indent=2) + "\n")
    return art


def _synthetic(stall_small: float, stall_large: float, read_share: float, peak_ratio: float,
               refusal_p99: int, rounds: int = 5) -> list[dict[str, Any]]:
    cells = []
    for c in CELLS:
        rows = []
        for r in range(rounds):
            jitter = 1 + 0.01 * (r % 3)
            live = c["live"]
            row: dict[str, Any] = {"round": r, "map": c["map"], "mode": c["mode"], "live": live or 7_456_256,
                                   "writers": c["writers"], "readers": c["readers"], "compactions": 2,
                                   "inserts_ok": 0 if c["mode"] == "full" else 10, "inserts_refused": 0,
                                   "index_mem_used": 1_713_152}
            stall = (stall_small if live == SMALL else stall_large) * jitter
            if c["mode"] == "full":
                row["inserts_refused"] = 10
                row["insert"] = {"p99_ns": refusal_p99, "max_ns": 50_000_000}
            else:
                row["insert"] = {"max_ns": int(stall), "p99_ns": 1000}
                row["read"] = {"max_ns": int(stall * read_share)}
                row["heap_peak"] = 0
                row["heap_peak"] = int(peak_prediction(row) * peak_ratio)
            rows.append(row)
        cells.append({"cell": cell_id(c), **{k: c[k] for k in ("map", "mode", "live", "writers", "readers")},
                      "rounds_raw": rows})
    return cells


def _refused(fn, *args) -> str:
    try:
        fn(*args)
    except DriverError as e:
        return str(e)
    raise AssertionError(f"{fn.__name__}{args} was not refused")


def self_test_identity_and_pin() -> None:
    import tempfile
    # The probe: a binary reporting the wrong build, or without the mode, is refused.
    with tempfile.TemporaryDirectory() as d:
        def fake(name: str, body: str) -> Path:
            p = Path(d) / name
            p.write_text("#!/bin/sh\n" + body + "\n")
            p.chmod(0o755)
            return p
        on = fake("on", """echo '{"mode":"identify","occ_stats":true,"cpu_affinity":"0"}'""")
        off = fake("off", """echo '{"mode":"identify","occ_stats":false,"cpu_affinity":"0"}'""")
        old = fake("old", "echo 'missing --mode' >&2; exit 101")
        assert probe(off)["occ_stats"] is False and probe(on, counters=True)["occ_stats"] is True
        assert "occ_stats" in _refused(probe, on)
        assert "occ_stats" in _refused(probe, off, True)
        assert "no `--mode identify`" in _refused(probe, old)
        # The call site: run_one itself refuses a row whose CPUs differ from the pin it was given.
        c = CELLS[0]
        row = {"map": c["map"], "mode": c["mode"], "writers": c["writers"], "readers": c["readers"],
               "cpu_affinity": "999"}
        liar = fake("liar", "echo '" + json.dumps(row) + "'")
        mine = str(min(os.sched_getaffinity(0))) if hasattr(os, "sched_getaffinity") else "0"
        assert "ran on CPUs '999'" in _refused(run_one, liar, c, 0, mine)
        assert run_one(liar, c, 0, None)["cpu_affinity"] == "999"
    # The pin: a row whose CPUs differ from the requested list is refused; an equal list in another spelling is not.
    check_affinity({"cpu_affinity": "0-3"}, "0,1,2,3", "x")
    check_affinity({"cpu_affinity": "5"}, None, "x")
    assert "ran on CPUs '0-7'" in _refused(check_affinity, {"cpu_affinity": "0-7"}, "0-3", "x")
    assert "ran on CPUs None" in _refused(check_affinity, {}, "0-3", "x")
    assert child_pin("none") is None and child_pin("off") is None
    if hasattr(os, "sched_setaffinity"):
        assert child_pin("0-3") == "0-3"


def _counters_row(cpu_ns: int | None, tsc_hz: int = 2_000_000_000, wall_s: float = 3.0,
                  threads: int = 14, **over: int) -> dict[str, Any]:
    counters = {"sample_spins": 1_000, "sample_spin_cycles": 60_000_000, "gate_blocked_entries": 5,
                "gate_wait_cycles": 300_000_000, "quiesce_calls": 2, "quiesce_drain_cycles": 120_000_000,
                "tsc_hz": tsc_hz, "thread_cpu_ns": cpu_ns,
                "thread_cpu_cycles": None if cpu_ns is None else cpu_ns * tsc_hz // 1_000_000_000}
    counters.update(over)
    return {"elapsed_s": wall_s, "threads": threads, "counters": counters}


def self_test_wait_counters() -> None:
    # 14 threads over 3.0 s of wall time, but the CPUs they may use gave them 6.0 s of CPU in all.
    # The wall-times-threads denominator would be 3.0 * 2e9 * 14 = 8.4e10 ticks; the measured one is 1.2e10.
    row = _counters_row(6_000_000_000)
    rep_ = wait_report(row)
    wall_x_threads = row["elapsed_s"] * row["counters"]["tsc_hz"] * row["threads"]
    assert rep_["thread_cpu_cycles"] == 12_000_000_000 != wall_x_threads
    assert abs(rep_["share_of_thread_cpu"]["gate_wait_cycles"] - 300e6 / 12e9) < 1e-12
    assert abs(rep_["share_of_thread_cpu"]["gate_wait_cycles"] - 300e6 / wall_x_threads) > 0.01
    assert rep_["wall_s"] == 3.0 and set(rep_["counters"]) == set(WAIT_COUNTERS)
    # A denominator that is not the measured CPU time (here wall x threads written into the cycles field) is refused.
    assert "is not thread_cpu_ns" in _refused(
        wait_report, _counters_row(6_000_000_000, thread_cpu_cycles=int(3.0 * 2e9 * 14)))
    # No per-thread clock, a zero reading, or a build without occ-stats: refused, never divided by.
    assert "missing or zero" in _refused(wait_report, _counters_row(None))
    assert "missing or zero" in _refused(wait_report, _counters_row(0))
    assert "no `counters`" in _refused(wait_report, {"elapsed_s": 1.0})
    # The harness derives the denominator from the per-thread clock and not from wall time (decision point).
    src = (REPO_ROOT / "crates/expanse/benches/reclaim_stall.rs").read_text()
    code = "\n".join(ln for ln in src.splitlines() if not ln.lstrip().startswith("//"))
    body = code[code.index("fn window_counters"):]
    body = body[:body.index("\n}\n")]
    assert "thread_cpu_ns" in body and "elapsed" not in body and "threads as f64" not in body, body
    assert code.count("CLOCK_THREAD_CPUTIME_ID") == 1


# --- METHODOLOGY §42.3: G2 and G3 of the bounded chunk free (#1481) ----------

# The cells: §29.2's SyncExpanseBlobMap overwrite cells at W = 1, R = 2.
FREE_GATE_CELLS: tuple[dict[str, Any], ...] = tuple(
    {"map": "sync", "mode": "overwrite", "live": live, "writers": 1, "readers": 2}
    for live in (SMALL, LARGE))
# The cell that decides; the other is reported and decides nothing.
FREE_GATE_DECIDING_LIVE = SMALL
G2_FACTOR = 0.25
G3_FACTOR = 1.02
BUILDS = ("base", "head")
# `--quick`: a smoke run of the same code, not a measurement.
QUICK_CAP_MIB = 64
QUICK_LIVE = (20_000, 60_000)
QUICK_ROUNDS = 3  # the fewest the BCa estimator accepts
HARNESS_REL = "crates/expanse/benches/reclaim_stall.rs"


def largest_noncompact_ns(row: dict[str, Any], where: str) -> int:
    """G2's per-round statistic: the largest latency among inserts that ran no compaction."""
    top = row.get("insert_top_noncompact_ns")
    if not isinstance(top, list) or not top:
        raise DriverError(f"{where}: the row has no `insert_top_noncompact_ns` "
                          "(a harness without `--noncompact-top`, or a window with no such insert)")
    return int(max(top))


def gate_checks(rows: list[dict[str, Any]]) -> list[int]:
    """Rounds that are not §29's window: two compactions and no refused insert."""
    return [row["round"] for row in rows
            if row["compactions"] != EXPECTED_COMPACTIONS or row["inserts_refused"] != 0]


def evaluate_free_gate(cells: list[dict[str, Any]], deciding_live: int = FREE_GATE_DECIDING_LIVE) -> dict[str, Any]:
    """G2 and G3 as METHODOLOGY §42.3 states them, per live set.

    G2: `H` and `B` are the head's and the base's largest non-compaction insert
    latency, the median over rounds with the BCa 95% interval over rounds;
    PASS iff the upper bound of `H` <= 0.25 x the lower bound of `B`.
    G3: `heap_peak`, the same estimator; PASS iff the upper bound of the head's
    <= 1.02 x the lower bound of the base's. Decided at `deciding_live`; any
    other live set is reported with verdict None. A cell whose rounds are not
    §29's two-compaction window is VOID, not read.
    """
    out: dict[str, Any] = {"G2": [], "G3": [], "deciding_live": deciding_live}
    lives = sorted({c["live"] for c in cells})
    for live in lives:
        per = {b: find(cells, live=live, build=b) for b in BUILDS}
        bad = {b: gate_checks(per[b]["rounds_raw"]) for b in BUILDS}
        void = any(bad.values())
        h = interval([float(largest_noncompact_ns(r, f"head L{live} round {r['round']}"))
                      for r in per["head"]["rounds_raw"]])
        b = interval([float(largest_noncompact_ns(r, f"base L{live} round {r['round']}"))
                      for r in per["base"]["rounds_raw"]])
        hp = interval([float(r["heap_peak"]) for r in per["head"]["rounds_raw"]])
        bp = interval([float(r["heap_peak"]) for r in per["base"]["rounds_raw"]])
        g2_pass = h["ci_upper"] <= G2_FACTOR * b["ci_lower"]
        g3_pass = hp["ci_upper"] <= G3_FACTOR * bp["ci_lower"]
        deciding = live == deciding_live

        def verdict(ok: bool) -> str | None:
            if not deciding:
                return None
            return "VOID" if void else ("PASS" if ok else "FAIL")
        common = {"live": live, "deciding": deciding, "rounds_not_two_compactions_or_refused": bad}
        out["G2"].append({**common, "H_ns": h, "B_ns": b, "bound_ns": G2_FACTOR * b["ci_lower"],
                          "rule": "PASS iff H ci_upper <= 0.25 x B ci_lower", "verdict": verdict(g2_pass)})
        out["G3"].append({**common, "head_heap_peak": hp, "base_heap_peak": bp,
                          "bound_bytes": G3_FACTOR * bp["ci_lower"],
                          "rule": "PASS iff head ci_upper <= 1.02 x base ci_lower", "verdict": verdict(g3_pass)})
    return out


def git_out(*args: str, cwd: Path = REPO_ROOT) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=True, stdout=subprocess.PIPE, text=True).stdout.strip()


def base_tree(base_ref: str, into: Path) -> str:
    """A detached worktree of `base_ref` at `into`, carrying the head's harness file.

    The harness is the instrument and must be the same in both builds; the
    engine under it is the base's. Returns the base commit."""
    sha = git_out("rev-parse", "--verify", f"{base_ref}^{{commit}}")
    head = git_out("rev-parse", "HEAD")
    if sha == head:
        raise DriverError(f"the base {base_ref} is the head commit {head}: nothing to compare")
    subprocess.run(["git", "worktree", "add", "--detach", str(into), sha], cwd=REPO_ROOT, check=True,
                   stdout=subprocess.DEVNULL)
    shutil.copy2(REPO_ROOT / HARNESS_REL, into / HARNESS_REL)
    return sha


def measure_free_gate(base_ref: str, rounds: int, out_path: Path, quick: bool = False) -> dict[str, Any]:
    if not base_ref:
        raise DriverError("--free-gate needs --base-ref: the comparison is the point of the run")
    out_abs = out_path.resolve()
    if quick and (REPO_ROOT / "docs").resolve() in out_abs.parents:
        raise DriverError(f"--quick writes scratch output only, never under docs/ ({out_path})")
    applied = bench_pin.apply("reclaim_stall.py --free-gate")
    cells_def = FREE_GATE_CELLS
    extra: tuple[str, ...] = ("--noncompact-top", "1")
    deciding = FREE_GATE_DECIDING_LIVE
    if quick:
        rounds = QUICK_ROUNDS
        cells_def = tuple({**c, "live": live} for c, live in zip(FREE_GATE_CELLS, QUICK_LIVE))
        extra += ("--cap-mib", str(QUICK_CAP_MIB))
        deciding = QUICK_LIVE[0]
    prov = new_provenance("concurrency", 1481, "§42.3 G2/G3 bounded chunk free (base vs head)", REPO_ROOT,
                          cell_isolation="process", rounds=rounds, core_pin_applied=applied,
                          quick=quick)
    tmp = Path(tempfile.mkdtemp(prefix="reclaim-free-base-"))
    tree = tmp / "tree"
    try:
        prov["base_commit"] = base_tree(base_ref, tree)
        prov["head_commit"] = git_out("rev-parse", "HEAD")
        exes = {"head": build(), "base": build(tree=tree, target_dir=tmp / "target")}
        for exe in exes.values():
            probe(exe)
        pin = child_pin(applied)
        by_cell: dict[tuple[str, int], dict[str, Any]] = {}
        order = [(b, c) for c in cells_def for b in BUILDS]
        for r in range(rounds):
            for b, c in rotated(tuple(order), r):
                key = (b, c["live"])
                start = begin_cell(prov, f"cell:{b}:{cell_id(c)}:r{r}")
                # The same seed for both builds within a round and cell.
                seed = 0x1481_0000 + 97 * r + cells_def.index(c)
                row = run_one(exes[b], c, r, pin, extra, seed)
                row["load"] = end_cell(start)
                row["build"] = b
                largest_noncompact_ns(row, f"{b} {cell_id(c)} round {r}")
                cell = by_cell.setdefault(key, {"cell": f"{b}:{cell_id(c)}", "build": b,
                                                **{k: c[k] for k in ("map", "mode", "live", "writers", "readers")},
                                                "rounds_raw": []})
                cell["rounds_raw"].append(row)
    finally:
        subprocess.run(["git", "worktree", "remove", "--force", str(tree)], cwd=REPO_ROOT,
                       check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        shutil.rmtree(tmp, ignore_errors=True)
    cells = [by_cell[(b, c["live"])] for c in cells_def for b in BUILDS]
    for cell in cells:
        loads = [row["load"] for row in cell["rounds_raw"]]
        cell["load"] = max(loads, key=lambda ld: ld.get("foreign_busy_cpus") or 0.0)
    art = {"provenance": prov, "throughput": cells,
           "gates": evaluate_free_gate(cells, deciding_live=deciding)}
    if quick:
        art["note"] = "quick smoke run on a 64 MiB arena: not a measurement, decides nothing"
    else:
        rel = check_bench_provenance.rel_path(out_abs)
        findings = check_bench_provenance.findings_for(rel, json.loads(json.dumps(art)))
        if findings:
            raise DriverError("the artifact fails the provenance gate: " + "; ".join(findings))
    out_path.write_text(json.dumps(art, indent=2) + "\n")
    return art


def free_gate_lines(gates: dict[str, Any]) -> list[str]:
    """The run's printed verdict lines."""
    lines = []
    for g, (hk, bk, unit) in (("G2", ("H_ns", "B_ns", "ns")), ("G3", ("head_heap_peak", "base_heap_peak", "B"))):
        for e in gates[g]:
            h, b = e[hk], e[bk]
            role = "decides" if e["deciding"] else "reported, decides nothing"
            lines.append(
                f"{g} L={e['live']:,} ({role}): head median {h['median']:,.0f} {unit} "
                f"[{h['ci_lower']:,.0f}, {h['ci_upper']:,.0f}], base median {b['median']:,.0f} {unit} "
                f"[{b['ci_lower']:,.0f}, {b['ci_upper']:,.0f}] ({h['ci_method']}); "
                f"{e['rule']}: bound {e.get('bound_ns', e.get('bound_bytes')):,.0f} {unit}; "
                f"verdict {e['verdict'] or 'none (reported)'}")
    return lines


def _gate_cells(h: float, b: float, hp: float, bp: float, large_head_scale: float = 1.0,
                rounds: int = 5, compactions: int = 2) -> list[dict[str, Any]]:
    cells = []
    for c in FREE_GATE_CELLS:
        for build_ in BUILDS:
            rows = []
            for r in range(rounds):
                j = 1 + 0.01 * (r % 3)
                lat = (h if build_ == "head" else b) * j
                if build_ == "head" and c["live"] == LARGE:
                    lat *= large_head_scale
                peak = (hp if build_ == "head" else bp) * j
                rows.append({"round": r, "compactions": compactions, "inserts_refused": 0,
                             "insert_top_noncompact_ns": [int(lat), int(lat * 0.9)],
                             "insert_top_ns": [int(lat * 10)], "heap_peak": int(peak)})
            cells.append({"cell": f"{build_}:{cell_id(c)}", "build": build_,
                          **{k: c[k] for k in ("map", "mode", "live", "writers", "readers")}, "rounds_raw": rows})
    return cells


def self_test_free_gate() -> None:
    def verdicts(cells):
        v = evaluate_free_gate(cells)
        return ({e["live"]: e["verdict"] for e in v["G2"]}, {e["live"]: e["verdict"] for e in v["G3"]})
    # G2 PASS, G3 PASS: the head's stall far under 0.25 x the base's, the peak unchanged.
    g2, g3 = verdicts(_gate_cells(2e6, 30e6, 1.0e9, 1.0e9))
    assert g2 == {SMALL: "PASS", LARGE: None} and g3 == {SMALL: "PASS", LARGE: None}, (g2, g3)
    # G2 negative control: the head at 0.3 x the base fails; G3 unaffected.
    g2, g3 = verdicts(_gate_cells(9e6, 30e6, 1.0e9, 1.0e9))
    assert g2[SMALL] == "FAIL" and g3[SMALL] == "PASS", (g2, g3)
    # G2 reads the interval, not the medians: medians at 0.24 x pass only if the upper bound does.
    e = evaluate_free_gate(_gate_cells(7.45e6, 30e6, 1.0e9, 1.0e9))["G2"][0]
    assert e["H_ns"]["median"] < 0.25 * e["B_ns"]["median"], e
    assert e["H_ns"]["ci_upper"] > 0.25 * e["B_ns"]["ci_lower"], e
    assert e["verdict"] == "FAIL", e
    e = evaluate_free_gate(_gate_cells(1.0e9, 1.0e9, 1.015e9, 1.0e9))["G3"][0]
    assert e["head_heap_peak"]["median"] < 1.02 * e["base_heap_peak"]["median"], e
    assert e["head_heap_peak"]["ci_upper"] > 1.02 * e["base_heap_peak"]["ci_lower"], e
    assert e["verdict"] == "FAIL", e
    # G2 reads the non-compaction list, not `insert_top_ns` (ten times larger here).
    assert verdicts(_gate_cells(2e6, 30e6, 1.0e9, 1.0e9))[0][SMALL] == "PASS"
    # G3 negative control: a head peak 3% above the base fails; G2 unaffected.
    g2, g3 = verdicts(_gate_cells(2e6, 30e6, 1.03e9, 1.0e9))
    assert g3[SMALL] == "FAIL" and g2[SMALL] == "PASS", (g2, g3)
    # The 3,600,000 cell decides nothing: a failing large cell leaves the verdicts to the small one.
    cells = _gate_cells(2e6, 30e6, 1.0e9, 1.0e9, large_head_scale=10.0)
    assert evaluate_free_gate(cells)["G2"][1]["H_ns"]["median"] > 0.25 * 30e6
    g2, g3 = verdicts(cells)
    assert g2 == {SMALL: "PASS", LARGE: None}, g2
    # A window without its two compactions voids the deciding cell.
    g2, g3 = verdicts(_gate_cells(2e6, 30e6, 1.0e9, 1.0e9, compactions=1))
    assert g2[SMALL] == "VOID" and g3[SMALL] == "VOID"
    # A row without the non-compaction list is refused, never read as zero.
    cells = _gate_cells(2e6, 30e6, 1.0e9, 1.0e9)
    del cells[0]["rounds_raw"][0]["insert_top_noncompact_ns"]
    assert "insert_top_noncompact_ns" in _refused(evaluate_free_gate, cells)
    cells = _gate_cells(2e6, 30e6, 1.0e9, 1.0e9)
    cells[1]["rounds_raw"][0]["insert_top_noncompact_ns"] = []
    assert "insert_top_noncompact_ns" in _refused(evaluate_free_gate, cells)
    # The printed lines carry both verdicts and the rule.
    lines = free_gate_lines(evaluate_free_gate(_gate_cells(2e6, 30e6, 1.0e9, 1.0e9)))
    assert len(lines) == 4 and sum("verdict PASS" in ln for ln in lines) == 2
    assert sum("decides nothing" in ln for ln in lines) == 2
    # No base: refused before anything runs.
    assert "--base-ref" in _refused(measure_free_gate, "", 1, Path("x.json"))
    # The harness records the list only behind the flag, from a generation read outside the timed call.
    src = (REPO_ROOT / HARNESS_REL).read_text()
    code = "\n".join(ln for ln in src.splitlines() if not ln.lstrip().startswith("//"))
    i = code.index("} else if noncompact {")
    body = code[i:code.index("} else {", i)]
    assert body.index("timed_insert(") < body.index("let after = m.with_locked") < body.index("if after == g")
    assert "push_top(&mut ops.top_noncompact, ns)" in body
    assert code.count("top_noncompact, ns)") == 1


def self_test() -> int:
    assert len(CELLS) == 10 and len({cell_id(c) for c in CELLS}) == 10
    assert rotated(CELLS, 3)[0] is CELLS[3] and sorted(map(cell_id, rotated(CELLS, 7))) == sorted(map(cell_id, CELLS))
    # Linear scaling at the §27 cost: P2 LINEAR, and the 3,600,000 stall it
    # predicts (about 200 ms) is material by §29.4's 100 ms rule.
    v = evaluate(_synthetic(11.2e6, 11.2e6 * 18, 1.0, 1.0, 2_000))
    assert all(e["verdict"] == "LINEAR" for e in v["P2"]), v["P2"]
    assert v["stall_material"] is True
    # A linear stall under 100 ms at the large set is not material.
    assert evaluate(_synthetic(4e6, 4e6 * 18, 1.0, 1.0, 2_000))["stall_material"] is False
    assert all(e["verdict"] == "HOLDS" for e in v["P3"] + v["P5"]) and all(
        e.get("verdict", "HOLDS") == "HOLDS" for e in v["P4"])
    for e in v["P1"] + v["P2"]:
        iv = e.get("stall_ns") or e["per_record_ratio"]
        assert iv["ci_method"] and iv["ci_lower"] <= iv["mean"] <= iv["ci_upper"]
    # Three times the linear cost per record at the large set: SUPERLINEAR and material.
    v = evaluate(_synthetic(11.2e6, 11.2e6 * 18 * 3, 1.0, 1.0, 2_000))
    assert all(e["verdict"] == "SUPERLINEAR" for e in v["P2"]) and v["stall_material"] is True
    # Readers that do not wait, a peak off the formula, a slow refusal: each fails.
    v = evaluate(_synthetic(11.2e6, 11.2e6 * 18, 0.1, 1.3, 500_000))
    assert all(e["verdict"] == "FAILS" for e in v["P3"]), v["P3"]
    assert all(e["verdict"] == "FAILS" for e in v["P4"] if "verdict" in e)
    assert all(e["verdict"] == "FAILS" for e in v["P5"])
    # A window without its two compactions is flagged.
    cells = _synthetic(11.2e6, 11.2e6 * 18, 1.0, 1.0, 2_000)
    cells[0]["rounds_raw"][2]["compactions"] = 1
    assert evaluate(cells)["checks"][0]["rounds_without_two_compactions_or_with_refusals"] == [2]
    assert CI_METHOD_BCA
    self_test_identity_and_pin()
    self_test_wait_counters()
    self_test_free_gate()
    print("reclaim_stall.py self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--rounds", type=int, default=ROUNDS)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--wait-counters", action="store_true",
                    help="after the timed rounds, record each sync stall window's wait counters from an occ-stats build")
    ap.add_argument("--free-gate", action="store_true",
                    help="METHODOLOGY §42.3's G2 and G3: base and head builds in one run")
    ap.add_argument("--base-ref", default="", help="the base for --free-gate (the merge base on main)")
    ap.add_argument("--quick", action="store_true",
                    help="with --free-gate: a 64 MiB smoke run, scratch output only, decides nothing")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.out:
        ap.error("--out is required")
    if os.environ.get("EXPANSE_RECLAIM_STALL_ROUNDS"):
        args.rounds = int(os.environ["EXPANSE_RECLAIM_STALL_ROUNDS"])
    if args.quick and not args.free_gate:
        ap.error("--quick applies to --free-gate only")
    if args.free_gate:
        art = measure_free_gate(args.base_ref, args.rounds, args.out, args.quick)
        for line in free_gate_lines(art["gates"]):
            print(line)
        return 0
    art = measure(args.rounds, args.out, args.wait_counters)
    print(json.dumps(art["predictions"], indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
