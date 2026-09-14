#!/usr/bin/env python3
"""Mixed read/write concurrency instrument over `crates/expanse/benches/concurrency.rs` (Refs #568).

The bench measures each engine and read/write mix in interleaved rounds of
500 ms windows: one window per thread count per round, in a Williams order,
each window started at a barrier after its threads' setup and divided by its
own elapsed time. This driver turns those windows into an artifact a gate can
be read from (`METHODOLOGY.md` §10.3):

- applies the reference-host core pin (`scripts/bench_pin.py`) before building
  or running anything;
- builds the bench once and runs it once per (engine, workload) group, with a
  load snapshot around each group (`bench_provenance.begin_cell` / `end_cell`);
- refuses a run whose windows miss a requested thread count, or whose rounds
  do not form a balanced Williams design (every thread count in every position,
  and after every other thread count, equally often);
- reports, per (engine, workload, threads) cell, the mean read, write and total
  ops/s with BCa 95% intervals (`scripts/bca_bootstrap.py`), their medians, and
  every window under `rounds_raw`;
- reports the scaling factor C(N) = T(N) / T(1), paired within each round, with
  its BCa 95% interval;
- refuses fewer than 15 rounds unless `--quick`, which writes under
  `results/quick/` and never to a committed artifact (AGENTS.md §8.5);
- runs `scripts/check_bench_provenance.py`'s own checks on the artifact before
  writing it.

Usage:
    python3 docs/benchmarks/concurrency/scripts/mixed_concurrency.py \\
        --out docs/benchmarks/concurrency/results/baseline_concurrent_mixed.json
    python3 docs/benchmarks/concurrency/scripts/mixed_concurrency.py --quick --engines map
    python3 docs/benchmarks/concurrency/scripts/mixed_concurrency.py --self-test
    python3 docs/benchmarks/concurrency/scripts/mixed_concurrency.py --write-assets RUN1,RUN2
    python3 docs/benchmarks/concurrency/scripts/mixed_concurrency.py --check-assets

`--self-test` runs without Cargo and is run by CI's `lint` job. The committed
artifact and its second run (`baseline_concurrent_mixed_run2.json`) also feed
the README hero chart's and the sync32 health chart's data
(`docs/assets/data/bench_assets.json`) and the architecture visualizer's
concurrency panel: `--write-assets` writes those blocks from the two runs,
citing their CI run ids, and `--check-assets`, also run by `lint`, fails when
they differ from what the runs produce.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
import bca_bootstrap  # noqa: E402
import check_bench_provenance  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402
from bench_provenance import add_load, begin_cell, end_cell, new_provenance  # noqa: E402

CI_METHODS = frozenset(
    v for k, v in vars(bca_bootstrap).items()
    if k.startswith("CI_METHOD_") and isinstance(v, str)
)

HARNESS = "crates/expanse/benches/concurrency.rs"
WORKLOAD_ID = "core_concurrency"
WINDOW_MS = 500
# METHODOLOGY.md §10.3: `benches/concurrency.rs` is an instrument only with at
# least 15 windows per cell.
MIN_ROUNDS = 15
ENGINE_KEYS = (
    "map", "set", "blob", "blob_mutex", "blob_rwlock_btree", "blob_skiplist",
    "str", "str_mutex", "bytes", "bytes_mutex", "str_dashmap", "sync32",
)
SYNC32 = "sync32"
DEFAULT_OUT = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "baseline_concurrent_mixed.json"
QUICK_OUT = REPO_ROOT / "results" / "quick" / "concurrent_mixed.json"
RATIO = (
    "C(N) = T(N) / T(1), paired within an interleaved round; T is (read + write) "
    "operations per second over one window's own elapsed time"
)
LOAD_SCOPE = (
    "engine and workload group: its thread counts run interleaved in one process, "
    "so every cell of the group carries the group's load"
)
SAMPLE_KEYS = (
    "workload_id", "engine_key", "engine", "workload", "read_pct", "write_rate",
    "threads", "round", "position", "elapsed_s", "read_ops", "write_ops",
    "busy", "ok", "refused",
)
RAW_KEYS = ("round", "position", "elapsed_s", "read_ops", "write_ops", "busy", "ok", "refused")


class InstrumentError(RuntimeError):
    """A run that cannot produce a publishable artifact (AGENTS.md §8.1)."""


def williams_period(n_levels: int) -> int:
    """Rounds in one full Williams cycle over `n_levels` thread counts."""
    return n_levels if n_levels % 2 == 0 else 2 * n_levels


def williams_order(n_levels: int, round_idx: int) -> list[int]:
    """Row `round_idx` of the Williams design `benches/concurrency.rs` runs.

    Used by the self-test to build synthetic windows; a real run is checked for
    the design's balance property instead, so this copy cannot mask a harness
    that orders its windows differently.
    """
    if n_levels == 0:
        return []
    base = [0 if k == 0 else ((k + 1) // 2 if k % 2 == 1 else n_levels - k // 2)
            for k in range(n_levels)]
    period = williams_period(n_levels)
    row = round_idx % period
    order = [(b + row % n_levels) % n_levels for b in base]
    if row >= n_levels:
        order.reverse()
    return order


def resolve_rounds(requested: int | None, n_levels: int, quick: bool) -> int:
    """The round count: a whole number of Williams cycles, at least 15 unless quick."""
    period = williams_period(n_levels)
    if requested is None:
        floor = 3 if quick else MIN_ROUNDS
        return period * -(-floor // period)
    if requested % period:
        raise InstrumentError(
            f"--rounds {requested} is not a whole number of Williams cycles "
            f"({period} rounds for {n_levels} thread counts); position and carryover "
            f"balance would be incomplete"
        )
    if not quick and requested < MIN_ROUNDS:
        raise InstrumentError(
            f"--rounds {requested} is below the {MIN_ROUNDS} windows per cell METHODOLOGY.md "
            f"§10.3 requires; use --quick for a scratch run"
        )
    if requested < 3:
        raise InstrumentError("a BCa interval needs at least 3 windows per cell")
    return requested


def resolve_out(out: str | None, quick: bool) -> Path:
    """The artifact path; a quick run may not write a committed results file."""
    path = Path(out).resolve() if out else (QUICK_OUT if quick else DEFAULT_OUT)
    committed = (REPO_ROOT / "docs" / "benchmarks").resolve()
    if quick and committed in path.parents:
        raise InstrumentError(
            f"--quick may not write under docs/benchmarks/ ({path}); quick runs go to "
            f"results/quick/ (AGENTS.md §8.5)"
        )
    return path


def parse_csv_ints(text: str, what: str) -> list[int]:
    try:
        values = [int(p) for p in text.split(",") if p.strip()]
    except ValueError as exc:
        raise InstrumentError(f"{what} must be comma-separated integers: {text!r}") from exc
    if not values:
        raise InstrumentError(f"{what} must not be empty")
    return values


def parse_engines(text: str) -> list[str]:
    keys = list(ENGINE_KEYS) if text == "all" else [k.strip() for k in text.split(",") if k.strip()]
    unknown = [k for k in keys if k not in ENGINE_KEYS]
    if unknown or not keys:
        raise InstrumentError(f"unknown engine key(s) {unknown}; known: {', '.join(ENGINE_KEYS)}")
    return keys


def build_bench() -> Path:
    """Builds the bench once and returns its executable."""
    cmd = ["cargo", "bench", "-p", "expanse-trie", "--bench", "concurrency", "--no-run",
           "--message-format=json-render-diagnostics"]
    proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        raise InstrumentError(f"`{' '.join(cmd)}` failed:\n{proc.stderr[-4000:]}")
    exe = None
    for line in proc.stdout.splitlines():
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (msg.get("reason") == "compiler-artifact"
                and msg.get("target", {}).get("name") == "concurrency"
                and msg.get("executable")):
            exe = msg["executable"]
    if not exe:
        raise InstrumentError("cargo reported no `concurrency` bench executable")
    return Path(exe)


def run_group(exe: Path, key: str, read_pct: int | None, threads: list[int], rounds: int,
              samples: Path) -> None:
    """Runs one engine and workload group of the bench, windows to `samples`."""
    env = dict(os.environ)
    env.update({
        "EXPANSE_BENCH_ENGINES": key,
        "EXPANSE_BENCH_THREADS": ",".join(str(t) for t in threads),
        "EXPANSE_BENCH_ROUNDS": str(rounds),
        "EXPANSE_BENCH_SAMPLES": str(samples),
    })
    if read_pct is not None:
        env["EXPANSE_BENCH_WORKLOADS"] = str(read_pct)
    proc = subprocess.run([str(exe)], cwd=REPO_ROOT / "crates" / "expanse", env=env,
                          capture_output=True, text=True)
    sys.stdout.write(proc.stdout)
    if proc.returncode != 0:
        raise InstrumentError(
            f"bench group {key} (read {read_pct}%) exited {proc.returncode}:\n{proc.stderr[-4000:]}"
        )


def read_samples(path: Path) -> list[dict[str, Any]]:
    """Every window the bench appended, validated field by field."""
    if not path.is_file():
        raise InstrumentError(f"the bench wrote no samples file ({path})")
    rows = []
    for n, line in enumerate(path.read_text().splitlines(), start=1):
        if not line.strip():
            continue
        row = json.loads(line)
        missing = [k for k in SAMPLE_KEYS if k not in row]
        if missing:
            raise InstrumentError(f"{path}:{n}: sample lacks {missing}")
        if row["workload_id"] != WORKLOAD_ID:
            raise InstrumentError(f"{path}:{n}: workload_id {row['workload_id']!r} is not {WORKLOAD_ID!r}")
        if not (isinstance(row["elapsed_s"], (int, float)) and row["elapsed_s"] > 0):
            raise InstrumentError(f"{path}:{n}: elapsed_s {row['elapsed_s']!r} is not positive")
        rows.append(row)
    if not rows:
        raise InstrumentError(f"the bench wrote an empty samples file ({path})")
    return rows


def balance_problems(rows: list[dict[str, Any]], threads: list[int], rounds: int) -> list[str]:
    """Whether one table's windows form a balanced Williams design.

    Over whole cycles every thread count must take every position `rounds / n`
    times and directly follow every other thread count `rounds / n` times.
    """
    n = len(threads)
    problems = []
    by_round: dict[int, list[dict[str, Any]]] = {}
    for row in rows:
        by_round.setdefault(row["round"], []).append(row)
    if sorted(by_round) != list(range(rounds)):
        problems.append(f"rounds {sorted(by_round)} are not 0..{rounds - 1}")
        return problems
    positions: dict[tuple[int, int], int] = {}
    follows: dict[tuple[int, int], int] = {}
    for round_idx, windows in sorted(by_round.items()):
        windows = sorted(windows, key=lambda w: w["position"])
        if [w["position"] for w in windows] != list(range(n)):
            problems.append(f"round {round_idx}: positions {[w['position'] for w in windows]} are not 0..{n - 1}")
            continue
        if sorted(w["threads"] for w in windows) != sorted(threads):
            problems.append(f"round {round_idx}: thread counts {[w['threads'] for w in windows]} are not {threads}")
            continue
        for w in windows:
            positions[(w["threads"], w["position"])] = positions.get((w["threads"], w["position"]), 0) + 1
        for a, b in zip(windows, windows[1:]):
            follows[(a["threads"], b["threads"])] = follows.get((a["threads"], b["threads"]), 0) + 1
    if problems:
        return problems
    if rounds % williams_period(n):
        return [f"{rounds} rounds are not a whole number of Williams cycles for {n} thread counts"]
    expected = rounds // n
    uneven = {k: v for k, v in positions.items() if v != expected}
    if uneven or len(positions) != n * n:
        problems.append(f"position balance: expected every (threads, position) {expected} times, got {positions}")
    if n > 1:
        uneven_follow = {k: v for k, v in follows.items() if v != expected}
        if uneven_follow or len(follows) != n * (n - 1):
            problems.append(f"carryover balance: expected every ordered pair {expected} times, got {follows}")
    return problems


def _median(values: list[float]) -> float:
    s = sorted(values)
    mid = len(s) // 2
    return s[mid] if len(s) % 2 else (s[mid - 1] + s[mid]) / 2


def summarize_cell(rows: list[dict[str, Any]], base: dict[int, float] | None,
                   load: dict[str, Any]) -> dict[str, Any]:
    """One (engine, workload, threads) cell from its windows."""
    rows = sorted(rows, key=lambda r: r["round"])
    first = rows[0]
    read = [r["read_ops"] / r["elapsed_s"] for r in rows]
    write = [r["write_ops"] / r["elapsed_s"] for r in rows]
    total = [a + b for a, b in zip(read, write)]
    read_mean, read_lo, read_hi, read_method = bca_bootstrap_ci_with_method(read, confidence=0.95)
    write_mean, write_lo, write_hi, write_method = bca_bootstrap_ci_with_method(write, confidence=0.95)
    total_mean, total_lo, total_hi, total_method = bca_bootstrap_ci_with_method(total, confidence=0.95)
    cell: dict[str, Any] = {
        "workload_id": WORKLOAD_ID,
        "engine_key": first["engine_key"],
        "engine": first["engine"],
        "workload": first["workload"],
        "read_pct": first["read_pct"],
        "write_rate": first["write_rate"],
        "threads": first["threads"],
        "rounds": len(rows),
        "window_ms": WINDOW_MS,
        "read_ops_s_mean": read_mean,
        "read_ops_s_ci_lower": read_lo,
        "read_ops_s_ci_upper": read_hi,
        "read_ops_s_ci_method": read_method,
        "read_ops_s_median": _median(read),
        "write_ops_s_mean": write_mean,
        "write_ops_s_ci_lower": write_lo,
        "write_ops_s_ci_upper": write_hi,
        "write_ops_s_ci_method": write_method,
        "write_ops_s_median": _median(write),
        "total_ops_s_mean": total_mean,
        "total_ops_s_ci_lower": total_lo,
        "total_ops_s_ci_upper": total_hi,
        "total_ops_s_ci_method": total_method,
        "total_ops_s_median": _median(total),
    }
    if first["engine_key"] == SYNC32:
        busy = sum(r["busy"] for r in rows)
        attempts = busy + sum(r["ok"] for r in rows)
        cell["busy_pct"] = 100.0 * busy / attempts if attempts else 0.0
        cell["refused_writes"] = sum(r["refused"] for r in rows)
    if base is not None and first["threads"] != 1:
        ratios = [t / base[r["round"]] for t, r in zip(total, rows)]
        scale_mean, scale_lo, scale_hi, scale_method = bca_bootstrap_ci_with_method(ratios, confidence=0.95)
        cell.update({
            "scaling_c_n_mean": scale_mean,
            "scaling_c_n_ci_lower": scale_lo,
            "scaling_c_n_ci_upper": scale_hi,
            "scaling_c_n_ci_method": scale_method,
            "scaling_c_n_median": _median(ratios),
        })
    cell["load"] = dict(load, scope=LOAD_SCOPE)
    cell["rounds_raw"] = [{k: r[k] for k in RAW_KEYS} for r in rows]
    return cell


def summarize_group(rows: list[dict[str, Any]], threads: list[int], rounds: int,
                    load: dict[str, Any]) -> list[dict[str, Any]]:
    """Every cell of one bench group; refuses a missing thread count or an unbalanced order."""
    tables: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for row in rows:
        tables.setdefault((row["engine_key"], row["workload"]), []).append(row)
    cells = []
    for (key, workload), table in tables.items():
        seen = sorted({r["threads"] for r in table})
        if seen != sorted(threads):
            raise InstrumentError(
                f"{key} ({workload}): windows cover thread counts {seen}, not {sorted(threads)} — "
                f"the bench drops thread counts above the CPUs it may use"
            )
        problems = balance_problems(table, threads, rounds)
        if problems:
            raise InstrumentError(f"{key} ({workload}): " + "; ".join(problems))
        base = None
        if 1 in threads:
            base = {}
            for r in table:
                if r["threads"] == 1:
                    base[r["round"]] = (r["read_ops"] + r["write_ops"]) / r["elapsed_s"]
            if any(v <= 0 for v in base.values()):
                raise InstrumentError(f"{key} ({workload}): a one-thread window measured no operations")
        for t in threads:
            cells.append(summarize_cell([r for r in table if r["threads"] == t], base, load))
    return cells


def artifact_problems(path: Path, artifact: dict[str, Any]) -> list[str]:
    """`check_bench_provenance.py`'s findings for this artifact, as it will be checked in CI."""
    try:
        rel = str(path.resolve().relative_to((REPO_ROOT / "docs" / "benchmarks").resolve()))
    except ValueError:
        rel = path.name
    return check_bench_provenance.findings_for(rel, artifact)


