#!/usr/bin/env python3
"""METHODOLOGY §29: the stall and the memory peak of reclamation at the arena cap (#1300 item 1).

Runs `crates/expanse/benches/reclaim_stall.rs` once per (cell, round), each in
its own process, with the cell order rotated per round, takes a load window
around every process, and writes one artifact whose `throughput` cells carry
their rounds verbatim. It then reads §29.3's predictions P1-P5 from the rounds
and records each verdict beside the cells.

`--spin-yield` is METHODOLOGY §37 (#1479), a mode of its own with its own
artifact: cells O, C, O-large and S of the same harness and writer_scaling's
count cell H, each in the default build and in `ablation-gate-spin-then-yield`,
each cell's process pinned to its registered CPU list, with §37.5's paired
statistics and §37.6's run verdict. `--spin-yield-pair` reads one pair's
dispatches and prints the pair verdict, honouring the host guard's record.

Usage:
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --out ARTIFACT.json
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --spin-yield [--out ARTIFACT.json]
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --spin-yield --quick --out scratch/sy.json
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --spin-yield-pair RUN1.json RUN2.json
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
import sys
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


def build() -> Path:
    out = subprocess.run(
        ["cargo", "build", "--release", "-p", "expanse-trie", "--bench", "reclaim_stall",
         "--message-format=json-render-diagnostics"],
        cwd=REPO_ROOT, check=True, stdout=subprocess.PIPE, text=True,
    ).stdout
    exes = [json.loads(ln).get("executable") for ln in out.splitlines() if ln.startswith("{")]
    exes = [e for e in exes if e and Path(e).name.startswith("reclaim_stall")]
    if len(exes) != 1:
        raise DriverError(f"expected one reclaim_stall executable, found {exes}")
    return Path(exes[0])


def run_one(exe: Path, c: dict[str, Any], r: int) -> dict[str, Any]:
    argv = [str(exe), "--map", c["map"], "--mode", c["mode"], "--writers", str(c["writers"]),
            "--readers", str(c["readers"]), "--round", str(r), "--seed", str(0x1300_0000 + 97 * r + CELLS.index(c))]
    if c["live"] is not None:
        argv += ["--live", str(c["live"])]
    if c["mode"] == "full":
        argv += ["--seconds", str(c["seconds"])]
    out = subprocess.run(argv, check=True, stdout=subprocess.PIPE, text=True).stdout.strip().splitlines()
    if len(out) != 1:
        raise DriverError(f"{cell_id(c)} round {r}: expected one JSON row, got {len(out)} lines")
    row = json.loads(out[0])
    for k in ("map", "mode", "writers", "readers"):
        if row[k] != c[k]:
            raise DriverError(f"{cell_id(c)} round {r}: the row reports {k}={row[k]!r}")
    return row


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


def measure(rounds: int, out_path: Path) -> dict[str, Any]:
    applied = bench_pin.apply("reclaim_stall.py")
    prov = new_provenance("concurrency", 1300, "§29 reclaim stall (no ratio gate)", REPO_ROOT,
                          cell_isolation="process", rounds=rounds, core_pin_applied=applied)
    exe = build()
    by_cell: dict[str, dict[str, Any]] = {}
    for r in range(rounds):
        for c in rotated(CELLS, r):
            cid = cell_id(c)
            start = begin_cell(prov, f"cell:{cid}:r{r}")
            row = run_one(exe, c, r)
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
    print("reclaim_stall.py self-test PASSED")
    return 0


# ---------------------------------------------------------------------------
# METHODOLOGY §37 (#1479): do the writer-side wait loops gain from yielding?
# ---------------------------------------------------------------------------

SY_FEATURE = "ablation-gate-spin-then-yield"
SY_DEFAULT = "default"
# §37.3: the build list, default first; a row is labelled by its build.
SY_BUILDS = (SY_DEFAULT, SY_FEATURE)
SY_ROUNDS = 8
SY_QUICK_ROUNDS = 3
SY_PIN_CORES = "0,2,4,6,8,10,12,14"
SY_PIN_SMT = "0-15"
SY_RESULTS = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "spin_then_yield_reclaim_stall.json"
# §37.4's cells. O, C, O-large and S are reclaim_stall overwrite windows on
# SyncExpanseBlobMap; H is writer_scaling's count cell (§23).
SY_CELLS: tuple[dict[str, Any], ...] = (
    {"name": "O", "kind": "stall", "role": "primary", "writers": 12, "readers": 2, "live": SMALL, "pin": SY_PIN_CORES},
    {"name": "C", "kind": "stall", "role": "control", "writers": 4, "readers": 2, "live": SMALL, "pin": SY_PIN_CORES},
    {"name": "O-large", "kind": "stall", "role": "reported", "writers": 12, "readers": 2, "live": LARGE,
     "pin": SY_PIN_CORES},
    {"name": "S", "kind": "stall", "role": "reported", "writers": 12, "readers": 2, "live": SMALL, "pin": SY_PIN_SMT},
    {"name": "H", "kind": "harm", "role": "harm", "writers": 4, "readers": 1, "live": None, "pin": SY_PIN_CORES},
)
SY_O_VERDICTS = ("GAIN", "LOSS", "OPEN")
SY_C_VERDICTS = ("FLAT", "MOVED")
SY_H_VERDICTS = ("HARM", "NO_HARM")
SY_RUN_VERDICTS = ("RUN_PASS", "RUN_REJECT", "RUN_OPEN")


def sy_cell(name: str) -> dict[str, Any]:
    hits = [c for c in SY_CELLS if c["name"] == name]
    if len(hits) != 1:
        raise DriverError(f"no §37.4 cell named {name!r}")
    return hits[0]


def sy_schedule(rounds: int, cells: tuple[dict[str, Any], ...] = SY_CELLS) -> list[dict[str, Any]]:
    """Every timed process of a §37 run, in execution order (§37.5).

    One process per (build, cell, round). The cell order rotates by round as
    §29.2's does; within a round the two builds of a cell run back to back, and
    which goes first alternates by round.
    """
    out = []
    for r in range(rounds):
        builds = SY_BUILDS if r % 2 == 0 else SY_BUILDS[::-1]
        for c in rotated(cells, r):
            for order, b in enumerate(builds):
                out.append({"round": r, "cell": c["name"], "build": b, "order": order})
    return out


def sy_seed(c: dict[str, Any], r: int) -> int:
    """One seed per (cell, round), shared by both builds so each per-round ratio is paired."""
    return 0x1479_0000 + 97 * r + [x["name"] for x in SY_CELLS].index(c["name"])


def sy_build_reclaim(features: list[str]) -> Path:
    """The reclaim_stall bench built with `features`, its path read from cargo's own report."""
    cmd = ["cargo", "build", "--release", "-p", "expanse-trie", "--bench", "reclaim_stall",
           "--message-format=json-render-diagnostics"]
    if features:
        cmd += ["--features", ",".join(features)]
    out = subprocess.run(cmd, cwd=REPO_ROOT, check=True, stdout=subprocess.PIPE, text=True).stdout
    exes = [json.loads(ln).get("executable") for ln in out.splitlines() if ln.startswith("{")]
    exes = [e for e in exes if e and Path(e).name.startswith("reclaim_stall")]
    if len(exes) != 1:
        raise DriverError(f"expected one reclaim_stall executable for features {features}, found {exes}")
    return Path(exes[0])


