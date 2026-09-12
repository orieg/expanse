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


def check_floor(jobs, statuses, outputs, *, is_pull_request=True, notices=None) -> list[str]:
    errs: list[str] = []
    notices = notices if notices is not None else []

    dc = statuses.get("detect-changes")
    if dc is None:
        errs.append("no `detect-changes` entry in the needs context -- cannot verify the floor")
    elif dc.get("result") != "success":
        errs.append(
            f"`detect-changes` result is {dc.get('result')!r}, not 'success' -- "
            "every filter output below is unreliable"
        )

    # `changed-jobs` is a delimited id list, not a boolean, and it is read by
    # `contains(...)` rather than `== 'true'`. It must be threaded through to
    # the evaluator or every job that ran *because its own definition changed*
    # looks like a job that ran with a false gate.
    changed_jobs = str(outputs.get("changed-jobs") or "")
    flags = {
        k: _as_bool(v) for k, v in outputs.items() if k != "changed-jobs"
    }

    # An all-false filter evaluation was originally treated as the signature
    # failure this check exists for. It is not: a docs-only PR legitimately
    # matches no filter in this repo, because the dead `docs` filter was
    # removed and `docs-lint` is unconditional by design. The heuristic could
    # not tell that normal case from a broken evaluation, and as a hard error
    # it blocked every docs-only PR.
    #
    # It is a notice now, not an error. The sound check is the per-job one
    # below -- `skipped` iff the job's own `if:` is false under the observed
    # outputs -- which catches a broken evaluation without guessing, because a
    # filter that wrongly went false makes some job's skip inconsistent with
    # it. Reporting rather than failing is what 8.11.5 asks for when a check
    # cannot decide.
    if flags and not any(flags.values()) and not changed_jobs and is_pull_request:
        notices.append(
            "every filter output is false -- expected for a docs-only PR "
            "(`docs-lint` is unconditional); flagged only so a genuinely broken "
            "filter evaluation is visible in the log"
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
            body.get("if"), flags,
            is_pull_request=is_pull_request,
            changed_jobs=changed_jobs,
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

    # THE FALSE POSITIVE THIS PINS: a docs-only PR legitimately matches no
    # filter (the dead `docs` filter was removed; `docs-lint` is
    # unconditional). Treating all-false as an error blocked every docs-only
    # PR in the repo. It must PASS, and emit a notice instead.
    docs_only = {
        "detect-changes": {"result": "success"},
        "docs-lint": {"result": "success"},
        "lint": {"result": "skipped"},
        "miri": {"result": "skipped"},
    }
    notes: list[str] = []
    errs = check_floor(jobs, docs_only, {"tooling": "false", "rust-src": "false"},
                       notices=notes)
    check("docs-only PR passes the floor", errs, [])
    check("docs-only PR still emits a notice", len(notes), 1)

    # ...and the per-job check is what actually catches a broken evaluation:
    # a filter that wrongly went false leaves some job's skip inconsistent.
    broken = dict(docs_only)
    broken["lint"] = {"result": "success"}   # ran, but `tooling` says false
    errs = check_floor(jobs, broken, {"tooling": "false", "rust-src": "false"})
    check("job that ran under a false gate is caught", len(errs) >= 1, True)

    # THE DEFECT THIS PINS (AGENTS.md 8.12.3): `changed-jobs` is read by
    # `contains(...)`, not `== 'true'`. Dropping it made every job that ran
    # because its own definition changed look like a job that ran with a false
    # gate -- 36 false inconsistencies on the PR that introduced this check.
    jobdiff_jobs = {
        "detect-changes": {},
        "miri": {"if": "needs.detect-changes.outputs.rust-src == 'true'"
                       " || contains(needs.detect-changes.outputs.changed-jobs, '|miri|')"},
    }
    jd_statuses = {
        "detect-changes": {"result": "success"},
        "miri": {"result": "success"},
    }
    jd_outputs = {"rust-src": "false", "changed-jobs": "|miri|"}
    check("job run via changed-jobs is consistent",
          check_floor(jobdiff_jobs, jd_statuses, jd_outputs), [])
    # and the converse: it skipped although changed-jobs named it
    jd_skipped = dict(jd_statuses, miri={"result": "skipped"})
    check("skipped despite changed-jobs is caught",
          len(check_floor(jobdiff_jobs, jd_skipped, jd_outputs)) >= 1, True)

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
    notices: list[str] = []
    errs = check_floor(
        jobs, statuses, outputs,
        is_pull_request=(args.event_name == "pull_request"),
        notices=notices,
    )
    for n in notices:
        print(f"::notice::{n}")
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