def run(args: argparse.Namespace) -> int:
    threads = parse_csv_ints(args.threads, "--threads")
    workloads = parse_csv_ints(args.workloads, "--workloads")
    if any(not 0 <= w <= 100 for w in workloads):
        raise InstrumentError("--workloads are read percentages (0-100)")
    engines = parse_engines(args.engines)
    rounds = resolve_rounds(args.rounds, len(threads), args.quick)
    out = resolve_out(args.out, args.quick)

    pin = bench_pin.apply("mixed_concurrency.py")
    exe = build_bench()
    prov = new_provenance(
        suite="concurrency",
        issue=568,
        ratio=RATIO,
        repo_root=REPO_ROOT,
        core_pin=pin,
        harness=HARNESS,
        window_ms=WINDOW_MS,
        rounds=rounds,
        threads=threads,
    )
    print(f"core pin: {pin} | threads {threads} | rounds {rounds} | engines {engines}")
    cells: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory() as tmp:
        for key in engines:
            for read_pct in ([None] if key == SYNC32 else workloads):
                label = f"group:{key}" + ("" if read_pct is None else f":R{read_pct}")
                samples = Path(tmp) / (label.replace(":", "_") + ".jsonl")
                start = begin_cell(prov, label)
                run_group(exe, key, read_pct, threads, rounds, samples)
                load = end_cell(start)
                cells.extend(summarize_group(read_samples(samples), threads, rounds, load))
    add_load(prov, "end")
    artifact = {"provenance": prov, "throughput": cells}
    problems = artifact_problems(out, artifact)
    if problems:
        raise InstrumentError("artifact would fail check_bench_provenance.py:\n  " + "\n  ".join(problems))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(artifact, indent=2) + "\n")
    print(f"Wrote artifact to {out.relative_to(REPO_ROOT) if REPO_ROOT in out.parents else out}")
    return 0