def sy_probe(exe: Path, build: str, counters: bool) -> dict[str, Any]:
    """Refuses a harness that lacks the §37 identify mode, or that is not the build it is labelled (§8.1)."""
    proc = subprocess.run([str(exe), "--map", "sync", "--mode", "identify"],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
    lines = [ln for ln in proc.stdout.splitlines() if ln.startswith("{")]
    if proc.returncode != 0 or len(lines) != 1:
        raise DriverError(
            f"the reclaim_stall harness at {exe} has no `--mode identify` (exit {proc.returncode}): it predates "
            f"METHODOLOGY §37.4's prerequisites, so no §37 cell can run on it. Dispatch at a ref that carries "
            f"them.\n{proc.stderr.strip()[-400:]}")
    row = json.loads(lines[0])
    want = {"gate_wait_variant": build == SY_FEATURE, "occ_stats": counters}
    got = {k: row.get(k) for k in want}
    if got != want:
        raise DriverError(f"the {build} build at {exe} reports {got}, expected {want}")
    return row


def sy_preexec(pin: str | None):
    return None if pin is None else bench_pin.child_affinity(pin)


def sy_check_affinity(row: dict[str, Any], pin: str | None, where: str) -> None:
    """§37.4: the applied list is recorded in each row and refused if it differs."""
    if pin is None:
        return
    got = row.get("cpu_affinity")
    if got is None or bench_pin.expand(str(got)) != bench_pin.expand(pin):
        raise DriverError(f"{where}: the process ran on CPUs {got!r}, not the cell's pin {pin}")


def sy_run_stall(exe: Path, c: dict[str, Any], r: int, build: str, pin: str | None) -> dict[str, Any]:
    argv = [str(exe), "--map", "sync", "--mode", "overwrite", "--writers", str(c["writers"]),
            "--readers", str(c["readers"]), "--round", str(r), "--seed", str(sy_seed(c, r)),
            "--live", str(c["live"])]
    where = f"cell {c['name']} {build} round {r}"
    proc = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False,
                          preexec_fn=sy_preexec(pin))
    if proc.returncode != 0:
        raise DriverError(f"{where}: harness exit {proc.returncode}\n{proc.stderr.strip()[-400:]}")
    out = proc.stdout.strip().splitlines()
    if len(out) != 1:
        raise DriverError(f"{where}: expected one JSON row, got {len(out)} lines")
    row = json.loads(out[0])
    for k, v in (("map", "sync"), ("mode", "overwrite"), ("writers", c["writers"]), ("readers", c["readers"]),
                 ("live", c["live"]), ("gate_wait_variant", build == SY_FEATURE)):
        if row.get(k) != v:
            raise DriverError(f"{where}: the row reports {k}={row.get(k)!r}, expected {v!r}")
    sy_check_affinity(row, pin, where)
    row["build"], row["cell_name"], row["pin"] = build, c["name"], pin
    return row


def sy_harm_env(pin: str | None) -> dict[str, str]:
    env = dict(os.environ)
    env.pop("EXPANSE_BENCH_PIN_APPLIED", None)
    if pin is not None:
        env["EXPANSE_BENCH_PIN_APPLIED"] = pin
    return env


