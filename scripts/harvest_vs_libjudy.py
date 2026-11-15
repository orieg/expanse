#!/usr/bin/env python3
"""Fold one `bench_vs_libjudy` run into a committable `results/baseline_vs_libjudy*.json`.

The bare-metal workflow uploads two files for a `vs_libjudy` run: the harness's
per-round paired ratios and byte counts (`vs-libjudy-ratios.json`,
`EXPANSE_BENCH_RATIOS_JSON`) and the windowed driver's load record
(`load-vs_libjudy.json`, one window per `(distribution, population)` cell,
`scripts/bench_windowed.py --mode markers`). Neither alone is the artifact a
published figure cites: the first has no host, commit or load, the second no
numbers. This joins them (AGENTS.md sections 8.7 and 8.17, #1662):

  - every series keeps its per-round `ratios` verbatim and gains `point`,
    `ci_lower`, `ci_upper` and `ci_method` from `scripts/bca_bootstrap.py`
    (2000 resamples, seed 42), and the `load` of its cell's window;
  - every `memory` record is carried verbatim — exact `JudyLMemUsed` byte
    counts per round, which carry no interval (section 8.4) — and gains
    `bytes_per_key_over_stock` against the stock record of its cell;
  - `provenance` is the load record's, with `ci_run` and an `estimators`
    block saying what each column is.

A run whose load record is not `ok`, whose harness exited non-zero, or whose
cell has no window is refused, not folded (section 8.1).

Usage:
  python3 scripts/harvest_vs_libjudy.py --ratios vs-libjudy-ratios.json \\
      --load load-vs_libjudy.json --run-url <url> --out results/baseline_vs_libjudy.json
  python3 scripts/harvest_vs_libjudy.py --self-test
"""

from __future__ import annotations

import argparse
import copy
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

SCHEMA = "expanse.baseline.vs_libjudy.v2"
RESAMPLES = 2000
SEED = 42


def window_id(dist: str, pop: int) -> str:
    """The `BENCH_WINDOW` id the harness prints for a cell."""
    return f"{dist}/pop={pop}"


def harvest(ratios: dict, load: dict, run_url: str) -> dict:
    """The committable artifact, or `ValueError` naming why the run is refused."""
    prov = copy.deepcopy(load["provenance"])
    if prov.get("load_status") != "ok":
        raise ValueError(f"load record is {prov.get('load_status')!r}, not 'ok': "
                         f"{prov.get('load_findings')}")
    if prov.get("harness_exit_status") != 0:
        raise ValueError(f"harness exited {prov.get('harness_exit_status')!r}")
    windows = {}
    for w in prov.get("windows", []):
        # `bench_windowed.Windower` records `id`, and `merged` with every id
        # when a window too short to resolve was folded into its neighbour.
        for wid in w.get("merged", [w.get("id")]):
            windows[wid] = w.get("load")

    series = []
    for s in ratios["series"]:
        wid = window_id(s["dist"], s["pop"])
        if not isinstance(windows.get(wid), dict):
            raise ValueError(f"{s['id']}: no load window {wid!r} in the load record")
        point, lo, hi, method = bca_bootstrap_ci_with_method(
            s["ratios"], num_resamples=RESAMPLES, seed=SEED)
        out = dict(s)
        out["point"] = round(point, 4)
        out["ci_lower"] = round(lo, 4)
        out["ci_upper"] = round(hi, 4)
        out["ci_method"] = method
        out["load"] = windows[wid]
        series.append(out)

    stock = {(m["dist"], m["pop"]): m for m in ratios.get("memory", []) if m["arm"] == "stock"}
    memory = []
    for m in ratios.get("memory", []):
        out = dict(m)
        den = stock.get((m["dist"], m["pop"]))
        if den is None:
            raise ValueError(f"{m['id']}: no stock memory record for its cell")
        if m["arm"] != "stock":
            out["bytes_per_key_over_stock"] = round(m["bytes_per_key"] / den["bytes_per_key"], 4)
        memory.append(out)
    if not memory:
        raise ValueError("the ratios file carries no `memory` list: the harness predates #1662")

    prov["ci_run"] = run_url
    prov["estimators"] = {
        "ratio": "subject / stock wall-clock ns per op, formed inside each round "
                 "(arms interleaved, order rotated per round); `point` is the mean of "
                 "the per-round ratios",
        "columns": "point, ci_lower, ci_upper: BCa 95% over the per-round ratios, "
                   f"scripts/bca_bootstrap.py ({RESAMPLES} resamples, seed {SEED}); "
                   "memory.bytes_used: exact JudyLMemUsed per round, no interval",
        "raw": "every series carries its per-round `ratios` verbatim",
    }
    return {
        "schema": SCHEMA,
        "harness": ratios["harness"],
        "rounds": ratios["rounds"],
        "hit_rate": ratios["hit_rate"],
        "denominator_arm": ratios["denominator_arm"],
        "note": "Ratios are subject/stock; <1.00 means libexpanse is faster or smaller. "
                "`expanse_dl` is the like-for-like arm.",
        "provenance": prov,
        "series": series,
        "memory": memory,
    }