# The published surfaces that quote this instrument besides the suite README:
# the README hero chart and the sync32 health chart (`bench_assets.json`, drawn
# by `scripts/generate_asset_svgs.py`) and the architecture visualizer's
# concurrency panel (`visualizer_data.json` and the HTML's embedded copy).
ASSETS_JSON = REPO_ROOT / "docs" / "assets" / "data" / "bench_assets.json"
VISUALIZER_JSON = REPO_ROOT / "docs" / "visualizer_data.json"
VISUALIZER_HTML = REPO_ROOT / "docs" / "architecture_visualizer.html"
VISUALIZER_KEY = "ycsb_benchmarks.concurrency_scaling"
RUN2_OUT = DEFAULT_OUT.with_name("baseline_concurrent_mixed_run2.json")
# Chart rows by engine key: the 100%-read bars are the Expanse OCC arms and the
# three baselines that admit concurrent readers (the coarse-mutex arms stay in
# the README table), and the 50/50 lines add no arm the bars do not show.
CHART_READ = ("set", "map", "blob", "bytes", "str", "str_dashmap", "blob_skiplist", "blob_rwlock_btree")
CHART_MIXED = ("str_dashmap", "blob_skiplist", "map", "set", "blob", "bytes", "str")
EXPANSE_OCC = frozenset({"map", "set", "blob", "bytes", "str"})
CHART_LABEL = {"blob_rwlock_btree": "RwLock<BTreeMap<u64, ...>>"}


