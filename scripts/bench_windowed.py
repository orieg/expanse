#!/usr/bin/env python3
"""Run one wall-clock bench target with a load window around every timed case (#1214).

AGENTS.md section 8.17 makes a published wall-clock result without its load
snapshot inadmissible, and asks for a snapshot between compared runs and
mid-sweep. The suites this driver serves ran as one `cargo bench` process with
no snapshot inside it, so their host load was known only at the job's start
and at harvest.

The target is built first and its binary run once, so compilation is never
inside a window. While it runs, the driver reads its output line by line and
takes a snapshot at every window boundary: the host's busy CPU from
`/proc/stat` and the harness's own CPU from `/proc/<pid>/stat`
(`bench_provenance.pid_load_snapshot`). Their difference is what else was
resident during that case. Two kinds of boundary are read:

  criterion  criterion's own progress lines on stderr: `Benchmarking <id>`
             opens a window, `Benchmarking <id>: Analyzing` closes it. Setup
             outside the bench closures and the analysis are not inside it.
  markers    `BENCH_WINDOW begin|end <id>` lines a custom-main harness prints
             around each timed case (`art_common::bench_window`).

A window shorter than `MIN_WINDOW_S` has no resolvable busy figure, so it is
merged into the next one and the merged window names every case it covers.

Output: `--out` holds `provenance` with `load_windows: true` and one entry per
window under `provenance.windows`. A window that could not attribute, a marker
out of order, or a run with no window at all makes the record inadmissible:
`load_status` says so and `load_findings` says why, and the driver still exits
with the harness's own status, so a loud load finding never discards the
completed measurement (section 8.1). The workflow fails the job on it after the
harvest.

The harness is a `[[bench]]` target (`--target`, built with `cargo bench
--no-run` and run with `--bench` from its package directory) or an example
with its own `main` (`--example`, built with `cargo build --release --example`
and run from the repository root, the working directory `cargo run` gives it,
so its relative paths resolve as they did before the driver).

Usage:
  bench_windowed.py --suite S --mode criterion|markers --package P \\
      (--target T | --example E) --out load-S.json [-- harness args]
  bench_windowed.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import bench_provenance as bp  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parent.parent
ISSUE = 1214

MARKER = re.compile(r"^BENCH_WINDOW (begin|end) (\S.*)$")
CRITERION_STATE = re.compile(
    r"^Benchmarking (?P<id>.+?): (?P<state>Warming up|Collecting|Analyzing|Profiling)\b")
CRITERION_START = re.compile(r"^Benchmarking (?P<id>\S.*)$")


def boundary(line: str, mode: str) -> tuple[str, str] | None:
    """`("begin" | "end", id)` if `line` opens or closes a window, else `None`."""
    line = line.rstrip("\r\n")
    if mode == "markers":
        m = MARKER.match(line)
        return (m.group(1), m.group(2).strip()) if m else None
    if mode == "criterion":
        m = CRITERION_STATE.match(line)
        if m:
            return ("end", m.group("id")) if m.group("state") == "Analyzing" else None
        m = CRITERION_START.match(line)
        # `Benchmarking <id>: <state>` lines are matched above; a start line
        # carries the id alone.
        return ("begin", m.group("id")) if m else None
    raise ValueError(f"unknown mode {mode!r}")


class Windower:
    """Turns a stream of boundaries into closed, attributed windows.

    `snap(label, prev)` takes a snapshot; `close(start)` attributes the window
    that opened at `start`. Both are injected so the bookkeeping is testable
    without a process to read.
    """

    def __init__(self, snap, close, min_window_s: float = bp.MIN_WINDOW_S):
        self.snap, self.close, self.min_window_s = snap, close, min_window_s
        self.windows: list[dict] = []
        self.findings: list[str] = []
        self._open: tuple[str, dict] | None = None
        # A closed window too short to resolve, carried into the next one.
        self._carry: tuple[list[str], dict] | None = None
        # The ids and opening snapshot of the last recorded window, so a
        # trailing window too short to resolve can be folded back into it.
        self._last: tuple[list[str], dict] | None = None

    def feed(self, kind: str, wid: str) -> None:
        if kind == "begin":
            if self._open is not None:
                self.findings.append(f"window {wid!r} opened while {self._open[0]!r} was open")
                return
            if self._carry is not None:
                ids, start = self._carry
                self._carry = None
                self._open = (wid, start)
                self._ids = ids + [wid]
            else:
                self._open = (wid, self.snap(f"window:{wid}", None))
                self._ids = [wid]
            return
        if self._open is None or self._open[0] != wid:
            self.findings.append(
                f"window {wid!r} closed but {self._open[0] if self._open else None!r} was open")
            return
        _, start = self._open
        self._open = None
        load = self.close(start)
        # A window is merged into the next when it is too short to resolve.
        # `wall_s` is rounded to 1 ms while the attribution tests the unrounded
        # interval, so a window at the minimum can read as long enough here and
        # still carry no figure (#1214: `growth/expanse`, `wall_s` 0.1 with own
        # and foreign CPU None). Whatever could not attribute is merged too.
        short = load.get("wall_s") is not None and load["wall_s"] < self.min_window_s
        unresolved = not isinstance(load.get("foreign_busy_cpus"), (int, float))
        if short or unresolved:
            self._carry = (self._ids, start)
            return
        self._record(self._ids, load)
        self._last = (self._ids, start)

    def _record(self, ids: list[str], load: dict) -> None:
        entry = {"id": ids[-1] if len(ids) == 1 else "+".join(ids), "load": load}
        if len(ids) > 1:
            entry["merged"] = ids
        self.windows.append(entry)

    def finish(self) -> None:
        if self._open is not None:
            self.findings.append(f"window {self._open[0]!r} never closed")
            self._open = None
        if self._carry is not None:
            # Too short, and nothing followed to merge it into. Fold it back
            # into the last recorded window, re-closed from that window's
            # start (#1214: `hashbrown_container_dists`' last case). With no
            # window before it, record it as measured, which is to say
            # without a number.
            ids, start = self._carry
            self._carry = None
            if self._last is not None:
                last_ids, last_start = self._last
                self.windows.pop()
                ids, start = last_ids + ids, last_start
            self._record(ids, self.close(start))
        if not self.windows:
            self.findings.append("no window boundary was read — the harness at this ref "
                                 "does not mark its timed cases")
        for w in self.windows:
            load = w["load"]
            if not all(isinstance(load.get(k), (int, float))
                       for k in ("busy_cpus_since_prev", "foreign_busy_cpus")):
                self.findings.append(f"window {w['id']!r} could not attribute its load "
                                     f"({load})")


def build_command(package: str, kind: str, name: str) -> list[str]:
    """The cargo invocation that builds, and does not run, the harness."""
    # `json-render-diagnostics`: artifacts as JSON on stdout, which is parsed,
    # and compiler diagnostics rendered on stderr, so a failed build says why.
    if kind == "bench":
        return ["cargo", "bench", "--no-run", "--message-format=json-render-diagnostics",
                "-p", package, "--bench", name]
    if kind == "example":
        return ["cargo", "build", "--release", "--message-format=json-render-diagnostics",
                "-p", package, "--example", name]
    raise ValueError(f"unknown harness kind {kind!r}")


def pick_executable(cargo_json: str, kind: str, name: str) -> tuple[Path, Path] | None:
    """`(executable, package directory)` of the `kind` target `name` in cargo's JSON output."""
    for line in cargo_json.splitlines():
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:  # discipline:allow(error-swallowing) cargo interleaves non-JSON build lines; only compiler-artifact messages are read
            continue
        if (msg.get("reason") == "compiler-artifact" and msg.get("executable")
                and msg["target"]["name"] == name and kind in msg["target"]["kind"]):
            return Path(msg["executable"]), Path(msg["manifest_path"]).parent
    return None


