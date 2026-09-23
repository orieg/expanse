#!/usr/bin/env python3
"""scripts/miri_ub_sites.py — the concurrent wrappers' undefined-behaviour sites, pinned (#1086).

The `Sync*` wrappers reach two classes of undefined behaviour from safe code
with two threads (crates/expanse/src/sync.rs module docs, #1086): a data race
on an optimistic load, and a `&T` / `&mut T` overlap on the engine. No required
lane can observe either — a Miri run stops at the first, and the threaded
`sync::tests` are compiled out under Miri. This script is the instrument that
does: it runs each workload in `sync.rs`'s `mod miri_ub_sites` under Miri, one
process per (workload, check), and compares what Miri reports with
`.github/miri-ub-sites.json`.

  check     MIRIFLAGS                                              isolates
  race      -Zmiri-disable-stacked-borrows                         the data race
  alias-sb  -Zmiri-disable-data-race-detector                      aliasing, Stacked Borrows
  alias-tb  -Zmiri-disable-data-race-detector -Zmiri-tree-borrows  aliasing, Tree Borrows

The manifest names the toolchain and host target it was generated with, and
the census runs only there: Miri's schedule for a seed depends on both (the
target selects the SIMD kernels, the toolchain the interpreter), so the same
code on another target or nightly can report a different lowest failing seed.
A run on any other host target is could-not-check. Moving to a new toolchain
or target means regenerating the manifest (`--observe`), an explicit edit.

Seeds run one after another, one Miri process each, and the lowest seed that
reports undefined behaviour is the observation. `-Zmiri-many-seeds` is not
used: it runs seeds in parallel and reports whichever fails first by wall
clock, so the reported site would vary between runs of identical code.

A manifest entry expects either `ub` — Miri reports undefined behaviour whose
message contains the entry's `error` and whose innermost frame contains one of
its `frame` strings — or `clean`: every seed in the range passes. `clean` is an observation at that
seed budget, not a proof; an entry's optional `note` says which it is. `frame`
may list several:
a race report's innermost frame is whichever access Miri caught second, and the
schedule decides which side that is, so a race site names both.

Verdicts fail closed (AGENTS.md §5 "String-Gated Inverted Assertions", §8.1):
  - an expected site that reports nothing is a mismatch, "stopped reproducing":
    a fix flips its entry to `clean` by an explicit edit, never by a schedule
    change or a toolchain bump;
  - a different message or frame is a mismatch: a different site surfaced;
  - a run with neither UB nor a passing test (a build error, a filter that
    matched nothing, a failed assertion) is could-not-check, never clean.

Exit: 0 every entry as the manifest says; 1 a mismatch; 2 could-not-check or
an invalid manifest.

Usage:
  python3 scripts/miri_ub_sites.py --check                       # every entry
  python3 scripts/miri_ub_sites.py --check --only map_leaf_two_writers
  python3 scripts/miri_ub_sites.py --observe                     # report, judge nothing
  python3 scripts/miri_ub_sites.py --toolchain nightly --check   # another toolchain name for the same build
  python3 scripts/miri_ub_sites.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Tuple

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / ".github" / "miri-ub-sites.json"
SYNC_RS = ROOT / "crates" / "expanse" / "src" / "sync.rs"
NIGHTLY = ROOT / ".github" / "workflows" / "nightly.yml"
MODULE = "sync::miri_ub_sites"

CHECKS: Dict[str, str] = {
    "race": "-Zmiri-disable-stacked-borrows",
    "alias-sb": "-Zmiri-disable-data-race-detector",
    "alias-tb": "-Zmiri-disable-data-race-detector -Zmiri-tree-borrows",
}
EXPECTS = ("ub", "clean")

UB_LINE = re.compile(r"error: Undefined Behavior: (.*)")
FRAME0 = re.compile(r"^\s+0: (\S.*?)\s*$", re.MULTILINE)
PASSED = re.compile(r"test result: ok\. (\d+) passed")
FN_NAME = re.compile(r"fn (\w+)\(")


@dataclass(frozen=True)
class Observation:
    kind: str  # "ub" | "clean" | "error"
    message: str = ""
    frame: str = ""
    detail: str = ""


@dataclass(frozen=True)
class Entry:
    test: str
    check: str
    expect: str
    error: str = ""
    frames: Tuple[str, ...] = ()


def classify(rc: int, output: str) -> Observation:
    """What one Miri run reported. Pure: the verdict logic never sees cargo."""
    m = UB_LINE.search(output)
    if m:
        frame = FRAME0.search(output, m.end())
        return Observation("ub", m.group(1).strip(), frame.group(1) if frame else "")
    passed = [int(n) for n in PASSED.findall(output)]
    if rc == 0 and passed and all(n >= 1 for n in passed):
        return Observation("clean")
    # Neither UB nor a run that passed a test: a build error, an empty filter,
    # or a failed assertion. None of those says anything about the site.
    tail = " | ".join(line.strip() for line in output.strip().splitlines()[-3:])
    return Observation("error", detail=f"rc={rc}, passing results={passed}: {tail}")


def judge(entry: Entry, obs: Observation) -> Tuple[str, str]:
    """(`ok` | `mismatch` | `could-not-check`, reason)."""
    if obs.kind == "error":
        return "could-not-check", obs.detail
    if entry.expect == "clean":
        if obs.kind == "clean":
            return "ok", "clean"
        return "mismatch", f"expected clean, Miri reports: {obs.message} (innermost frame `{obs.frame}`)"
    if obs.kind == "clean":
        return "mismatch", (
            "stopped reproducing: every seed passed. If a change removed this site, "
            "flip the entry to `clean` in .github/miri-ub-sites.json"
        )
    if entry.error not in obs.message:
        return "mismatch", f"a different UB surfaced: `{obs.message}` (expected `{entry.error}`)"
    if not any(f in obs.frame for f in entry.frames):
        want = " or ".join(f"`{f}`" for f in entry.frames)
        return "mismatch", f"the UB moved: innermost frame `{obs.frame}` (expected {want})"
    return "ok", f"{obs.message} @ {obs.frame}"


def module_tests(source: str) -> List[str]:
    """Test names defined in `mod miri_ub_sites` of sync.rs."""
    start = source.find("mod miri_ub_sites {")
    if start < 0:
        return []
    # The module ends at the first line that is exactly "}" after its start.
    end = source.find("\n}\n", start)
    body = source[start:end if end >= 0 else len(source)]
    # Line by line rather than one regex over the body: a `#[test]` line,
    # any further attribute lines, then the `fn` line it names. (A single
    # pattern repeating an attribute group whose contents may themselves
    # contain `]#[` backtracks exponentially.)
    names: List[str] = []
    pending = False
    for line in body.splitlines():
        s = line.strip()
        if s == "#[test]":
            pending = True
        elif pending and s.startswith("#["):
            continue
        elif pending:
            m = FN_NAME.match(s)
            if m:
                names.append(m.group(1))
            pending = False
    return names


def load_manifest(data: dict, tests: List[str]) -> Tuple[List[Entry], str, List[str]]:
    """Entries, seed range, and every reason the manifest is invalid."""
    errors: List[str] = []
    seeds = data.get("seeds", "")
    if not re.fullmatch(r"\d+\.\.\d+", str(seeds)):
        errors.append(f"`seeds` must look like `0..4`, got `{seeds}`")
    if not re.fullmatch(r"nightly-\d{4}-\d{2}-\d{2}", str(data.get("toolchain", ""))):
        errors.append(f"`toolchain` must be a dated nightly (`nightly-YYYY-MM-DD`), got `{data.get('toolchain')}`")
    if not data.get("target"):
        errors.append("`target` must name the host target the manifest was generated on")
    entries: List[Entry] = []
    seen = set()
    for i, raw in enumerate(data.get("sites", [])):
        frame = raw.get("frame", [])
        frames = (frame,) if isinstance(frame, str) else tuple(frame)
        e = Entry(
            test=raw.get("test", ""),
            check=raw.get("check", ""),
            expect=raw.get("expect", ""),
            error=raw.get("error", ""),
            frames=frames,
        )
        where = f"sites[{i}] ({e.test} / {e.check})"
        if e.test not in tests:
            errors.append(f"{where}: no test `{e.test}` in {MODULE}")
        if e.check not in CHECKS:
            errors.append(f"{where}: unknown check `{e.check}` (one of {', '.join(CHECKS)})")
        if e.expect not in EXPECTS:
            errors.append(f"{where}: expect must be `ub` or `clean`")
        if e.expect == "ub" and (not e.error or not e.frames or not all(e.frames)):
            errors.append(f"{where}: an expected site names its `error` and `frame`")
        if (e.test, e.check) in seen:
            errors.append(f"{where}: listed twice")
        seen.add((e.test, e.check))
        entries.append(e)
    for t in tests:
        if not any(e.test == t for e in entries):
            errors.append(f"test `{t}` in {MODULE} has no manifest entry")
    return entries, str(seeds), errors


def seed_range(seeds: str) -> range:
    lo, hi = (int(x) for x in seeds.split(".."))
    return range(lo, hi)


def first_seed(runs) -> Observation:
    """The observation over a seed range: the lowest seed that reports UB or
    cannot be checked, else clean. `runs` yields one Observation per seed, in
    order, and is consumed only as far as needed."""
    last = Observation("error", detail="empty seed range")
    for obs in runs:
        if obs.kind != "clean":
            return obs
        last = obs
    return last


def host_target(toolchain: str) -> str:
    """The host target `rustc +TOOLCHAIN -vV` reports, or "" if it cannot run."""
    try:
        cp = subprocess.run(["rustc", f"+{toolchain}", "-vV"], capture_output=True, text=True)
    except FileNotFoundError:
        return ""
    return parse_host(cp.stdout) if cp.returncode == 0 else ""


def parse_host(vv: str) -> str:
    m = re.search(r"^host: (\S+)$", vv, re.MULTILINE)
    return m.group(1) if m else ""


def workflow_pin(workflow: str) -> Tuple[str, str]:
    """(runner, toolchain) of the `miri-ub-sites` job in nightly.yml."""
    start = workflow.find("\n  miri-ub-sites:\n")
    if start < 0:
        return "", ""
    nxt = re.search(r"\n  [a-z0-9-]+:\n", workflow[start + 1:])
    body = workflow[start: start + 1 + nxt.start()] if nxt else workflow[start:]
    runner = re.search(r"runs-on: (\S+)", body)
    tc = re.search(r"toolchain: (\S+)", body)
    return (runner.group(1) if runner else "", tc.group(1) if tc else "")


# The hosted runner that provides each target the manifest may name.
RUNNERS = {"aarch64-apple-darwin": "macos-latest"}


def run_seed(entry: Entry, seed: int, toolchain: Optional[str]) -> Observation:
    cmd = ["cargo"]
    if toolchain:
        cmd.append(f"+{toolchain}")
    cmd += ["miri", "test", "-p", "expanse-trie", "--lib", "--",
            "--ignored", "--exact", f"{MODULE}::{entry.test}"]
    env = dict(os.environ)
    env["MIRIFLAGS"] = f"{CHECKS[entry.check]} -Zmiri-seed={seed}"
    try:
        cp = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    except FileNotFoundError as exc:
        return Observation("error", detail=f"cannot run cargo: {exc}")
    obs = classify(cp.returncode, cp.stdout + cp.stderr)
    if obs.kind == "ub":
        return Observation("ub", obs.message, obs.frame, detail=f"seed {seed}")
    return obs


def run_one(entry: Entry, seeds: str, toolchain: Optional[str]) -> Observation:
    return first_seed(run_seed(entry, s, toolchain) for s in seed_range(seeds))


def main_run(args: argparse.Namespace) -> int:
    try:
        data = json.loads(MANIFEST.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        print(f"cannot read {MANIFEST.relative_to(ROOT)}: {exc}", file=sys.stderr)
        return 2
    entries, seeds, errors = load_manifest(data, module_tests(SYNC_RS.read_text()))
    if args.seeds:
        if not re.fullmatch(r"\d+\.\.\d+", args.seeds):
            print(f"--seeds must look like 0..4, got {args.seeds}", file=sys.stderr)
            return 2
        seeds = args.seeds
    if errors:
        for e in errors:
            print(f"::error::miri-ub-sites manifest: {e}")
        return 2
    toolchain = args.toolchain or data["toolchain"]
    host = host_target(toolchain)
    if host != data["target"]:
        msg = (f"the manifest was generated on {data['target']} with {data['toolchain']}; "
               f"this host is {host or 'unknown'} ({toolchain})")
        if not args.observe:
            print(f"::error::miri-ub-sites could not check: {msg}")
            return 2
        print(f"::notice::observing off the manifest's target: {msg}")
    if args.only:
        entries = [e for e in entries if e.test == args.only]
        if not entries:
            print(f"--only {args.only}: no manifest entry", file=sys.stderr)
            return 2
    worst = 0
    for e in entries:
        obs = run_one(e, seeds, toolchain)
        if args.observe:
            status = "could-not-check" if obs.kind == "error" else "observed"
            reason = obs.detail if obs.kind == "error" else (
                "clean" if obs.kind == "clean" else f"[{obs.detail}] {obs.message} @ {obs.frame}")
        else:
            status, reason = judge(e, obs)
        print(f"{status:16} {e.test:24} {e.check:9} {reason}", flush=True)
        if status == "mismatch":
            worst = max(worst, 1)
            print(f"::error::{e.test} ({e.check}): {reason}")
        elif status == "could-not-check":
            worst = 2
            print(f"::error::{e.test} ({e.check}) could not be checked: {reason}")
    return worst


def self_test() -> int:
    race = (
        "test sync::miri_ub_sites::map_leaf_two_writers ... error: Undefined Behavior: "
        "Data race detected between (1) non-atomic read on thread `unnamed-3` and "
        "(2) non-atomic write on thread `unnamed-2` at alloc102268+0x40\n"
        "    = note: stack backtrace:\n"
        "            0: expanse_trie::map::MapCore::insert_inner\n"
        "                at src/map.rs:1388:17: 1388:55\n"
        "            1: expanse_trie::map::MapCore::insert::{closure#0}\n"
    )
    alias = (
        "error: Undefined Behavior: write access through <249921> at alloc412838[0x40] is forbidden\n"
        "  = note: stack backtrace:\n"
        "          0: expanse_trie::map::MapCore::insert_inner\n"
    )
    clean = "running 1 test\ntest x ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored\n"
    empty = "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored\n"
    panic = "thread 'x' panicked at src/sync.rs:1:1:\nassertion failed\ntest result: FAILED. 0 passed; 1 failed\n"
    build = "error[E0425]: cannot find value `x` in this scope\nerror: could not compile `expanse-trie`\n"

    def check(name: str, got, want) -> None:
        if got != want:
            raise AssertionError(f"{name}: got {got!r}, want {want!r}")

    # classify: each shape a run can take.
    o = classify(1, race)
    check("race kind", o.kind, "ub")
    check("race frame", o.frame, "expanse_trie::map::MapCore::insert_inner")
    check("race message", o.message.startswith("Data race detected"), True)
    check("alias kind", classify(1, alias).kind, "ub")
    check("clean", classify(0, clean).kind, "clean")
    check("zero tests is not clean", classify(0, empty).kind, "error")
    check("assertion is not clean", classify(101, panic).kind, "error")
    check("build error is not clean", classify(101, build).kind, "error")
    # A passing line with a non-zero exit is not clean either (a later seed failed without UB).
    check("rc overrides a passing line", classify(1, clean).kind, "error")

    site = Entry("map_leaf_two_writers", "race", "ub", "Data race detected", ("MapCore::insert_inner",))
    either = Entry("map_leaf_two_writers", "race", "ub", "Data race detected",
                   ("MapCore::insert_inner", "MapCore::root_is_tree"))
    fixed = Entry("map_leaf_two_writers", "race", "clean")
    check("site reproduces", judge(site, classify(1, race))[0], "ok")
    check("site stopped", judge(site, classify(0, clean))[0], "mismatch")
    check("stopped says flip", "flip the entry" in judge(site, classify(0, clean))[1], True)
    check("other message", judge(site, classify(1, alias))[0], "mismatch")
    moved = race.replace("MapCore::insert_inner", "MapCore::occ_snapshot")
    check("frame moved", judge(site, classify(1, moved))[0], "mismatch")
    other_side = race.replace("MapCore::insert_inner", "MapCore::root_is_tree")
    check("either side of a race", judge(either, classify(1, other_side))[0], "ok")
    check("neither side", judge(either, classify(1, moved))[0], "mismatch")
    check("clean stays clean", judge(fixed, classify(0, clean))[0], "ok")
    check("clean regressed", judge(fixed, classify(1, race))[0], "mismatch")
    check("error never ok (ub)", judge(site, classify(101, build))[0], "could-not-check")
    check("error never ok (clean)", judge(fixed, classify(0, empty))[0], "could-not-check")

    # The seed walk: lowest non-clean seed wins, and stops there.
    ub = classify(1, race)
    ok_run = classify(0, clean)
    seen: List[int] = []

    def runs(seq):
        for i, o in enumerate(seq):
            seen.append(i)
            yield o

    check("all clean", first_seed(runs([ok_run, ok_run, ok_run])).kind, "clean")
    seen.clear()
    check("first UB wins", first_seed(runs([ok_run, ub, classify(1, alias)])).message, ub.message)
    check("stops at the first UB", seen, [0, 1])
    check("error is not skipped", first_seed(iter([ok_run, classify(101, build), ub])).kind, "error")
    check("empty range is not clean", first_seed(iter([])).kind, "error")
    check("seed range", list(seed_range("2..5")), [2, 3, 4])

    # The census of the module, and the manifest rules.
    src = (
        "mod miri_ub_sites {\n    #[test]\n    #[cfg_attr(miri, ignore = \"x\")]\n"
        "    fn a() {}\n    #[test]\n    fn b() {}\n    fn helper() {}\n}\n"
        "mod other {\n    #[test]\n    fn c() {}\n}\n"
    )
    check("module tests", module_tests(src), ["a", "b"])
    # The input CodeQL flagged for the regex this parser replaced: a `#[test]`
    # followed by many `]#[` repetitions. Linear now, and still no test.
    hostile = "mod miri_ub_sites {\n    #[test]#[" + "]#[" * 50_000 + "\n}\n"
    check("pathological attributes", module_tests(hostile), [])
    ok = {"seeds": "0..4", "toolchain": "nightly-2026-09-05", "target": "aarch64-apple-darwin", "sites": [
        {"test": "a", "check": "race", "expect": "ub", "error": "Data race", "frame": ["f", "g"]},
        {"test": "b", "check": "alias-tb", "expect": "clean"},
    ]}
    check("valid manifest", load_manifest(ok, ["a", "b"])[2], [])
    bad = {"seeds": "4", "toolchain": "nightly", "sites": [
        {"test": "a", "check": "tsan", "expect": "ub"},
        {"test": "a", "check": "tsan", "expect": "maybe"},
        {"test": "z", "check": "race", "expect": "clean"},
    ]}
    errs = " ".join(load_manifest(bad, ["a", "b"])[2])
    for needle in ("`seeds`", "dated nightly", "`target`", "unknown check", "expect must be", "names its `error`",
                   "listed twice", "no test `z`", "test `b`"):
        check(f"manifest rejects: {needle}", needle in errs, True)

    # Host detection, and the workflow pin read from a job body.
    check("host", parse_host("rustc 1.100.0-nightly\nhost: aarch64-apple-darwin\nrelease: 1.100.0\n"),
          "aarch64-apple-darwin")
    check("host absent", parse_host("rustc 1.0\n"), "")
    wf = ("jobs:\n  test-tsan:\n    runs-on: ubuntu-latest\n  miri-ub-sites:\n    runs-on: macos-latest\n"
          "    steps:\n      - uses: x\n        with:\n          toolchain: nightly-2026-09-05\n  bench-report:\n"
          "    runs-on: ubuntu-latest\n")
    check("workflow pin", workflow_pin(wf), ("macos-latest", "nightly-2026-09-05"))

    # The committed manifest and module agree now, and the nightly job runs
    # the manifest's toolchain on a runner of the manifest's target.
    committed = json.loads(MANIFEST.read_text())
    runner, tc = workflow_pin(NIGHTLY.read_text())
    check("nightly job toolchain is the manifest's", tc, committed.get("toolchain"))
    check("nightly job runner provides the manifest's target", runner, RUNNERS.get(committed.get("target")))
    entries, _, errors = load_manifest(committed, module_tests(SYNC_RS.read_text()))
    check("committed manifest valid", errors, [])
    check("committed manifest non-empty", len(entries) > 0, True)
    print(f"miri_ub_sites self-test OK ({len(entries)} manifest entries)")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="judge every entry against the manifest")
    mode.add_argument("--observe", action="store_true", help="print what each entry reports")
    mode.add_argument("--self-test", action="store_true")
    ap.add_argument("--only", help="one workload")
    ap.add_argument("--toolchain", help="run `cargo +TOOLCHAIN miri`")
    ap.add_argument("--seeds", help="override the manifest seed range (e.g. 0..2) for a quick development run; a verdict needs the manifest's own range")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    return main_run(args)


if __name__ == "__main__":
    sys.exit(main())