def _find(artifact: dict[str, Any], where: str, engine_key: str, threads: int,
          read_pct: int | None = None, workload: str | None = None) -> dict[str, Any]:
    hits = [c for c in artifact["throughput"]
            if c["engine_key"] == engine_key and c["threads"] == threads
            and (read_pct is None or c["read_pct"] == read_pct)
            and (workload is None or c["workload"] == workload)]
    if len(hits) != 1:
        raise InstrumentError(f"{where}: expected one {engine_key} cell at N={threads} "
                              f"(read {read_pct}%, workload {workload}), found {len(hits)}")
    return hits[0]


def _run_facts(artifact: dict[str, Any], where: str) -> tuple[dict[str, Any], str, float]:
    """The provenance a published block cites, and the run's largest foreign load."""
    prov = artifact.get("provenance") or {}
    missing = [k for k in ("commit", "core_pin", "rounds", "threads", "window_ms") if k not in prov]
    cpu = (prov.get("host") or {}).get("cpu_model")
    if missing or not cpu:
        raise InstrumentError(f"{where}: provenance lacks {missing or ['host.cpu_model']}")
    cells = artifact.get("throughput") or []
    if not cells:
        raise InstrumentError(f"{where}: no throughput cells")
    return prov, cpu, max(c["load"]["foreign_busy_cpus"] for c in cells)