def sy_run_harm(ws, exe: Path, role: str, r: int, position: int, build: str, pin: str | None,
                quick: bool) -> dict[str, Any]:
    row = ws.spin_yield_harm_row(exe, role, r, position, quick, preexec_fn=sy_preexec(pin), env=sy_harm_env(pin))
    if pin is not None and not ws.pins_equal(row.get("cpu_pin"), pin):
        raise DriverError(f"cell H {build} round {r}: the row records cpu_pin {row.get('cpu_pin')!r}, not {pin}")
    row["build"], row["cell_name"], row["pin"] = build, "H", pin
    return row


def stall_throughput(row: dict[str, Any]) -> float:
    """§37.5 `T`: the window's overwrites, all writers together, over its duration."""
    done = int(row["inserts_ok"]) + int(row["inserts_refused"])
    if done != int(row["appends"]):
        raise DriverError(f"cell {row.get('cell_name')} round {row.get('round')}: {done} inserts timed, "
                          f"{row['appends']} appends scheduled")
    return done / float(row["elapsed_s"])


def sy_interval(values: list[float]) -> dict[str, Any]:
    """BCa 95% over the per-round ratios, 2,000 resamples, seed 42 (§37.5)."""
    if len(values) < 3:
        raise DriverError(f"a BCa interval needs at least 3 per-round ratios, got {len(values)}")
    point, lo, hi, method = bca_bootstrap_ci_with_method(values, confidence=0.95, num_resamples=2000, seed=42)
    return {"mean": point, "ci_lower": lo, "ci_upper": hi, "ci_method": method,
            "median": statistics.median(values), "n": len(values), "per_round": values}


def sy_paired(rows: list[dict[str, Any]], cell: str, value, variant_over_default: bool) -> list[float]:
    """Per-round ratios of one cell's two builds, each round paired with itself."""
    by: dict[tuple[str, int], float] = {}
    for row in rows:
        if row["cell_name"] != cell:
            continue
        key = (row["build"], int(row["round"]))
        if key in by:
            raise DriverError(f"cell {cell}: round {key[1]} holds two {key[0]} rows")
        by[key] = float(value(row))
    rounds = sorted({r for _, r in by})
    out = []
    for r in rounds:
        d, v = by.get((SY_DEFAULT, r)), by.get((SY_FEATURE, r))
        if d is None or v is None or d <= 0 or v <= 0:
            raise DriverError(f"cell {cell}: round {r} is unpaired or non-positive (default {d}, variant {v})")
        out.append(v / d if variant_over_default else d / v)
    return out


def sy_cell_o(iv: dict[str, Any]) -> str:
    """§37.6 item 1: `GAIN` when the lower bound of `g_stall` is above 1.0, `LOSS` when its upper bound is below."""
    lo, hi = _bounds(iv)
    if lo > 1.0:
        return "GAIN"
    if hi < 1.0:
        return "LOSS"
    return "OPEN"


def sy_cell_c(iv: dict[str, Any]) -> str:
    """§37.6 item 2: `FLAT` when the interval of `g_stall` contains 1.0."""
    lo, hi = _bounds(iv)
    return "FLAT" if lo <= 1.0 <= hi else "MOVED"


def sy_cell_h(iv: dict[str, Any]) -> str:
    """§37.6 item 3: `HARM` when the upper bound of `h` is below 1.0."""
    _, hi = _bounds(iv)
    return "HARM" if hi < 1.0 else "NO_HARM"


def _bounds(iv: dict[str, Any]) -> tuple[float, float]:
    lo, hi = iv.get("ci_lower"), iv.get("ci_upper")
    for v in (lo, hi):
        if isinstance(v, bool) or not isinstance(v, (int, float)) or v != v:
            raise ValueError(f"an interval bound is {v!r}, not a number")
    if lo > hi:
        raise ValueError(f"interval [{lo}, {hi}] has its lower bound above its upper")
    return float(lo), float(hi)


def sy_run_verdict(o: str, c: str, h: str) -> str:
    """§37.6 item 4, total over the three cells' verdicts.

    `RUN_REJECT` when O is `LOSS` or H is `HARM`, whatever the other cells
    show; `RUN_PASS` when O is `GAIN` and C is `FLAT` and H is `NO_HARM`;
    `RUN_OPEN` in every other case, a `GAIN` beside a control that `MOVED`
    among them. A label outside a cell's vocabulary is refused.

    The PASS test's `h == "NO_HARM"` is implied by the REJECT test before it
    (H has two labels), so deleting it changes no answer: it is written out
    because §37.6 lists it, and no negative control can isolate it.
    """
    for name, v, allowed in (("O", o, SY_O_VERDICTS), ("C", c, SY_C_VERDICTS), ("H", h, SY_H_VERDICTS)):
        if v not in allowed:
            raise ValueError(f"cell {name}: verdict {v!r} is not one of {allowed}")
    if o == "LOSS" or h == "HARM":
        return "RUN_REJECT"
    if o == "GAIN" and c == "FLAT" and h == "NO_HARM":
        return "RUN_PASS"
    return "RUN_OPEN"


def sy_admissibility(rows: list[dict[str, Any]]) -> list[str]:
    """§37.5: an overwrite round that did not complete exactly two compactions, or refused an insert, voids the run."""
    bad = [f"cell {r['cell_name']} {r['build']} round {r['round']}: {r['compactions']} compactions, "
           f"{r['inserts_refused']} refused inserts" for r in rows
           if r.get("mode") == "overwrite" and (r["compactions"] != EXPECTED_COMPACTIONS or r["inserts_refused"] != 0)]
    return bad


