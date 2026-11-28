#!/usr/bin/env python3
"""
Runner for the DNS-shaped key suite (docs/benchmarks/dns_names/).

Five harnesses (`crates/expanse/benches/dns_*.rs`), per METHODOLOGY.md:

  dns_get                     12 round processes   results/baseline_get.json
  dns_predecessor             12 round processes   results/baseline_predecessor.json
  dns_closest_encloser        12 round processes   results/baseline_closest_encloser.json
  dns_predecessor_concurrent   8 round processes   results/baseline_concurrent_readers.json
  dns_memory                   1 process           results/baseline_memory.json

A timed harness times one round per process (`--round K`, arm order rotated by
`K`), because a later round in the same process reuses memory the round before
it freed. This runner starts the processes, each inside one load window
(`bench_windowed.run_bench_window`; AGENTS.md section 8.17), merges their
rows, and adds to every cell the per-arm medians and the paired per-round
ratio of the subject arm to each other arm: the geometric mean of the
per-round ratios with a BCa 95% interval on the per-round log ratios. The
concurrent artifact carries S(W) = throughput(W) / throughput(1) per round
with its interval. The memory census is exact and has no interval; after the
last timed process the runner adds to it the bounds
`scripts/dns_name_bounds.py` derives for the same names (prediction E1).

`--quick` writes to the gitignored results/quick/ and never touches a
committed artifact (section 8.5). Its processes are too short for a load
window each, so one window spans them all.

The written artifacts are judged by `check_bench_provenance.findings_for`,
and a finding fails the run. Two entry points hold the host-wide benchmark
lock and the core pin before calling this file: the bare-metal workflow and
../run.sh. Running this file directly skips both.

  run_all.py [--quick] [--rounds N] [--host-desc D] [--run-id URL]
  run_all.py --self-test
"""

import argparse
import json
import math
import os
import platform
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bench_provenance import (  # noqa: E402
    add_load, attach, begin_cell, end_cell, estimators, git_sha, host_facts, rewrite,
)
import bench_windowed  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

SUITE = "dns_names"
ISSUE = 1800

CONFIDENCE = 0.95
RESAMPLES = 2000
SEED = 42
#: BCa needs n >= 3 (`scripts/bca_bootstrap.py`).
MIN_ROUNDS = 3

#: Round processes per timed harness (METHODOLOGY sections 3 and 5).
ROUNDS = 12
ROUNDS_CONCURRENT = 8
ROUNDS_QUICK = 3

SUBJECT = "expanse_ordered"
#: The arms each timed harness runs, subject first.
TIMED = {
    "dns_get": ("baseline_get.json",
                ("expanse_ordered", "expanse_direct", "btreemap", "qp_trie")),
    "dns_predecessor": ("baseline_predecessor.json",
                        ("expanse_ordered", "expanse_ordered_owned", "expanse_direct",
                         "btreemap")),
    "dns_closest_encloser": ("baseline_closest_encloser.json",
                             ("expanse_ordered", "expanse_direct", "btreemap", "qp_trie")),
}
#: Extra pairs beside subject-over-arm: the allocating predecessor against the twin.
EXTRA_PAIRS = {"dns_predecessor": (("expanse_ordered_owned", "btreemap"),)}
CONCURRENT = ("dns_predecessor_concurrent", "baseline_concurrent_readers.json")
CONCURRENT_ARMS = ("clone_per_reader", "sync_locked", "sync_get", "sync_reader_get")
READERS = (1, 2, 4, 8)
MEMORY = ("dns_memory", "baseline_memory.json")
ARTIFACTS = [f for f, _ in TIMED.values()] + [CONCURRENT[1], MEMORY[1]]

ESTIMATOR_RATIO = (
    "paired per round: the first-named arm's ns per operation over the second's, so a "
    "value below 1 means the first-named arm is faster. The point is the geometric mean "
    "of the per-round ratios and the interval is BCa 95% on the per-round log ratios "
    "(2000 resamples), which is not the quotient of the two per-arm medians. In the "
    "concurrent artifact `scaling` is throughput at W readers over throughput at 1 "
    "reader, paired per round, mean with BCa 95% on the per-round quotients, and "
    "`clone_per_reader_over_sync_locked` is a throughput ratio (above 1 = the cloned "
    "readers do more)")