def asset_blocks(run1: dict[str, Any], run2: dict[str, Any], run_ids: tuple[str, str],
                 assets: dict[str, Any], visualizer: dict[str, Any]) -> dict[str, Any]:
    """The chart and visualizer blocks the two committed runs publish (AGENTS.md §8.2, §8.7).

    Levels and ratios come from run 1, with run 2's beside them wherever a chart
    prints one, so a cell the second run does not reproduce reads as two values
    (rule 18). Run 2 must have measured the same commit and the same cells. Descriptive fields the artifact
    does not carry — the chart's keyspace and retraction notes, the visualizer's
    population and keyspace — are kept from the current files.
    """
    w1, w2 = DEFAULT_OUT.name, RUN2_OUT.name
    prov, cpu, foreign1 = _run_facts(run1, w1)
    prov2, _, foreign2 = _run_facts(run2, w2)
    if prov["commit"] != prov2["commit"]:
        raise InstrumentError(f"{w1} and {w2} measured different commits "
                              f"({prov['commit']} vs {prov2['commit']})")
    cover1 = {(c["engine_key"], c["workload"], c["threads"]) for c in run1["throughput"]}
    cover2 = {(c["engine_key"], c["workload"], c["threads"]) for c in run2["throughput"]}
    if cover1 != cover2:
        raise InstrumentError(f"{w1} and {w2} cover different cells: {sorted(cover1 ^ cover2)}")
    threads = list(prov["threads"])
    if threads[0] != 1:
        raise InstrumentError(f"{w1}: the scaling figures need a one-thread cell, threads are {threads}")
    ref = str(prov["commit"])[:8]
    host = f"reference host -- {cpu}, pin {prov['core_pin']}"
    config = (f"threads {','.join(str(t) for t in threads)}, {prov['rounds']} interleaved rounds of "
              f"{prov['window_ms']} ms windows, largest foreign busy CPUs over a group "
              f"{max(foreign1, foreign2):.2f} across both runs")
    source = ("docs/benchmarks/concurrency/README.md section 12; written by "
              "docs/benchmarks/concurrency/scripts/mixed_concurrency.py --write-assets from "
              f"results/{w1} (run 1), with results/{w2} as its second run")

    def chart_row(key: str, read_pct: int) -> dict[str, Any]:
        cells = [_find(run1, w1, key, t, read_pct=read_pct) for t in threads]
        cells2 = [_find(run2, w2, key, t, read_pct=read_pct) for t in threads]
        return {
            "arm": CHART_LABEL.get(key, cells[-1]["engine"]),
            "mops": [round(c["total_ops_s_mean"] / 1e6, 1) for c in cells],
            "mops_run2": [round(c["total_ops_s_mean"] / 1e6, 1) for c in cells2],
            "scale_16t": round(cells[-1]["scaling_c_n_mean"], 2),
            "scale_16t_run2": round(cells2[-1]["scaling_c_n_mean"], 2),
            "kind": "expanse" if key in EXPANSE_OCC else "other",
        }

    old_meta = (assets.get("concurrency") or {}).get("meta") or {}
    concurrency = {
        "meta": {"source": source, "host": host, "run": run_ids[0], "run2": run_ids[1], "ref": ref,
                 "config": config, "keyspace": old_meta.get("keyspace", "bounded 2xPOP, ~50% hit rate (#375)"),
                 **({"retraction": old_meta["retraction"]} if "retraction" in old_meta else {})},
        "threads": threads,
        "read_100": [chart_row(k, 100) for k in CHART_READ],
        "read_write_50_50": [chart_row(k, 50) for k in CHART_MIXED],
    }

    s32_workloads = list(dict.fromkeys(c["workload"] for c in run1["throughput"] if c["engine_key"] == SYNC32))
    if not s32_workloads:
        raise InstrumentError(f"{w1}: no {SYNC32} cells")
    s32_threads = sorted({c["threads"] for c in run1["throughput"] if c["engine_key"] == SYNC32})
    duties = []
    for workload in s32_workloads:
        rows = []
        for t in s32_threads:
            c = _find(run1, w1, SYNC32, t, workload=workload)
            c2 = _find(run2, w2, SYNC32, t, workload=workload)
            busy = sum(r["busy"] for r in c["rounds_raw"])
            ok = sum(r["ok"] for r in c["rounds_raw"])
            rows.append({"readers": t, "reads_per_s": round(c["read_ops_s_mean"]),
                         "writes_per_s": round(c["write_ops_s_mean"]), "busy": busy, "attempts": busy + ok,
                         "busy_pct": round(c["busy_pct"], 3), "refused": c["refused_writes"],
                         "busy_pct_run2": round(c2["busy_pct"], 3), "refused_run2": c2["refused_writes"]})
        duty = workload.split(" / ")[0].removeprefix("writer ").removesuffix(" duty")
        duties.append({"duty": duty, "rows": rows})
    sync32_health = {
        "meta": {"source": source, "host": host, "run": run_ids[0], "run2": run_ids[1], "ref": ref,
                 "config": (f"{config}; one writer thread (full duty or deadline-paced), N readers on "
                            f"try_get over a 2x keyspace"),
                 "workload_id": WORKLOAD_ID},
        "threads": s32_threads,
        "duties": duties,
    }

    old_vis = visualizer["ycsb_benchmarks"]["concurrency_scaling"]

    def vis_workload(read_pct: int, name: str, note: str) -> dict[str, Any]:
        cells = [_find(run1, w1, "map", t, read_pct=read_pct) for t in threads]
        cells2 = [_find(run2, w2, "map", t, read_pct=read_pct) for t in threads]
        return {
            "workload": name,
            "metric_note": note,
            "rows": [{"threads": c["threads"], "read_mops": round(c["read_ops_s_mean"] / 1e6, 1)} for c in cells],
            "scale_at_16t": f"{cells[-1]['read_ops_s_mean'] / cells[0]['read_ops_s_mean']:.2f}x",
            "scale_at_16t_run2": f"{cells2[-1]['read_ops_s_mean'] / cells2[0]['read_ops_s_mean']:.2f}x",
        }

    vis_block = {
        "harness": HARNESS,
        "arm": "SyncExpanseMap",
        "population": old_vis["population"],
        "keyspace": old_vis["keyspace"],
        "window_ms": prov["window_ms"],
        "rounds": prov["rounds"],
        "thread_counts": threads,
        "metric": "read ops/s (M): the mean over a thread count's windows",
        "workloads": [
            vis_workload(100, "100% read", "pure read scaling: no writer is active"),
            vis_workload(50, "50% read / 50% write",
                         "mixed-workload read-op rate, NOT read scaling: every bench thread picks a read "
                         "or a write per operation in one loop, so a thread waiting on a write is not reading"),
        ],
    }
    vis_prov = (f"measured: {host}, CI runs {run_ids[0]} and {run_ids[1]}, ref {ref}; {HARNESS} through "
                f"docs/benchmarks/concurrency/scripts/mixed_concurrency.py, {config} - {source}. Per-thread "
                f"rows are rounded to 0.1 Mops/s; scale_at_16t is run 1's mean read ops/s at {threads[-1]} "
                f"threads over that at 1 thread before rounding, so it follows the rows only to within the "
                f"rounding envelope; scale_at_16t_run2 is the same ratio in run 2. The 95/5 mix is NOT a published cell: the bare-metal sweep runs "
                f"workloads 100 and 50.")
    return {"concurrency": concurrency, "sync32_health": sync32_health,
            "visualizer": vis_block, "visualizer_provenance": vis_prov}