def _self_test() -> int:
    failures = []
    ratios = {
        "harness": "h", "rounds": 3, "hit_rate": 0.5, "denominator_arm": "stock",
        "series": [{"id": "random/100/get/expanse_dl_over_stock", "dist": "random",
                    "pop": 100, "metric": "get_ns", "arm": "expanse_dl",
                    "ratios": [0.9, 0.95, 1.0]}],
        "memory": [
            {"id": "random/100/expanse_dl", "dist": "random", "pop": 100,
             "arm": "expanse_dl", "keys": 100, "bytes_used": [800, 800, 800],
             "deterministic": True, "bytes_per_key": 8.0},
            {"id": "random/100/stock", "dist": "random", "pop": 100, "arm": "stock",
             "keys": 100, "bytes_used": [1000, 1000, 1000], "deterministic": True,
             "bytes_per_key": 10.0},
        ],
    }
    win = {"busy_cpus_since_prev": 1.0, "foreign_busy_cpus": 0.0}
    load = {"provenance": {"load_status": "ok", "harness_exit_status": 0, "loads": [],
                           "windows": [{"id": "random/pop=100", "load": win}]}}
    out = harvest(ratios, load, "u")
    s = out["series"][0]
    if s["ratios"] != [0.9, 0.95, 1.0] or s["load"] != win or "ci_method" not in s:
        failures.append(f"series not folded: {s}")
    if not s["ci_lower"] <= s["point"] <= s["ci_upper"]:
        failures.append(f"point outside its interval: {s}")
    if out["memory"][0].get("bytes_per_key_over_stock") != 0.8:
        failures.append(f"memory ratio: {out['memory'][0]}")
    if "bytes_per_key_over_stock" in out["memory"][1]:
        failures.append("the stock record carries a ratio against itself")

    def refused(name, r, ld, want):
        try:
            harvest(r, ld, "u")
        except ValueError as e:
            if want not in str(e):
                failures.append(f"{name}: refused for the wrong reason: {e}")
            return
        failures.append(f"{name}: was not refused")

    bad = copy.deepcopy(load)
    bad["provenance"]["load_status"] = "inadmissible"
    refused("inadmissible load", ratios, bad, "not 'ok'")
    bad = copy.deepcopy(load)
    bad["provenance"]["harness_exit_status"] = 101
    refused("harness failure", ratios, bad, "exited")
    bad = copy.deepcopy(load)
    bad["provenance"]["windows"] = []
    refused("missing window", ratios, bad, "no load window")
    merged = copy.deepcopy(load)
    merged["provenance"]["windows"] = [{"id": "seq/pop=1+random/pop=100", "load": win,
                                        "merged": ["seq/pop=1", "random/pop=100"]}]
    if harvest(ratios, merged, "u")["series"][0]["load"] != win:
        failures.append("a merged window did not attribute to its cells")
    old = copy.deepcopy(ratios)
    del old["memory"]
    refused("pre-#1662 harness", old, load, "no `memory` list")

    for f in failures:
        print(f"FAIL: {f}")
    print(f"harvest_vs_libjudy self-test: {'FAIL' if failures else 'ok'}")
    return 1 if failures else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--ratios")
    ap.add_argument("--load")
    ap.add_argument("--run-url")
    ap.add_argument("--out")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    if not (a.ratios and a.load and a.run_url and a.out):
        ap.error("--ratios, --load, --run-url and --out are required")
    try:
        art = harvest(json.loads(Path(a.ratios).read_text()),
                      json.loads(Path(a.load).read_text()), a.run_url)
    except ValueError as e:
        print(f"harvest_vs_libjudy: refused: {e}", file=sys.stderr)
        return 1
    Path(a.out).write_text(json.dumps(art, indent=2) + "\n")
    print(f"harvest_vs_libjudy: {len(art['series'])} series, {len(art['memory'])} "
          f"memory records -> {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