ESTIMATOR_COLUMNS = (
    "`ns_per_op` and `ops_per_s` are medians over the cell's round processes of that "
    "round's one timed pass; the memory census publishes exact requested bytes, one "
    "count per cell")
ESTIMATOR_RAW = (
    "every timed cell carries rounds_raw, one row per (round process, arm) with that "
    "process's load window; the memory census (under `memory`) has no rounds")


def interval(samples: list[float], log: bool) -> dict:
    """Interval of `samples`: the mean, or with `log` the geometric mean (BCa on the logs)."""
    data = [math.log(s) for s in samples] if log else list(samples)
    back = math.exp if log else (lambda x: x)
    if len(data) < MIN_ROUNDS:
        return {"point": back(math.fsum(data) / len(data)), "ci_lower": None,
                "ci_upper": None, "ci_method": None, "n": len(data),
                "why_no_interval": f"fewer than {MIN_ROUNDS} rounds; BCa needs n >= 3"}
    point, lo, hi, ci_method = bca_bootstrap_ci_with_method(data, CONFIDENCE, RESAMPLES, SEED)
    return {"point": back(point), "ci_lower": back(lo), "ci_upper": back(hi),
            "ci_method": ci_method, "n": len(data)}


def by_round(rows: list[dict], arm: str, field: str) -> dict[int, float]:
    """`{round: value}` of `field` over `arm`'s rows; a round measured twice is refused."""
    out = {}
    for r in rows:
        if r["arm"] != arm:
            continue
        if r["round"] in out:
            raise RuntimeError(f"{arm}/{field}: round {r['round']} measured twice")
        out[r["round"]] = float(r[field])
    return out


def paired(num: dict[int, float], den: dict[int, float], what: str, log: bool) -> dict:
    """Interval of the per-round quotient `num / den` over the rounds both carry."""
    rounds = sorted(set(num) & set(den))
    if not rounds or len(rounds) != len(num) or len(rounds) != len(den):
        raise RuntimeError(f"{what}: the two arms were not measured in the same rounds")
    for rd in rounds:
        if num[rd] <= 0.0 or den[rd] <= 0.0:
            raise RuntimeError(f"{what}: round {rd} has a non-positive sample")
    return interval([num[rd] / den[rd] for rd in rounds], log)


def worst_window(rows: list[dict]) -> dict:
    """The cell's load: the largest host and foreign busy-CPU figure over its round processes."""
    def top(key):
        vals = [r["load"].get(key) for r in rows if isinstance(r.get("load"), dict)]
        vals = [v for v in vals if isinstance(v, (int, float)) and not isinstance(v, bool)]
        return max(vals) if vals else None
    return {"aggregation": "maximum over the cell's round processes",
            "busy_cpus_since_prev": top("busy_cpus_since_prev"),
            "own_busy_cpus": top("own_busy_cpus"),
            "foreign_busy_cpus": top("foreign_busy_cpus")}


def merge_cells(runs: list, list_key: str, key_fields: tuple) -> list[dict]:
    """One cell list from `(payload, window)` per round process, rows carrying their window."""
    first = runs[0][0][list_key]
    cells = []
    for idx, cell in enumerate(first):
        key = tuple(cell[f] for f in key_fields)
        rows = []
        for payload, window in runs:
            other = payload[list_key][idx]
            if tuple(other[f] for f in key_fields) != key:
                raise RuntimeError(f"round processes disagree on the cells at index {idx}")
            # Exact fields (probe counts, hits) must not move between rounds.
            for f, v in cell.items():
                if f != "rounds_raw" and other[f] != v:
                    raise RuntimeError(f"{key}: `{f}` differs between round processes")
            rows.extend({**r, "load": window} for r in other["rounds_raw"])
        cells.append({**{k: v for k, v in cell.items() if k != "rounds_raw"},
                      "rounds_raw": rows})
    return cells


def annotate_timed(bench: str, cells: list[dict]) -> None:
    """Per-arm medians and the paired ratios of one timed harness's cells, in place."""
    arms = TIMED[bench][1]
    for cell in cells:
        series = {a: by_round(cell["rounds_raw"], a, "ns_per_op") for a in arms}
        cell["ns_per_op"] = {a: statistics.median(series[a].values()) for a in arms}
        pairs = [(SUBJECT, a) for a in arms if a != SUBJECT] + list(EXTRA_PAIRS.get(bench, ()))
        cell["ratios"] = {f"{n}_over_{d}": paired(series[n], series[d], f"{bench} {n}/{d}", True)
                          for n, d in pairs}
        cell["rounds"] = len(series[SUBJECT])


