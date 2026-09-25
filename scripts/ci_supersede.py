#!/usr/bin/env python3
"""Force-cancel a pull request's superseded `ci.yml` runs.

`ci-gate` runs `if: always()`, and must: a job skipped by its condition
reports "Success" to branch protection, so a gate that skipped on a cancelled
run would render the required context green over a run that verified nothing.
But an `always()` job also needs a runner after its run is cancelled, and
under a workflow-level concurrency group the superseded run keeps the group
until that job has had one -- so a new push waited behind its own cancelled
predecessor for as long as the runner pool was full.

`ci.yml` therefore gives every pull-request run its own concurrency group, and
this script, run first in `detect-changes`, does the superseding: it
force-cancels every older, unfinished `ci.yml` run of the same pull-request
head (branch and repository). A force-cancel does not start `always()` jobs,
so the old gate never takes a runner, and the old run's checks conclude
`cancelled`, which branch protection treats as a failure: nothing fails open.
The run that called it is never touched, nor is any newer run.

A cancellation the token cannot make (a fork's read-only token) or the API
refuses is reported by name as a warning and does not fail the run; the only
consequence is the old run finishing on its own.

Run:  ci_supersede.py --repo OWNER/NAME --run-id ID --branch HEAD_REF
                      --head-repo OWNER/NAME
      ci_supersede.py --self-test
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys

WORKFLOW = "ci.yml"
UNFINISHED = frozenset({"queued", "in_progress", "waiting", "pending", "requested"})


def superseded(runs: list[dict], run_id: int, branch: str, head_repo: str) -> list[dict]:
    """The runs this run supersedes: older, unfinished, same PR head."""
    return [
        r for r in runs
        if r.get("id") != run_id
        and (r.get("id") or 0) < run_id
        and r.get("event") == "pull_request"
        and r.get("head_branch") == branch
        and ((r.get("head_repository") or {}).get("full_name") == head_repo)
        and r.get("status") in UNFINISHED
    ]


def _gh(args: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(["gh", *args], capture_output=True, text=True)


def list_runs(repo: str, branch: str) -> list[dict]:
    proc = _gh(["api", "--paginate",
                f"repos/{repo}/actions/workflows/{WORKFLOW}/runs?branch={branch}&event=pull_request&per_page=100"])
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip())
    out, dec, i, runs = proc.stdout.strip(), json.JSONDecoder(), 0, []
    while i < len(out):
        page, i = dec.raw_decode(out, i)
        runs.extend(page.get("workflow_runs", []))
        while i < len(out) and out[i].isspace():
            i += 1
    return runs


def main_run(repo: str, run_id: int, branch: str, head_repo: str) -> int:
    try:
        runs = list_runs(repo, branch)
    except RuntimeError as exc:
        print(f"::warning::could not list {WORKFLOW} runs of {head_repo}:{branch} ({exc}) -- "
              "no superseded run was cancelled; older runs finish on their own")
        return 0
    targets = superseded(runs, run_id, branch, head_repo)
    if not targets:
        print(f"no superseded {WORKFLOW} run of {head_repo}:{branch} to cancel")
        return 0
    failed = []
    for r in targets:
        proc = _gh(["api", "-X", "POST", f"repos/{repo}/actions/runs/{r['id']}/force-cancel"])
        if proc.returncode == 0:
            print(f"force-cancelled superseded run {r['id']} (head {str(r.get('head_sha'))[:8]}, {r.get('status')})")
        else:
            failed.append(f"{r['id']} ({proc.stderr.strip()})")
    if failed:
        print(f"::warning::could not cancel superseded run(s): {'; '.join(failed)} -- "
              "they finish on their own; a fork's read-only token cannot cancel")
    return 0


def self_test() -> int:
    failures: list[str] = []

    def check(label, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    def run(i, status="in_progress", branch="feat/x", repo="o/r", event="pull_request"):
        return {"id": i, "status": status, "head_branch": branch, "event": event,
                "head_repository": {"full_name": repo}, "head_sha": f"{i:08x}"}

    runs = [
        run(10),                       # older, running: superseded
        run(11, status="queued"),      # older, queued (the stuck always() gate case)
        run(12, status="completed"),   # older, finished: left alone
        run(13, branch="feat/y"),      # another branch
        run(14, repo="fork/r"),        # same branch name, another repository
        run(15, event="push"),         # not a pull-request run
        run(20),                       # this run
        run(21),                       # newer: never touched
    ]
    got = [r["id"] for r in superseded(runs, 20, "feat/x", "o/r")]
    check("older unfinished runs of the same head only", got, [10, 11])
    check("the calling run is never a target",
          20 in [r["id"] for r in superseded(runs, 20, "feat/x", "o/r")], False)
    check("a newer run is never a target",
          21 in [r["id"] for r in superseded(runs, 20, "feat/x", "o/r")], False)
    check("nothing to cancel on a first run", superseded([run(20)], 20, "feat/x", "o/r"), [])

    if failures:
        for f in failures:
            print(f"::error::ci_supersede self-test: {f}")
        return 1
    print("ci_supersede --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--repo")
    ap.add_argument("--run-id", type=int)
    ap.add_argument("--branch")
    ap.add_argument("--head-repo")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not (args.repo and args.run_id and args.branch and args.head_repo):
        print("::error::--repo, --run-id, --branch and --head-repo are required", file=sys.stderr)
        return 1
    return main_run(args.repo, args.run_id, args.branch, args.head_repo)


if __name__ == "__main__":
    sys.exit(main())