def build(package: str, kind: str, name: str) -> tuple[Path, Path]:
    """Builds the harness; returns `(executable, package directory)`."""
    cmd = build_command(package, kind, name)
    out = subprocess.run(cmd, cwd=REPO_ROOT, check=True, stdout=subprocess.PIPE,
                         text=True).stdout
    found = pick_executable(out, kind, name)
    if found is None:
        raise SystemExit(f"bench_windowed: `{' '.join(cmd)}` named no executable for {name!r}")
    return found


def harness_argv(exe: Path, kind: str, harness_args: list[str]) -> list[str]:
    """A bench binary takes `--bench` as `cargo bench` passes it; an example takes its own args."""
    return [str(exe), "--bench", *harness_args] if kind == "bench" else [str(exe), *harness_args]


def run_bench_window(prov: dict, package: str, target: str, harness_args: list[str],
                     label: str | None = None) -> tuple[subprocess.CompletedProcess, dict]:
    """Runs one bench target as one process inside one load window.

    For a runner that times a whole bench process per artifact
    (`art_comparison`'s and `hashbrown_comparison`'s `run_all.py`): the
    target is built first, outside the window, then its binary runs once
    between `bench_provenance.begin_cell` and `end_cell`. The process is a
    reaped child, so the children's CPU over the window is its own CPU and
    the remainder is foreign. The window is appended to `prov["windows"]`
    (and `prov["load_windows"]` set) and returned, for the runner to store
    on the artifact it timed. Output is captured; the caller checks the
    exit status.
    """
    exe, pkg_dir = build(package, "bench", target)
    env = dict(os.environ)
    env.setdefault("CRITERION_HOME", str(REPO_ROOT / "target" / "criterion"))
    start = bp.begin_cell(prov, f"cell:{label or target}")
    res = subprocess.run(harness_argv(exe, "bench", harness_args), cwd=pkg_dir, env=env,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    window = bp.end_cell(start)
    prov["load_windows"] = True
    prov.setdefault("windows", []).append({"id": label or target, "load": window})
    return res, window


def judge(suite: str, written: list[tuple[str, dict]]) -> list[str]:
    """`check_bench_provenance.findings_for` over artifacts a runner wrote.

    `written` is `(file name, artifact)`; each is judged under its committed
    name, `<suite>/results/<file>`, so the per-artifact rules apply to a
    `--quick` copy as they would to the committed one. A runner prints the
    findings and exits non-zero on any (section 8.1): an artifact the gate
    would refuse is not left to be discovered at commit time.
    """
    import check_bench_provenance as cbp  # noqa: PLC0415 -- the gate's own function judges the artifact
    out = []
    for name, obj in written:
        rel = f"{suite}/results/{name}"
        exempt, _ = cbp.grandfather_status(rel, obj)
        if not exempt:
            out.extend(cbp.findings_for(rel, obj))
    return out


def run(args) -> int:
    if bp.cpu_jiffies() == (None, None):
        # The windows are /proc readings; off Linux there is nothing to take.
        print("bench_windowed: /proc/stat is unreadable, so no load window can be taken; "
              "this driver runs on Linux only", file=sys.stderr)
        return 2
    kind, name = ("bench", args.target) if args.target else ("example", args.example)
    exe, pkg_dir = build(args.package, kind, name)
    target_field = {"bench_target": name} if kind == "bench" else {"example": name}
    prov = bp.new_provenance(
        args.suite, ISSUE, "as published by the harness; this record carries load only",
        REPO_ROOT, **target_field, window_mode=args.mode, load_windows=True)
    env = dict(os.environ)
    # Criterion otherwise spawns `cargo metadata` to find its output directory.
    env.setdefault("CRITERION_HOME", str(REPO_ROOT / "target" / "criterion"))
    # `cargo bench` runs a bench from its package directory and `cargo run`
    # runs an example from where it was invoked; each keeps that here.
    cwd = pkg_dir if kind == "bench" else REPO_ROOT
    proc = subprocess.Popen(harness_argv(exe, kind, args.harness_args), cwd=cwd, env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                            bufsize=1)
    win = Windower(lambda label, prev: bp.pid_load_snapshot(label, proc.pid, prev),
                   lambda start: bp.pid_window(start, proc.pid))
    assert proc.stdout is not None
    for line in proc.stdout:
        # Read before the process is reaped: an exited, unreaped harness still
        # reports its final CPU times, a reaped one reports none.
        b = boundary(line, args.mode)
        if b is not None:
            win.feed(*b)
        sys.stdout.write(line)
        sys.stdout.flush()
    win.finish()
    status = proc.wait()
    bp.add_load(prov, "end")
    prov["windows"] = win.windows
    prov["load_status"] = "inadmissible" if win.findings else "ok"
    prov["load_findings"] = win.findings
    prov["harness_exit_status"] = status
    Path(args.out).write_text(json.dumps({"provenance": prov}, indent=2) + "\n")
    for f in win.findings:
        print(f"::warning::bench_windowed: {args.suite}: {f}")
    print(f"bench_windowed: {len(win.windows)} window(s), load_status="
          f"{prov['load_status']} -> {args.out}")
    return status


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------

def _self_test() -> int:
    failures = []

    def check(name, got, want):
        if got != want:
            failures.append(f"{name}: got {got!r}, want {want!r}")

    check("marker begin", boundary("BENCH_WINDOW begin sequential/pop=10000\n", "markers"),
          ("begin", "sequential/pop=10000"))
    check("marker end", boundary("BENCH_WINDOW end growth/btree", "markers"),
          ("end", "growth/btree"))
    check("other line", boundary("  pop=  10000 | dist=sequential", "markers"), None)
    cid = "comparative_set_contains/sequential/10000/expanse"
    check("criterion start", boundary(f"Benchmarking {cid}\n", "criterion"), ("begin", cid))
    check("criterion warmup", boundary(f"Benchmarking {cid}: Warming up for 3.0000 s",
                                       "criterion"), None)
    check("criterion collecting", boundary(
        f"Benchmarking {cid}: Collecting 100 samples in estimated 5.0 s (10k iterations)",
        "criterion"), None)
    check("criterion analyzing", boundary(f"Benchmarking {cid}: Analyzing", "criterion"),
          ("end", cid))
    check("criterion result", boundary(f"{cid}  time:   [1.0 ns 1.1 ns 1.2 ns]", "criterion"),
          None)

    # Windower bookkeeping over a fake clock: each snapshot is a time, and a
    # window's load is its length.
    clock = iter(range(100))

    def snap(label, prev):
        return {"label": label, "t": next(clock)}

    def close(start):
        t = next(clock)
        wall = (t - start["t"]) / 10
        return {"wall_s": wall, "busy_cpus_since_prev": 1.0, "own_busy_cpus": 1.0,
                "foreign_busy_cpus": 0.0}

    w = Windower(snap, close, min_window_s=0.15)
    for kind, wid in [("begin", "a"), ("end", "a"), ("begin", "b"), ("end", "b")]:
        w.feed(kind, wid)
    w.finish()
    # a: opened at 0, closed at 1 -> 0.1 s, below 0.15, carried into b, which
    # closes at 2 -> 0.2 s over both.
    check("short window merged", [x["id"] for x in w.windows], ["a+b"])
    check("merged ids listed", w.windows[0].get("merged"), ["a", "b"])
    check("merge is not a finding", w.findings, [])

    w = Windower(snap, close, min_window_s=0.0)
    w.feed("begin", "a")
    w.feed("begin", "b")
    w.feed("end", "c")
    w.finish()
    check("nested open is a finding", any("opened while" in f for f in w.findings), True)
    check("mismatched close is a finding", any("closed but" in f for f in w.findings), True)
    check("unclosed is a finding", any("never closed" in f for f in w.findings), True)

    w = Windower(snap, close)
    w.finish()
    check("no window is a finding", any("no window boundary" in f for f in w.findings), True)

    def close_none(start):
        return {"wall_s": 5.0, "busy_cpus_since_prev": None, "own_busy_cpus": None,
                "foreign_busy_cpus": None}

    w = Windower(snap, close_none, min_window_s=0.0)
    w.feed("begin", "a")
    w.feed("end", "a")
    w.finish()
    check("unattributed window is a finding",
          any("could not attribute" in f for f in w.findings), True)

    # A window at the minimum by its rounded length that still could not
    # attribute (the unrounded interval was shorter) merges into the next.
    closes = iter([
        {"wall_s": 0.1, "busy_cpus_since_prev": 1.1, "own_busy_cpus": None,
         "foreign_busy_cpus": None},
        {"wall_s": 5.0, "busy_cpus_since_prev": 1.0, "own_busy_cpus": 1.0,
         "foreign_busy_cpus": 0.0},
    ])
    w = Windower(snap, lambda start: next(closes), min_window_s=0.1)
    for kind, wid in [("begin", "growth/expanse"), ("end", "growth/expanse"),
                      ("begin", "growth/hashbrown"), ("end", "growth/hashbrown")]:
        w.feed(kind, wid)
    w.finish()
    check("an unattributed window at the minimum merges",
          [x["id"] for x in w.windows], ["growth/expanse+growth/hashbrown"])
    check("the merged window attributes", w.findings, [])

    # A last window too short to attribute folds back into the one before it,
    # re-closed from that window's start.
    clock2 = iter(range(100))
    closes2 = iter([
        {"wall_s": 5.0, "busy_cpus_since_prev": 1.0, "own_busy_cpus": 1.0,
         "foreign_busy_cpus": 0.0},
        {"wall_s": 0.095, "busy_cpus_since_prev": None, "own_busy_cpus": None,
         "foreign_busy_cpus": None},
        {"wall_s": 5.1, "busy_cpus_since_prev": 1.0, "own_busy_cpus": 1.0,
         "foreign_busy_cpus": 0.0},
    ])
    w = Windower(lambda label, prev: {"label": label, "t": next(clock2)},
                 lambda start: dict(next(closes2), start_t=start["t"]), min_window_s=0.1)
    for kind, wid in [("begin", "clustered"), ("end", "clustered"),
                      ("begin", "zipfian"), ("end", "zipfian")]:
        w.feed(kind, wid)
    w.finish()
    check("a short last window folds back", [x["id"] for x in w.windows], ["clustered+zipfian"])
    check("re-closed from the earlier window's start", w.windows[0]["load"]["start_t"], 0)
    check("the folded window attributes", w.findings, [])

    # Harness kinds: the build command, the artifact picked from cargo's
    # output, and the argv the binary is run with.
    check("bench build", build_command("expanse-trie", "bench", "domain")[:3],
          ["cargo", "bench", "--no-run"])
    ex_cmd = build_command("expanse-capi", "example", "bench_vs_libjudy")
    check("example build is release", "--release" in ex_cmd and "--example" in ex_cmd, True)
    check("example build does not run", ex_cmd[1], "build")
    try:
        build_command("p", "test", "x")
        check("unknown kind raises", False, True)
    except ValueError:
        pass
    def art(name, kind, exe):
        return json.dumps({
            "reason": "compiler-artifact", "executable": exe,
            "manifest_path": "/r/crates/expanse-capi/Cargo.toml",
            "target": {"name": name, "kind": [kind]}})

    cargo_out = "\n".join([
        "   Compiling expanse-capi v0.8.0",
        art("bench_vs_libjudy", "lib", None),
        art("vs_stock", "bench", "/r/target/release/deps/vs_stock-1"),
        art("bench_vs_libjudy", "example", "/r/target/release/examples/bench_vs_libjudy"),
    ])
    check("example artifact picked", pick_executable(cargo_out, "example", "bench_vs_libjudy"),
          (Path("/r/target/release/examples/bench_vs_libjudy"), Path("/r/crates/expanse-capi")))
    check("bench artifact picked", pick_executable(cargo_out, "bench", "vs_stock")[0],
          Path("/r/target/release/deps/vs_stock-1"))
    check("a bench is not taken for an example",
          pick_executable(cargo_out, "example", "vs_stock"), None)
    check("bench argv", harness_argv(Path("/b"), "bench", ["--quick"]), ["/b", "--bench", "--quick"])
    check("example argv", harness_argv(Path("/e"), "example", ["--rounds", "3"]),
          ["/e", "--rounds", "3"])
    check("vs_libjudy cell marker",
          boundary("BENCH_WINDOW begin sequential/pop=100000", "markers"),
          ("begin", "sequential/pop=100000"))

    for f in failures:
        print(f"  FAIL {f}")
    print(f"bench_windowed.py --self-test: "
          f"{'all checks passed' if not failures else f'{len(failures)} failure(s)'}")
    return 1 if failures else 0


def main() -> int:
    if "--self-test" in sys.argv[1:]:
        return _self_test()
    p = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    p.add_argument("--suite", required=True)
    p.add_argument("--mode", required=True, choices=("criterion", "markers"))
    p.add_argument("--package", required=True)
    harness = p.add_mutually_exclusive_group(required=True)
    harness.add_argument("--target", help="a [[bench]] target")
    harness.add_argument("--example", help="an example with its own main")
    p.add_argument("--out", required=True)
    p.add_argument("harness_args", nargs="*")
    return run(p.parse_args())


if __name__ == "__main__":
    sys.exit(main())