def sy_report(stall_rows: list[dict[str, Any]], harm_rows: list[dict[str, Any]]) -> dict[str, Any]:
    """§37.5's statistics and §37.6's per-run verdict, from the timed rows of one run."""
    g_stall = {c["name"]: sy_interval(sy_paired(stall_rows, c["name"], stall_ns, False))
               for c in SY_CELLS if c["kind"] == "stall"}
    g_thr = {c["name"]: sy_interval(sy_paired(stall_rows, c["name"], stall_throughput, True))
             for c in SY_CELLS if c["kind"] == "stall"}
    h = sy_interval(sy_paired(harm_rows, "H", lambda r: r["writer_mops"], True))
    cells = {"O": sy_cell_o(g_stall["O"]), "C": sy_cell_c(g_stall["C"]), "H": sy_cell_h(h)}
    return {"g_stall": g_stall, "g_thr": g_thr, "h": h, "cell_verdicts": cells,
            "run_verdict": sy_run_verdict(cells["O"], cells["C"], cells["H"])}


def sy_counter_shares(row: dict[str, Any]) -> dict[str, Any]:
    """§37.2: each wait as a share of the window's thread time (stall cells; no verdict reads it)."""
    k = row["counters"]
    tt = int(k["thread_time_cycles"])
    return {**k, "shares_of_thread_time": {
        name: (int(k[name]) / tt if tt > 0 else None)
        for name in ("sample_spin_cycles", "gate_wait_cycles", "quiesce_drain_cycles")}}


def sy_cells_out(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """The artifact's `throughput` cells: one per (cell, build), each with its rounds verbatim."""
    out = []
    for c in SY_CELLS:
        for b in SY_BUILDS:
            mine = sorted((r for r in rows if r["cell_name"] == c["name"] and r["build"] == b),
                          key=lambda r: int(r["round"]))
            loads = [r["load"] for r in mine]
            out.append({"cell": f"{c['name']}:{b}", "name": c["name"], "build": b, "role": c["role"],
                        "kind": c["kind"], "writers": c["writers"], "readers": c["readers"], "live": c["live"],
                        "pin": c["pin"], "rounds_raw": mine,
                        "load": max(loads, key=lambda ld: ld.get("foreign_busy_cpus") or 0.0)})
    return out


def sy_measure(out_path: Path, rounds: int, quick: bool) -> dict[str, Any]:
    """`--spin-yield`: §37.4's cells in both builds, §37.5's schedule, §37.6's run verdict."""
    import writer_scaling as ws  # the harm cell's harness and checks (§37.4)

    committed = out_path.resolve().is_relative_to((REPO_ROOT / "docs" / "benchmarks").resolve())
    if quick and committed:
        raise DriverError("--quick writes only to a scratch path outside docs/benchmarks (AGENTS.md §8.5)")
    smoke = quick
    requested = os.environ.get("EXPANSE_BENCH_PIN") or SY_PIN_SMT
    if os.environ.get("EXPANSE_BENCH_PIN_APPLIED") or requested != SY_PIN_SMT:
        if not smoke:
            raise DriverError(
                f"the §37 driver runs under EXPANSE_BENCH_PIN={SY_PIN_SMT}, which holds every cell's pin, and pins "
                f"each cell's process itself; got EXPANSE_BENCH_PIN={os.environ.get('EXPANSE_BENCH_PIN')!r}, "
                f"EXPANSE_BENCH_PIN_APPLIED={os.environ.get('EXPANSE_BENCH_PIN_APPLIED')!r} (§37.4, §37.5)")
    os.environ.setdefault("EXPANSE_BENCH_PIN", requested)
    can_pin = hasattr(os, "sched_setaffinity")
    if not can_pin:
        if not smoke:
            raise DriverError("this host has no sched_setaffinity, so no cell can take its registered pin (§37.4)")
        os.environ["EXPANSE_BENCH_PIN"] = "off"
        sys.stderr.write("::notice:: smoke run on a host without sched_setaffinity: cells run unpinned, "
                         "and nothing this run produces is a §37.4 cell\n")
    applied = bench_pin.apply("reclaim_stall.py --spin-yield")
    if can_pin and not smoke:
        held = set(bench_pin.expand(applied)) if applied not in ("none", "off") else set()
        for c in SY_CELLS:
            if not set(bench_pin.expand(c["pin"])) <= held:
                raise DriverError(f"the driver's pin {applied!r} does not hold cell {c['name']}'s pin {c['pin']}")

    def pin_of(c: dict[str, Any]) -> str | None:
        return c["pin"] if can_pin else None

    stall_bin = {b: sy_build_reclaim([] if b == SY_DEFAULT else [b]) for b in SY_BUILDS}
    stall_cnt = {b: sy_build_reclaim(["occ-stats"] if b == SY_DEFAULT else ["occ-stats", b]) for b in SY_BUILDS}
    for b in SY_BUILDS:
        sy_probe(stall_bin[b], b, counters=False)
        sy_probe(stall_cnt[b], b, counters=True)
    harm_bin = {b: ws.build_binaries(None if b == SY_DEFAULT else b, verbose=True) for b in SY_BUILDS}

    prov = new_provenance(
        "concurrency", 1479,
        "per-round ratios of the two builds of one cell, BCa 95% over rounds (2,000 resamples, seed 42): "
        "g_stall = stall default / stall variant; g_thr = throughput variant / default; h = cell H writer "
        "Mops/s variant / default (METHODOLOGY §37.5)",
        REPO_ROOT, cell_isolation="process", rounds=rounds, core_pin_applied=applied)
    schedule = sy_schedule(rounds)
    print(f"§37 spin-then-yield: {len(schedule)} timed processes, {rounds} rounds, quick={quick}", flush=True)
    stall_rows: list[dict[str, Any]] = []
    harm_rows: list[dict[str, Any]] = []
    for i, run in enumerate(schedule):
        c, r, b = sy_cell(run["cell"]), run["round"], run["build"]
        start = begin_cell(prov, f"cell:{c['name']}:{b}:r{r}")
        if c["kind"] == "stall":
            row = sy_run_stall(stall_bin[b], c, r, b, pin_of(c))
            row["load"] = end_cell(start)
            stall_rows.append(row)
        else:
            row = sy_run_harm(ws, harm_bin[b][0], "throughput", r, run["order"], b, pin_of(c), quick)
            row["load"] = end_cell(start)
            harm_rows.append(row)
        print(f"  [{i + 1}/{len(schedule)}] {c['name']} {b} r{r}", flush=True)
    # §37.2: one counters pass per (cell, build), after every timed round; no verdict reads it.
    counters = []
    for c in SY_CELLS:
        for b in SY_BUILDS:
            if c["kind"] == "stall":
                row = sy_run_stall(stall_cnt[b], c, 0, b, pin_of(c))
                counters.append({"cell": c["name"], "build": b, **sy_counter_shares(row)})
            else:
                row = sy_run_harm(ws, harm_bin[b][1], "counters", 0, 0, b, pin_of(c), quick)
                counters.append({"cell": "H", "build": b, "counters_row": row})
    report = sy_report(stall_rows, harm_rows)
    void = sy_admissibility(stall_rows)
    if quick:
        void.append("--quick: a smoke run of the instrument, not a §37 run")
    if not can_pin:
        void.append("unpinned: the host has no sched_setaffinity, so no cell took its registered pin (§37.5)")
    art = {
        "provenance": prov,
        "throughput": sy_cells_out(stall_rows + harm_rows),
        "spin_then_yield": {
            "issue": 1479,
            "preregistration": "docs/benchmarks/concurrency/METHODOLOGY.md §37",
            "builds": list(SY_BUILDS),
            "cells": [dict(c) for c in SY_CELLS],
            "rounds": rounds,
            "quick": quick,
            "schedule": "one process per (build, cell, round); the cell order rotates by round; within a round "
                        "the two builds of a cell run back to back, the first alternating by round (§37.5)",
            **report,
            "counters": counters,
            "void": void,
            "not_decided_here": [
                "§37.3's gates on the build (P37.1, P37.2, disassembly, Loom): decided on the pull request "
                "that adds the feature, not by this driver",
                "the workflow's host guard: its record is attached at provenance.host_guard after the run, "
                "and combine_spin_yield_verdict voids a run it disturbed",
            ],
        },
    }
    # A smoke run to scratch is checked as if it were the committed artifact,
    # so its shape is gated too. The guard's record is attached by the
    # workflow after the driver exits, so only its absence is excused here.
    rel = check_bench_provenance.rel_path((out_path if committed else SY_RESULTS).resolve())
    findings = [f for f in check_bench_provenance.findings_for(rel, json.loads(json.dumps(art)))
                if "provenance.host_guard` missing" not in f]
    if findings and not smoke:
        raise DriverError("the artifact fails the provenance gate: " + "; ".join(findings))
    for f in findings:
        # A smoke run is void already; a host without /proc cannot close a load window.
        sys.stderr.write(f"::notice:: smoke artifact would fail the provenance gate: {f}\n")
        void.append(f"provenance gate: {f}")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(art, indent=2) + "\n")
    return art


