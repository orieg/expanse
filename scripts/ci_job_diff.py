#!/usr/bin/env python3
"""Decide which `ci.yml` job definitions a diff actually touched.

`.github/workflows/ci.yml` sat in the `rust-src` path filter, so any edit to it
ran the entire matrix. The reasoning was sound -- an edit here can rewrite a
safety job's build commands, flags or toolchain, `main` is protected, and PR
time is the only place a broken job is observable before it lands -- but the
conclusion was coarse: one glob stood in for 43 independent job definitions,
so adding a step to `lint` woke Miri, ASan, loom, fuzz, Callgrind, every
cross-compile lane and 34 binding checks.

A path glob cannot express "the diff touched job X". This can: it loads the
workflow at both revisions and compares each job's body.

It does NOT decide whether a job was *removed* -- running the matrix never
proved that either. `.github/ci-jobs.txt` is the snapshot that does, checked
by `check_ci_filters.py` the way `check_public_api.py` checks the Rust surface.

FAIL CLOSED (AGENTS.md 8.1). Every one of these emits `ci-workflow-all=true`,
i.e. the old behaviour, plus a `::notice::` naming the reason:
  - either revision fails to read or parse;
  - a job is added or removed;
  - anything outside `jobs:` changed (`on:`, `env:`, `permissions:`,
    `concurrency:`, `defaults:`), since those affect every job.
The narrowing only ever happens when the comparison fully succeeds.

Run:  ci_job_diff.py --base <sha> --head <sha> [--file PATH]
      ci_job_diff.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys

import yaml

WORKFLOW = ".github/workflows/ci.yml"
DELIM = "|"


def _emit(all_changed: bool, jobs: list[str]) -> str:
    """Render the GITHUB_OUTPUT lines, and ONLY those.

    `$GITHUB_OUTPUT` accepts `key=value` lines and nothing else -- a workflow
    command written into it fails the step with "Unable to process file command
    'output' successfully". So notices never go through here; they go to stdout
    via `_notice`. Pinned by `test_output_lines_are_key_value` below.

    Job ids are delimited on both sides so `contains()` -- which is substring,
    not word -- cannot match a prefix (`test-wasm` is a prefix of
    `test-wasm64`).
    """
    listed = DELIM + DELIM.join(sorted(jobs)) + DELIM if jobs else ""
    return "\n".join([
        f"ci-workflow-all={'true' if all_changed else 'false'}",
        f"changed-jobs={listed}",
    ])


def _notice(reason: str) -> None:
    """Workflow commands go to stdout, never to the output file."""
    if reason:
        print(f"::notice::ci_job_diff: {reason}")


def _write_output(text: str) -> None:
    """Append to `$GITHUB_OUTPUT` when running under Actions; otherwise print,
    so the script stays usable (and testable) from a shell."""
    dest = os.environ.get("GITHUB_OUTPUT")
    if dest:
        with open(dest, "a", encoding="utf-8") as fh:
            fh.write(text + "\n")
    else:
        print(text)


def _read(rev: str, path: str) -> str | None:
    try:
        out = subprocess.run(
            ["git", "show", f"{rev}:{path}"],
            capture_output=True, text=True, check=False,
        )
    except OSError:
        return None
    if out.returncode != 0:
        return None
    return out.stdout


def diff_jobs(base_text: str | None, head_text: str | None) -> tuple[bool, list[str], str]:
    """Returns (all_changed, changed_job_ids, reason)."""
    if base_text is None or head_text is None:
        return True, [], "could not read one revision of the workflow -- running everything"
    try:
        base = yaml.safe_load(base_text)
        head = yaml.safe_load(head_text)
    except yaml.YAMLError as exc:
        return True, [], f"workflow did not parse ({exc.__class__.__name__}) -- running everything"
    if not isinstance(base, dict) or not isinstance(head, dict):
        return True, [], "workflow is not a mapping -- running everything"

    base_jobs = base.get("jobs") or {}
    head_jobs = head.get("jobs") or {}
    if set(base_jobs) != set(head_jobs):
        added = sorted(set(head_jobs) - set(base_jobs))
        removed = sorted(set(base_jobs) - set(head_jobs))
        return True, [], (
            f"job set changed (added: {added or 'none'}, removed: {removed or 'none'}) "
            "-- running everything"
        )

    # Anything outside `jobs:` affects every job.
    base_top = {k: v for k, v in base.items() if k != "jobs"}
    head_top = {k: v for k, v in head.items() if k != "jobs"}
    if base_top != head_top:
        # `str(k)`: YAML 1.1 parses the `on:` trigger key as the boolean True,
        # so the key set is not all strings.
        keys = sorted(
            str(k) for k in set(base_top) | set(head_top)
            if base_top.get(k) != head_top.get(k)
        )
        return True, [], f"workflow-level key(s) changed ({', '.join(keys)}) -- running everything"

    changed = [name for name in head_jobs if base_jobs[name] != head_jobs[name]]
    return False, changed, ""


def self_test() -> int:
    failures: list[str] = []

    def check(label, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    A = """
