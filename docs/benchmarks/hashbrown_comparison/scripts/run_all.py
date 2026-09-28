#!/usr/bin/env python3
"""
Master benchmark runner for Hashbrown / SwissTable vs BTreeMap vs ExpanseMap.

Executes all 5 benchmark harnesses:
1. hashbrown_native_suite (Criterion Native port)
2. hashbrown_ycsb (YCSB A-F workloads, through scripts/ycsb_bench.py)
3. hashbrown_tail_latency (HdrHistogram P50-P99.99)
4. hashbrown_container_dists (Ankerl/Tessil key distributions)
5. hashbrown_memory_alloc (GlobalAlloc live heap tracking)

Saves JSON outputs into results/ and regenerates SVG comparison charts.

Each single-process pillar runs its arms over paired rounds (the arm order
rotating per round) and emits every round under a cell key the provenance gate
reads (`cells`, `latency`, `throughput`), or, for the memory census, under
`memory`. This runner adds to every timed cell a BCa 95% interval per arm and
metric and a paired per-round ratio interval of Expanse against each
competitor (`annotate`).

Each single-process pillar is built before its window opens and runs as one
process inside one load window (`bench_windowed.run_bench_window`), stored on
the artifact it wrote (`load`) and in `provenance.windows`; a closing snapshot
follows, and every artifact is re-stamped with the whole series. The written
artifacts are then judged by `check_bench_provenance.findings_for`, and a
finding fails the run (AGENTS.md section 8.17, #1214).

`--skip-ycsb` re-measures the four single-process pillars and leaves the YCSB
pillar's committed runs as they are: its driver owns its rounds and artifact,
and a re-run of it is a separate measurement with its own two runs.

  run_all.py [--quick] [--skip-ycsb]
  run_all.py --self-test
"""

import os
import sys
import json
import subprocess
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
# `docs/benchmarks/<suite>` -> the repo root is three levels up, as in every
# other runner. This read `.parent.parent` and resolved to `docs/`; cargo walks
# up to find a manifest, so the sweep still ran and it went unnoticed.
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"
SCRIPTS_DIR = BASE_DIR / "scripts"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bench_provenance import (  # noqa: E402
    add_load, attach, estimators, git_sha, host_facts, rewrite,
)
import bench_windowed  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

SUITE = "hashbrown_comparison"

CONFIDENCE = 0.95
RESAMPLES = 2000
SEED = 42
#: BCa needs n >= 3 (`scripts/bca_bootstrap.py`).
MIN_ROUNDS = 3

ARMS = ("expanse", "hashbrown", "btree")
SUBJECT = "expanse"
COMPETITORS = ("hashbrown", "btree")

ESTIMATOR_RATIO = (
    "paired per round: for throughput (`ratios`), Expanse over the named competitor, "
    "above 1 = Expanse faster; for tail latency (`ratio_over_expanse`), the competitor "
    "over Expanse, above 1 = Expanse lower. The point is the mean of the per-round "
    "quotients with its BCa 95% interval, which is not the quotient of the two per-arm "
    "means")
ESTIMATOR_COLUMNS = (
    "each published per-arm figure (mops, ns_per_op, mops_items, insert_mops, "
    "lookup_mops, and the tail-latency percentiles) is the mean over the cell's rounds "
    "of that round's value; `intervals` holds the same mean with its BCa 95% interval. "
    "The memory census publishes exact live-heap bytes per key, one count per cell")
ESTIMATOR_RAW = (
    "every timed cell carries rounds_raw, the per-round rows verbatim; the memory "
    "census (under `memory`) has no rounds")


BENCHES = [
    ("hashbrown_native_suite", "baseline_native.json"),
    ("hashbrown_tail_latency", "baseline_tail_latency.json"),
    ("hashbrown_container_dists", "baseline_distributions.json"),
    ("hashbrown_memory_alloc", "baseline_memory.json"),
]

def run_ycsb(quick: bool) -> None:
    """The YCSB pillar goes through `scripts/ycsb_bench.py` (#1005).

    One `cargo bench` pass per cell cannot carry an interval. The driver owns
    the rounds, the per-round load snapshots, the BCa intervals and the paired
    ratios, judges its artifact by `check_bench_provenance.py` before writing
    it, and confines `--quick` to `results/quick/` itself.
    """
    print(f"==> Running benchmark: hashbrown_ycsb through scripts/ycsb_bench.py (quick={quick})...")
    cmd = [sys.executable, str(REPO_ROOT / "scripts" / "ycsb_bench.py"), "--suite", "hashbrown"]
    if quick:
        cmd.append("--quick")
    res = subprocess.run(cmd, cwd=REPO_ROOT)
    if res.returncode != 0:
        print("Error running the hashbrown YCSB driver", file=sys.stderr)
        sys.exit(1)