def _literal_span(html: str, var: str) -> tuple[int, int]:
    """Where the HTML's embedded `let <var> = {...}` literal starts and ends."""
    needle = f"\n  let {var} = "
    at = html.find(needle)
    if at < 0:
        raise InstrumentError(f"{VISUALIZER_HTML.name} declares no `let {var} =`")
    start = at + len(needle)
    depth, in_string, escaped = 0, False, False
    for i in range(start, len(html)):
        ch = html[i]
        if in_string:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_string = False
        elif ch == '"':
            in_string = True
        elif ch in "[{":
            depth += 1
        elif ch in "]}":
            depth -= 1
            if depth == 0:
                return start, i + 1
    raise InstrumentError(f"{VISUALIZER_HTML.name}: `{var}` literal never closes")


def publish_assets(run_ids: tuple[str, str] | None) -> list[str]:
    """Writes the blocks when given the two CI run ids; checks them when not.

    Returns the surfaces whose committed text differs from what the two
    committed artifacts produce, as it stood before any write.
    """
    runs = []
    for path in (DEFAULT_OUT, RUN2_OUT):
        if not path.is_file():
            raise InstrumentError(f"{path.relative_to(REPO_ROOT)} is not committed")
        runs.append(json.loads(path.read_text()))
    assets = json.loads(ASSETS_JSON.read_text(encoding="utf-8"))
    vis = json.loads(VISUALIZER_JSON.read_text(encoding="utf-8"))
    html = VISUALIZER_HTML.read_text(encoding="utf-8")
    write = run_ids is not None
    if not write:
        meta = (assets.get("concurrency") or {}).get("meta") or {}
        run_ids = (meta.get("run"), meta.get("run2"))
        if not all(run_ids):
            raise InstrumentError(f"{ASSETS_JSON.name} names no run pair; write it with --write-assets RUN1,RUN2")
    blocks = asset_blocks(runs[0], runs[1], run_ids, assets, vis)
    new_assets = dict(assets, concurrency=blocks["concurrency"], sync32_health=blocks["sync32_health"])
    new_vis = json.loads(json.dumps(vis))
    new_vis["ycsb_benchmarks"]["concurrency_scaling"] = blocks["visualizer"]
    new_vis["provenance"][VISUALIZER_KEY] = blocks["visualizer_provenance"]
    start, end = _literal_span(html, "YCSB_BENCHMARKS_DATA")
    literal = json.dumps(new_vis["ycsb_benchmarks"], indent=2, ensure_ascii=False).replace("\n", "\n  ")
    outputs = [
        (ASSETS_JSON, json.dumps(new_assets, indent=2, ensure_ascii=True) + "\n"),
        (VISUALIZER_JSON, json.dumps(new_vis, indent=2, ensure_ascii=False) + "\n"),
        (VISUALIZER_HTML, html[:start] + literal + html[end:]),
    ]
    drifted = [str(p.relative_to(REPO_ROOT)) for p, text in outputs if p.read_text(encoding="utf-8") != text]
    if write:
        for path, text in outputs:
            path.write_text(text, encoding="utf-8")
    return drifted


def _synthetic_rows(key: str, workload: str, read_pct: int | None, threads: list[int],
                    rounds: int) -> list[dict[str, Any]]:
    rows = []
    for round_idx in range(rounds):
        for position, idx in enumerate(williams_order(len(threads), round_idx)):
            t = threads[idx]
            jitter = 1.0 + 0.01 * ((round_idx * 7 + position * 3) % 5)
            rows.append({
                "workload_id": WORKLOAD_ID, "engine_key": key, "engine": key.upper(),
                "workload": workload, "read_pct": read_pct, "write_rate": None,
                "threads": t, "round": round_idx, "position": position,
                "elapsed_s": 0.5 + 0.001 * position,
                "read_ops": int(1_000_000 * t ** 0.8 * jitter),
                "write_ops": int(400_000 * t ** 0.5 * jitter),
                "busy": 3 * t if key == SYNC32 else 0,
                "ok": int(1_000_000 * t ** 0.8 * jitter) if key == SYNC32 else 0,
                "refused": 0,
            })
    return rows


