#!/usr/bin/env python3
"""Assert the rollup's skip set is consistent with the filter outputs it saw.

`ci-gate` treats `skipped` as passing. That is the only workable rule for a
conditional matrix, but on its own it cannot tell *skipped because irrelevant*
from *skipped because the filter evaluation was wrong*. #671 proved the
evaluation can be silently wrong in the safe direction (everything ran).
Nothing checked the unsafe direction: if `detect-changes` succeeds but emits
all-false -- a paths-filter upgrade changing quantifier semantics, an API edge
on a very large PR, a renamed filter key resolving to empty string -- every
conditional job skips, `ci-gate` finds no `failure`, and a green required
context sits over a run in which nothing was verified (AGENTS.md 8.1).

This is the floor under that. It asserts:

  1. `detect-changes` succeeded (a filter set nobody computed gates nothing).
  2. The jobs declared unconditional actually ran -- never `skipped`.
  3. For every job, `skipped` matches its `if:` evaluating false under the
     filter outputs that were actually observed. A job that skipped while its
     own gate says it should have run is the silent-narrowing case.

Fails closed: an unparseable input, an unknown `if:` term, or a missing job is
an error, not a pass.

Run:  check_gate_floor.py --statuses <json> --outputs <json> [--event-name push]
      check_gate_floor.py --self-test
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from check_ci_filters import (  # noqa: E402
    UNCONDITIONAL_JOBS,
    evaluate_if,
    load_ci,
)


def _as_bool(value) -> bool:
    """paths-filter outputs arrive as the strings 'true'/'false'."""
    if isinstance(value, bool):
        return value
    return str(value).strip().lower() == "true"


def check_floor(jobs, statuses, outputs, *, is_pull_request=True) -> list[str]:
    errs: list[str] = []

    dc = statuses.get("detect-changes")
    if dc is None:
        errs.append("no `detect-changes` entry in the needs context -- cannot verify the floor")
    elif dc.get("result") != "success":
        errs.append(
            f"`detect-changes` result is {dc.get('result')!r}, not 'success' -- "
            "every filter output below is unreliable"
        )

    flags = {k: _as_bool(v) for k, v in outputs.items()}

    # A totally empty filter evaluation is the signature failure this exists
    # for. Called out separately so the error names the cause, not a symptom.
    if flags and not any(flags.values()) and is_pull_request:
        errs.append(
            "every filter output is false -- a pull request that matches no filter at all "
            "is far more likely a broken filter evaluation than a real no-op diff"
        )

    for name, body in jobs.items():
        st = statuses.get(name)
        if st is None:
            continue  # not in ci-gate's needs; check_ci_gate.py owns that
        result = st.get("result")
        if name in UNCONDITIONAL_JOBS:
            if result == "skipped":
                errs.append(f"{name!r} is unconditional but was skipped")
            continue
        should_run, unknown = evaluate_if(
            body.get("if"), flags, is_pull_request=is_pull_request
        )
        if unknown:
            errs.append(f"{name!r}: `if:` term not understood, cannot verify: {unknown!r}")
            continue
        if result == "skipped" and should_run:
            errs.append(
                f"{name!r} was skipped, but its `if:` is true under the observed filter "
                "outputs -- the matrix was narrowed by something other than the gate"
            )
        if result not in (None, "skipped") and not should_run:
            errs.append(
                f"{name!r} ran with result {result!r}, but its `if:` is false under the "
                "observed filter outputs"
            )
    return errs


def self_test() -> int:
    failures = []

    def check(label, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    jobs = {
        "detect-changes": {},
        "docs-lint": {},
        "ci-gate": {"if": "always()"},
        "lint": {"if": "needs.detect-changes.outputs.tooling == 'true'"},
        "miri": {"if": "needs.detect-changes.outputs.rust-src == 'true'"},
    }
    ok_statuses = {
        "detect-changes": {"result": "success"},
        "docs-lint": {"result": "success"},
        "lint": {"result": "success"},
        "miri": {"result": "skipped"},
    }
    ok_outputs = {"tooling": "true", "rust-src": "false"}
    check("consistent run passes", check_floor(jobs, ok_statuses, ok_outputs), [])

    # the motivating defect, pinned: all-false outputs with everything skipped
    all_skipped = {
        "detect-changes": {"result": "success"},
        "docs-lint": {"result": "success"},
        "lint": {"result": "skipped"},
        "miri": {"result": "skipped"},
    }
    errs = check_floor(jobs, all_skipped, {"tooling": "false", "rust-src": "false"})
    check("all-false filter evaluation is caught", len(errs) >= 1, True)

    # a job skipped while its own gate says run
    bad = dict(ok_statuses)
    bad["lint"] = {"result": "skipped"}
    errs = check_floor(jobs, bad, ok_outputs)
    check("skipped-but-should-run is caught", any("was skipped" in e for e in errs), True)

    # an unconditional job that skipped
    bad2 = dict(ok_statuses)
    bad2["docs-lint"] = {"result": "skipped"}
    errs = check_floor(jobs, bad2, ok_outputs)
    check("unconditional skip is caught", any("unconditional" in e for e in errs), True)

    # detect-changes failure is fatal
    bad3 = dict(ok_statuses)
    bad3["detect-changes"] = {"result": "failure"}
    errs = check_floor(jobs, bad3, ok_outputs)
    check("failed detect-changes is caught", any("detect-changes" in e for e in errs), True)

    # on push, a false filter still runs the job -- skipping it is a violation
    push_jobs = {
        "detect-changes": {},
        "t": {"if": "needs.detect-changes.outputs.rust-src == 'true' "
                    "|| github.event_name != 'pull_request'"},
    }
    push_statuses = {"detect-changes": {"result": "success"}, "t": {"result": "skipped"}}
    errs = check_floor(push_jobs, push_statuses, {"rust-src": "false"}, is_pull_request=False)
    check("push fallback is honoured", any("was skipped" in e for e in errs), True)

    if failures:
        for f in failures:
            print(f"::error::check_gate_floor self-test: {f}")
        return 1
    print("check_gate_floor --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--statuses", help="JSON of the `needs` context")
    ap.add_argument("--outputs", help="JSON of detect-changes outputs")
    ap.add_argument("--event-name", default="pull_request")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.statuses or not args.outputs:
        print("::error::--statuses and --outputs are required", file=sys.stderr)
        return 1
    try:
        statuses = json.loads(args.statuses)
        outputs = json.loads(args.outputs)
    except json.JSONDecodeError as exc:
        print(f"::error::could not parse gate inputs ({exc}) -- failing closed")
        return 1

    _, jobs, _, _ = load_ci()
    errs = check_floor(
        jobs, statuses, outputs, is_pull_request=(args.event_name == "pull_request")
    )
    if errs:
        for e in errs:
            print(f"::error::{e}")
        print(f"\ngate floor: {len(errs)} inconsistency(ies) between the skip set and the filters")
        return 1
    skipped = sum(1 for v in statuses.values() if v.get("result") == "skipped")
    print(f"gate floor OK: {len(statuses)} dependencies, {skipped} skipped, all consistent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