def annotate_concurrent(cells: list[dict]) -> dict:
    """S(W) per cell in place; returns the cloned-over-locked throughput ratio at each W."""
    series = {(c["arm"], c["readers"]): by_round(c["rounds_raw"], c["arm"], "ops_per_s")
              for c in cells}
    for cell in cells:
        arm, w = cell["arm"], cell["readers"]
        cell["ops_per_s"] = statistics.median(series[(arm, w)].values())
        cell["scaling"] = paired(series[(arm, w)], series[(arm, 1)], f"S({w}) {arm}", False)
        cell["rounds"] = len(series[(arm, w)])
        cell["load"] = worst_window(cell["rounds_raw"])
    return {str(w): paired(series[("clone_per_reader", w)], series[("sync_locked", w)],
                           f"clone/locked W={w}", True)
            for w in sorted({c["readers"] for c in cells})}


def build_artifact(bench: str, runs: list, prov: dict) -> dict:
    """Merges and annotates one harness's `(payload, window)` runs into its artifact."""
    head = {k: v for k, v in runs[0][0].items()
            if k not in ("cells", "throughput", "memory", "round")}
    if bench in TIMED:
        cells = merge_cells(runs, "cells", ("population", "order", "probe_class"))
        annotate_timed(bench, cells)
        return attach({**head, "rounds": len(runs), "cells": cells}, prov)
    if bench == CONCURRENT[0]:
        cells = merge_cells(runs, "throughput", ("arm", "readers"))
        ratio = annotate_concurrent(cells)
        return attach({**head, "rounds": len(runs), "throughput": cells,
                       "clone_per_reader_over_sync_locked": ratio}, prov)
    (payload, window), = runs
    art = attach({**head, "memory": payload["memory"]}, prov)
    art["load"] = window
    return art


def derived_bounds(populations: list[int]) -> list[dict]:
    """What `scripts/dns_name_bounds.py` derives for the suite's names (prediction E1)."""
    import dns_name_bounds as b  # noqa: PLC0415 -- CPU-heavy; imported after the last timed process
    out = []
    for n in populations:
        names = b.gen_names(n)
        nul_total = sum(len(b.key_nul(nm)) for nm in names)
        lo, hi = b.btreemap_bytes_bounds(n, nul_total)
        row = {"population": n, "key_nul_bytes": nul_total,
               "btreemap_bytes_bounds": [lo, hi]}
        for arm, enc in (("expanse_direct", "direct"), ("expanse_ordered", "ordered_bytes_stored")):
            c = b.strmap_census([b.ENCODERS[enc](nm) for nm in names])
            row[arm] = {"shells": c["shells"], "suffixes": c["suffixes"],
                        "over_submaps": c["over_submaps"],
                        "mem_used_lower_bound": b.mem_used_lower_bound(c)}
        out.append(row)
    return out


def parse_payload(bench: str, stdout: str) -> dict:
    start = stdout.find("{")
    if start == -1:
        print(f"{bench} printed no JSON payload:\n{stdout}", file=sys.stderr)
        sys.exit(1)
    return json.loads(stdout[start:])


def harness_args(bench: str, quick: bool, rnd: int) -> list[str]:
    args = (["--quick"] if quick else []) + ["--json"]
    return args if bench == MEMORY[0] else args + ["--round", str(rnd)]


def plan(quick: bool, rounds: int | None) -> list[tuple[str, int]]:
    """(harness, round processes), timed harnesses first and the census last."""
    def n(default):
        return rounds if rounds is not None else (ROUNDS_QUICK if quick else default)
    return ([(b, n(ROUNDS)) for b in TIMED] + [(CONCURRENT[0], n(ROUNDS_CONCURRENT))]
            + [(MEMORY[0], 1)])


def artifact_name(bench: str) -> str:
    if bench in TIMED:
        return TIMED[bench][0]
    return CONCURRENT[1] if bench == CONCURRENT[0] else MEMORY[1]