def self_test() -> int:
    """Checks the design helpers, the cell summaries and the artifact contract without Cargo."""
    def expect_error(fn, *args, what: str) -> None:
        try:
            fn(*args)
        except InstrumentError:
            return
        raise AssertionError(f"expected InstrumentError: {what}")

    # Williams rows: the textbook rows for 4 and the mirrored second half for 3.
    assert [williams_order(4, r) for r in range(4)] == [[0, 1, 3, 2], [1, 2, 0, 3], [2, 3, 1, 0], [3, 0, 2, 1]]
    assert williams_order(3, 0) == [0, 1, 2] and williams_order(3, 3) == list(reversed(williams_order(3, 0)))
    assert williams_period(3) == 6 and williams_period(4) == 4

    # Round resolution: whole cycles, at least 15 for a committed artifact.
    assert resolve_rounds(None, 3, quick=False) == 18
    assert resolve_rounds(None, 5, quick=False) == 20
    assert resolve_rounds(None, 3, quick=True) == 6
    expect_error(resolve_rounds, 12, 3, False, what="12 rounds below the §10.3 floor")
    expect_error(resolve_rounds, 20, 3, False, what="20 rounds is not whole cycles of 6")
    expect_error(resolve_out, str(DEFAULT_OUT), True, what="quick run writing a committed artifact")

    threads = [1, 2, 4]
    rounds = 18
    load = {"since": "group:map:R50", "wall_s": 27.0, "busy_cpus_since_prev": 4.0,
            "own_busy_cpus": 4.0, "foreign_busy_cpus": 0.0}
    rows = _synthetic_rows("map", "50% Read / 50% Write", 50, threads, rounds)
    assert balance_problems(rows, threads, rounds) == []
    cells = summarize_group(rows, threads, rounds, load)
    assert [c["threads"] for c in cells] == threads
    for c in cells:
        assert len(c["rounds_raw"]) == rounds and set(c["rounds_raw"][0]) == set(RAW_KEYS)
        for prefix in ("read_ops_s_", "write_ops_s_", "total_ops_s_"):
            assert c[prefix + "ci_lower"] <= c[prefix + "mean"] <= c[prefix + "ci_upper"], c
            assert c[prefix + "ci_method"] in CI_METHODS, c[prefix + "ci_method"]
        assert c["load"]["foreign_busy_cpus"] == 0.0 and c["load"]["scope"] == LOAD_SCOPE
    assert "scaling_c_n_mean" not in cells[0]
    assert cells[2]["scaling_c_n_ci_method"] in CI_METHODS and cells[2]["scaling_c_n_mean"] > 1.0

    # The sync32 group: several duty tables in one process, busy telemetry kept.
    s32 = (_synthetic_rows(SYNC32, "writer full duty / N readers try_get", None, threads, rounds)
           + _synthetic_rows(SYNC32, "writer 10k/s / N readers try_get", None, threads, rounds))
    s32_cells = summarize_group(s32, threads, rounds, load)
    assert len(s32_cells) == 2 * len(threads) and all("busy_pct" in c for c in s32_cells)

    # Negative controls: a dropped thread count, a swapped order, a zero window.
    expect_error(summarize_group, [r for r in rows if r["threads"] != 4], threads, rounds, load,
                 what="a thread count the bench dropped")
    swapped = [dict(r) for r in rows]
    for r in swapped:
        if r["round"] == 0 and r["position"] in (0, 1):
            r["position"] = 1 - r["position"]
    assert balance_problems(swapped, threads, rounds), "a swapped round must break position balance"
    with tempfile.TemporaryDirectory() as tmp:
        bad = Path(tmp) / "zero.jsonl"
        bad.write_text(json.dumps(dict(rows[0], elapsed_s=0)) + "\n")
        expect_error(read_samples, bad, what="a window with zero elapsed time")
        expect_error(read_samples, Path(tmp) / "absent.jsonl", what="a missing samples file")

    # The artifact contract, checked by the gate's own code: provenance, rounds,
    # per-cell attribution and construction labels.
    prov = new_provenance(suite="concurrency", issue=568, ratio=RATIO, repo_root=REPO_ROOT,
                          core_pin="self-test", harness=HARNESS, window_ms=WINDOW_MS,
                          rounds=rounds, threads=threads)
    add_load(prov, "end")
    artifact = {"provenance": prov, "throughput": cells + s32_cells}
    problems = artifact_problems(DEFAULT_OUT, artifact)
    assert problems == [], problems
    no_load = {"provenance": prov, "throughput": [{k: v for k, v in c.items() if k != "load"} for c in cells]}
    assert artifact_problems(DEFAULT_OUT, no_load), "cells without load attribution must fail the gate"
    source = Path(__file__).read_text()
    assert check_bench_provenance.producer_problems(
        "docs/benchmarks/concurrency/scripts/mixed_concurrency.py", source) == []

    # The published blocks: every value from run 1 (chart rows are totals, the
    # visualizer's rows read ops), the second run cited beside it, and a pair
    # that measured different commits or different cells refused.
    def fake_cell(key: str, pct: int | None, workload: str, t: int) -> dict[str, Any]:
        return {"engine_key": key, "engine": key.upper(), "workload": workload, "read_pct": pct,
                "threads": t, "read_ops_s_mean": 1e6 * t, "write_ops_s_mean": 5e5 * t,
                "total_ops_s_mean": 1.5e6 * t, "scaling_c_n_mean": float(t), "busy_pct": 0.25,
                "refused_writes": 0, "rounds_raw": [{"busy": 1, "ok": 399}],
                "load": {"foreign_busy_cpus": 0.01 * t}}
    full = [fake_cell(k, pct, f"{pct}% Read / {100 - pct}% Write", t)
            for k in CHART_READ for pct in (100, 50) for t in threads]
    full += [fake_cell(SYNC32, None, "writer 10k/s / N readers try_get", t) for t in threads]
    fake_prov = {"commit": "0123456789abcdef", "core_pin": "self-test", "rounds": rounds,
                 "threads": threads, "window_ms": WINDOW_MS, "host": {"cpu_model": "synthetic"}}
    run_a = {"provenance": fake_prov, "throughput": full}
    current = ({"concurrency": {"meta": {"keyspace": "k", "retraction": "r"}}},
               {"ycsb_benchmarks": {"concurrency_scaling": {"population": 1, "keyspace": "k"}}})
    run_b = {"provenance": fake_prov, "throughput": [
        dict(c, total_ops_s_mean=2 * c["total_ops_s_mean"], read_ops_s_mean=2 * c["read_ops_s_mean"],
             scaling_c_n_mean=c["scaling_c_n_mean"] / 2, busy_pct=0.5, refused_writes=3) for c in full]}
    blocks = asset_blocks(run_a, run_b, ("1", "2"), *current)
    chart = blocks["concurrency"]
    assert [r["kind"] for r in chart["read_100"]] == ["expanse"] * 5 + ["other"] * 3, chart["read_100"]
    assert chart["meta"]["retraction"] == "r" and chart["meta"]["ref"] == "01234567"
    assert "0.04 across both runs" in chart["meta"]["config"], chart["meta"]["config"]
    map_mixed = next(r for r in chart["read_write_50_50"] if r["arm"] == "MAP")
    assert map_mixed == {"arm": "MAP", "mops": [1.5, 3.0, 6.0], "mops_run2": [3.0, 6.0, 12.0],
                         "scale_16t": 4.0, "scale_16t_run2": 2.0, "kind": "expanse"}, map_mixed
    s32 = blocks["sync32_health"]["duties"]
    assert [d["duty"] for d in s32] == ["10k/s"] and s32[0]["rows"][0]["attempts"] == 400, s32
    assert s32[0]["rows"][0]["busy_pct_run2"] == 0.5 and s32[0]["rows"][0]["refused_run2"] == 3, s32
    vis_mixed = blocks["visualizer"]["workloads"][1]
    assert [r["read_mops"] for r in vis_mixed["rows"]] == [1.0, 2.0, 4.0] and vis_mixed["scale_at_16t"] == "4.00x"
    expect_error(asset_blocks, run_a, dict(run_a, provenance=dict(fake_prov, commit="f" * 16)), ("1", "2"),
                 *current, what="two runs of different commits")
    expect_error(asset_blocks, run_a, dict(run_a, throughput=[c for c in full if c["engine_key"] != "str"]),
                 ("1", "2"), *current, what="two runs covering different cells")
    html = 'x\n  let YCSB_BENCHMARKS_DATA = {"a": "}{", "b": [1, {"c": 2}]};\n  let NEXT = [];'
    start, end = _literal_span(html, "YCSB_BENCHMARKS_DATA")
    assert json.loads(html[start:end]) == {"a": "}{", "b": [1, {"c": 2}]}, html[start:end]

    print("mixed_concurrency.py self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--threads", default="1,2,4,8,16", help="thread counts (comma-separated)")
    ap.add_argument("--workloads", default="100,95,50", help="read percentages (comma-separated)")
    ap.add_argument("--engines", default="map,set",
                    help=f"engine keys (comma-separated) or 'all': {', '.join(ENGINE_KEYS)}")
    ap.add_argument("--rounds", type=int, default=None,
                    help="rounds per group; whole Williams cycles, at least 15 unless --quick")
    ap.add_argument("--out", default=None, help="artifact path")
    ap.add_argument("--quick", action="store_true",
                    help="scratch run: one or more short cycles, written under results/quick/")
    ap.add_argument("--self-test", action="store_true", help="run the self-test and exit")
    ap.add_argument("--write-assets", metavar="RUN1,RUN2",
                    help="write the chart and visualizer blocks from the committed run 1 and run 2 "
                         "artifacts, citing these two CI run ids, and exit")
    ap.add_argument("--check-assets", action="store_true",
                    help="exit non-zero if the chart and visualizer blocks differ from what the "
                         "committed run 1 and run 2 artifacts produce")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    try:
        if args.write_assets and args.check_assets:
            raise InstrumentError("--write-assets and --check-assets are separate steps")
        if args.check_assets:
            drifted = publish_assets(None)
            if drifted:
                raise InstrumentError(f"{', '.join(drifted)} differ from what the committed runs produce; "
                                      f"regenerate with --write-assets RUN1,RUN2")
            print("mixed_concurrency.py: chart and visualizer blocks match the committed runs")
            return 0
        if args.write_assets:
            ids = tuple(p.strip() for p in args.write_assets.split(","))
            if len(ids) != 2 or not all(i.isdigit() for i in ids):
                raise InstrumentError("--write-assets takes two CI run ids: RUN1,RUN2")
            drifted = publish_assets((ids[0], ids[1]))
            print(f"mixed_concurrency.py: wrote {', '.join(drifted) or 'nothing (already in sync)'}")
            return 0
        return run(args)
    except InstrumentError as exc:
        sys.stderr.write(f"mixed_concurrency.py: {exc}\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())