on: [push]
jobs:
  lint:
    steps:
      - run: echo a
  miri:
    steps:
      - run: echo m
"""
    # one job's body changed -> only that job
    B = A.replace("echo a", "echo a && echo b")
    all_c, jobs, _ = diff_jobs(A, B)
    check("single job edit narrows", (all_c, jobs), (False, ["lint"]))

    # identical -> nothing
    check("no change", diff_jobs(A, A)[:2], (False, []))

    # job removed -> fail closed
    C = """
on: [push]
jobs:
  lint:
    steps:
      - run: echo a
"""
    all_c, jobs, reason = diff_jobs(A, C)
    check("removed job fails closed", all_c, True)
    check("removal is named", "removed" in reason, True)

    # top-level change -> fail closed
    D = A.replace("on: [push]", "on: [push, pull_request]")
    all_c, _, reason = diff_jobs(A, D)
    check("top-level change fails closed", all_c, True)

    # unparseable -> fail closed
    all_c, _, reason = diff_jobs(A, "jobs:\n  - [unbalanced\n")
    check("parse failure fails closed", all_c, True)

    # unreadable revision -> fail closed
    check("missing revision fails closed", diff_jobs(None, A)[0], True)

    # THE DEFECT THIS PINS (AGENTS.md 8.12.3): a `::notice::` written into
    # $GITHUB_OUTPUT fails the step with "Unable to process file command
    # 'output' successfully", which failed `detect-changes` and skipped 42
    # jobs behind it. Every line of the output payload must be `key=value`.
    for payload in (_emit(True, []), _emit(False, ["lint", "miri"])):
        for line in payload.split("\n"):
            if "=" not in line.split("=", 1)[0] + "=" or line.startswith("::"):
                failures.append(f"output line is not key=value: {line!r}")
            key = line.split("=", 1)[0]
            if not key or not re.fullmatch(r"[A-Za-z0-9_-]+", key):
                failures.append(f"output line has no valid key: {line!r}")

    # delimiter guards against contains() prefix matching
    out = _emit(False, ["test-wasm64"])
    check("wasm64 listed delimited", "changed-jobs=|test-wasm64|" in out, True)
    check("test-wasm is not a substring match", "|test-wasm|" in out, False)

    # the #870 shape: a step added to `lint` and nothing else
    check("870 shape narrows to lint", diff_jobs(A, B)[1], ["lint"])

    if failures:
        for f in failures:
            print(f"::error::ci_job_diff self-test: {f}")
        return 1
    print("ci_job_diff --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--base")
    ap.add_argument("--head", default="HEAD")
    ap.add_argument("--file", default=WORKFLOW)
    ap.add_argument("--json", action="store_true", help="emit JSON instead of output lines")
    args = ap.parse_args()
    if args.self_test:
        return self_test()

    if not args.base:
        _write_output(_emit(True, []))
        _notice("no base revision given -- running everything")
        return 0

    all_changed, jobs, reason = diff_jobs(
        _read(args.base, args.file), _read(args.head, args.file)
    )
    if args.json:
        print(json.dumps({"all": all_changed, "jobs": sorted(jobs), "reason": reason}))
        return 0
    if not all_changed and not jobs:
        reason = "workflow unchanged in this diff"
    elif not all_changed:
        reason = f"narrowed to {len(jobs)} changed job definition(s): {' '.join(sorted(jobs))}"
    _write_output(_emit(all_changed, jobs))
    _notice(reason)
    return 0


if __name__ == "__main__":
    sys.exit(main())
