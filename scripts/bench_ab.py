#!/usr/bin/env python3
"""Two-commit interleaving for the concurrent FFI harnesses (#568 PR 3).

A before/after claim on a concurrent cell is admissible only when the two
builds ran interleaved inside the same cell, round by round (docs/BENCHMARKING.md
rule 18): a head run taken on its own is compared against the host of the day,
not against the base commit. `interleave` drives two binaries of the same
harness — the base build and the head build — through one cell, alternating
which goes first every round and handing each process the round number
(`--rounds 1 --round-offset K`) so the harness's own arm order (competitor
first on even rounds) stays continuous across processes. Each build's rows
come back separately; the runner reduces them with the estimator it uses for a
single-build cell.

The base binary is provided, never built here: the reference host's session
directory is an rsync'd tree, not a checkout, so the base tree is a second
synced tree and its commit is passed in beside it. Its SHA-256 is recorded in
the artifact so a reader can tell two "base" runs apart.

Self-test: `python3 scripts/bench_ab.py --self-test`.
"""
from __future__ import annotations

import hashlib
import sys
from pathlib import Path


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def order_for_round(round_: int) -> tuple[str, str]:
    """Which build goes first in a round: base on even rounds, head on odd."""
    return ("base", "head") if round_ % 2 == 0 else ("head", "base")


def interleave(base_bin: Path, head_bin: Path, cell_args: list, rounds: int, env: dict,
               run_cell) -> tuple[list, list]:
    """Runs `rounds` rounds of one cell, base and head alternating within each.

    `run_cell(argv, env) -> list[dict]` is the runner's own one-process driver.
    Every process is asked for exactly one round at its offset; a process that
    emits any other number of rows voids the cell (section 8.1: never pad).
    Returns `(base_rows, head_rows)`, each row tagged with `build`.
    """
    bins = {"base": Path(base_bin), "head": Path(head_bin)}
    for name, exe in bins.items():
        if not exe.is_file():
            raise RuntimeError(f"{name} binary does not exist: {exe}")
    sinks: dict[str, list] = {"base": [], "head": []}
    for r in range(rounds):
        for build in order_for_round(r):
            argv = [str(bins[build]), *cell_args, "--rounds", "1", "--round-offset", str(r)]
            rows = run_cell(argv, env)
            if len(rows) != 1:
                raise RuntimeError(
                    f"{build} process for round {r} emitted {len(rows)} rows, expected exactly 1: "
                    f"{' '.join(argv)}")
            row = dict(rows[0])
            if row.get("round") != r:
                raise RuntimeError(
                    f"{build} process for round {r} reported round {row.get('round')!r}; the harness "
                    f"predates --round-offset")
            row["build"] = build
            sinks[build].append(row)
    return sinks["base"], sinks["head"]


def ab_provenance(base_bin: Path, base_commit: str, head_commit: str, rounds: int) -> dict:
    """The `provenance.ab` block of a two-commit artifact."""
    return {
        "mode": "two-commit interleaved per round",
        "base_commit": base_commit,
        "head_commit": head_commit,
        "base_binary_sha256": sha256_of(Path(base_bin)),
        "rounds_per_build": rounds,
        "harness": "one harness, two engines: the head tree's harness sources built against each "
                   "tree's engine, so only the engine differs between the two builds",
        "order": "base first on even rounds, head first on odd rounds; each process runs one round "
                 "at its offset so the harness's competitor-first alternation is continuous",
    }


def _self_test() -> int:
    import tempfile

    fails = 0

    def check(name, cond):
        nonlocal fails
        print(f"  {'ok ' if cond else 'FAIL'} {name}")
        fails += 0 if cond else 1

    check("even rounds start with base", order_for_round(0) == ("base", "head"))
    check("odd rounds start with head", order_for_round(3) == ("head", "base"))

    with tempfile.TemporaryDirectory() as d:
        base, head = Path(d) / "base", Path(d) / "head"
        base.write_bytes(b"base")
        head.write_bytes(b"head")
        calls = []

        def fake_run(argv, env):
            calls.append(argv)
            off = int(argv[argv.index("--round-offset") + 1])
            return [{"round": off, "expanse_reader_mops": 1.0 if argv[0].endswith("base") else 2.0}]

        b, h = interleave(base, head, ["map", "1", "8"], 4, {}, fake_run)
        check("four rounds per build", len(b) == 4 and len(h) == 4)
        check("rows tagged", all(r["build"] == "base" for r in b) and all(r["build"] == "head" for r in h))
        check("eight processes, one round each",
              len(calls) == 8 and all("--rounds" in c and c[c.index("--rounds") + 1] == "1" for c in calls))
        check("alternation: round 0 base first, round 1 head first",
              calls[0][0].endswith("base") and calls[2][0].endswith("head"))
        check("offsets continuous", [int(c[c.index("--round-offset") + 1]) for c in calls] == [0, 0, 1, 1, 2, 2, 3, 3])

        def two_rows(argv, env):
            off = int(argv[argv.index("--round-offset") + 1])
            return [{"round": off}, {"round": off}]

        try:
            interleave(base, head, ["map", "1", "8"], 1, {}, two_rows)
            check("a process emitting two rows voids the cell", False)
        except RuntimeError as e:
            check("a process emitting two rows voids the cell", "expected exactly 1" in str(e))

        def wrong_round(argv, env):
            return [{"round": 0}]

        try:
            interleave(base, head, ["map", "1", "8"], 2, {}, wrong_round)
            check("a harness ignoring --round-offset is refused", False)
        except RuntimeError as e:
            check("a harness ignoring --round-offset is refused", "predates --round-offset" in str(e))

        prov = ab_provenance(base, "aaaa", "bbbb", 4)
        check("provenance carries the base binary hash",
              prov["base_binary_sha256"] == hashlib.sha256(b"base").hexdigest() and prov["rounds_per_build"] == 4)
        try:
            interleave(Path(d) / "missing", head, [], 1, {}, fake_run)
            check("a missing base binary is refused", False)
        except RuntimeError as e:
            check("a missing base binary is refused", "does not exist" in str(e))
    print("bench_ab self-test:", "ok" if fails == 0 else f"{fails} failure(s)")
    return 1 if fails else 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(_self_test())
    print(__doc__)
