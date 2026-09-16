#!/usr/bin/env python3
"""Driver for the RocksDB memtable concurrent read-scaling arm (#802).

Pre-registered in `docs/benchmarks/rocksdb_memtable/METHODOLOGY.md` section 5.
That section is the contract; this file executes it and nothing more. It does
not choose thresholds, and it does not decide verdicts.

What it owns, and why the C++ binary does not:

  * **The rounds and their order.** `bench_memtable_concurrent` runs exactly one
    (mode, readers) cell per invocation. This driver invokes it so that
    `(mode x readers)` interleaves *within* each round (AGENTS.md 8.20.2) --
    a block of all R=1 followed by a block of all R=7 confounds thermal drift
    with the effect the sweep is trying to see.

  * **The load snapshots.** A concurrent artifact owes, per timed cell,
    `load.foreign_busy_cpus` -- the host's busy CPU over that cell minus the
    runner's own children's -- and `host.scaling_governor_by_cpu` for the pin
    set (`scripts/check_bench_provenance.py`). A process boundary per cell is
    what makes that measurable at all, which is the other reason the binary
    emits one cell rather than a sweep.

  * **The intervals.** Paired BCa 95% bootstrap on `S(R) = T(R)/T(1)`, paired
    because both arms of the ratio come from the same round. The binary emits
    raw per-cell throughput and never an interval or a ratio.

Usage:
    python3 docs/benchmarks/rocksdb_memtable/scripts/concurrent_read_scaling.py \
        --out docs/benchmarks/rocksdb_memtable/results/baseline_concurrent_reads.json
    python3 docs/benchmarks/rocksdb_memtable/scripts/concurrent_read_scaling.py \
        --readers 1,2,3,4,5,6,7 --modes idle --out baseline_concurrent_reads_heldout.json
    python3 docs/benchmarks/rocksdb_memtable/scripts/concurrent_read_scaling.py --quick
    python3 docs/benchmarks/rocksdb_memtable/scripts/concurrent_read_scaling.py --self-test
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_pin  # noqa: E402
import bench_provenance as prov  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

BENCH = REPO_ROOT / "integrations" / "rocksdb" / "build" / "bench_memtable_concurrent"
DEFAULT_OUT = REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable" / "results" / "baseline_concurrent_reads.json"

#: METHODOLOGY section 5.2. `idle` is the control; `free` is reported and never gated.
MODES = ("idle", "paced", "free")
#: One writer thread accompanies every cell, so R = 7 is 8 threads on 8 physical
#: P-cores. R = 8 would put 9 runnable threads on 8 cores.
READERS = (1, 2, 4, 7)
PACED_RATE = 250000.0
CSV_FIELDS = ("round", "writer_mode", "readers", "read_ops", "write_ops", "elapsed_s",
              "read_mops", "writer_exhausted")
#: `ExpanseMemTableRep::SeekLockScope`, as the binary's `--lock` spells it:
#: `full` holds `mutex_` for the whole locate phase, `trie` only around
#: `expanse_map_prev_at_or_before` (the #802 narrowed-mutex arm), and `opt`
#: takes no lock in the locate phase, reading the trie through a per-thread
#: sync-map reader handle (METHODOLOGY section 5.16).
LOCK_SCOPES = ("full", "trie", "opt")


def parse_row(text: str) -> dict:
    """Parses the single CSV data row the binary emits, or raises.

    Fails loudly on a missing or malformed row rather than returning a default:
    a cell that produced nothing must stop the sweep, not contribute a zero
    (AGENTS.md 8.1).
    """
    rows = [ln.strip() for ln in text.splitlines()
            if ln.strip() and not ln.startswith("#") and not ln.startswith("round,")]
    if len(rows) != 1:
        raise RuntimeError(f"expected exactly one CSV data row, got {len(rows)}:\n{text}")
    parts = rows[0].split(",")
    if len(parts) != len(CSV_FIELDS):
        raise RuntimeError(f"expected {len(CSV_FIELDS)} CSV fields, got {len(parts)}: {rows[0]!r}")
    out = dict(zip(CSV_FIELDS, parts))
    # The binary names the scope it built, on its own line. A driver that asked
    # for one scope and read a row from the other would publish the wrong arm.
    locks = [ln.split("=", 1)[1].strip() for ln in text.splitlines() if ln.startswith("# lock_scope=")]
    if len(locks) != 1 or locks[0] not in LOCK_SCOPES:
        raise RuntimeError(f"expected one `# lock_scope=` line naming one of {LOCK_SCOPES}, got {locks}")
    # The reader handles the rep registered over the cell (section 5.16 has the
    # artifact carry it per cell): one per reader thread under `opt`, 0 otherwise.
    handles = [ln.split("=", 1)[1].strip() for ln in text.splitlines() if ln.startswith("# reader_handles=")]
    if len(handles) != 1 or not handles[0].isdigit():
        raise RuntimeError(f"expected one `# reader_handles=<count>` line, got {handles}")
    return {
        "round": int(out["round"]),
        "writer_mode": out["writer_mode"],
        "readers": int(out["readers"]),
        "read_ops": int(out["read_ops"]),
        "write_ops": int(out["write_ops"]),
        "elapsed_s": float(out["elapsed_s"]),
        "read_mops": float(out["read_mops"]),
        # 1 when the writer ran out of pre-encoded keys and stopped before
        # the window closed. Expected in the free cell, which is reported and
        # never gated; the binary makes it fatal for a paced cell, where it
        # would corrupt the duty cycle.
        "writer_exhausted": int(out["writer_exhausted"]),
        "lock_scope": locks[0],
        "reader_handles": int(handles[0]),
    }


def duty_cycle(write_ops: int, elapsed_s: float, insert_ns: float) -> float:
    """Writer's measured lock duty: achieved rate x insert cost.

    Computed from the achieved write rate, never from the offered one --
    METHODOLOGY section 5.2 requires the cell to record what the writer actually
    managed, because a paced writer can miss its schedule.
    """
    if elapsed_s <= 0.0:
        raise ValueError(f"elapsed_s must be > 0, got {elapsed_s}")
    return (write_ops / elapsed_s) * insert_ns * 1e-9


def parse_readers(text: str) -> tuple[int, ...]:
    """`--readers 1,3,5`: distinct reader counts, ascending, always including 1.

    `S(R) = T(R) / T(1)` pairs each cell with the same round's `R = 1` cell, so a
    sweep without `R = 1` pays for its cells and returns no ratio. That is
    refused at the command line rather than found after the host is spent
    (AGENTS.md 8.1).
    """
    try:
        vals = [int(v) for v in text.split(",")]
    except ValueError:
        raise argparse.ArgumentTypeError(f"--readers takes comma-separated integers, got {text!r}") from None
    if any(v < 1 for v in vals):
        raise argparse.ArgumentTypeError(f"reader counts must be >= 1, got {text!r}")
    if len(set(vals)) != len(vals):
        raise argparse.ArgumentTypeError(f"reader counts repeat in {text!r}")
    if 1 not in vals:
        raise argparse.ArgumentTypeError(f"--readers must include 1, the denominator of S(R); got {text!r}")
    return tuple(sorted(vals))


def parse_modes(text: str) -> tuple[str, ...]:
    """`--modes idle,paced`: writer modes from METHODOLOGY section 5.2, in its order."""
    vals = text.split(",")
    unknown = [v for v in vals if v not in MODES]
    if unknown or not vals or len(set(vals)) != len(vals):
        raise argparse.ArgumentTypeError(f"--modes takes distinct values from {MODES}, got {text!r}")
    return tuple(m for m in MODES if m in vals)


def parse_lock_scopes(text: str) -> tuple[str, ...]:
    """`--lock-scopes full,trie`: seek lock scopes to interleave, in LOCK_SCOPES order."""
    vals = text.split(",")
    unknown = [v for v in vals if v not in LOCK_SCOPES]
    if unknown or len(set(vals)) != len(vals):
        raise argparse.ArgumentTypeError(f"--lock-scopes takes distinct values from {LOCK_SCOPES}, got {text!r}")
    return tuple(s for s in LOCK_SCOPES if s in vals)


def paced_rate_report(rows: list[dict], offered: float) -> dict:
    """The paced writer's achieved rate per reader count, and the cells not to gate.

    Per `R`, never averaged. A mean duty over every paced cell is how the
    `R = 7` shortfall went unseen in METHODOLOGY section 5.7: the `R <= 4` cells
    held 250,000 inserts/s and the `R = 7` cells reached ~140,000, and one mean
    over all four read as a writer on schedule (section 5.8).

    A paced cell is flagged, and METHODOLOGY section 5.9 excludes it from
    gating, when its achieved rate is above the offered rate or its writer ran
    out of keys. A rate below the offered one is recorded, not flagged: section
    5.9 derives that a shortfall of any size leaves H1 deciding the same
    question. The comparison is literal. The schedule starts after the window
    opens and inserts first at `n = 0`, so a writer exactly on schedule could in
    principle exceed the offered rate by one insert over the window
    (`1 / elapsed_s` inserts/s); thread start and join latency of that size
    have not been observed, and a cell that did so would be reported rather
    than argued away.
    """
    per: dict[str, dict] = {}
    flags: list[dict] = []
    for r in rows:
        if r["writer_mode"] != "paced":
            continue
        if r["elapsed_s"] <= 0.0:
            raise ValueError(f"elapsed_s must be > 0, got {r['elapsed_s']} for {r.get('cell')}")
        rate = r["write_ops"] / r["elapsed_s"]
        e = per.setdefault(str(r["readers"]), {"rounds": 0, "achieved_min_ops_per_s": rate,
                                               "achieved_max_ops_per_s": rate})
        e["rounds"] += 1
        e["achieved_min_ops_per_s"] = min(e["achieved_min_ops_per_s"], rate)
        e["achieved_max_ops_per_s"] = max(e["achieved_max_ops_per_s"], rate)
        reasons = []
        if rate > offered:
            reasons.append("above_offered")
        if r.get("writer_exhausted"):
            reasons.append("writer_exhausted")
        if reasons:
            flags.append({"cell": r.get("cell"), "readers": r["readers"], "round": r["round"],
                          "achieved_ops_per_s": rate, "reasons": reasons})
    for e in per.values():
        e["max_shortfall_fraction"] = 1.0 - e["achieved_min_ops_per_s"] / offered
    return {
        "offered_ops_per_s": offered,
        "per_readers": dict(sorted(per.items(), key=lambda kv: int(kv[0]))),
        "flags": flags,
        "rule": ("METHODOLOGY section 5.9: a paced cell whose achieved rate is above the "
                 "offered rate, or whose writer ran out of keys, is reported and not gated. "
                 "Read per_readers before gating any paced cell; a duty averaged over R hides "
                 "a short cell."),
    }


def scaling_ratios(rows: list[dict], mode: str, lock: str = "full") -> dict:
    """Paired BCa 95% interval on S(R) = T(R)/T(1) for one writer mode.

    Paired: the numerator and denominator of each resampled ratio come from the
    same round, so the interval carries the round-to-round covariance instead of
    treating the two arms as independent samples.
    """
    by_round: dict[int, dict[int, float]] = {}
    for r in rows:
        if r["writer_mode"] != mode or r.get("lock_scope", "full") != lock:
            continue
        by_round.setdefault(r["round"], {})[r["readers"]] = r["read_mops"]

    out: dict[str, dict] = {}
    rounds = sorted(by_round)
    for R in sorted({r["readers"] for r in rows
                     if r["writer_mode"] == mode and r.get("lock_scope", "full") == lock}):
        if R == 1:
            continue
        per_round = []
        for rd in rounds:
            cell = by_round[rd]
            if 1 not in cell or R not in cell or cell[1] <= 0.0:
                continue
            per_round.append(cell[R] / cell[1])
        if len(per_round) < 3:
            # BCa needs n >= 3; say so rather than emit a degenerate interval.
            out[f"S({R})"] = {"point": None, "ci": None, "n": len(per_round),
                              "why_no_interval": "fewer than 3 paired rounds"}
            continue
        point, lo, hi, ci_method = bca_bootstrap_ci_with_method(per_round)
        out[f"S({R})"] = {"point": point, "ci": [lo, hi], "ci_method": ci_method,
                          "n": len(per_round), "rounds_raw": per_round}
    return out


def lock_scope_ratios(rows: list[dict], mode: str, variant: str = "trie",
                      default: str = "full") -> dict:
    """Paired BCa 95% intervals on `S_variant(R) / S_default(R)`, and the `R = 1` control.

    AGENTS.md 8.20.2's decision statistic for a concurrency change: the ratio of
    the two arms' scaling factors at `R >= 2`, with `T_variant(1) / T_default(1)`
    reported as `T(1)`, the control cell. Each round's quotient uses that
    round's four cells only, `(T_v(R) / T_v(1)) / (T_d(R) / T_d(1))`, so drift
    shared by both arms within a round cancels instead of widening the interval.
    """
    by: dict[tuple[str, int], dict[int, float]] = {}
    for r in rows:
        if r["writer_mode"] == mode:
            by.setdefault((r.get("lock_scope", "full"), r["round"]), {})[r["readers"]] = r["read_mops"]
    rounds = sorted({rd for _, rd in by})
    out: dict[str, dict] = {}
    for R in sorted({r["readers"] for r in rows if r["writer_mode"] == mode}):
        per_round = []
        for rd in rounds:
            v, d = by.get((variant, rd), {}), by.get((default, rd), {})
            need = (1,) if R == 1 else (1, R)
            if not all(cell.get(k, 0.0) > 0.0 for cell in (v, d) for k in need):
                continue
            per_round.append(v[1] / d[1] if R == 1 else (v[R] / v[1]) / (d[R] / d[1]))
        key = "T(1)" if R == 1 else f"S({R})"
        if len(per_round) < 3:
            out[key] = {"point": None, "ci": None, "n": len(per_round),
                        "why_no_interval": "fewer than 3 paired rounds"}
            continue
        point, lo, hi, ci_method = bca_bootstrap_ci_with_method(per_round)
        out[key] = {"point": point, "ci": [lo, hi], "ci_method": ci_method,
                    "n": len(per_round), "rounds_raw": per_round}
    return out


#: The paired scope ratios METHODOLOGY section 5.16 has the artifact carry, as
#: `variant/default`: O1 reads `opt/full`, O2 and O3 `opt/trie`, and `trie/full`
#: is reported beside them.
SCOPE_PAIRS = (("opt", "full"), ("opt", "trie"), ("trie", "full"))


def scope_pair_ratios(rows: list[dict], mode: str, variant: str, default: str) -> dict:
    """Section 5.16's three statistics for `variant/default`, per reader count.

    For each `R`, from each round's cells only:

    - `T(R)`: the absolute ratio `T_variant(R) / T_default(R)`; `T(1)` is the
      single-reader control, `T(7)` the gated absolute ratio;
    - `S(R)` for `R >= 2`: the scaling ratio `(T_v(R) / T_v(1)) / (T_d(R) / T_d(1))`,
      the same quotient `lock_scope_ratios` computes.

    Each carries a BCa 95% interval over the rounds and its `rounds_raw`, or no
    interval and the reason when fewer than 3 rounds pair.
    """
    by: dict[tuple[str, int], dict[int, float]] = {}
    for r in rows:
        if r["writer_mode"] == mode:
            by.setdefault((r.get("lock_scope", "full"), r["round"]), {})[r["readers"]] = r["read_mops"]
    rounds = sorted({rd for _, rd in by})
    out: dict[str, dict] = {}

    def summarise(key: str, per_round: list[float]) -> None:
        if len(per_round) < 3:
            out[key] = {"point": None, "ci": None, "n": len(per_round),
                        "why_no_interval": "fewer than 3 paired rounds"}
            return
        point, lo, hi, ci_method = bca_bootstrap_ci_with_method(per_round)
        out[key] = {"point": point, "ci": [lo, hi], "ci_method": ci_method,
                    "n": len(per_round), "rounds_raw": per_round}

    for R in sorted({r["readers"] for r in rows if r["writer_mode"] == mode}):
        absolute, scaling = [], []
        for rd in rounds:
            v, d = by.get((variant, rd), {}), by.get((default, rd), {})
            if v.get(R, 0.0) > 0.0 and d.get(R, 0.0) > 0.0:
                absolute.append(v[R] / d[R])
            if R != 1 and all(cell.get(k, 0.0) > 0.0 for cell in (v, d) for k in (1, R)):
                scaling.append((v[R] / v[1]) / (d[R] / d[1]))
        summarise(f"T({R})", absolute)
        if R != 1:
            summarise(f"S({R})", scaling)
    return out


def preflight(bench: Path) -> None:
    """Run one throwaway cell so a broken binary reports before the sweep.

    The first attempt on the reference host died at cell 1 of 60 because the
    binary could not load `libexpanse.so` -- recoverable, but only after the
    build had been paid for, and the traceback named a `RuntimeError` about an
    exit code rather than the loader. Check once, up front, and say what is
    actually wrong (AGENTS.md 8.1).
    """
    cwd = str(Path(bench).resolve().parent.parent)
    res = subprocess.run([str(bench), "--mode", "idle", "--readers", "1",
                          "--window-seconds", "0.05"],
                         capture_output=True, text=True, cwd=cwd)
    if res.returncode == 0:
        return
    hint = ""
    if "shared object" in res.stderr or "image not found" in res.stderr \
            or "dyld" in res.stderr or res.returncode == 127:
        hint = ("\nThe binary cannot load libexpanse. The Makefile links it against the "
                "relative path ../../target/release/libexpanse.{so,dylib}, so it only "
                "resolves when run from integrations/rocksdb/ -- this driver sets that as "
                "the working directory. Check the release library exists: "
                "cargo build --release -p expanse-capi")
    raise RuntimeError(
        f"preflight failed: {bench} exited {res.returncode} (cwd {cwd})\n"
        f"stderr:\n{res.stderr}{hint}")


def run_sweep(bench: Path, rounds: int, window_s: float, readers: tuple,
              modes: tuple, paced_rate: float, provenance: dict,
              lock_scopes: tuple = ("full",)) -> list[dict]:
    preflight(bench)
    rows: list[dict] = []
    # `new_provenance` already took the opening snapshot; a second one here
    # would leave two cells labelled `start` and make `since` ambiguous.
    for rd in range(rounds):
        # Interleave (mode x readers) within the round, not across it.
        # (lock scope x mode x readers) interleaves within the round, so both
        # arms of a paired ratio share each round's drift (AGENTS.md 8.20.2).
        for lock, mode in [(s, m) for s in lock_scopes for m in modes]:
            for R in readers:
                label = (f"cell:{mode}:R{R}:round{rd}" if tuple(lock_scopes) == ("full",)
                         else f"cell:{lock}:{mode}:R{R}:round{rd}")
                start = prov.begin_cell(provenance, label)
                cmd = [str(bench), "--mode", mode, "--readers", str(R),
                       "--round", str(rd), "--window-seconds", str(window_s),
                       "--paced-rate", str(paced_rate), "--lock", lock]
                # Run from the integration directory, as `make -C
                # integrations/rocksdb bench-concurrent` does. The Makefile links
                # the binary against the RELATIVE path
                # `../../target/release/libexpanse.so`, and because a cargo
                # cdylib carries no SONAME the linker records that path verbatim
                # as DT_NEEDED. A DT_NEEDED containing a slash is resolved
                # against the process's working directory and ignores any rpath,
                # so the binary only loads from inside integrations/rocksdb/.
                # Invoking it by absolute path from the repo root failed with
                # `cannot open shared object file` on Linux (run 34715695469);
                # macOS resolved it anyway, which is why a local smoke passed.
                # Setting cwd rather than relinking keeps the single-threaded
                # bench's linkage -- and so its published cells -- untouched.
                res = subprocess.run(cmd, capture_output=True, text=True,
                                     cwd=str(Path(bench).resolve().parent.parent))
                if res.returncode != 0:
                    raise RuntimeError(
                        f"{label}: {' '.join(cmd)} exited {res.returncode}\n"
                        f"stdout:\n{res.stdout}\nstderr:\n{res.stderr}")
                row = parse_row(res.stdout)
                if row["lock_scope"] != lock:
                    raise RuntimeError(f"{label}: asked for --lock {lock}, the binary ran "
                                       f"{row['lock_scope']}")
                row["load"] = prov.end_cell(start)
                row["cell"] = label
                rows.append(row)
    prov.add_load(provenance, "end")
    return rows


def build_artifact(rows: list[dict], provenance: dict, insert_ns: float,
                   window_s: float, paced_rate: float) -> dict:
    if not rows:
        raise ValueError("build_artifact needs at least one row")
    scopes = [s for s in LOCK_SCOPES if any(r.get("lock_scope", "full") == s for r in rows)]
    first = [r for r in rows if r.get("lock_scope", "full") == scopes[0]]
    modes_run = [m for m in MODES if any(r["writer_mode"] == m for r in rows)]
    cells = []
    for r in rows:
        c = dict(r)
        c["writer_duty_cycle"] = (
            duty_cycle(r["write_ops"], r["elapsed_s"], insert_ns)
            if r["writer_mode"] != "idle" else 0.0
        )
        c["rounds_raw"] = [{"round": r["round"], "read_mops": r["read_mops"],
                            "read_ops": r["read_ops"], "write_ops": r["write_ops"],
                            "elapsed_s": r["elapsed_s"]}]
        cells.append(c)

    payload = {
        "schema": "expanse.baseline.v1",
        "kind": "wall_clock_bca",
        "suite": "rocksdb_concurrent",
        "fixture": "integrations/rocksdb/benches/bench_memtable_concurrent.cc",
        "workload_id": "rocksdb_memtable_concurrent_read_scaling",
        "pre_registration": "docs/benchmarks/rocksdb_memtable/METHODOLOGY.md section 5",
        "settings": {"window_seconds": window_s, "paced_rate_ops_per_s": paced_rate,
                     "insert_ns_used_for_duty": insert_ns,
                     "readers": sorted({r["readers"] for r in rows}),
                     "modes": modes_run,
                     "lock_scopes": scopes},
        "cells": cells,
        # `scaling` and `paced_rate_check` read the first scope that ran, which
        # is `full` whenever it ran, so every reader of a single-scope artifact
        # sees the shape it always has. A two-scope run adds the keys below.
        "scaling": {mode: scaling_ratios(rows, mode, scopes[0])
                    for mode in sorted({r["writer_mode"] for r in first})},
        "paced_rate_check": paced_rate_report(first, paced_rate),
        "verdicts": None,
        "why_no_verdicts": (
            "This driver emits the intervals and decides nothing. Verdicts are read from two "
            "runs against the pre-registration in force; for the runs METHODOLOGY section 5.9 "
            "fixes, scripts/concurrent_verdicts.py applies its rules."
        ),
    }
    if len(scopes) > 1:
        payload["scaling_by_lock_scope"] = {
            s: {mode: scaling_ratios(rows, mode, s) for mode in modes_run} for s in scopes}
        payload["paced_rate_check_by_lock_scope"] = {
            s: paced_rate_report([r for r in rows if r.get("lock_scope", "full") == s], paced_rate)
            for s in scopes}
        if {"full", "trie"} <= set(scopes):
            payload["lock_scope_ratio"] = {mode: lock_scope_ratios(rows, mode) for mode in modes_run}
        # METHODOLOGY section 5.16: every pair among the scopes that ran, each
        # with the absolute ratio `T(R)` beside the scaling ratio `S(R)`.
        pairs = [f"{v}/{d}" for v, d in SCOPE_PAIRS if {v, d} <= set(scopes)]
        if "opt" in scopes:
            payload["scope_pair_ratios"] = {
                pair: {mode: scope_pair_ratios(rows, mode, *pair.split("/")) for mode in modes_run}
                for pair in pairs}
            payload["reader_handles_by_cell"] = [
                {"cell": r.get("cell"), "lock_scope": r["lock_scope"], "writer_mode": r["writer_mode"],
                 "readers": r["readers"], "round": r["round"], "reader_handles": r.get("reader_handles")}
                for r in rows]
    # `attach` RETURNS the carrying dict; it does not mutate in place. Dropping
    # the return shipped an artifact with no provenance block at all, which the
    # self-test below is what caught.
    return prov.attach(payload, provenance)


def self_test() -> int:
    fails: list[str] = []

    def check(name, got, want):
        if got != want:
            fails.append(f"{name}: got {got!r}, want {want!r}")

    # --- CSV parsing ------------------------------------------------------
    good = ("# rocksdb_memtable_concurrent_read_scaling\n"
            "round,writer_mode,readers,read_ops,write_ops,elapsed_s,read_mops,writer_exhausted\n"
            "# lock_scope=full\n"
            "# reader_handles=0\n"
            "2,paced,4,123456,6789,2.001000,0.0617,0\n")
    row = parse_row(good)
    check("reader_handles", row["reader_handles"], 0)
    opt_row = parse_row(good.replace("=full", "=opt").replace("# reader_handles=0", "# reader_handles=4"))
    check("lock_scope opt", opt_row["lock_scope"], "opt")
    check("reader_handles opt", opt_row["reader_handles"], 4)
    for name, text in (("no reader_handles line", good.replace("# reader_handles=0\n", "")),
                       ("two reader_handles lines", good.replace("# reader_handles=0\n",
                                                                 "# reader_handles=0\n# reader_handles=1\n")),
                       ("non-numeric reader_handles", good.replace("=0\n2,", "=x\n2,"))):
        try:
            parse_row(text)
        except RuntimeError:
            pass
        except Exception as exc:  # noqa: BLE001
            fails.append(f"{name}: raised {type(exc).__name__}, expected RuntimeError")
        else:
            fails.append(f"{name}: did not raise")
    check("writer_exhausted", row["writer_exhausted"], 0)
    check("writer_exhausted parses 1",
          parse_row(good.replace(",0.0617,0", ",0.0617,1"))["writer_exhausted"], 1)
    check("round", row["round"], 2)
    check("mode", row["writer_mode"], "paced")
    check("readers", row["readers"], 4)
    check("read_ops", row["read_ops"], 123456)
    check("write_ops", row["write_ops"], 6789)
    check("lock_scope", row["lock_scope"], "full")
    check("lock_scope trie", parse_row(good.replace("=full", "=trie"))["lock_scope"], "trie")
    for name, text in (("no lock line", good.replace("# lock_scope=full\n", "")),
                       ("two lock lines", good.replace("# lock_scope=full\n", "# lock_scope=full\n# lock_scope=trie\n")),
                       ("unknown lock", good.replace("=full", "=rw"))):
        try:
            parse_row(text)
        except RuntimeError:
            pass
        except Exception as exc:  # noqa: BLE001
            fails.append(f"{name}: raised {type(exc).__name__}, expected RuntimeError")
        else:
            fails.append(f"{name}: did not raise")
    for name, text in (("no data row", "# only a comment\n"),
                       ("two data rows", good + "3,idle,1,1,0,1.0,0.1,0\n"),
                       ("short row", "1,idle,2\n")):
        try:
            parse_row(text)
        except RuntimeError:
            pass
        except Exception as exc:  # noqa: BLE001
            fails.append(f"{name}: raised {type(exc).__name__}, expected RuntimeError")
        else:
            fails.append(f"{name}: did not raise")

    # --- section 5.16's scope pairs ---------------------------------------
    # Five rounds; per round opt reads 2x full at R=1 and 3x at R=7, and trie
    # reads 1.5x full at R=7. Scaling T(7)/T(1) is 2 for full and 3 for both
    # trie and opt. So opt/full: T(1)=2, T(7)=3, S(7)=1.5; opt/trie: T(1)=2,
    # T(7)=2, S(7)=1; trie/full: T(1)=1, T(7)=1.5, S(7)=1.5.
    pair_rows = []
    for rd in range(5):
        base = 1.0 + 0.1 * rd
        for lock, t1, t7 in (("full", base, 2 * base), ("trie", base, 3 * base), ("opt", 2 * base, 6 * base)):
            pair_rows.append({"round": rd, "writer_mode": "idle", "readers": 1, "read_mops": t1,
                              "lock_scope": lock})
            pair_rows.append({"round": rd, "writer_mode": "idle", "readers": 7, "read_mops": t7,
                              "lock_scope": lock})
    for (v, d), want in {("opt", "full"): {"T(1)": 2.0, "T(7)": 3.0, "S(7)": 1.5},
                         ("opt", "trie"): {"T(1)": 2.0, "T(7)": 2.0, "S(7)": 1.0},
                         ("trie", "full"): {"T(1)": 1.0, "T(7)": 1.5, "S(7)": 1.5}}.items():
        got = scope_pair_ratios(pair_rows, "idle", v, d)
        for key, value in want.items():
            if got.get(key, {}).get("point") is None or abs(got[key]["point"] - value) > 1e-9:
                fails.append(f"scope_pair_ratios {v}/{d} {key}: got {got.get(key)}, want {value}")
            elif len(got[key].get("rounds_raw", [])) != 5:
                fails.append(f"scope_pair_ratios {v}/{d} {key} carries {len(got[key].get('rounds_raw', []))} rounds_raw")
    thin = scope_pair_ratios([r for r in pair_rows if r["round"] < 2], "idle", "opt", "full")
    if thin["T(7)"]["ci"] is not None or "fewer than 3" not in thin["T(7)"].get("why_no_interval", ""):
        fails.append(f"two paired rounds must carry no interval and say why: {thin['T(7)']}")
    # The call site: a three-scope artifact carries every pair and every cell's handle count.
    art_rows = [dict(r, read_ops=1, write_ops=0, elapsed_s=1.0, writer_exhausted=0, reader_handles=7 if r["readers"] == 7 and r["lock_scope"] == "opt" else 0,
                     cell=f"cell:{r['lock_scope']}:idle:R{r['readers']}:round{r['round']}",
                     load={"since": "start", "wall_s": 1.0, "busy_cpus_since_prev": 1.0,
                           "own_busy_cpus": 1.0, "foreign_busy_cpus": 0.0}) for r in pair_rows]
    p3 = prov.new_provenance("rocksdb_concurrent", 802, "T(R)/T(1)", repo_root=REPO_ROOT)
    art3 = build_artifact(art_rows, p3, 226.142, 2.0, PACED_RATE)
    check("three-scope artifact pairs", sorted(art3.get("scope_pair_ratios", {})), ["opt/full", "opt/trie", "trie/full"])
    check("three-scope artifact T(7) opt/trie", round(art3["scope_pair_ratios"]["opt/trie"]["idle"]["T(7)"]["point"], 9), 2.0)
    check("handle count per cell", sorted({(c["lock_scope"], c["readers"], c["reader_handles"])
                                           for c in art3.get("reader_handles_by_cell", [])}),
          [("full", 1, 0), ("full", 7, 0), ("opt", 1, 0), ("opt", 7, 7), ("trie", 1, 0), ("trie", 7, 0)])
    two = build_artifact([r for r in art_rows if r["lock_scope"] != "opt"], p3, 226.142, 2.0, PACED_RATE)
    if "scope_pair_ratios" in two:
        fails.append("a run without opt must not carry section 5.16's scope pairs")

    # --- duty cycle, from the ACHIEVED rate -------------------------------
    # 250,000 inserts in 1 s at 226.142 ns each = 5.654% of wall time.
    d = duty_cycle(250000, 1.0, 226.14201718)
    if abs(d - 0.05653550) > 1e-6:
        fails.append(f"duty_cycle: got {d}, want ~0.0565355")
    check("idle duty", duty_cycle(0, 1.0, 226.142), 0.0)
    try:
        duty_cycle(1, 0.0, 226.142)
    except ValueError:
        pass
    else:
        fails.append("duty_cycle with elapsed_s=0 did not raise")

    # --- paired scaling ---------------------------------------------------
    # A perfectly flat structure: every round has T(4) == T(1), so S(4) == 1.
    flat = []
    for rd in range(5):
        base = 1.0 + 0.01 * rd
        flat.append({"round": rd, "writer_mode": "paced", "readers": 1, "read_mops": base})
        flat.append({"round": rd, "writer_mode": "paced", "readers": 4, "read_mops": base})
    s_flat = scaling_ratios(flat, "paced")
    if abs(s_flat["S(4)"]["point"] - 1.0) > 1e-9:
        fails.append(f"flat S(4) point: {s_flat['S(4)']['point']}")
    lo, hi = s_flat["S(4)"]["ci"]
    if not (abs(lo - 1.0) < 1e-9 and abs(hi - 1.0) < 1e-9):
        fails.append(f"flat S(4) interval should be degenerate at 1.0, got [{lo}, {hi}]")
    # And the cell must SAY it is degenerate. A ratio that is identically 1.0 in
    # every round gives a one-point bootstrap distribution, so BCa's corrections
    # are vacuous; an artifact that records `bca` there is claiming a
    # construction that did not happen (AGENTS.md §8.1, #880).
    if s_flat["S(4)"].get("ci_method") != "degenerate":
        fails.append(f"flat S(4) must be labelled degenerate, got "
                     f"{s_flat['S(4)'].get('ci_method')!r}")
    # Pairing matters: the same marginals with the ratio varying per round must
    # still produce a ratio near 4, which an unpaired treatment would smear.
    linear = []
    for rd in range(5):
        base = 1.0 + 0.5 * rd          # strong round-to-round drift
        linear.append({"round": rd, "writer_mode": "paced", "readers": 1, "read_mops": base})
        linear.append({"round": rd, "writer_mode": "paced", "readers": 4, "read_mops": 4 * base})
    s_lin = scaling_ratios(linear, "paced")
    if abs(s_lin["S(4)"]["point"] - 4.0) > 1e-9:
        fails.append(f"paired S(4) under drift: {s_lin['S(4)']['point']} (pairing lost?)")
    if s_lin["S(4)"].get("ci_method") != "degenerate":
        fails.append(f"paired S(4) under exact 4x drift is still a one-point bootstrap, "
                     f"got {s_lin['S(4)'].get('ci_method')!r}")
    # A cell whose ratio actually varies between rounds is the `bca` case, and
    # must be labelled as one: a cell list where every label reads `degenerate`
    # would also satisfy the two checks above.
    jittered = []
    for rd, (t1, t4) in enumerate(((1.0, 3.6), (1.1, 4.6), (0.9, 3.4),
                                   (1.05, 4.3), (1.2, 4.5))):
        jittered.append({"round": rd, "writer_mode": "paced", "readers": 1, "read_mops": t1})
        jittered.append({"round": rd, "writer_mode": "paced", "readers": 4, "read_mops": t4})
    s_jit = scaling_ratios(jittered, "paced")
    if s_jit["S(4)"].get("ci_method") != "bca":
        fails.append(f"a varying S(4) should be a clean BCa interval, got "
                     f"{s_jit['S(4)'].get('ci_method')!r}")
    # Too few rounds: report why, never a degenerate interval.
    few = [{"round": 0, "writer_mode": "idle", "readers": 1, "read_mops": 1.0},
           {"round": 0, "writer_mode": "idle", "readers": 2, "read_mops": 2.0}]
    s_few = scaling_ratios(few, "idle")
    if s_few["S(2)"]["ci"] is not None or "fewer than 3" not in s_few["S(2)"]["why_no_interval"]:
        fails.append(f"n<3 should carry no interval and say why: {s_few['S(2)']}")

    # --- artifact shape the provenance gate reads -------------------------
    rows = []
    for rd in range(4):
        for mode in ("idle", "paced"):
            for R in (1, 2):
                rows.append({"round": rd, "writer_mode": mode, "readers": R,
                             "read_ops": 1000 * R, "write_ops": 0 if mode == "idle" else 500,
                             "elapsed_s": 1.0, "read_mops": 0.1 * R, "writer_exhausted": 0,
                             "cell": f"cell:{mode}:R{R}:round{rd}",
                             "load": {"since": "start", "wall_s": 1.0,
                                      "busy_cpus_since_prev": 1.5, "own_busy_cpus": 1.2,
                                      "foreign_busy_cpus": 0.3}})
    p = prov.new_provenance("rocksdb_concurrent", 802, "T(R)/T(1)", repo_root=REPO_ROOT)
    art = build_artifact(rows, p, 226.142, 2.0, PACED_RATE)
    for key in ("schema", "cells", "scaling", "provenance"):
        if key not in art:
            fails.append(f"artifact missing {key}")
    if art.get("provenance", {}).get("host") is None:
        fails.append("artifact provenance carries no host block")
    if "estimators" not in art.get("provenance", {}):
        fails.append("artifact provenance carries no estimators block")
    if "core_pin" not in art.get("provenance", {}):
        fails.append("artifact provenance does not record the core pin")
    for c in art["cells"]:
        if not c.get("rounds_raw"):
            fails.append("a cell carries no rounds_raw")
            break
        fb = c.get("load", {}).get("foreign_busy_cpus")
        if not isinstance(fb, (int, float)) or isinstance(fb, bool):
            fails.append("a cell carries no numeric load.foreign_busy_cpus")
            break
    # --- paced rate, per R, and the cells section 5.9 does not gate --------
    pr_rows = [
        {"round": 0, "writer_mode": "paced", "readers": 1, "write_ops": 500000,
         "elapsed_s": 2.0001, "writer_exhausted": 0, "cell": "p1"},
        {"round": 1, "writer_mode": "paced", "readers": 1, "write_ops": 499000,
         "elapsed_s": 2.0, "writer_exhausted": 0, "cell": "p1b"},
        {"round": 0, "writer_mode": "paced", "readers": 7, "write_ops": 280000,
         "elapsed_s": 2.0, "writer_exhausted": 0, "cell": "p7"},
        {"round": 0, "writer_mode": "paced", "readers": 4, "write_ops": 500010,
         "elapsed_s": 2.0, "writer_exhausted": 0, "cell": "p4"},
        {"round": 0, "writer_mode": "paced", "readers": 2, "write_ops": 120000,
         "elapsed_s": 2.0, "writer_exhausted": 1, "cell": "p2"},
        {"round": 0, "writer_mode": "idle", "readers": 7, "write_ops": 900000,
         "elapsed_s": 2.0, "writer_exhausted": 0, "cell": "i7"},
    ]
    rep = paced_rate_report(pr_rows, PACED_RATE)
    check("per-R keys, paced only, in R order", list(rep["per_readers"]), ["1", "2", "4", "7"])
    check("R=1 rounds", rep["per_readers"]["1"]["rounds"], 2)
    check("R=1 min is the worse round", rep["per_readers"]["1"]["achieved_min_ops_per_s"], 249500.0)
    check("R=7 shortfall is reported, not averaged away",
          round(rep["per_readers"]["7"]["max_shortfall_fraction"], 6), 0.44)
    check("flagged cells", sorted((f["cell"], tuple(f["reasons"])) for f in rep["flags"]),
          [("p2", ("writer_exhausted",)), ("p4", ("above_offered",))])
    # A writer 44% short is recorded and NOT flagged: section 5.9 derives that a
    # shortfall of any size leaves H1 deciding the same question.
    if any(f["cell"] == "p7" for f in rep["flags"]):
        fails.append("a short paced cell must be reported per R, not flagged")
    try:
        paced_rate_report([dict(pr_rows[0], elapsed_s=0.0)], PACED_RATE)
    except ValueError:
        pass
    else:
        fails.append("paced_rate_report with elapsed_s=0 did not raise")
    # The call site: the artifact carries the report built from ITS rows. One
    # paced cell above the offered rate must surface in the artifact itself.
    fast = [dict(r) for r in rows]
    i = next(i for i, r in enumerate(fast) if r["writer_mode"] == "paced")
    fast[i] = dict(fast[i], write_ops=int(PACED_RATE) + 1, elapsed_s=1.0)
    art_fast = build_artifact(fast, p, 226.142, 2.0, PACED_RATE)
    got = art_fast.get("paced_rate_check", {}).get("flags")
    if not got or got[0]["cell"] != fast[i]["cell"] or got[0]["reasons"] != ["above_offered"]:
        fails.append(f"the artifact must carry the above-offered paced cell in "
                     f"paced_rate_check.flags, got {got!r}")

    # --- --readers and --modes ------------------------------------------------
    check("parse_readers sorts", parse_readers("7,1,3"), (1, 3, 7))
    check("parse_modes keeps section 5.2's order", parse_modes("free,idle"), ("idle", "free"))
    for name, call, text in (("readers without 1", parse_readers, "2,4"),
                             ("readers repeat", parse_readers, "1,2,2"),
                             ("readers zero", parse_readers, "0,1"),
                             ("readers not ints", parse_readers, "1,x"),
                             ("unknown mode", parse_modes, "idle,burst"),
                             ("repeated mode", parse_modes, "idle,idle")):
        try:
            call(text)
        except argparse.ArgumentTypeError:
            pass
        else:
            fails.append(f"{name}: {text!r} was accepted")
    # The call site: the artifact records the sweep its rows came from, which is
    # what tells a held-out run from a section 5.9 run.
    idle_only = [r for r in rows if r["writer_mode"] == "idle"]
    art_idle = build_artifact(idle_only, p, 226.142, 2.0, PACED_RATE)
    check("settings.modes from rows", art_idle["settings"]["modes"], ["idle"])
    check("settings.readers from rows", art_idle["settings"]["readers"],
          sorted({r["readers"] for r in idle_only}))
    check("an idle-only sweep reports no paced reader counts",
          art_idle["paced_rate_check"]["per_readers"], {})

    # --- seek lock scopes ------------------------------------------------------
    check("parse_lock_scopes keeps LOCK_SCOPES order", parse_lock_scopes("trie,full"), ("full", "trie"))
    for text in ("full,rw", "trie,trie"):
        try:
            parse_lock_scopes(text)
        except argparse.ArgumentTypeError:
            pass
        else:
            fails.append(f"parse_lock_scopes accepted {text!r}")
    # Paired per round: drift b differs every round, the trie arm reads 1.1x at
    # R = 1 and holds 0.9 of it at R = 7 where full holds 0.6, so every round's
    # quotient is (0.9 / 1.1 * 1.1) / 0.6 = 1.5 exactly, and T(1) is 1.1.
    lk = []
    for rd in range(5):
        b = 1.0 + 0.3 * rd
        for scope, t1, t7 in (("full", b, 0.6 * b), ("trie", 1.1 * b, 0.99 * b)):
            lk.append({"round": rd, "writer_mode": "idle", "readers": 1, "read_mops": t1, "lock_scope": scope})
            lk.append({"round": rd, "writer_mode": "idle", "readers": 7, "read_mops": t7, "lock_scope": scope})
    lr = lock_scope_ratios(lk, "idle")
    if abs(lr["S(7)"]["point"] - 1.5) > 1e-9 or abs(lr["T(1)"]["point"] - 1.1) > 1e-9:
        fails.append(f"lock_scope_ratios: S(7) {lr['S(7)']['point']}, T(1) {lr['T(1)']['point']}; want 1.5, 1.1")
    # Unpaired, the same cells would not give 1.5: round 0's trie arm over round 4's
    # full arm is (0.99 / 1.1) / (0.6 * 2.2 / 2.2) -- pairing is what the fixture pins,
    # so shuffling one arm's rounds must move the point.
    shuffled = [dict(r, round=(4 - r["round"])) if r["lock_scope"] == "trie" and r["readers"] == 7 else r for r in lk]
    if abs(lock_scope_ratios(shuffled, "idle")["S(7)"]["point"] - 1.5) < 1e-6:
        fails.append("lock_scope_ratios did not pair by round: mismatched rounds still gave 1.5")
    # scaling_ratios reads one scope: the trie rows must not overwrite full's.
    check("scaling_ratios keeps scopes apart", round(scaling_ratios(lk, "idle", "full")["S(7)"]["point"], 9), 0.6)
    check("scaling_ratios trie scope", round(scaling_ratios(lk, "idle", "trie")["S(7)"]["point"], 9), 0.9)
    # The call site: a two-scope artifact carries the ratio and both scopes'
    # scaling, and `scaling` is still full's; a one-scope artifact adds nothing.
    two = [dict(r, cell=f"c{i}", load={"foreign_busy_cpus": 0.0}, write_ops=0, elapsed_s=2.0,
                writer_exhausted=0, read_ops=1) for i, r in enumerate(lk)]
    art_two = build_artifact(two, p, 226.142, 2.0, PACED_RATE)
    check("two-scope settings", art_two["settings"]["lock_scopes"], ["full", "trie"])
    check("two-scope scaling is full's", round(art_two["scaling"]["idle"]["S(7)"]["point"], 9), 0.6)
    got = (art_two.get("lock_scope_ratio") or {}).get("idle", {}).get("S(7)", {}).get("point")
    check("two-scope ratio at the call site", None if got is None else round(got, 9), 1.5)
    check("two-scope per-scope scaling", sorted(art_two["scaling_by_lock_scope"]), ["full", "trie"])
    one = build_artifact([r for r in two if r["lock_scope"] == "full"], p, 226.142, 2.0, PACED_RATE)
    if "lock_scope_ratio" in one or "scaling_by_lock_scope" in one:
        fails.append("a one-scope artifact must not carry lock-scope keys")

    # The idle control must record a zero duty, not a missing one.
    idle = [c for c in art["cells"] if c["writer_mode"] == "idle"]
    if not idle or any(c["writer_duty_cycle"] != 0.0 for c in idle):
        fails.append("idle cells must carry writer_duty_cycle 0.0")

    if fails:
        print("concurrent_read_scaling.py --self-test: FAILED")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("concurrent_read_scaling.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--bench", type=Path, default=BENCH)
    ap.add_argument("--rounds", type=int, default=5)
    ap.add_argument("--window-seconds", type=float, default=2.0)
    ap.add_argument("--paced-rate", type=float, default=PACED_RATE)
    ap.add_argument("--insert-ns", type=float, default=None,
                    help="insert cost used to turn an achieved write rate into a duty cycle; "
                         "defaults to the suite artifact's fillrandom cell")
    ap.add_argument("--readers", type=parse_readers, default=READERS,
                    help="comma-separated reader counts, including 1 (default: section 5.2's "
                         "1,2,4,7). METHODOLOGY section 5.12's held-out runs use 1,2,3,4,5,6,7")
    ap.add_argument("--modes", type=parse_modes, default=MODES,
                    help="comma-separated writer modes (default: idle,paced,free); section "
                         "5.12's held-out runs use idle")
    ap.add_argument("--lock-scopes", type=parse_lock_scopes, default=("full",),
                    help="comma-separated seek lock scopes, interleaved within each round "
                         "(default: full). The #802 narrowed-mutex arm uses full,trie")
    ap.add_argument("--quick", action="store_true",
                    help="smoke shape only: 1 round, short window, R in {1,2}. Writes under "
                         "results/quick/ so it cannot overwrite a committed baseline (8.5)")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    if not args.bench.is_file():
        print(f"::error::benchmark binary not found: {args.bench}\n"
              f"build it with: make -C integrations/rocksdb bench-concurrent", file=sys.stderr)
        return 1

    insert_ns = args.insert_ns
    if insert_ns is None:
        sys.path.insert(0, str(REPO_ROOT / "scripts"))
        from rocksdb_locate_bound import load_arms
        insert_ns = load_arms()["insert_ns"]

    readers, modes, rounds, window_s = args.readers, args.modes, args.rounds, args.window_seconds
    out = args.out
    if args.quick:
        readers, modes, rounds, window_s = (1, 2), ("idle", "paced"), 1, 0.25
        out = REPO_ROOT / "docs" / "benchmarks" / "rocksdb_memtable" / "results" / "quick" / out.name

    # The core pin, before anything is timed and before `new_provenance`, which
    # records `core_pin` from the variable this call publishes. The bare-metal
    # workflow sources `bench_pin.sh` before invoking this driver, and this call
    # then verifies the affinity actually arrived rather than re-applying it;
    # run by hand with no such shell, it applies the pin itself. On the hybrid
    # reference host a cell whose threads land on efficiency cores measures
    # 1.576x the P-core time and no interval says so (#639).
    pin = bench_pin.apply("concurrent_read_scaling.py")

    provenance = prov.new_provenance(
        "rocksdb_concurrent", 802, "T(R)/T(1)", repo_root=REPO_ROOT,
        pre_registration="docs/benchmarks/rocksdb_memtable/METHODOLOGY.md section 5",
    )
    provenance["host"] = prov.host_facts(pin)
    provenance["estimators"] = prov.estimators(
        ratio="paired BCa 95% on S(R) = T(R)/T(1), resampled over rounds",
        columns="per-cell aggregate read Mops/s",
        raw="rounds_raw",
    )

    rows = run_sweep(args.bench, rounds, window_s, readers, modes, args.paced_rate, provenance,
                     args.lock_scopes)

    # A concurrent artifact owes a numeric `load.foreign_busy_cpus` on every
    # timed cell (scripts/check_bench_provenance.py). The delta is read from
    # /proc/stat, which exists on the Linux reference host and not on a macOS
    # dev box -- where every cell would carry null and the artifact would be
    # rejected later, after the sweep had been paid for. Refuse now and name
    # the reason (AGENTS.md 8.1). `--quick` is exempt: it writes under
    # results/quick/ and is a shape smoke, never a baseline.
    if not args.quick:
        blind = [r["cell"] for r in rows
                 if not isinstance(r["load"].get("foreign_busy_cpus"), (int, float))
                 or isinstance(r["load"].get("foreign_busy_cpus"), bool)]
        if blind:
            print(f"::error::{len(blind)} of {len(rows)} cells carry no busy-CPU delta "
                  f"(first: {blind[0]}). The host exposes no /proc/stat, so this run cannot "
                  f"say whether anything else was resident while it was taken, and a "
                  f"committed artifact must (AGENTS.md 8.17). Run it on the reference host, "
                  f"or use --quick for a shape smoke under results/quick/.", file=sys.stderr)
            return 1

    # Belt and braces: the binary already refuses a paced cell that ran dry,
    # but an artifact is what gets read later, so the driver checks the field
    # it records rather than trusting an exit code it no longer has.
    bad = [r["cell"] for r in rows
           if r["writer_mode"] == "paced" and r.get("writer_exhausted")]
    if bad:
        print(f"::error::{len(bad)} paced cell(s) exhausted the writer key supply "
              f"(first: {bad[0]}). Their achieved rate understates the offered rate, so "
              f"the duty cycle computed from it is wrong and they cannot be gated "
              f"(METHODOLOGY section 5.2).", file=sys.stderr)
        return 1
    report = paced_rate_report(rows, args.paced_rate)
    if len(args.lock_scopes) > 1:
        for scope in args.lock_scopes:
            for R, e in paced_rate_report([r for r in rows if r["lock_scope"] == scope],
                                          args.paced_rate)["per_readers"].items():
                print(f"[{scope}] paced R={R}: achieved {e['achieved_min_ops_per_s']:,.0f}-"
                      f"{e['achieved_max_ops_per_s']:,.0f} inserts/s")
    for R, e in report["per_readers"].items():
        print(f"paced R={R}: achieved {e['achieved_min_ops_per_s']:,.0f}-"
              f"{e['achieved_max_ops_per_s']:,.0f} inserts/s over {e['rounds']} round(s) "
              f"(offered {args.paced_rate:,.0f}; max shortfall {e['max_shortfall_fraction']:.1%})")
    above = [f for f in report["flags"] if "above_offered" in f["reasons"]]
    if above:
        print(f"::warning::{len(above)} paced cell(s) ran above the offered rate (first: "
              f"{above[0]['cell']}, {above[0]['achieved_ops_per_s']:,.1f} inserts/s). "
              f"METHODOLOGY section 5.9 reports these and does not gate them; the artifact's "
              f"paced_rate_check.flags names every one.")
    n_exh = sum(1 for r in rows
                if r["writer_mode"] == "free" and r.get("writer_exhausted"))
    if n_exh:
        print(f"note: {n_exh} free cell(s) exhausted the key supply and stopped early. "
              f"Expected on a fast host; those cells are reported, never gated.")

    art = build_artifact(rows, provenance, insert_ns, window_s, args.paced_rate)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(art, indent=2) + "\n")
    print(f"wrote {out} ({len(rows)} cells, {rounds} rounds)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
