#!/usr/bin/env python3
"""METHODOLOGY §29: the stall and the memory peak of reclamation at the arena cap (#1300 item 1).

Runs `crates/expanse/benches/reclaim_stall.rs` once per (cell, round), each in
its own process, with the cell order rotated per round, takes a load window
around every process, and writes one artifact whose `throughput` cells carry
their rounds verbatim. It then reads §29.3's predictions P1-P5 from the rounds
and records each verdict beside the cells.

Usage:
    python3 docs/benchmarks/concurrency/scripts/reclaim_stall.py --out ARTIFACT.json
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


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--rounds", type=int, default=ROUNDS)
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.out:
        ap.error("--out is required")
    if os.environ.get("EXPANSE_RECLAIM_STALL_ROUNDS"):
        args.rounds = int(os.environ["EXPANSE_RECLAIM_STALL_ROUNDS"])
    art = measure(args.rounds, args.out)
    print(json.dumps(art["predictions"], indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
