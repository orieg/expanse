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

With `--require-ancestor-of BRANCH` it first fails unless the commit is on
that branch: a tag cut from a stale checkout or a side branch is refused
before anything waits on CI.

Run:  release_ci_gate.py --repo OWNER/NAME --sha SHA [--dispatch-ref REF]
                         [--deadline-minutes N] [--poll-seconds N]
                         [--require-ancestor-of BRANCH]
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


def reached_from(compare_status: str) -> bool:
    """Whether `sha` is on the branch, from the status of `compare/{sha}...{branch}`.

    The compare API reports the branch relative to `sha`: `identical` (the
    branch head is `sha`) and `ahead` (the branch contains `sha` and more) mean
    `sha` is an ancestor of the branch head. `behind` and `diverged` mean it is
    not, and anything else is not an answer.
    """
    return compare_status in ("identical", "ahead")


def on_branch(repo: str, sha: str, branch: str) -> int:
    """Fails unless `sha` is an ancestor of `branch`'s head. Fails closed on an API error."""
    try:
        status = json.loads(_gh(["api", f"repos/{repo}/compare/{sha}...{branch}"])).get("status")
    except (RuntimeError, json.JSONDecodeError) as exc:
        print(f"::error::cannot compare {sha} with {branch} ({exc}) -- failing closed. Check network connectivity or GitHub API status, then re-run")
        return 1
    if not reached_from(status):
        print(f"::error::{sha} is not on {branch} (compare status {status!r}): a release is cut "
              f"from a commit of {branch}, never from a side branch or a stale checkout. "
              f"Fast-forward or merge {branch} to include {sha}, or check out {branch} and re-cut the release")
        return 1
    print(f"release gate for {sha}: on {branch} ({status})")
    return 0


def gate(repo: str, sha: str, dispatch_ref: str | None, deadline_s: float, poll_s: float) -> int:
    deadline = time.monotonic() + deadline_s
    dispatched = False
    while True:
        try:
            verdict, detail = decide(fetch(repo, sha))
        except (RuntimeError, json.JSONDecodeError, KeyError) as exc:
            print(f"::error::cannot read the ci.yml runs of {sha} ({exc}) -- failing closed. "
                  f"Check network connectivity or GH_TOKEN permissions, then re-run the release")
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
                print(f"::error::could not dispatch {WORKFLOW} on {dispatch_ref} ({exc}). "
                      f"Dispatch manually with `gh workflow run {WORKFLOW} --ref {dispatch_ref}` and re-run the release")
                return 1
            dispatched = True
            print(f"::notice::dispatched a full {WORKFLOW} run on {dispatch_ref} for {sha}")
        if time.monotonic() >= deadline:
            hint = (f" The run dispatched on {dispatch_ref!r} never appeared for {sha}: "
                    "the ref may have moved to another commit.") if dispatched else ""
            print(f"::error::timed out waiting for a full {WORKFLOW} run on {sha} ({detail}).{hint} "
                  f"Check GitHub Actions workflow status or increase --deadline-minutes, then re-run the release")
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

    # The released commit must be on the branch. The compare API describes the
    # branch relative to the commit, so "behind" is the off-branch answer.
    check("branch head is the commit", reached_from("identical"), True)
    check("branch moved past the commit", reached_from("ahead"), True)
    check("commit is ahead of the branch (not merged)", reached_from("behind"), False)
    check("commit on a side branch", reached_from("diverged"), False)
    check("no status is not a pass", reached_from(None), False)

    # Stub _gh for integration-level tests below.
    import sys as _sys
    _mod = _sys.modules[__name__]
    _orig_gh = _mod._gh

    # Test: on_branch builds the compare request as compare/{sha}...{branch}.
    _gh_calls: list[list[str]] = []
    def _fake_gh(args: list[str]) -> str:
        _gh_calls.append(args)
        return '{"status": "ahead"}'
    try:
        _mod._gh = _fake_gh
        on_branch("o/r", "abc123def", "main")
        check("on_branch builds compare/{sha}...{branch}",
              _gh_calls, [["api", "repos/o/r/compare/abc123def...main"]])
    finally:
        _mod._gh = _orig_gh

    # Test: on_branch returns non-zero when commit is not on branch.
    def _fake_gh_behind(args: list[str]) -> str:
        return '{"status": "behind"}'
    try:
        _mod._gh = _fake_gh_behind
        rc = on_branch("o/r", "abc123def", "main")
        check("on_branch returns non-zero when off-branch (behind)", rc != 0, True)
    finally:
        _mod._gh = _orig_gh

    # Test: on_branch also rejects "diverged" with a different error message.
    def _fake_gh_diverged(args: list[str]) -> str:
        return '{"status": "diverged"}'
    try:
        _mod._gh = _fake_gh_diverged
        rc = on_branch("o/r", "abc123def", "main")
        check("on_branch returns non-zero when off-branch (diverged)", rc != 0, True)
    finally:
        _mod._gh = _orig_gh

    # `gate`, with `fetch` stubbed. The failed-verdict case uses a FULL run
    # that concluded failure and a deadline an hour away, so the only way it
    # returns non-zero is by the failure branch; the deadline case uses a
    # pending run and a deadline that has already passed.
    _orig_fetch = fetch

    def gate_rc(classified, deadline_s):
        _mod.fetch = lambda repo, sha: classified
        try:
            return gate("o/r", "abc", None, deadline_s, 0)
        finally:
            _mod.fetch = _orig_fetch

    check("gate: a full successful run returns 0", gate_rc([full_ok], 3600), 0)
    check("gate: a full failed run returns 1 by the failure branch", gate_rc([full_ok, full_bad], 3600), 1)
    check("gate: a pending run past the deadline returns 1", gate_rc([pending], 0.0), 1)

    # `main` must stop on `on_branch`'s answer before it reads any run.
    def main_rc(status, extra):
        reads = []
        _mod._gh = lambda args: '{"status": "%s"}' % status
        _mod.fetch = lambda repo, sha: reads.append(sha) or [full_ok]
        saved = _sys.argv
        _sys.argv = ["release_ci_gate.py", "--repo", "o/r", "--sha", "abc", "--poll-seconds", "0"] + extra
        try:
            return main(), reads
        finally:
            _sys.argv = saved
            _mod._gh = _orig_gh
            _mod.fetch = _orig_fetch

    check("main: --require-ancestor-of with a commit off the branch returns 1, and reads no run",
          main_rc("behind", ["--require-ancestor-of", "main"]), (1, []))
    check("main: --require-ancestor-of with a commit on the branch goes on to the run",
          main_rc("ahead", ["--require-ancestor-of", "main"]), (0, ["abc"]))
    check("main: without --require-ancestor-of the branch is not asked",
          main_rc("behind", []), (0, ["abc"]))

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
    ap.add_argument("--require-ancestor-of", metavar="BRANCH",
                    help="fail unless --sha is an ancestor of this branch's head")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.repo or not args.sha:
        print("::error::--repo and --sha are required", file=sys.stderr)
        return 1
    if args.require_ancestor_of:
        rc = on_branch(args.repo, args.sha, args.require_ancestor_of)
        if rc:
            return rc
    return gate(args.repo, args.sha, args.dispatch_ref, args.deadline_minutes * 60, args.poll_seconds)


if __name__ == "__main__":
    sys.exit(main())