def run_full(prov: dict, steps: list[tuple[str, int]]) -> dict:
    """Every harness, one load window per process: `{harness: [(payload, window)]}`."""
    out = {}
    for bench, n in steps:
        print(f"==> {bench} ({n} process(es))", flush=True)
        out[bench] = []
        for r in range(n):
            res, window = bench_windowed.run_bench_window(
                prov, "expanse-trie", bench, harness_args(bench, False, r),
                label=f"{bench}/round={r}")
            if res.returncode != 0:
                print(f"{bench} round {r} failed:\n{res.stderr}", file=sys.stderr)
                sys.exit(1)
            out[bench].append((parse_payload(bench, res.stdout), window))
    return out


def run_quick(prov: dict, steps: list[tuple[str, int]]) -> dict:
    """Every harness's processes inside one load window (a quick process is too short for one)."""
    built = {b: bench_windowed.build("expanse-trie", "bench", b) for b, _ in steps}
    label = "quick/all-harnesses"
    start = begin_cell(prov, f"cell:{label}")
    outs = {}
    for bench, n in steps:
        exe, pkg_dir = built[bench]
        print(f"==> {bench} ({n} process(es), quick)", flush=True)
        outs[bench] = []
        for r in range(n):
            argv = bench_windowed.harness_argv(exe, "bench", harness_args(bench, True, r))
            res = subprocess.run(argv, cwd=pkg_dir, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, text=True)
            if res.returncode != 0:
                print(f"{bench} round {r} failed:\n{res.stderr}", file=sys.stderr)
                sys.exit(1)
            outs[bench].append(parse_payload(bench, res.stdout))
    window = end_cell(start)
    prov["load_windows"] = True
    prov.setdefault("windows", []).append({"id": label, "load": window})
    return {b: [(p, window) for p in ps] for b, ps in outs.items()}


def summarize(bench: str, art: dict) -> None:
    """The console summary a pull request comment shows. The artifact is the record."""
    def fmt(iv):
        if iv["ci_lower"] is None:
            return f"{iv['point']:.3f} (no interval)"
        return f"{iv['point']:.3f} [{iv['ci_lower']:.3f}, {iv['ci_upper']:.3f}] ({iv['ci_method']})"
    for c in art.get("cells", []):
        print(f"  {bench} n={c['population']} order={c['order']} class={c['probe_class']}")
        for arm, ns in c["ns_per_op"].items():
            print(f"      {arm:40} {ns:10.2f} ns/op")
        for name, iv in c["ratios"].items():
            print(f"      {name:40} {fmt(iv)}")
    for c in art.get("throughput", []):
        print(f"  {bench} {c['arm']:17} W={c['readers']} {c['ops_per_s'] / 1e6:9.3f} Mops/s  "
              f"S(W) {fmt(c['scaling'])}")
    for w, iv in art.get("clone_per_reader_over_sync_locked", {}).items():
        print(f"  {bench} clone_per_reader / sync_locked throughput at W={w}: {fmt(iv)}")
    for r in art.get("memory", []):
        print(f"  {bench} n={r['population']} order={r['order']:8} {r['arm']:16} "
              f"{r['bytes_per_name']:8.2f} B/name requested")