def combine_spin_yield_verdict(paths: list[Path | str]) -> dict[str, Any]:
    """The pair verdict of one pair's dispatches, in order, read from their artifacts (§37.5, §37.6 item 5).

    Refuses an artifact without the `spin_then_yield` section, one taken with
    `--quick`, one without a host-guard record, and dispatches at different
    commits (a pair is one SHA). A run the host guard voids, or that the
    driver recorded void (§37.5: pin, compactions, refusals), is a void
    dispatch whatever its run verdict reads. Replacement runs follow §36.6.
    """
    import writer_scaling as ws

    entries: list[str] = []
    reasons: list[list[str]] = []
    commits = set()
    for p in map(Path, paths):
        if not p.exists():
            raise ValueError(f"artifact does not exist: {p}")
        art = json.loads(p.read_text(encoding="utf-8"))
        sec = art.get("spin_then_yield")
        if not sec:
            raise ValueError(f"{p} lacks a 'spin_then_yield' section")
        if sec.get("quick"):
            raise ValueError(f"{p} was run with --quick; not a §37 run")
        commit = art.get("provenance", {}).get("commit")
        if not commit:
            raise ValueError(f"{p} records no commit in its provenance")
        commits.add(commit)
        why = ws.set_host_guard_void_reasons(art, str(p)) + list(sec.get("void") or [])
        if sec.get("run_verdict") not in SY_RUN_VERDICTS:
            raise ValueError(f"{p}: run verdict {sec.get('run_verdict')!r} is not one of {SY_RUN_VERDICTS}")
        reasons.append(why)
        entries.append(ws.SET_VOID_RUN if why else sec["run_verdict"])
    if len(commits) != 1:
        raise ValueError(f"the dispatches ran at {len(commits)} commits {sorted(commits)}; a pair is one SHA")
    return {"commit": commits.pop(), "dispatches": entries, "void_reasons": reasons,
            "pair": ws.set_pair_from_dispatches(entries)}