def interval(samples: list[float]) -> dict:
    """Mean of `samples`, its BCa 95% interval and the construction that produced it."""
    if len(samples) < MIN_ROUNDS:
        return {"point": sum(samples) / len(samples), "ci_lower": None, "ci_upper": None,
                "ci_method": None,
                "why_no_interval": f"fewer than {MIN_ROUNDS} rounds; BCa needs n >= 3"}
    point, lo, hi, ci_method = bca_bootstrap_ci_with_method(
        samples, CONFIDENCE, RESAMPLES, SEED)
    return {"point": point, "ci_lower": lo, "ci_upper": hi, "ci_method": ci_method}


def _by_round(rows: list[dict], arm: str, field: str, **match) -> dict[int, float]:
    """`{round: value}` of `field` over `arm`'s rows that match every `match` item."""
    out = {}
    for r in rows:
        if r["arm"] == arm and all(r.get(k) == v for k, v in match.items()):
            if r["round"] in out:
                raise RuntimeError(f"{arm}/{field} {match}: round {r['round']} measured twice")
            out[r["round"]] = float(r[field])
    return out


def _paired(num: dict[int, float], den: dict[int, float], what: str) -> dict:
    """Interval of the per-round quotient `num / den` over the rounds both carry."""
    rounds = sorted(set(num) & set(den))
    if len(rounds) != len(num) or len(rounds) != len(den):
        raise RuntimeError(f"{what}: the two arms were not measured in the same rounds")
    for rd in rounds:
        if den[rd] <= 0.0:
            raise RuntimeError(f"{what}: round {rd} has a non-positive denominator")
    return interval([num[rd] / den[rd] for rd in rounds])


def annotate_throughput(rows: list[dict], metrics: dict[str, tuple[str, dict]]) -> dict:
    """`{"intervals": ..., "ratios": ...}` for throughput rows (higher is better).

    `metrics` maps a published name to `(row field, row filter)`.
    """
    intervals, ratios = {}, {}
    for name, (field, match) in metrics.items():
        series = {arm: _by_round(rows, arm, field, **match) for arm in ARMS}
        intervals[name] = {arm: interval([v for _, v in sorted(series[arm].items())])
                           for arm in ARMS}
        ratios[name] = {f"{SUBJECT}_over_{c}": _paired(series[SUBJECT], series[c],
                                                       f"{name} {SUBJECT}/{c}")
                        for c in COMPETITORS}
    return {"intervals": intervals, "ratios": ratios}


NATIVE_METRICS = {
    "lookup_hit": ("mops", {"op": "lookup_hit"}),
    "lookup_miss": ("mops", {"op": "lookup_miss"}),
    "iter_all": ("mops_items", {"op": "iter_all"}),
    "insert_growing": ("mops", {"op": "insert_growing"}),
}
DIST_METRICS = {
    "insert_mops": ("insert_mops", {}),
    "lookup_mops": ("lookup_mops", {}),
}
PERCENTILES = ("p50_ns", "p75_ns", "p90_ns", "p95_ns", "p99_ns", "p99_9_ns", "p99_99_ns",
               "max_ns")


def annotate(bench_name: str, payload):
    """Adds intervals and paired ratios to every timed cell of a harness payload.

    The memory census has no rounds and is returned unchanged.
    """
    if bench_name == "hashbrown_native_suite":
        for cell in payload:
            cell.update(annotate_throughput(cell["rounds_raw"], NATIVE_METRICS))
    elif bench_name == "hashbrown_container_dists":
        for cell in payload["throughput"]:
            cell.update(annotate_throughput(cell["rounds_raw"], DIST_METRICS))
    elif bench_name == "hashbrown_tail_latency":
        cells = {c["arm"]: c for c in payload["latency"]}
        series = {arm: {q: {r["round"]: float(r[q]) for r in cells[arm]["rounds_raw"]}
                        for q in PERCENTILES} for arm in ARMS}
        for arm in ARMS:
            cells[arm]["intervals"] = {q: interval([v for _, v in sorted(series[arm][q].items())])
                                       for q in PERCENTILES}
        for c in COMPETITORS:
            cells[c]["ratio_over_expanse"] = {
                q: _paired(series[c][q], series[SUBJECT][q], f"{q} {c}/{SUBJECT}")
                for q in PERCENTILES}
    elif bench_name != "hashbrown_memory_alloc":
        raise ValueError(f"no annotation defined for {bench_name!r}")
    return payload


def stamp(parsed, prov: dict, window: dict) -> dict:
    """The artifact a bench's payload is written as: provenance, and its window."""
    out = attach(parsed, prov)
    # The load window of the bench process that produced this file.
    out["load"] = window
    return out


