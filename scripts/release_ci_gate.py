#!/usr/bin/env python3
"""Release gate: the released commit must have passed a FULL `ci.yml` run.

A push to `main` that lands a tree CI already passed in full runs `ci.yml`'s
fast lane only (lint, docs-lint; `push_tree_verified.py`): its
`CI Gate / All Checks Passed` check succeeds although no test, Miri, sanitizer,
Callgrind or binding job ran on that commit (docs/CI.md section 3). The release gate therefore
cannot read that check-run. It reads the commit's `ci.yml` runs instead and
classifies each one:

  full     the `fast-lane` job succeeded, so every slow lane was released; or
           the run completed with no `fast-lane` job at all, which only a run
           from before the fast lane existed can do
  partial  the `fast-lane` job concluded anything but success (a push, a
           draft, or a failed lint / docs-lint)
  pending  the run, or its `fast-lane` job, has not concluded yet

The most recent full-or-pending run decides: success passes, any other
conclusion fails, pending waits. When the commit has no such run, the gate
dispatches `ci.yml` on the release ref once and waits for that run, so a
release needs no manual full run first. It fails closed: an API error, a
missing ref to dispatch on, or the deadline is a failure, never a pass.

Run:  release_ci_gate.py --repo OWNER/NAME --sha SHA [--dispatch-ref REF]
                         [--deadline-minutes N] [--poll-seconds N]
      release_ci_gate.py --self-test
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time

WORKFLOW = "ci.yml"
FAST_LANE_NAME = "Core / Fast Lane Passed"
GATE_NAME = "CI Gate / All Checks Passed"


def classify(run: dict, jobs: list[dict]) -> str:
    """Return 'full', 'partial' or 'pending' for one `ci.yml` run."""
    by_name = {j.get("name"): j for j in jobs}
    fast = by_name.get(FAST_LANE_NAME)
    if fast is None:
        if run.get("status") != "completed":
            return "pending"  # the job list is not complete until the run is
        # Completed without a fast lane: a run from before it existed, which
        # ran every job its filters selected. Without the gate job it is not
        # a ci.yml rollup run at all.
        return "full" if GATE_NAME in by_name else "partial"
    if fast.get("status") != "completed":
        return "pending"
    return "full" if fast.get("conclusion") == "success" else "partial"


def decide(classified: list[tuple[dict, str]]) -> tuple[str, str]:
    """Return (verdict, detail); verdict is pass | fail | wait | dispatch."""
    candidates = [(r, c) for r, c in classified if c in ("full", "pending")]
    if not candidates:
        partial = len(classified)
        return "dispatch", (
            f"no full ci.yml run on this commit ({partial} run(s), none of them full: "
            "a push to main whose tree already passed runs the fast lane only)"
        )
    run, cls = max(candidates, key=lambda rc: (rc[0].get("created_at") or "", rc[0].get("id") or 0))
    ident = f"run {run.get('id')} ({run.get('event')}, attempt {run.get('run_attempt', 1)})"
    if cls == "pending" or run.get("status") != "completed":
        return "wait", f"{ident} has not concluded"
    if run.get("conclusion") == "success":
        return "pass", f"{ident} is a full run and concluded success"
    return "fail", (
        f"the latest full ci.yml run on this commit, {ident}, concluded "
        f"{run.get('conclusion')!r}. Fix it or re-run it to success, then re-run the release"
    )


def _gh(args: list[str]) -> str:
    proc = subprocess.run(["gh", *args], capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(f"gh {' '.join(args)} failed ({proc.returncode}): {proc.stderr.strip()}")
    return proc.stdout


def _pages(path: str) -> list[dict]:
    out = _gh(["api", "--paginate", path])
    dec, i, pages = json.JSONDecoder(), 0, []
    out = out.strip()
    while i < len(out):
        obj, i = dec.raw_decode(out, i)
        pages.append(obj)
        while i < len(out) and out[i].isspace():
            i += 1
    return pages


def fetch(repo: str, sha: str) -> list[tuple[dict, str]]:
    runs = [
        r
        for page in _pages(f"repos/{repo}/actions/workflows/{WORKFLOW}/runs?head_sha={sha}&per_page=100")
        for r in page.get("workflow_runs", [])
    ]
    classified = []
    for run in runs:
        jobs = [
            j
            for page in _pages(f"repos/{repo}/actions/runs/{run['id']}/jobs?per_page=100&filter=latest")
            for j in page.get("jobs", [])
        ]
        classified.append((run, classify(run, jobs)))
    return classified


def gate(repo: str, sha: str, dispatch_ref: str | None, deadline_s: float, poll_s: float) -> int:
    deadline = time.monotonic() + deadline_s
    dispatched = False
    while True:
        try:
            verdict, detail = decide(fetch(repo, sha))
        except (RuntimeError, json.JSONDecodeError, KeyError) as exc:
            print(f"::error::cannot read the ci.yml runs of {sha} ({exc}) -- failing closed")
            return 1
        print(f"release gate for {sha}: {verdict} -- {detail}")
        if verdict == "pass":
            return 0
        if verdict == "fail":
            print(f"::error::{detail}")
            return 1
        if verdict == "dispatch" and not dispatched:
            if not dispatch_ref:
                print(f"::error::{detail}; no --dispatch-ref to start one on. Run "
                      f"`gh workflow run {WORKFLOW} --ref <branch-or-tag>` and re-run the release")
                return 1
            try:
                _gh(["workflow", "run", WORKFLOW, "--repo", repo, "--ref", dispatch_ref])
            except RuntimeError as exc:
                print(f"::error::could not dispatch {WORKFLOW} on {dispatch_ref} ({exc})")
                return 1
            dispatched = True
            print(f"::notice::dispatched a full {WORKFLOW} run on {dispatch_ref} for {sha}")
        if time.monotonic() >= deadline:
            hint = (f" The run dispatched on {dispatch_ref!r} never appeared for {sha}: "
                    "the ref may have moved to another commit.") if dispatched else ""
            print(f"::error::timed out waiting for a full {WORKFLOW} run on {sha} ({detail}).{hint}")
            return 1
        time.sleep(poll_s)


def self_test() -> int:
    failures: list[str] = []

    def check(label, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    def run(i, event, status="completed", conclusion="success", created="2026-09-25T10:00:00Z"):
        return {"id": i, "event": event, "status": status, "conclusion": conclusion,
                "created_at": created, "run_attempt": 1}

    def job(name, status="completed", conclusion="success"):
        return {"name": name, "status": status, "conclusion": conclusion}

    gate_ok = job(GATE_NAME)
    # classify
    check("fast lane passed -> full",
          classify(run(1, "schedule"), [job(FAST_LANE_NAME), gate_ok]), "full")
    check("fast lane skipped -> partial",
          classify(run(1, "push"), [job(FAST_LANE_NAME, conclusion="skipped"), gate_ok]), "partial")
    check("fast lane still running -> pending",
          classify(run(1, "workflow_dispatch", status="in_progress", conclusion=None),
                   [job(FAST_LANE_NAME, status="in_progress", conclusion=None)]), "pending")
    check("in-progress run without its job list yet -> pending",
          classify(run(1, "push", status="queued", conclusion=None), []), "pending")
    check("completed run from before the fast lane -> full",
          classify(run(1, "push"), [job("Core / Linter & Formatting"), gate_ok]), "full")
    check("completed run with no gate job -> partial",
          classify(run(1, "push"), [job("Core / Linter & Formatting")]), "partial")

    # THE HOLE THIS GATE EXISTS FOR: a push to main whose rollup check-run
    # succeeded with only the fast lane. The previous gate read that
    # check-run's conclusion alone and would have released the commit.
    push_only = [(run(1, "push"), classify(run(1, "push"),
                                            [job(FAST_LANE_NAME, conclusion="skipped"), gate_ok]))]
    check("fast-lane-only push does not pass", decide(push_only)[0], "dispatch")

    full_ok = (run(2, "schedule", created="2026-09-25T11:00:00Z"), "full")
    full_bad = (run(3, "workflow_dispatch", conclusion="failure", created="2026-09-25T12:00:00Z"), "full")
    pending = (run(4, "workflow_dispatch", status="in_progress", conclusion=None,
                   created="2026-09-25T13:00:00Z"), "pending")
    check("a successful full run passes", decide(push_only + [full_ok])[0], "pass")
    check("the latest full run decides: failure", decide(push_only + [full_ok, full_bad])[0], "fail")
    check("a newer pending run is waited for", decide([full_ok, full_bad, pending])[0], "wait")
    check("no runs at all dispatches", decide([])[0], "dispatch")

    if failures:
        for f in failures:
            print(f"::error::release_ci_gate self-test: {f}")
        return 1
    print("release_ci_gate --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--repo")
    ap.add_argument("--sha")
    ap.add_argument("--dispatch-ref", help="branch or tag to dispatch ci.yml on when no full run exists")
    ap.add_argument("--deadline-minutes", type=float, default=180)
    ap.add_argument("--poll-seconds", type=float, default=60)
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.repo or not args.sha:
        print("::error::--repo and --sha are required", file=sys.stderr)
        return 1
    return gate(args.repo, args.sha, args.dispatch_ref, args.deadline_minutes * 60, args.poll_seconds)


if __name__ == "__main__":
    sys.exit(main())