def write_all(runs: dict, prov: dict, out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    for bench, pairs in runs.items():
        art = build_artifact(bench, pairs, prov)
        summarize(bench, art)
        path = out_dir / artifact_name(bench)
        path.write_text(json.dumps(art, indent=2) + "\n", encoding="utf-8")
        print(f"    wrote {path}")


def add_bounds(out_dir: Path) -> None:
    """Adds the derived bounds to the written memory artifact, after every timed process."""
    path = out_dir / MEMORY[1]
    art = json.loads(path.read_text(encoding="utf-8"))
    pops = sorted({r["population"] for r in art["memory"]})
    art["derived"] = {"source": "scripts/dns_name_bounds.py", "bounds": derived_bounds(pops)}
    path.write_text(json.dumps(art, indent=2) + "\n", encoding="utf-8")


def judge_written(out_dir: Path) -> list[str]:
    written = [(f, json.loads((out_dir / f).read_text(encoding="utf-8")))
               for f in ARTIFACTS if (out_dir / f).is_file()]
    return bench_windowed.judge(SUITE, written)


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------

def _synthetic(bench: str, rd: int) -> dict:
    """What one round process of `bench` prints: round `rd` alone."""
    head = {"benchmark": bench, "workload_id": bench, "quick": True, "round": rd,
            "min_window_ns": 20_000_000}
    if bench in TIMED:
        arms = TIMED[bench][1]
        rows = [{"round": rd, "arm": a, "ns_per_op": 100.0 * (k + 1) + rd * (k + 1),
                 "reps": 3, "position": k} for k, a in enumerate(arms)]
        return {**head, "cells": [{"population": 1000, "order": "sorted",
                                   "probe_class": "x", "probes": 500, "rounds_raw": rows}]}
    if bench == CONCURRENT[0]:
        cells = [{"population": 1000, "order": "sorted", "arm": a, "readers": w,
                  "probe_class": "x",
                  "rounds_raw": [{"round": rd, "arm": a, "readers": w, "window_s": 0.1,
                                  "cpus_allowed": "0-15",
                                  "ops_per_s": (1e6 + rd) * (w if a != "sync_locked" else 1)}]}
                 for a in CONCURRENT_ARMS for w in READERS]
        return {**head, "throughput": cells}
    return {**head, "memory": [{"population": 1000, "order": "sorted", "arm": "btreemap",
                                "requested_bytes": 80000, "allocations": 1100,
                                "bytes_per_name": 80.0}]}


def self_test() -> int:
    """The merge, annotation and stamping path, judged by the gate without running a bench."""
    prov = {"suite": SUITE, "issue": ISSUE, "commit": "abc1234", "host": host_facts(),
            "estimators": estimators(ESTIMATOR_RATIO, ESTIMATOR_COLUMNS, ESTIMATOR_RAW),
            "loads": []}
    add_load(prov, "start")
    window = {"since": "cell:x", "wall_s": 3.0, "busy_cpus_since_prev": 1.0,
              "own_busy_cpus": 0.99, "foreign_busy_cpus": 0.01}
    prov["load_windows"] = True
    prov["windows"] = [{"id": "x", "load": window}]
    prov["loads"].append({"label": "end", "busy_cpus_since_prev": 1.0})
    # The concurrent artifact owes the per-CPU governor map (the gate's
    # attribution rule); off Linux `host_facts` cannot read one.
    prov["host"].setdefault("scaling_governor_by_cpu", {"0": "performance"})
    prov["host"].setdefault("scaling_governor_pin_set", "0")

    benches = [b for b, _ in plan(True, None)]
    assert [artifact_name(b) for b in benches] == ARTIFACTS, "every harness has its artifact"
    arts = {}
    for bench in benches:
        n = 1 if bench == MEMORY[0] else 3
        art = build_artifact(bench, [(_synthetic(bench, rd), window) for rd in range(n)], prov)
        got = bench_windowed.judge(SUITE, [(artifact_name(bench), art)])
        assert got == [], f"{bench}: the harness's shape must pass the gate: {got}"
        arts[bench] = art

    # A paired ratio is formed per round, as a geometric mean: subject
    # 100 + rd against btreemap (third arm) 300 + 3 rd is exactly 1/3 each round.
    cell = arts["dns_get"]["cells"][0]
    r = cell["ratios"]["expanse_ordered_over_btreemap"]
    assert abs(r["point"] - 1 / 3) < 1e-12 and r["ci_method"] is not None, r
    assert cell["ns_per_op"]["expanse_ordered"] == 101.0, cell["ns_per_op"]
    assert all(row["load"] == window for row in cell["rounds_raw"]), "each row carries its window"
    assert "expanse_ordered_owned_over_btreemap" in arts["dns_predecessor"]["cells"][0]["ratios"]
    # S(W): the cloned readers scale with W, the locked ones do not.
    by = {(c["arm"], c["readers"]): c for c in arts[CONCURRENT[0]]["throughput"]}
    assert abs(by[("clone_per_reader", 8)]["scaling"]["point"] - 8.0) < 1e-9
    assert abs(by[("sync_locked", 8)]["scaling"]["point"] - 1.0) < 1e-9
    assert by[("sync_locked", 8)]["load"]["foreign_busy_cpus"] == 0.01
    ratio = arts[CONCURRENT[0]]["clone_per_reader_over_sync_locked"]["8"]
    assert abs(ratio["point"] - 8.0) < 1e-9, ratio
    assert "load" in arts[MEMORY[0]], "the census carries its one window"

    # Fewer than three rounds carry no interval and say why.
    short = interval([1.0, 2.0], True)
    assert short["ci_lower"] is None and "why_no_interval" in short, short
    # A round measured by two processes, or arms measured in different rounds,
    # cannot be paired; a moved exact field is refused.
    try:
        build_artifact("dns_get", [(_synthetic("dns_get", 0), window)] * 3, prov)
        raise AssertionError("a round measured twice must raise")
    except RuntimeError:
        pass
    try:
        paired({0: 1.0, 1: 1.0, 2: 1.0}, {0: 1.0, 1: 1.0}, "unpaired", True)
        raise AssertionError("an unpaired ratio must raise")
    except RuntimeError:
        pass
    moved = [(_synthetic("dns_get", rd), window) for rd in range(3)]
    moved[2][0]["cells"][0]["probes"] += 1
    try:
        build_artifact("dns_get", moved, prov)
        raise AssertionError("an exact field that moves between rounds must raise")
    except RuntimeError:
        pass
    # A payload without rounds is named by the gate rather than written quietly.
    bare = attach({"cells": [{"population": 1, "ns_per_op": {}}]}, prov)
    got = bench_windowed.judge(SUITE, [("baseline_get.json", bare)])
    assert any("rounds_raw" in g for g in got), f"a cell with no rounds must be a finding: {got}"

    # The derived bounds are the script's own, at a population its tests pin.
    with tempfile.TemporaryDirectory() as tmp:
        (Path(tmp) / MEMORY[1]).write_text(json.dumps(arts[MEMORY[0]]), encoding="utf-8")
        add_bounds(Path(tmp))
        d = json.loads((Path(tmp) / MEMORY[1]).read_text())["derived"]["bounds"][0]
        assert d["population"] == 1000 and d["expanse_ordered"]["shells"] > 0, d
    print("dns_names run_all.py --self-test: all checks passed")
    return 0


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.strip().splitlines()[0])
    ap.add_argument("--quick", action="store_true", help="smoke run into results/quick/ (gitignored)")
    ap.add_argument("--rounds", type=int, help="round processes per timed harness")
    ap.add_argument("--host-desc", default="", help="anonymized host description (never a hostname)")
    ap.add_argument("--run-id", default="", help="CI run URL this artifact came from")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        sys.exit(self_test())
    if args.rounds is not None and args.rounds < 1:
        sys.exit("--rounds takes a positive integer")
    quick = args.quick
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR
    steps = plan(quick, args.rounds)

    pin = os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset")
    prov = {
        "suite": SUITE,
        "issue": ISSUE,
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "estimators": estimators(ESTIMATOR_RATIO, ESTIMATOR_COLUMNS, ESTIMATOR_RAW),
        "core_pin": pin,
        "host_description": args.host_desc or None,
        "run_id": args.run_id or None,
        "quick": quick,
        "rounds_planned": dict(steps),
        "loads": [],
    }
    # Build every harness before the first snapshot, so compilation is not
    # running beside, or immediately before, a timed process.
    for bench, _ in steps:
        bench_windowed.build("expanse-trie", "bench", bench)
    add_load(prov, "start")
    runs = run_quick(prov, steps) if quick else run_full(prov, steps)
    add_load(prov, "end")
    write_all(runs, prov, out_dir)
    # Python CPU, so only after the last timed process and the closing snapshot.
    add_bounds(out_dir)
    # Each artifact was stamped before the bounds step; re-stamp so the
    # provenance block is the same object in all five.
    rewrite((out_dir / f for f in ARTIFACTS), prov)

    findings = judge_written(out_dir)
    print(f"==> check_bench_provenance: {len(ARTIFACTS)} artifact(s), {len(findings)} finding(s)")
    if findings and platform.system() != "Linux":
        # Off Linux there is no /proc, so no load window can be measured and
        # the gate's findings say exactly that. A run there is a smoke run.
        for f in findings:
            print(f"::notice::not admissible off Linux: {f}")
        if not quick:
            sys.exit("a full run needs /proc load windows: run it on the Linux reference host")
        return
    for f in findings:
        print(f"::error::{f}", file=sys.stderr)
    if findings:
        sys.exit(1)


if __name__ == "__main__":
    main()