def run_bench(bench_name: str, out_file: str, out_dir: Path, prov: dict, quick: bool = False):
    print(f"==> Running benchmark: {bench_name} (quick={quick})...")
    args = ["--quick"] if quick else []
    args.append("--json")
    res, window = bench_windowed.run_bench_window(prov, "expanse-trie", bench_name, args)
    if res.returncode != 0:
        print(f"Error running {bench_name}:", file=sys.stderr)
        print(res.stderr, file=sys.stderr)
        sys.exit(1)

    # Extract JSON string from stdout (ignoring any cargo compilation banners)
    stdout = res.stdout.strip()
    json_start = stdout.find("[")
    if json_start == -1 or (stdout.find("{") != -1 and stdout.find("{") < json_start):
        json_start = stdout.find("{")

    if json_start == -1:
        print(f"Failed to find JSON payload in {bench_name} output:\n{stdout}", file=sys.stderr)
        sys.exit(1)

    json_str = stdout[json_start:]
    parsed = annotate(bench_name, json.loads(json_str))

    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / out_file
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(stamp(parsed, prov, window), f, indent=2)
    print(f"    Saved results to {out_path}  (window: {window})")


def judge_written(out_dir: Path) -> int:
    """Judges every written single-process artifact by the provenance gate.

    The YCSB driver judges its own artifact before writing it.
    """
    written = []
    for _, out_file in BENCHES:
        path = out_dir / out_file
        if path.is_file():
            written.append((out_file, json.loads(path.read_text(encoding="utf-8"))))
    findings = bench_windowed.judge(SUITE, written)
    for f in findings:
        print(f"::error::{f}", file=sys.stderr)
    print(f"==> check_bench_provenance: {len(written)} artifact(s), {len(findings)} finding(s)")
    return len(findings)


def _synthetic_payloads() -> dict:
    """One payload per harness, in the shape each emits, with three rounds."""
    rounds = range(3)

    def lat_row(arm, rd):
        base = {"expanse": 70, "hashbrown": 25, "btree": 110}[arm] + rd
        return {"round": rd, **{q: base * (i + 1) for i, q in enumerate(PERCENTILES)}}

    native_rows = [{"round": rd, "op": op, "arm": arm, field: 10.0 + rd + k}
                   for rd in rounds for op, (field, _) in
                   ((m, NATIVE_METRICS[m]) for m in NATIVE_METRICS)
                   for k, arm in enumerate(ARMS)]
    dist_rows = [{"round": rd, "arm": arm, "insert_mops": 20.0 + rd + k,
                  "lookup_mops": 40.0 + rd * k}
                 for rd in rounds for k, arm in enumerate(ARMS)]
    return {
        "hashbrown_native_suite": [{"population": 10_000, "rounds": 3,
                                    "rounds_raw": native_rows}],
        "hashbrown_tail_latency": {
            "total_inserts": 100, "mode": "un_preallocated_dynamic_growth", "rounds": 3,
            **{arm: {q: 1.0 for q in PERCENTILES} for arm in ARMS},
            "latency": [{"arm": arm, "rounds_raw": [lat_row(arm, rd) for rd in rounds]}
                        for arm in ARMS]},
        "hashbrown_container_dists": {
            "uniform": {"distribution": "uniform", "population": 100},
            "rounds": 3,
            "throughput": [{"distribution": "uniform", "population": 100,
                            "rounds_raw": dist_rows}]},
        "hashbrown_memory_alloc": {"memory": [{"population": 1000,
                                               "random_keys_bytes_per_key": {"expanse": 1.0}}]},
    }