def sy_self_test() -> int:
    """§37's schedule, statistics and verdict, with one negative control per verdict clause."""
    import tempfile

    import writer_scaling as ws

    def iv(lo: float, hi: float) -> dict[str, Any]:
        return {"ci_lower": lo, "ci_upper": hi}

    def raises(fn, exc: type[Exception]) -> bool:
        """Whether `fn()` raises `exc`; any other exception propagates."""
        try:
            fn()
        except exc:
            return True
        return False

    # The registered constants (§37.7's silent constants).
    assert SY_ROUNDS == 8 and SY_BUILDS == ("default", "ablation-gate-spin-then-yield")
    assert [(c["name"], c["writers"], c["readers"], c["live"], c["pin"]) for c in SY_CELLS] == [
        ("O", 12, 2, 200_000, "0,2,4,6,8,10,12,14"), ("C", 4, 2, 200_000, "0,2,4,6,8,10,12,14"),
        ("O-large", 12, 2, 3_600_000, "0,2,4,6,8,10,12,14"), ("S", 12, 2, 200_000, "0-15"),
        ("H", 4, 1, None, "0,2,4,6,8,10,12,14")], SY_CELLS
    assert ws.SPIN_YIELD_HARM_CELL == {"read_op": "count_locked", "probe": "uniform", "writers": 4, "readers": 1}
    assert ws.SPIN_YIELD_HARM_PIN == SY_PIN_CORES and ws.SPIN_YIELD_FEATURE == SY_FEATURE
    # Schedule: one process per (build, cell, round); builds back to back, first alternating; cells rotate.
    sched = sy_schedule(SY_ROUNDS)
    assert len(sched) == SY_ROUNDS * len(SY_CELLS) * 2
    assert len({(s["build"], s["cell"], s["round"]) for s in sched}) == len(sched)
    for i in range(0, len(sched), 2):
        a, b = sched[i], sched[i + 1]
        assert a["cell"] == b["cell"] and a["round"] == b["round"] and {a["build"], b["build"]} == set(SY_BUILDS)
        assert a["build"] == SY_BUILDS[a["round"] % 2], (a, "the first build alternates by round")
    firsts = [next(s["cell"] for s in sched if s["round"] == r) for r in range(SY_ROUNDS)]
    assert firsts == [SY_CELLS[r % len(SY_CELLS)]["name"] for r in range(SY_ROUNDS)], firsts
    assert sy_seed(SY_CELLS[0], 3) != sy_seed(SY_CELLS[1], 3) and sy_seed(SY_CELLS[0], 3) != sy_seed(SY_CELLS[0], 4)

    # Cell O, §37.6 item 1: each clause.
    assert sy_cell_o(iv(1.01, 1.2)) == "GAIN"          # lower bound above 1.0
    assert sy_cell_o(iv(1.0, 1.2)) == "OPEN"           # a lower bound AT 1.0 is not above it
    assert sy_cell_o(iv(0.8, 0.99)) == "LOSS"          # upper bound below 1.0
    assert sy_cell_o(iv(0.8, 1.0)) == "OPEN"           # an upper bound AT 1.0 is not below it
    assert sy_cell_o(iv(0.9, 1.1)) == "OPEN"
    # Cell C, item 2: contains 1.0, bounds inclusive.
    assert sy_cell_c(iv(0.9, 1.1)) == "FLAT"
    assert sy_cell_c(iv(1.0, 1.1)) == "FLAT" and sy_cell_c(iv(0.9, 1.0)) == "FLAT"
    assert sy_cell_c(iv(1.01, 1.1)) == "MOVED" and sy_cell_c(iv(0.9, 0.99)) == "MOVED"
    # Cell H, item 3: harm only when the upper bound is below 1.0.
    assert sy_cell_h(iv(0.8, 0.99)) == "HARM"
    assert sy_cell_h(iv(0.8, 1.0)) == "NO_HARM" and sy_cell_h(iv(0.9, 1.3)) == "NO_HARM"
    # Bounds are refused, never judged, when not numbers or inverted.
    for bad in (iv(float("nan"), 1.0), iv(1.2, 1.1), {"ci_lower": True, "ci_upper": 1.0}, {}):
        assert raises(lambda: sy_cell_o(bad), ValueError), f"a malformed interval {bad} was judged"
    # Run verdict, item 4: every clause, and totality over the 3 x 2 x 2 labels.
    assert sy_run_verdict("GAIN", "FLAT", "NO_HARM") == "RUN_PASS"
    assert sy_run_verdict("GAIN", "MOVED", "NO_HARM") == "RUN_OPEN"     # control moved: not a pass
    assert sy_run_verdict("GAIN", "FLAT", "HARM") == "RUN_REJECT"       # harm blocks promotion
    assert sy_run_verdict("LOSS", "FLAT", "NO_HARM") == "RUN_REJECT"    # a loss rejects
    assert sy_run_verdict("LOSS", "MOVED", "NO_HARM") == "RUN_REJECT"   # whatever C shows
    assert sy_run_verdict("OPEN", "FLAT", "NO_HARM") == "RUN_OPEN"
    assert sy_run_verdict("OPEN", "FLAT", "HARM") == "RUN_REJECT"
    for o in SY_O_VERDICTS:
        for c in SY_C_VERDICTS:
            for h in SY_H_VERDICTS:
                assert sy_run_verdict(o, c, h) in SY_RUN_VERDICTS
    for args in (("PASS", "FLAT", "NO_HARM"), ("GAIN", "OPEN", "NO_HARM"), ("GAIN", "FLAT", "FLAT")):
        assert raises(lambda: sy_run_verdict(*args), ValueError), f"{args} was judged"

    # Statistics from synthetic rows: the variant halves cell O's stall, leaves C flat, costs H nothing.
    def stall_row(cell: str, b: str, r: int, stall: float, thr: float) -> dict[str, Any]:
        appends = 1_000_000
        return {"cell_name": cell, "build": b, "round": r, "mode": "overwrite", "compactions": 2,
                "inserts_refused": 0, "inserts_ok": appends, "appends": appends,
                "elapsed_s": appends / thr, "insert": {"max_ns": int(stall)}}

    def rows_for(o_gain: float, c_gain: float, h_gain: float) -> tuple[list, list]:
        srows, hrows = [], []
        for r in range(SY_ROUNDS):
            j = 1 + 0.01 * (r % 3)
            for name, gain in (("O", o_gain), ("C", c_gain), ("O-large", 1.0), ("S", 1.0)):
                g = gain if name != "C" else (c_gain * (1 + 0.02 * ((-1) ** r)))
                srows.append(stall_row(name, SY_DEFAULT, r, 3.2e7 * j, 2e6))
                srows.append(stall_row(name, SY_FEATURE, r, 3.2e7 * j / g, 2e6))
            hrows.append({"cell_name": "H", "build": SY_DEFAULT, "round": r, "writer_mops": 1.0 * j})
            hrows.append({"cell_name": "H", "build": SY_FEATURE, "round": r, "writer_mops": h_gain * j})
        return srows, hrows

    rep = sy_report(*rows_for(2.0, 1.0, 1.0))
    assert rep["cell_verdicts"] == {"O": "GAIN", "C": "FLAT", "H": "NO_HARM"}, rep["cell_verdicts"]
    assert rep["run_verdict"] == "RUN_PASS"
    assert abs(rep["g_stall"]["O"]["mean"] - 2.0) < 1e-9 and rep["g_stall"]["O"]["ci_method"]
    assert set(rep["g_stall"]) == set(rep["g_thr"]) == {"O", "C", "O-large", "S"}
    assert sy_report(*rows_for(2.0, 1.0, 0.8))["run_verdict"] == "RUN_REJECT"
    assert sy_report(*rows_for(0.5, 1.0, 1.0))["cell_verdicts"]["O"] == "LOSS"
    assert sy_report(*rows_for(2.0, 1.5, 1.0))["run_verdict"] == "RUN_OPEN"
    srows, hrows = rows_for(2.0, 1.0, 1.0)
    assert raises(lambda: sy_report(srows[:-1], hrows), DriverError), "an unpaired round was judged"
    # Admissibility (§37.5): a round without two compactions, or with a refusal, is named.
    srows[3]["compactions"] = 1
    srows[5]["inserts_refused"] = 2
    assert len(sy_admissibility(srows)) == 2

    # The pair, honouring the host-guard record as combine_set_ordered_readers_verdict does.
    quiet = {"start": {"verdict": "quiet"}, "watcher_ok": True, "run": {"samples": 9, "voids": 0, "verdict": "quiet"}}
    disturbed = {"start": {"verdict": "quiet"}, "watcher_ok": True,
                 "run": {"samples": 9, "voids": 1, "verdict": "disturbed", "reasons": ["foreign load on the pin"]}}
    with tempfile.TemporaryDirectory() as tmp:
        def write(name: str, verdict: str, guard: dict | None, sha: str = "a" * 40, quick: bool = False,
                  void: list | None = None) -> Path:
            prov: dict[str, Any] = {"commit": sha}
            if guard is not None:
                prov["host_guard"] = guard
            p = Path(tmp) / name
            p.write_text(json.dumps({"provenance": prov, "spin_then_yield": {
                "run_verdict": verdict, "quick": quick, "void": void or []}}))
            return p

        p1, p2 = write("r1.json", "RUN_PASS", quiet), write("r2.json", "RUN_PASS", quiet)
        got = combine_spin_yield_verdict([p1, p2])
        assert got["pair"]["verdict"] == "PASS" and got["dispatches"] == ["RUN_PASS", "RUN_PASS"], got
        assert combine_spin_yield_verdict([write("j1.json", "RUN_REJECT", quiet),
                                           write("j2.json", "RUN_REJECT", quiet)])["pair"]["verdict"] == "REJECTED"
        assert combine_spin_yield_verdict([p1, write("o.json", "RUN_OPEN", quiet)])["pair"]["verdict"] == "INTERMEDIATE"
        # A guard-voided run is VOID whatever its verdict reads, and is replaced.
        pv = write("v.json", "RUN_PASS", disturbed)
        got = combine_spin_yield_verdict([pv, p1, p2])
        assert got["dispatches"] == ["VOID", "RUN_PASS", "RUN_PASS"] and got["pair"]["verdict"] == "PASS", got
        assert got["void_reasons"][0], got
        # A run the driver recorded void (§37.5) is VOID too.
        pd = write("d.json", "RUN_PASS", quiet, void=["cell O default round 2: 1 compactions, 0 refused inserts"])
        assert combine_spin_yield_verdict([pd, p1])["dispatches"][0] == "VOID"
        # Refusals: a missing guard record, --quick, two SHAs, no section, no file.
        for paths, needle in (([p1, write("ng.json", "RUN_PASS", None)], "no provenance.host_guard record"),
                              ([p1, write("q.json", "RUN_PASS", quiet, quick=True)], "--quick"),
                              ([p1, write("b.json", "RUN_PASS", quiet, sha="b" * 40)], "one SHA"),
                              ([p1, Path(tmp) / "none.json"], "does not exist")):
            try:
                combine_spin_yield_verdict(paths)
            except ValueError as exc:
                assert needle in str(exc), (needle, str(exc))
            else:
                raise AssertionError(f"combine accepted {paths} ({needle})")
        bare = Path(tmp) / "bare.json"
        bare.write_text(json.dumps({"provenance": {"commit": "a" * 40, "host_guard": quiet}}))
        try:
            combine_spin_yield_verdict([p1, bare])
        except ValueError as exc:
            assert "lacks a 'spin_then_yield' section" in str(exc)
        else:
            raise AssertionError("an artifact without the section was combined")

    # The probe refuses a harness without the identify mode (§8.1).
    with tempfile.TemporaryDirectory() as tmp:
        fake = Path(tmp) / "reclaim_stall_old"
        fake.write_text("#!/bin/sh\necho 'unknown --map sync / --mode identify' >&2\nexit 101\n")
        fake.chmod(0o755)
        try:
            sy_probe(fake, SY_DEFAULT, counters=False)
        except DriverError as exc:
            assert "has no `--mode identify`" in str(exc), str(exc)
        else:
            raise AssertionError("the probe accepted a harness without the identify mode")
        wrong = Path(tmp) / "reclaim_stall_wrong"
        wrong.write_text('#!/bin/sh\necho \'{"gate_wait_variant": false, "occ_stats": false}\'\n')
        wrong.chmod(0o755)
        try:
            sy_probe(wrong, SY_FEATURE, counters=False)
        except DriverError as exc:
            assert "expected" in str(exc), str(exc)
        else:
            raise AssertionError("the probe accepted a default build labelled as the variant")
    # A row whose CPUs are not the cell's pin is refused.
    assert raises(lambda: sy_check_affinity({"cpu_affinity": "0-15"}, SY_PIN_CORES, "test"), DriverError), \
        "a row on the wrong CPUs was accepted"
    sy_check_affinity({"cpu_affinity": "0,2,4,6,8,10,12,14"}, SY_PIN_CORES, "test")
    print("reclaim_stall.py §37 self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--rounds", type=int, default=None)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--spin-yield", action="store_true",
                    help="METHODOLOGY §37 (#1479): cells O, C, O-large, S and H in the default and "
                         "ablation-gate-spin-then-yield builds")
    ap.add_argument("--quick", action="store_true",
                    help="with --spin-yield: a smoke run to a scratch path, 3 rounds unless --rounds")
    ap.add_argument("--spin-yield-pair", nargs="+", type=Path, metavar="ARTIFACT",
                    help="print the §37.6 pair verdict of one pair's dispatches, in order")
    args = ap.parse_args()
    if args.self_test:
        return self_test() or sy_self_test()
    if args.spin_yield_pair:
        print(json.dumps(combine_spin_yield_verdict(args.spin_yield_pair), indent=2))
        return 0
    if args.spin_yield:
        out = args.out or SY_RESULTS
        rounds = args.rounds or (SY_QUICK_ROUNDS if args.quick else SY_ROUNDS)
        if not args.quick and rounds != SY_ROUNDS:
            ap.error(f"a §37 run takes {SY_ROUNDS} rounds (§37.5); --rounds is for --quick smoke runs only")
        try:
            art = sy_measure(out, rounds, args.quick)
        except (DriverError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
            sys.stderr.write(f"spin-then-yield run failed: {exc} (AGENTS.md §8.1)\n")
            return 1
        sec = art["spin_then_yield"]
        for name, e in sec["g_stall"].items():
            print(f"  g_stall {name}: {e['mean']:.4f} [{e['ci_lower']:.4f}, {e['ci_upper']:.4f}]")
        for name, e in sec["g_thr"].items():
            print(f"  g_thr {name}: {e['mean']:.4f} [{e['ci_lower']:.4f}, {e['ci_upper']:.4f}]")
        h = sec["h"]
        print(f"  h: {h['mean']:.4f} [{h['ci_lower']:.4f}, {h['ci_upper']:.4f}]")
        print(f"  cells: {sec['cell_verdicts']}  run verdict: {sec['run_verdict']}")
        for reason in sec["void"]:
            sys.stderr.write(f"::warning:: this run is void as a §37 run: {reason}\n")
        print(f"Wrote artifact to {out}")
        return 0
    if args.rounds is None:
        args.rounds = ROUNDS
    if args.quick:
        ap.error("--quick applies to --spin-yield only")
    if not args.out:
        ap.error("--out is required")
    if os.environ.get("EXPANSE_RECLAIM_STALL_ROUNDS"):
        args.rounds = int(os.environ["EXPANSE_RECLAIM_STALL_ROUNDS"])
    art = measure(args.rounds, args.out)
    print(json.dumps(art["predictions"], indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
