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

`--step2-gate` is the two-build mode `METHODOLOGY.md` §13 pre-registers for
#568 Steps 1–2. The flags above describe the single-build sweep, and that mode
is unchanged when `--step2-gate` is absent. The Step 2 mode:

- refuses a head that does not contain #949 (`git merge-base --is-ancestor
  5c8c5802 HEAD`), a tracked change under `crates/` in the head checkout, and
  a pin other than `0-15`. Under `--quick` it records each of these and goes
  on;
- applies the pin, builds the bench from the head checkout, then builds it
  again from a detached `git worktree` at `1edfa952` with its own
  `CARGO_TARGET_DIR`. Only the head's `crates/expanse/benches/concurrency.rs`
  is copied into that worktree. `git diff --name-only 1edfa952` plus untracked
  files must list nothing else, both before and after the build. The two
  executables must differ. The worktree and its target are removed at the end;
- runs one bench process per window: `EXPANSE_BENCH_ENGINES` one arm of
  (`map`, `set`), `EXPANSE_BENCH_THREADS` one of (1, 16),
  `EXPANSE_BENCH_WORKLOADS=50`, `EXPANSE_BENCH_ROUNDS=1` and a fresh
  `EXPANSE_BENCH_SAMPLES` file, so every window starts from its own prefill.
  A process that exits non-zero, or that does not write exactly one row with
  the requested arm, thread count and read percentage, voids its round. The
  run is then discarded and nothing is written (§13.5);
- runs 48 rounds by default and refuses fewer, or an odd count. Each round
  runs every (threads, arm, build) window once, nested in that order. Head
  runs first in even rounds and baseline first in odd ones, and the thread
  order flips on the same schedule. Each window records its `round` and
  `position`, and a load snapshot is taken around every round;
- reports, per arm and thread count, the per-round ratio head / baseline of
  total ops/s with the BCa 95% interval of its mean. Beside it are each
  build's mean, median and BCa interval of total ops/s, and every window
  under `rounds_raw`;
- reads §13.4's verdict at 16 threads per arm: `PASS` when the lower bound is
  at least `STEP2_MARGIN` (`scripts/olc_bounds.py`, 1.5), `REFUTED` when the
  upper bound is below it, `INCONCLUSIVE` otherwise. The 1-thread control is
  reported with its interval and `NOT_GATED`. The gate block covers this run
  only; §13.4's two-run decision is read from two artifacts;
- writes `results/baseline_concurrent_step2_gate.json`, or with `--run2`
  `results/baseline_concurrent_step2_gate_run2.json`. Under `--quick` it writes
  `results/quick/baseline_concurrent_step2_gate.json`. A quick run may shorten the
  rounds and pick other thread counts, since a host with fewer than 16 CPUs
  drops the 16-thread window.

    python3 docs/benchmarks/concurrency/scripts/mixed_concurrency.py --step2-gate
    python3 docs/benchmarks/concurrency/scripts/mixed_concurrency.py --step2-gate --run2
    EXPANSE_BENCH_PIN=off python3 docs/benchmarks/concurrency/scripts/mixed_concurrency.py \\
        --step2-gate --quick --rounds 2 --threads 1,8

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
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Callable

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
import bca_bootstrap  # noqa: E402
import check_bench_provenance  # noqa: E402
import olc_bounds  # noqa: E402
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


def build_bench(root: Path = REPO_ROOT, target_dir: Path | None = None) -> Path:
    """Builds the bench once and returns its executable.

    `root` is the tree to build, which defaults to this checkout. `target_dir`,
    when given, becomes `CARGO_TARGET_DIR`, so a second tree never shares or
    overwrites the first tree's build.
    """
    cmd = ["cargo", "bench", "-p", "expanse-trie", "--bench", "concurrency", "--no-run",
           "--message-format=json-render-diagnostics"]
    env = None
    if target_dir is not None:
        env = dict(os.environ, CARGO_TARGET_DIR=str(target_dir))
    proc = subprocess.run(cmd, cwd=root, env=env, capture_output=True, text=True)
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


DEFAULT_THREADS = "1,2,4,8,16"
DEFAULT_WORKLOADS = "100,95,50"
DEFAULT_ENGINES = "map,set"


def run(args: argparse.Namespace) -> int:
    threads = parse_csv_ints(args.threads or DEFAULT_THREADS, "--threads")
    workloads = parse_csv_ints(args.workloads or DEFAULT_WORKLOADS, "--workloads")
    if any(not 0 <= w <= 100 for w in workloads):
        raise InstrumentError("--workloads are read percentages (0-100)")
    engines = parse_engines(args.engines or DEFAULT_ENGINES)
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


# --------------------------------------------------------------------------
# #568 Step 2: the two-build single-writer gate (METHODOLOGY.md §13)
# --------------------------------------------------------------------------

# §13.1: the last commit before multi-writer optimistic lock coupling landed.
STEP2_BASELINE_REF = "1edfa952"
# §13.1: #949, which the head must contain.
STEP2_REQUIRED_ANCESTOR = "5c8c5802"
STEP2_ARMS = ("map", "set")
STEP2_THREADS = (1, 16)
STEP2_GATE_THREADS = 16
STEP2_READ_PCT = 50
STEP2_ROUNDS = 48
STEP2_QUICK_ROUNDS = 4
STEP2_PIN = "0-15"
STEP2_RESAMPLES = 2000
STEP2_BUILDS = ("head", "baseline")
# §13.4. The margin is a choice, and `scripts/olc_bounds.py` sizes the rounds
# against it, so the two copies must stay equal. `step2_margin_problems` checks
# that before every run and in the self-test.
STEP2_GATE_MARGIN = 1.5
STEP2_DEFAULT_OUT = DEFAULT_OUT.with_name("baseline_concurrent_step2_gate.json")
STEP2_RUN2_OUT = DEFAULT_OUT.with_name("baseline_concurrent_step2_gate_run2.json")
STEP2_QUICK_OUT = QUICK_OUT.with_name("baseline_concurrent_step2_gate.json")
STEP2_METHODOLOGY = "docs/benchmarks/concurrency/METHODOLOGY.md section 13"
STEP2_RATIO = (
    "per-round ratio head / baseline of total operations per second, (read_ops + write_ops) "
    "/ elapsed_s from each build's own window of that round; ratio_* is the mean of those "
    "ratios with a one-sample BCa 95% interval"
)
STEP2_COLUMNS = (
    "throughput cells: the mean of one build's windows of total ops/s with a BCa 95% interval, "
    "and their median; ratio cells: the mean of the per-round ratios, which is not the "
    "quotient of the two builds' means"
)
STEP2_LOAD_SCOPE = (
    "round: a load snapshot is taken around every round; a cell carries the largest and the "
    "mean foreign busy CPUs over the rounds its windows ran in, and each raw window its "
    "round's value"
)
STEP2_ORDER = (
    "within a round every (threads, arm, build) window runs once, nested in that order; "
    "head runs first in even rounds and baseline first in odd rounds, and the thread order "
    "flips on the same schedule; arm order is fixed"
)
STEP2_GATE_RULE = (
    "per arm at the gate thread count: PASS when ratio_ci_lower >= margin, REFUTED when "
    "ratio_ci_upper < margin, INCONCLUSIVE otherwise"
)
STEP2_GATE_SCOPE = (
    "this run only; section 13.4 meets #568's gate when map and set both read PASS in two "
    "independent runs, decided by reading two artifacts, never by this block"
)
STEP2_RAW_KEYS = ("round", "position", "elapsed_s", "read_ops", "write_ops")


def step2_margin_problems(margin: float = STEP2_GATE_MARGIN) -> list[str]:
    """The gate margin must equal the one the rounds were sized against."""
    if margin != olc_bounds.STEP2_MARGIN:
        return [f"STEP2_GATE_MARGIN {margin} differs from scripts/olc_bounds.py STEP2_MARGIN "
                f"{olc_bounds.STEP2_MARGIN}; METHODOLOGY.md §13 fixes one margin"]
    return []


def step2_round_order(round_idx: int, arms: tuple[str, ...] = STEP2_ARMS,
                      threads: tuple[int, ...] = STEP2_THREADS) -> list[tuple[str, int, str]]:
    """The (arm, threads, build) windows of one round, in run order (§13.3)."""
    flip = round_idx % 2 == 1
    builds = tuple(reversed(STEP2_BUILDS)) if flip else STEP2_BUILDS
    thread_order = tuple(reversed(threads)) if flip else tuple(threads)
    return [(arm, t, build) for t in thread_order for arm in arms for build in builds]


def resolve_step2_rounds(requested: int | None, quick: bool) -> int:
    """48 rounds, or more; an even count so each build goes first equally often."""
    rounds = requested if requested is not None else (STEP2_QUICK_ROUNDS if quick else STEP2_ROUNDS)
    if rounds < 2 or rounds % 2:
        raise InstrumentError(
            f"--rounds {rounds}: the build order alternates between rounds, so the count must be "
            f"even and at least 2")
    if not quick and rounds < STEP2_ROUNDS:
        raise InstrumentError(
            f"--rounds {rounds} is below the {STEP2_ROUNDS} rounds METHODOLOGY.md §13.3 fixes; "
            f"use --quick for a scratch run")
    return rounds


def resolve_step2_out(out: str | None, run2: bool, quick: bool) -> Path:
    """The artifact path; neither committed Step 2 path is writable by a quick run."""
    if out and run2:
        raise InstrumentError("--run2 names the second run's committed path; do not pass --out with it")
    if out:
        chosen = out
    elif run2:
        chosen = str(STEP2_RUN2_OUT)
    else:
        chosen = str(STEP2_QUICK_OUT if quick else STEP2_DEFAULT_OUT)
    return resolve_out(chosen, quick)


def step2_pin_problems(pin: str, quick: bool) -> list[str]:
    """A committed run is void unless its pin is `0-15` (§13.5)."""
    if quick or pin == STEP2_PIN:
        return []
    return [f"core pin {pin!r} is not {STEP2_PIN}; METHODOLOGY.md §13.5 voids a run whose artifact "
            f"records another pin (use --quick for a scratch run)"]


def git_out(args: list[str], cwd: Path) -> str:
    """`git <args>` in `cwd`; any non-zero exit fails loud (AGENTS.md §8.1)."""
    proc = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise InstrumentError(f"`git {' '.join(args)}` exited {proc.returncode}: {proc.stderr.strip()[-2000:]}")
    return proc.stdout


def head_contains(ancestor: str, repo: Path = REPO_ROOT) -> bool:
    """`git merge-base --is-ancestor <ancestor> HEAD`, discriminating its exit codes.

    0 means contained and 1 means not contained. Anything else, such as an
    unknown commit in a shallow clone, is an execution failure and never
    counts as an answer.
    """
    proc = subprocess.run(["git", "merge-base", "--is-ancestor", ancestor, "HEAD"],
                          cwd=repo, capture_output=True, text=True)
    if proc.returncode == 0:
        return True
    if proc.returncode == 1:
        return False
    raise InstrumentError(
        f"`git merge-base --is-ancestor {ancestor} HEAD` exited {proc.returncode}: "
        f"{proc.stderr.strip()} (a shallow clone cannot answer; fetch full history)")


def check_head_ancestry(quick: bool, contains: Callable[[], bool]) -> bool:
    """Refuses a head without #949 unless quick; returns whether the head contains it."""
    ok = contains()
    if not ok:
        message = (f"the head does not contain {STEP2_REQUIRED_ANCESTOR} (#949); METHODOLOGY.md §13.5 "
                   f"voids the run")
        if not quick:
            raise InstrumentError(message)
        sys.stderr.write(f"mixed_concurrency.py: WARNING (--quick): {message}\n")
    return ok


def head_tree_problems(repo: Path = REPO_ROOT) -> list[str]:
    """Tracked changes under `crates/` would make the head commit misname what was built."""
    dirty = git_out(["status", "--porcelain", "--untracked-files=no", "--", "crates"], repo).strip()
    if dirty:
        return [f"the head checkout has uncommitted changes under crates/, so its commit does not "
                f"name the build:\n{dirty}"]
    return []


def baseline_tree_changes(tree: Path, commit: str) -> tuple[list[str], str]:
    """Every path in `tree` that differs from `commit`, untracked files included, and the diff stat."""
    changed = [p for p in git_out(["diff", "--name-only", commit], tree).splitlines() if p.strip()]
    untracked = [p for p in git_out(["ls-files", "--others", "--exclude-standard"], tree).splitlines()
                 if p.strip()]
    stat = git_out(["diff", "--stat", commit], tree)
    return sorted(set(changed) | set(untracked)), stat


def baseline_tree_problems(changed: list[str]) -> list[str]:
    """§13.5: the baseline tree differs from `1edfa952` in nothing but the bench file."""
    extra = [p for p in changed if p != HARNESS]
    if extra:
        return [f"the baseline tree differs from {STEP2_BASELINE_REF} outside {HARNESS}: {extra}; "
                f"METHODOLOGY.md §13.5 voids the run"]
    return []


def materialise_baseline(parent: Path, head_bench: Path, repo: Path = REPO_ROOT,
                         ref: str = STEP2_BASELINE_REF) -> tuple[Path, str]:
    """A detached worktree of `ref` under `parent`, with only the head's bench file copied in.

    Returns the tree and the full SHA it was checked out at.
    """
    tree = parent / "baseline-tree"
    git_out(["worktree", "add", "--detach", str(tree), ref], repo)
    shutil.copyfile(head_bench, tree / HARNESS)
    return tree, git_out(["rev-parse", "HEAD"], tree).strip()


def remove_baseline(parent: Path, tree: Path, repo: Path = REPO_ROOT) -> None:
    """Removes the worktree, its target directory and git's record of it."""
    subprocess.run(["git", "worktree", "remove", "--force", str(tree)], cwd=repo,
                   capture_output=True, text=True)
    shutil.rmtree(parent, ignore_errors=True)
    subprocess.run(["git", "worktree", "prune"], cwd=repo, capture_output=True, text=True)


def file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def executable_problems(head_sha: str, baseline_sha: str) -> list[str]:
    """Two builds of different engine code cannot yield one binary."""
    if head_sha == baseline_sha:
        return ["the head and baseline bench executables are byte-identical, so the baseline did not "
                "build the baseline engine"]
    return []


def run_window(exe: Path, cwd: Path, arm: str, threads: int, samples: Path) -> None:
    """One bench process for one window: one arm, one thread count, one round, a fresh prefill."""
    if samples.exists():
        raise InstrumentError(f"samples file {samples.name} already exists; every window needs a fresh one")
    env = dict(os.environ)
    env.update({
        "EXPANSE_BENCH_ENGINES": arm,
        "EXPANSE_BENCH_THREADS": str(threads),
        "EXPANSE_BENCH_WORKLOADS": str(STEP2_READ_PCT),
        "EXPANSE_BENCH_ROUNDS": "1",
        "EXPANSE_BENCH_SAMPLES": str(samples),
    })
    proc = subprocess.run([str(exe)], cwd=cwd, env=env, capture_output=True, text=True)
    if proc.returncode != 0:
        raise InstrumentError(
            f"bench process ({arm}, {threads} threads) exited {proc.returncode}:\n"
            f"{proc.stdout[-2000:]}\n{proc.stderr[-4000:]}")


def read_window(samples: Path, arm: str, threads: int) -> dict[str, Any]:
    """The single row one window's process wrote, checked against what was requested (§13.5)."""
    rows = read_samples(samples)
    if len(rows) != 1:
        raise InstrumentError(f"{samples.name}: the process wrote {len(rows)} window rows, not exactly one")
    row = rows[0]
    wrong = [f"{k} {row.get(k)!r} (requested {want!r})"
             for k, want in (("engine_key", arm), ("threads", threads), ("read_pct", STEP2_READ_PCT),
                             ("round", 0), ("position", 0))
             if row.get(k) != want]
    if wrong:
        raise InstrumentError(f"{samples.name}: the window reports " + ", ".join(wrong))
    return row


Launcher = Callable[[str, str, int, Path], None]


def run_step2_rounds(launch: Launcher, prov: dict[str, Any], rounds: int, scratch: Path,
                     arms: tuple[str, ...] = STEP2_ARMS,
                     threads: tuple[int, ...] = STEP2_THREADS,
                     progress: bool = True) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Every round of the design; returns the windows and one load record per round.

    `launch(build, arm, threads, samples)` runs one window's process. This
    function reads and validates what that process wrote, so a substituted
    launcher cannot skip the validation. Any failure voids the round and
    discards the run.
    """
    windows: list[dict[str, Any]] = []
    round_loads: list[dict[str, Any]] = []
    for r in range(rounds):
        start = begin_cell(prov, f"round:{r}")
        for position, (arm, t, build) in enumerate(step2_round_order(r, arms, threads)):
            samples = scratch / f"round{r}_pos{position}_{build}_{arm}_T{t}.jsonl"
            try:
                launch(build, arm, t, samples)
                row = read_window(samples, arm, t)
            except InstrumentError as exc:
                raise InstrumentError(
                    f"round {r} is void ({build} {arm} at {t} threads, position {position}): {exc}\n"
                    f"METHODOLOGY.md §13.5 discards the whole run; re-run from fresh builds") from exc
            windows.append({
                "build": build, "engine_key": arm, "engine": row["engine"], "workload": row["workload"],
                "read_pct": row["read_pct"], "threads": t, "round": r, "position": position,
                "elapsed_s": row["elapsed_s"], "read_ops": row["read_ops"], "write_ops": row["write_ops"],
            })
            if progress:
                print(f"step2: round {r + 1}/{rounds} position {position}: {build} {arm} {t}T ok", flush=True)
        round_loads.append(dict(end_cell(start), round=r))
    return windows, round_loads


def step2_order_problems(windows: list[dict[str, Any]], rounds: int, arms: tuple[str, ...],
                         threads: tuple[int, ...]) -> list[str]:
    """Whether the recorded windows are the §13.3 design, and each build led equally often."""
    problems = []
    by_round: dict[int, list[dict[str, Any]]] = {}
    for w in windows:
        by_round.setdefault(w["round"], []).append(w)
    if sorted(by_round) != list(range(rounds)):
        return [f"rounds {sorted(by_round)} are not 0..{rounds - 1}"]
    first = {b: 0 for b in STEP2_BUILDS}
    for r, ws in sorted(by_round.items()):
        ws = sorted(ws, key=lambda w: w["position"])
        got = [(w["engine_key"], w["threads"], w["build"]) for w in ws]
        if got != step2_round_order(r, arms, threads) or [w["position"] for w in ws] != list(range(len(ws))):
            problems.append(f"round {r}: windows {got} are not the pre-registered order")
            continue
        first[ws[0]["build"]] += 1
    if not problems and len(set(first.values())) != 1:
        problems.append(f"build order unbalanced: first in a round {first}")
    return problems


def window_total(w: dict[str, Any]) -> float:
    return (w["read_ops"] + w["write_ops"]) / w["elapsed_s"]


def paired_ratios(head: dict[int, dict[str, Any]],
                  baseline: dict[int, dict[str, Any]]) -> list[dict[str, Any]]:
    """Head / baseline per round, each from the same round's two windows."""
    if sorted(head) != sorted(baseline):
        raise InstrumentError(f"unpaired rounds: head {sorted(head)} vs baseline {sorted(baseline)}")
    out = []
    for r in sorted(head):
        h = window_total(head[r])
        b = window_total(baseline[r])
        if b <= 0:
            raise InstrumentError(f"round {r}: the baseline window measured no operations")
        out.append({"round": r, "head_position": head[r]["position"],
                    "baseline_position": baseline[r]["position"],
                    "head_total_ops_s": h, "baseline_total_ops_s": b, "ratio": h / b})
    return out


def step2_interval(values: list[float]) -> tuple[float, float | None, float | None, str]:
    """Mean, BCa 95% bounds and construction label; a quick run below 3 rounds gets no interval."""
    if len(values) < 3:
        return sum(values) / len(values), None, None, "not_computed_below_3_rounds"
    mean, lo, hi, bca_method = bca_bootstrap_ci_with_method(values, confidence=0.95,
                                                            num_resamples=STEP2_RESAMPLES)
    return mean, lo, hi, bca_method


def step2_verdict(ci_lower: float | None, ci_upper: float | None,
                  margin: float = STEP2_GATE_MARGIN) -> str:
    """§13.4, at the gate cell."""
    if ci_lower is None or ci_upper is None:
        return "NOT_EVALUABLE"
    if ci_lower >= margin:
        return "PASS"
    if ci_upper < margin:
        return "REFUTED"
    return "INCONCLUSIVE"


def _cell_load(round_ids: list[int], loads: dict[int, dict[str, Any]]) -> dict[str, Any]:
    foreign = [loads[r].get("foreign_busy_cpus") for r in round_ids]
    numeric = [f for f in foreign if isinstance(f, (int, float)) and not isinstance(f, bool)]
    complete = len(numeric) == len(foreign)
    return {
        "scope": STEP2_LOAD_SCOPE,
        "rounds": len(round_ids),
        # None unless every round attributed: a partial maximum is not a smaller number (§8.1).
        "foreign_busy_cpus": max(numeric) if complete and numeric else None,
        "foreign_busy_cpus_mean": round(sum(numeric) / len(numeric), 3) if complete and numeric else None,
        "wall_s": round(sum(loads[r].get("wall_s") or 0.0 for r in round_ids), 3),
    }


def summarize_step2(windows: list[dict[str, Any]], round_loads: list[dict[str, Any]], rounds: int,
                    arms: tuple[str, ...] = STEP2_ARMS, threads: tuple[int, ...] = STEP2_THREADS,
                    gate_threads: int = STEP2_GATE_THREADS,
                    commits: dict[str, str] | None = None) -> dict[str, Any]:
    """The throughput cells, the ratio cells and this run's gate block."""
    problems = step2_order_problems(windows, rounds, arms, threads)
    if problems:
        raise InstrumentError("the windows are not the §13.3 design: " + "; ".join(problems))
    loads = {entry["round"]: entry for entry in round_loads}
    if sorted(loads) != list(range(rounds)):
        raise InstrumentError(f"load records cover rounds {sorted(loads)}, not 0..{rounds - 1}")
    throughput: list[dict[str, Any]] = []
    ratios: list[dict[str, Any]] = []
    verdicts: dict[str, str] = {}
    for arm in arms:
        for t in threads:
            by_build = {b: {w["round"]: w for w in windows
                            if w["engine_key"] == arm and w["threads"] == t and w["build"] == b}
                        for b in STEP2_BUILDS}
            for b in STEP2_BUILDS:
                cell_windows = [by_build[b][r] for r in sorted(by_build[b])]
                totals = [window_total(w) for w in cell_windows]
                mean, lo, hi, total_method = step2_interval(totals)
                first = cell_windows[0]
                throughput.append({
                    "workload_id": WORKLOAD_ID, "engine_key": arm, "engine": first["engine"],
                    "workload": first["workload"], "read_pct": first["read_pct"], "threads": t,
                    "build": b, "commit": (commits or {}).get(b), "rounds": len(cell_windows),
                    "window_ms": WINDOW_MS,
                    "total_ops_s_mean": mean, "total_ops_s_ci_lower": lo, "total_ops_s_ci_upper": hi,
                    "total_ops_s_ci_method": total_method, "total_ops_s_median": _median(totals),
                    "load": _cell_load(sorted(by_build[b]), loads),
                    "rounds_raw": [dict({k: w[k] for k in STEP2_RAW_KEYS}, total_ops_s=tot,
                                        foreign_busy_cpus=loads[w["round"]].get("foreign_busy_cpus"))
                                   for w, tot in zip(cell_windows, totals)],
                })
            pairs = paired_ratios(by_build["head"], by_build["baseline"])
            values = [p["ratio"] for p in pairs]
            mean, lo, hi, ratio_method = step2_interval(values)
            gated = t == gate_threads
            verdict = step2_verdict(lo, hi) if gated else "NOT_GATED"
            if gated:
                verdicts[arm] = verdict
            first = by_build["head"][min(by_build["head"])]
            ratios.append({
                "workload_id": WORKLOAD_ID, "engine_key": arm, "engine": first["engine"],
                "workload": first["workload"], "read_pct": first["read_pct"], "threads": t,
                "rounds": len(pairs),
                "ratio_mean": mean, "ratio_ci_lower": lo, "ratio_ci_upper": hi,
                "ratio_ci_method": ratio_method, "ratio_median": _median(values),
                "gated": gated, "verdict": verdict,
                "load": _cell_load([p["round"] for p in pairs], loads),
                "rounds_raw": pairs,
            })
    if sorted(verdicts) != sorted(arms):
        raise InstrumentError(f"no gate cell at {gate_threads} threads for {sorted(set(arms) - set(verdicts))}")
    gate = {
        "methodology": STEP2_METHODOLOGY, "threads": gate_threads, "margin": STEP2_GATE_MARGIN,
        "margin_source": "scripts/olc_bounds.py STEP2_MARGIN", "rule": STEP2_GATE_RULE,
        "verdicts": verdicts, "scope": STEP2_GATE_SCOPE,
    }
    return {"throughput": throughput, "ratio": ratios, "gate": gate}


def step2_artifact_problems(path: Path, artifact: dict[str, Any], quick: bool) -> list[str]:
    """The provenance gate's findings, plus per-cell attribution on a committed run.

    The committed name `baseline_concurrent_step2_gate*.json` falls under
    `check_bench_provenance.py`'s `baseline_*` glob and its concurrent-name
    rule, so CI applies the same attribution check to the committed file.
    Running it here as well refuses a failing run on the host, before it is
    written. A quick run is exempt because it may run where `/proc` does not
    exist.
    """
    problems = artifact_problems(path, artifact)
    if not quick:
        problems.extend(check_bench_provenance.check_attribution(path.name, artifact))
    return problems


def run_step2(args: argparse.Namespace) -> int:
    quick = args.quick
    if args.engines is not None or args.workloads is not None:
        raise InstrumentError("--step2-gate fixes the arms (map, set) and the mix (50% read); "
                              "--engines and --workloads do not apply")
    if args.threads is not None and not quick:
        raise InstrumentError(f"--step2-gate runs threads {STEP2_THREADS}; --threads is for --quick only")
    threads = tuple(parse_csv_ints(args.threads, "--threads")) if args.threads else STEP2_THREADS
    gate_threads = max(threads)
    rounds = resolve_step2_rounds(args.rounds, quick)
    out = resolve_step2_out(args.out, args.run2, quick)
    problems = step2_margin_problems()
    if problems:
        raise InstrumentError("; ".join(problems))

    contains_949 = check_head_ancestry(quick, lambda: head_contains(STEP2_REQUIRED_ANCESTOR))
    head_dirty = head_tree_problems()
    if head_dirty and not quick:
        raise InstrumentError(head_dirty[0])

    pin = bench_pin.apply("mixed_concurrency.py --step2-gate")
    problems = step2_pin_problems(pin, quick)
    if problems:
        raise InstrumentError(problems[0])

    head_commit = git_out(["rev-parse", "HEAD"], REPO_ROOT).strip()
    print(f"step2: building the head bench at {head_commit}", flush=True)
    head_exe = build_bench()
    parent = Path(tempfile.mkdtemp(prefix="expanse-step2-"))
    tree = parent / "baseline-tree"
    try:
        tree, baseline_commit = materialise_baseline(parent, REPO_ROOT / HARNESS)
        changed, _ = baseline_tree_changes(tree, STEP2_BASELINE_REF)
        problems = baseline_tree_problems(changed)
        if problems:
            raise InstrumentError(problems[0])
        print(f"step2: building the baseline bench at {baseline_commit} with the head's {HARNESS}", flush=True)
        baseline_exe = build_bench(tree, target_dir=parent / "target")
        changed, stat = baseline_tree_changes(tree, STEP2_BASELINE_REF)
        problems = baseline_tree_problems(changed)
        if problems:
            raise InstrumentError(f"after the build: {problems[0]}")
        exe_sha = {"head": file_sha256(head_exe), "baseline": file_sha256(baseline_exe)}
        problems = executable_problems(exe_sha["head"], exe_sha["baseline"])
        if problems:
            raise InstrumentError(problems[0])

        prov = new_provenance(
            suite="concurrency", issue=568, ratio=STEP2_RATIO, repo_root=REPO_ROOT, core_pin=pin,
            harness=HARNESS, window_ms=WINDOW_MS, rounds=rounds, threads=list(threads),
            mode="step2_gate", methodology=STEP2_METHODOLOGY, quick=quick,
            arms=list(STEP2_ARMS), read_pct=STEP2_READ_PCT, order=STEP2_ORDER,
            bootstrap_resamples=STEP2_RESAMPLES,
            head_commit=head_commit, baseline_commit=baseline_commit, baseline_ref=STEP2_BASELINE_REF,
            required_ancestor=STEP2_REQUIRED_ANCESTOR, head_contains_required_ancestor=contains_949,
            head_crates_clean=not head_dirty,
            baseline_tree_changed_files=changed, baseline_tree_diff_stat=stat,
            head_bench_sha256=file_sha256(REPO_ROOT / HARNESS),
            executable_sha256=exe_sha,
        )
        prov["estimators"]["columns"] = STEP2_COLUMNS
        print(f"core pin: {pin} | threads {list(threads)} | rounds {rounds} | arms {list(STEP2_ARMS)}")

        exes = {"head": head_exe, "baseline": baseline_exe}
        cwds = {"head": REPO_ROOT / "crates" / "expanse", "baseline": tree / "crates" / "expanse"}

        def launch(build: str, arm: str, t: int, samples: Path) -> None:
            run_window(exes[build], cwds[build], arm, t, samples)

        scratch = parent / "samples"
        scratch.mkdir()
        windows, round_loads = run_step2_rounds(launch, prov, rounds, scratch, STEP2_ARMS, threads)
    finally:
        remove_baseline(parent, tree)
    add_load(prov, "end")
    prov["round_loads"] = round_loads
    summary = summarize_step2(windows, round_loads, rounds, STEP2_ARMS, threads, gate_threads,
                              commits={"head": head_commit, "baseline": baseline_commit})
    summary["gate"]["quick"] = quick
    artifact = {"provenance": prov, **summary}
    problems = step2_artifact_problems(out, artifact, quick)
    if problems:
        raise InstrumentError("artifact would fail check_bench_provenance.py:\n  " + "\n  ".join(problems))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(artifact, indent=2) + "\n")
    print(f"step2 gate verdicts at {gate_threads} threads (this run only): {summary['gate']['verdicts']}")
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
# Chart rows by engine key, grouped by key type, the only grouping a comparison
# holds within: the integer arms (no third-party arm of that type), the blob arms
# and their baselines, and the string arms and theirs. The coarse-mutex arms stay
# in the README table, and the 50/50 lines add no arm the bars do not show.
CHART_READ = ("set", "map", "blob", "blob_skiplist", "blob_rwlock_btree", "bytes", "str", "str_dashmap")
CHART_MIXED = ("map", "set", "blob", "blob_skiplist", "bytes", "str", "str_dashmap")
CHART_FAMILY = {
    "set": "u64 keys, 1M draws", "map": "u64 keys, 1M draws",
    "blob": "u64 -> 128-byte payload, 200k", "blob_skiplist": "u64 -> 128-byte payload, 200k",
    "blob_rwlock_btree": "u64 -> 128-byte payload, 200k",
    "bytes": "string keys, 100k", "str": "string keys, 100k", "str_dashmap": "string keys, 100k",
}
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
            "family": CHART_FAMILY[key],
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
    assert [r["kind"] for r in chart["read_100"]] == ["expanse"] * 3 + ["other"] * 2 + ["expanse"] * 2 + ["other"], chart["read_100"]
    assert [r["family"] for r in chart["read_100"]] == [CHART_FAMILY[k] for k in CHART_READ]
    assert chart["meta"]["retraction"] == "r" and chart["meta"]["ref"] == "01234567"
    assert "0.04 across both runs" in chart["meta"]["config"], chart["meta"]["config"]
    map_mixed = next(r for r in chart["read_write_50_50"] if r["arm"] == "MAP")
    assert map_mixed == {"arm": "MAP", "mops": [1.5, 3.0, 6.0], "mops_run2": [3.0, 6.0, 12.0],
                         "scale_16t": 4.0, "scale_16t_run2": 2.0, "kind": "expanse",
                         "family": CHART_FAMILY["map"]}, map_mixed
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

    step2_self_test()
    print("mixed_concurrency.py self-test PASSED")
    return 0


def _step2_launcher(factor: Callable[[str, int, int], float],
                    mutate: Callable[[dict[str, Any], str, int], list[dict[str, Any]]] | None = None) -> Launcher:
    """A launcher that writes synthetic windows in place of a bench process.

    Baseline throughput drifts strongly from round to round, and head is
    `factor(arm, threads, round)` times the same round's baseline. A ratio that
    pairs windows from two different rounds therefore reads a different value.
    """
    def launch(build: str, arm: str, t: int, samples: Path) -> None:
        r = int(samples.name.split("_")[0].removeprefix("round"))
        base = 1_000_000 * (1 + (r * 7) % 5) * (4 if t > 1 else 1)
        ops = base if build == "baseline" else int(round(base * factor(arm, t, r)))
        row = {"workload_id": WORKLOAD_ID, "engine_key": arm, "engine": arm.upper(),
               "workload": "50% Read / 50% Write", "read_pct": STEP2_READ_PCT, "write_rate": None,
               "threads": t, "round": 0, "position": 0, "elapsed_s": 0.5,
               "read_ops": ops // 2, "write_ops": ops - ops // 2, "busy": 0, "ok": 0, "refused": 0}
        rows = mutate(row, build, r) if mutate else [row]
        if rows:
            samples.write_text("".join(json.dumps(x) + "\n" for x in rows))
    return launch


def step2_self_test() -> None:
    """The Step 2 mode without Cargo: design, pairing, verdicts, refusals and the artifact."""
    def expect_error(fn, *args, what: str, **kwargs) -> str:
        try:
            fn(*args, **kwargs)
        except InstrumentError as exc:
            return str(exc)
        raise AssertionError(f"expected InstrumentError: {what}")

    def synthetic_loads(rounds: int, foreign: float | None = 0.02) -> list[dict[str, Any]]:
        return [{"round": r, "since": f"round:{r}", "wall_s": 9.0, "busy_cpus_since_prev": 8.5,
                 "own_busy_cpus": 8.5 - (foreign or 0.0), "foreign_busy_cpus": foreign} for r in range(rounds)]

    def scenario(factor, rounds, arms=STEP2_ARMS, threads=STEP2_THREADS, mutate=None):
        with tempfile.TemporaryDirectory() as tmp:
            windows, round_loads = run_step2_rounds(_step2_launcher(factor, mutate), {}, rounds, Path(tmp),
                                                    arms, threads, progress=False)
        assert [entry["round"] for entry in round_loads] == list(range(rounds))
        return windows, summarize_step2(windows, synthetic_loads(rounds), rounds, arms, threads,
                                        gate_threads=max(threads))

    def cell(summary, key, arm, t, build=None):
        hits = [c for c in summary[key] if c["engine_key"] == arm and c["threads"] == t
                and (build is None or c["build"] == build)]
        assert len(hits) == 1, (key, arm, t, build, len(hits))
        return hits[0]

    # The margin is the one the rounds were sized against.
    assert step2_margin_problems() == [] and STEP2_GATE_MARGIN == olc_bounds.STEP2_MARGIN
    assert step2_margin_problems(1.0), "a drifted margin must be refused"

    # The verdict line, directly: >= at the bound, < for refuted.
    assert step2_verdict(1.5, 2.0) == "PASS"
    assert step2_verdict(1.2, 1.8) == "INCONCLUSIVE"
    assert step2_verdict(1.0, 1.49) == "REFUTED"
    assert step2_verdict(None, None) == "NOT_EVALUABLE"

    # The order design over 48 rounds, from the plan and from the windows the loop recorded.
    plan_first = {b: sum(1 for r in range(STEP2_ROUNDS) if step2_round_order(r)[0][2] == b) for b in STEP2_BUILDS}
    assert plan_first == {"head": 24, "baseline": 24}, plan_first
    windows, summary = scenario(lambda arm, t, r: 2.0, STEP2_ROUNDS)
    assert len(windows) == STEP2_ROUNDS * len(STEP2_ARMS) * len(STEP2_THREADS) * len(STEP2_BUILDS)
    lead = [w for w in windows if w["position"] == 0]
    assert len(lead) == STEP2_ROUNDS
    assert sum(1 for w in lead if w["build"] == "head") == 24 and sum(1 for w in lead if w["build"] == "baseline") == 24
    assert all(w["threads"] == (1 if w["round"] % 2 == 0 else 16) for w in lead), "thread order alternates"
    assert all(w["build"] == ("head" if w["round"] % 2 == 0 else "baseline") for w in lead), "build order alternates"
    for arm in STEP2_ARMS:
        for t in STEP2_THREADS:
            pos = {(w["round"], w["build"]): w["position"] for w in windows
                   if w["engine_key"] == arm and w["threads"] == t}
            head_before = sum(1 for r in range(STEP2_ROUNDS) if pos[(r, "head")] < pos[(r, "baseline")])
            assert head_before == 24, (arm, t, head_before)
    assert step2_order_problems(windows, STEP2_ROUNDS, STEP2_ARMS, STEP2_THREADS) == []
    swapped = [dict(w) for w in windows]
    for w in swapped:
        if w["round"] == 0 and w["position"] in (0, 1):
            w["position"] = 1 - w["position"]
    assert step2_order_problems(swapped, STEP2_ROUNDS, STEP2_ARMS, STEP2_THREADS), "a swapped round must be refused"
    expect_error(summarize_step2, windows[1:], synthetic_loads(STEP2_ROUNDS), STEP2_ROUNDS,
                 what="a missing window in the summary")

    # PASS, and pairing within a round: every per-round ratio is exactly the factor,
    # although the baseline drifts several-fold between rounds.
    assert summary["gate"]["verdicts"] == {"map": "PASS", "set": "PASS"}, summary["gate"]
    assert summary["gate"]["margin"] == STEP2_GATE_MARGIN and summary["gate"]["threads"] == STEP2_GATE_THREADS
    for arm in STEP2_ARMS:
        gate_cell = cell(summary, "ratio", arm, 16)
        assert all(p["ratio"] == 2.0 for p in gate_cell["rounds_raw"]), \
            "a ratio must pair the head and baseline windows of the same round"
        assert gate_cell["ratio_ci_lower"] == 2.0 and gate_cell["ratio_ci_method"] in CI_METHODS
        assert cell(summary, "ratio", arm, 1)["verdict"] == "NOT_GATED"
        for build in STEP2_BUILDS:
            c = cell(summary, "throughput", arm, 16, build)
            assert c["total_ops_s_ci_lower"] <= c["total_ops_s_mean"] <= c["total_ops_s_ci_upper"], c
            assert c["total_ops_s_ci_method"] in CI_METHODS and len(c["rounds_raw"]) == STEP2_ROUNDS
            assert c["load"]["foreign_busy_cpus"] == 0.02 and c["load"]["scope"] == STEP2_LOAD_SCOPE

    # The artifact passes the provenance gate's own code, attribution included.
    prov = new_provenance(suite="concurrency", issue=568, ratio=STEP2_RATIO, repo_root=REPO_ROOT,
                          core_pin=STEP2_PIN, harness=HARNESS, window_ms=WINDOW_MS, rounds=STEP2_ROUNDS,
                          threads=list(STEP2_THREADS), mode="step2_gate")
    prov["estimators"]["columns"] = STEP2_COLUMNS
    add_load(prov, "end")
    artifact = {"provenance": prov, **summary}
    for path in (STEP2_DEFAULT_OUT, STEP2_RUN2_OUT):
        problems = step2_artifact_problems(path, artifact, quick=False)
        assert problems == [], problems
    unattributed = json.loads(json.dumps(artifact))
    unattributed["throughput"][0]["load"]["foreign_busy_cpus"] = None
    assert step2_artifact_problems(STEP2_DEFAULT_OUT, unattributed, quick=False), \
        "a committed cell without numeric foreign load must fail"

    # PASS exactly at the margin, REFUTED, and INCONCLUSIVE, each with a lower
    # bound above 1.0 so a weakened decision line would change the verdict.
    _, at_margin = scenario(lambda arm, t, r: 1.5, 8, arms=("map",))
    assert at_margin["gate"]["verdicts"] == {"map": "PASS"}, at_margin["gate"]
    _, refuted = scenario(lambda arm, t, r: 1.1 if r % 2 == 0 else 1.2, 8, arms=("map",))
    gate_cell = cell(refuted, "ratio", "map", 16)
    assert refuted["gate"]["verdicts"] == {"map": "REFUTED"} and gate_cell["ratio_ci_lower"] > 1.0, gate_cell
    _, unsure = scenario(lambda arm, t, r: 1.4 if r % 2 == 0 else 1.6, 8, arms=("map",))
    gate_cell = cell(unsure, "ratio", "map", 16)
    assert 1.0 < gate_cell["ratio_ci_lower"] < STEP2_GATE_MARGIN <= gate_cell["ratio_ci_upper"], gate_cell
    assert unsure["gate"]["verdicts"] == {"map": "INCONCLUSIVE"}, unsure["gate"]

    # A two-round quick run: no interval, no verdict.
    _, tiny = scenario(lambda arm, t, r: 2.0, 2, arms=("map",), threads=(1, 8))
    assert tiny["gate"] == dict(tiny["gate"], threads=8, verdicts={"map": "NOT_EVALUABLE"}), tiny["gate"]
    assert cell(tiny, "ratio", "map", 8)["ratio_ci_lower"] is None

    # A process's output voids the round: a missing window, a wrong thread count,
    # a wrong arm, a wrong mix, two rows.
    def at(r0, build0, change):
        return lambda row, build, r: change(row) if (r == r0 and build == build0) else [row]
    for what, mutate in (
        ("a missing window", at(1, "baseline", lambda row: [])),
        ("a wrong thread count", at(0, "head", lambda row: [dict(row, threads=8) if row["threads"] == 16 else row])),
        ("a wrong arm", at(1, "head", lambda row: [dict(row, engine_key="str")])),
        ("a wrong read percentage", at(0, "baseline", lambda row: [dict(row, read_pct=100)])),
        ("two rows from one process", at(1, "head", lambda row: [row, dict(row, round=1)])),
    ):
        message = expect_error(scenario, lambda arm, t, r: 2.0, 2, ("map",), STEP2_THREADS, mutate, what=what)
        assert "void" in message and "discards the whole run" in message, message

    # A real process: the environment one window gets, a fresh samples file, and a non-zero exit.
    with tempfile.TemporaryDirectory() as tmp:
        tmpd = Path(tmp)
        fake = tmpd / "fake_bench.py"
        fake.write_text(
            "import json, os, sys\n"
            "e = os.environ\n"
            "assert e['EXPANSE_BENCH_WORKLOADS'] == '50' and e['EXPANSE_BENCH_ROUNDS'] == '1', dict(e)\n"
            "assert ',' not in e['EXPANSE_BENCH_ENGINES'] and ',' not in e['EXPANSE_BENCH_THREADS']\n"
            "assert not os.path.exists(e['EXPANSE_BENCH_SAMPLES']), 'samples file not fresh'\n"
            "code = int(e.get('FAKE_EXIT', '0'))\n"
            "row = {'workload_id': 'core_concurrency', 'engine_key': e['EXPANSE_BENCH_ENGINES'], 'engine': 'X',\n"
            "       'workload': '50% Read / 50% Write', 'read_pct': 50, 'write_rate': None,\n"
            "       'threads': int(e['EXPANSE_BENCH_THREADS']), 'round': 0, 'position': 0, 'elapsed_s': 0.5,\n"
            "       'read_ops': 10, 'write_ops': 10, 'busy': 0, 'ok': 0, 'refused': 0}\n"
            "open(e['EXPANSE_BENCH_SAMPLES'], 'a').write(json.dumps(row) + '\\n')\n"
            "sys.exit(code)\n")
        exe = tmpd / "fake_bench"
        exe.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{fake}" "$@"\n')
        exe.chmod(0o755)
        samples = tmpd / "w.jsonl"
        run_window(exe, tmpd, "set", 16, samples)
        assert read_window(samples, "set", 16)["threads"] == 16
        expect_error(run_window, exe, tmpd, "set", 16, samples, what="a reused samples file")
        os.environ["FAKE_EXIT"] = "3"
        try:
            expect_error(run_window, exe, tmpd, "map", 1, tmpd / "x.jsonl", what="a non-zero exit")
        finally:
            del os.environ["FAKE_EXIT"]

    # Refusals: the pin, the rounds, the output paths, identical executables.
    assert step2_pin_problems(STEP2_PIN, quick=False) == []
    for pin in ("0-7", "off", "none", "0-15,16"):
        assert step2_pin_problems(pin, quick=False), f"pin {pin} on a committed run"
    assert step2_pin_problems("off", quick=True) == []
    assert resolve_step2_rounds(None, quick=False) == 48 and resolve_step2_rounds(50, quick=False) == 50
    assert resolve_step2_rounds(None, quick=True) == STEP2_QUICK_ROUNDS and resolve_step2_rounds(2, quick=True) == 2
    for n, quick in ((46, False), (47, False), (49, False), (3, True), (0, True)):
        expect_error(resolve_step2_rounds, n, quick, what=f"{n} rounds (quick={quick})")
    assert resolve_step2_out(None, False, True) == STEP2_QUICK_OUT
    assert resolve_step2_out(None, False, False) == STEP2_DEFAULT_OUT
    assert resolve_step2_out(None, True, False) == STEP2_RUN2_OUT
    for path in (STEP2_DEFAULT_OUT, STEP2_RUN2_OUT):
        expect_error(resolve_step2_out, str(path), False, True, what=f"quick run writing {path.name}")
    expect_error(resolve_step2_out, None, True, True, what="--run2 under --quick")
    expect_error(resolve_step2_out, "x.json", True, False, what="--run2 with --out")
    assert executable_problems("a", "b") == [] and executable_problems("a", "a")

    # The ancestry refusal, injected and then through real git exit codes; the
    # baseline tree's materialisation and its one-file rule, on a scratch repo.
    expect_error(check_head_ancestry, False, lambda: False, what="a head without #949")
    assert check_head_ancestry(False, lambda: True) is True
    with tempfile.TemporaryDirectory() as tmp, tempfile.TemporaryDirectory() as scratch:
        repo = Path(tmp)
        ident = ["-c", "user.name=self-test", "-c", "user.email=self-test@example.invalid",
                 "-c", "commit.gpgsign=false"]

        def git(*args: str) -> str:
            return subprocess.run(["git", *ident, *args], cwd=repo, check=True,
                                  capture_output=True, text=True).stdout.strip()

        git("init", "-q")
        (repo / HARNESS).parent.mkdir(parents=True)
        (repo / HARNESS).write_text("// baseline bench\n")
        (repo / "crates" / "expanse" / "lib.rs").write_text("// engine\n")
        git("add", "-A")
        git("commit", "-q", "-m", "a")
        first = git("rev-parse", "HEAD")
        (repo / "crates" / "expanse" / "lib.rs").write_text("// engine, head\n")
        git("commit", "-q", "-am", "b")
        second = git("rev-parse", "HEAD")
        assert head_contains(first, repo) is True and head_contains(second, repo) is True
        expect_error(head_contains, "0" * 40, repo, what="an unknown commit is not an answer")
        assert head_tree_problems(repo) == []
        (repo / "crates" / "expanse" / "lib.rs").write_text("// uncommitted\n")
        assert head_tree_problems(repo), "an uncommitted engine change in the head must be refused"
        git("checkout", "-q", "--", ".")

        head_bench = Path(scratch) / "head_bench.rs"
        head_bench.write_text("// head bench\n")
        parent = Path(scratch) / "parent"
        parent.mkdir()
        tree, sha = materialise_baseline(parent, head_bench, repo=repo, ref=first)
        assert sha == first and (tree / HARNESS).read_text() == "// head bench\n"
        changed, stat = baseline_tree_changes(tree, first)
        assert changed == [HARNESS] and HARNESS in stat and baseline_tree_problems(changed) == [], changed
        (tree / "crates" / "expanse" / "lib.rs").write_text("// drifted\n")
        assert baseline_tree_problems(baseline_tree_changes(tree, first)[0]), "an extra changed file"
        git("-C", str(tree), "checkout", "-q", "--", "crates/expanse/lib.rs")
        (tree / "stray.txt").write_text("untracked\n")
        assert baseline_tree_problems(baseline_tree_changes(tree, first)[0]), "an untracked file"
        git("checkout", "-q", "--detach", first)
        assert head_contains(second, repo) is False, "a head behind the required commit"
        remove_baseline(parent, tree, repo=repo)
        assert not parent.exists() and str(tree) not in git("worktree", "list")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--threads", default=None, help="thread counts (comma-separated)")
    ap.add_argument("--workloads", default=None, help="read percentages (comma-separated)")
    ap.add_argument("--engines", default=None,
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
    ap.add_argument("--step2-gate", action="store_true",
                    help="the #568 Step 2 two-build gate run (METHODOLOGY.md section 13): head vs "
                         "1edfa952, one process per window, map and set at 1 and 16 threads")
    ap.add_argument("--run2", action="store_true",
                    help="with --step2-gate: write the second run's committed artifact")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    try:
        if args.step2_gate:
            if args.write_assets or args.check_assets:
                raise InstrumentError("--step2-gate is a measurement run; --write-assets and "
                                      "--check-assets are separate steps")
            return run_step2(args)
        if args.run2:
            raise InstrumentError("--run2 applies only with --step2-gate")
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