def self_test() -> int:
    """The runner's stamping path, judged by the gate without running a bench."""
    prov = {"suite": SUITE, "commit": "abc1234", "host": host_facts(),
            "estimators": estimators("synthetic"), "loads": []}
    add_load(prov, "start")
    window = {"since": "cell:hashbrown_tail_latency", "wall_s": 3.0,
              "busy_cpus_since_prev": 1.0, "own_busy_cpus": 0.99, "foreign_busy_cpus": 0.01}
    prov["load_windows"] = True
    prov["windows"] = [{"id": "hashbrown_tail_latency", "load": window}]
    prov["loads"].append({"label": "end", "busy_cpus_since_prev": 1.0})

    # Every harness's shape, annotated and stamped as `run_bench` does, passes
    # the gate that the committed artifacts are held to.
    payloads = _synthetic_payloads()
    assert set(payloads) == {b for b, _ in BENCHES}, "a synthetic payload per harness"
    for bench_name, out_file in BENCHES:
        art = stamp(annotate(bench_name, payloads[bench_name]), prov, window)
        got = bench_windowed.judge(SUITE, [(out_file, art)])
        assert got == [], f"{bench_name}: the harness's shape must pass the gate: {got}"
        assert art.get("load") == window, "the window must be stored on the artifact it timed"

    # The annotations: every interval names its construction, and a paired
    # ratio is formed per round, not from the two means.
    native = annotate("hashbrown_native_suite", _synthetic_payloads()["hashbrown_native_suite"])[0]
    iv = native["intervals"]["lookup_hit"]["expanse"]
    assert iv["ci_method"] is not None and iv["ci_lower"] <= iv["point"] <= iv["ci_upper"], iv
    r = native["ratios"]["lookup_hit"]["expanse_over_btree"]
    # expanse = 10 + rd, btree = 12 + rd: the mean of the per-round quotients.
    want = sum((10 + rd) / (12 + rd) for rd in range(3)) / 3
    assert abs(r["point"] - want) < 1e-12, (r, want)
    tail = annotate("hashbrown_tail_latency", _synthetic_payloads()["hashbrown_tail_latency"])
    hb = next(c for c in tail["latency"] if c["arm"] == "hashbrown")
    want = sum((25 + rd) / (70 + rd) for rd in range(3)) / 3
    assert abs(hb["ratio_over_expanse"]["p50_ns"]["point"] - want) < 1e-12, hb
    assert "ratio_over_expanse" not in next(c for c in tail["latency"] if c["arm"] == "expanse")
    # Fewer than three rounds carry no interval, and say why.
    short = interval([1.0, 2.0])
    assert short["ci_lower"] is None and "why_no_interval" in short, short
    # Two arms measured in different rounds cannot be paired.
    try:
        _paired({0: 1.0, 1: 1.0, 2: 1.0}, {0: 1.0, 1: 1.0}, "unpaired")
        raise AssertionError("an unpaired ratio must raise")
    except RuntimeError:
        pass

    # A payload without rounds — the shape the harnesses emitted before
    # #1214's follow-up — is named by the gate rather than written quietly.
    got = bench_windowed.judge(SUITE, [("baseline_native.json",
                                        stamp([{"population": 1, "lookup_hit": {}}],
                                              prov, window))])
    assert any("rounds_raw" in g for g in got), f"a payload with no rounds must be a finding: {got}"
    got = bench_windowed.judge(SUITE, [("baseline_tail_latency.json",
                                        stamp({"expanse": {"p99_ns": 1.0}}, prov, window))])
    assert any("no cell list" in g for g in got), f"a payload under no cell key must be a finding: {got}"
    print("hashbrown_comparison run_all.py --self-test: all checks passed")
    return 0

def main():
    if "--self-test" in sys.argv:
        sys.exit(self_test())
    quick = "--quick" in sys.argv or "-q" in sys.argv
    print(f"Starting Hashbrown vs BTreeMap vs Expanse benchmark suite (quick={quick})...\n")

    # A --quick run produces reduced-sweep smoke data. Route it to the
    # gitignored results/quick/ scratch dir so it can never overwrite the
    # committed results/baseline_*.json — the corruption class fixed for the
    # llm_inference suite in #352.
    out_dir = RESULTS_DIR / "quick" if quick else RESULTS_DIR

    prov = {
        "suite": "hashbrown_comparison",
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "estimators": estimators(ESTIMATOR_RATIO, ESTIMATOR_COLUMNS, ESTIMATOR_RAW),
        "core_pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED", "unset"),
        "quick": quick,
        "loads": [],
    }
    add_load(prov, "start")

    for bench_name, out_file in BENCHES:
        run_bench(bench_name, out_file, out_dir, prov, quick=quick)

    # After the single-process pillars: the driver takes its own load snapshots
    # around each of its rounds and writes its own provenance block.
    if "--skip-ycsb" in sys.argv:
        print("==> Skipping the YCSB pillar (--skip-ycsb); its committed runs are unchanged.")
    else:
        run_ycsb(quick)

    add_load(prov, "end")
    # The artifacts were written inside the loop above, before this
    # snapshot existed; re-stamp so each carries the whole load series.
    rewrite((out_dir / f for _, f in BENCHES), prov)

    if quick:
        print("\n==> Skipping chart regeneration (--quick).")
        print(f"    Quick smoke results were written to {out_dir} (gitignored);")
        print("    the committed results/baseline_*.json and SVG charts were")
        print("    not touched. Regenerating the committed charts from")
        print("    reduced-sweep data would ship blank/mislabeled SVGs.")
    else:
        print("\n==> Generating SVG comparison charts...")
        subprocess.run([sys.executable, str(SCRIPTS_DIR / "generate_charts.py")], check=True)
        print("\nAll benchmarks and charts generated successfully!")
    if judge_written(out_dir):
        sys.exit(1)

if __name__ == "__main__":
    main()
