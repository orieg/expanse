#!/usr/bin/env python3
"""Decide whether a push to main re-verifies a tree CI already verified.

The branch ruleset does not require a pull request to be up to date before it
merges (`strict_required_status_checks_policy` is false), so a squash merge
can land a tree that no CI run ever built: the pull request's changes on top
of a `main` that moved after its last run. Of the 40 pull requests merged
before this script existed, 23 landed a tree that differed from their head's.

A push to main may therefore skip the slow lanes only when the pushed commit's
tree is byte-identical to the tree of the merged pull request's head, AND that
head's latest full `ci.yml` run succeeded (full: its `fast-lane` job
succeeded, as `release_ci_gate.py` classifies it). Then the slow lanes would
rebuild exactly the code they already passed. Anything else -- a different
tree, no associated pull request, a head whose latest full run failed or is
still running, an API error -- answers false, and the push runs the full
matrix. Every doubt resolves toward running more, never less.

Writes `push-verified=true|false` to $GITHUB_OUTPUT (or stdout) with a
`::notice::` naming the reason.

Run:  push_tree_verified.py --repo OWNER/NAME --sha SHA
      push_tree_verified.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path


def verdict(merge_tree: str | None, head_tree: str | None, head_gate: str | None) -> tuple[bool, str]:
    """Pure decision: (verified, reason)."""
    if not merge_tree or not head_tree:
        return False, "could not resolve both trees"
    if merge_tree != head_tree:
        return False, ("the merged tree differs from the pull request head's tree "
                       "(the pull request was not up to date with main)")
    if head_gate != "pass":
        return False, f"the pull request head's latest full ci.yml run is not a pass ({head_gate})"
    return True, "the merged tree is the pull request head's tree, and that head passed a full ci.yml run"


def _api(path: str) -> object:
    proc = subprocess.run(["gh", "api", path], capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(f"gh api {path}: {proc.stderr.strip()}")
    return json.loads(proc.stdout)


def evaluate(repo: str, sha: str) -> tuple[bool, str]:
    # The full-run classification is release_ci_gate.py's, so the two gates
    # cannot disagree about what a full run is.
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import release_ci_gate

    pulls = _api(f"repos/{repo}/commits/{sha}/pulls")
    merged = [p for p in pulls if p.get("merge_commit_sha") == sha]
    if not merged:
        return False, "no merged pull request has this commit as its merge commit"
    pr = merged[0]
    head = pr["head"]["sha"]
    merge_tree = _api(f"repos/{repo}/git/commits/{sha}")["tree"]["sha"]
    head_tree = _api(f"repos/{repo}/git/commits/{head}")["tree"]["sha"]
    gate, detail = release_ci_gate.decide(release_ci_gate.fetch(repo, head))
    ok, reason = verdict(merge_tree, head_tree, gate)
    return ok, f"#{pr['number']} head {head[:8]}: {reason} [{detail}]"


def write_output(verified: bool) -> None:
    line = f"push-verified={'true' if verified else 'false'}\n"
    target = os.environ.get("GITHUB_OUTPUT")
    if target:
        with open(target, "a", encoding="utf-8") as fh:
            fh.write(line)
    else:
        sys.stdout.write(line)


def self_test() -> int:
    failures: list[str] = []

    def check(label, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    check("identical tree, head passed -> verified", verdict("t1", "t1", "pass")[0], True)
    # THE DEFECT THIS EXISTS FOR: a pull request merged behind main. Its head
    # passed, but the squashed tree is not the tree that passed.
    check("different tree -> full run", verdict("t2", "t1", "pass")[0], False)
    check("identical tree, head's full run failed -> full run", verdict("t1", "t1", "fail")[0], False)
    check("identical tree, head still running -> full run", verdict("t1", "t1", "wait")[0], False)
    check("identical tree, head never had a full run -> full run",
          verdict("t1", "t1", "dispatch")[0], False)
    check("unresolved tree -> full run", verdict(None, "t1", "pass")[0], False)
    if failures:
        for f in failures:
            print(f"::error::push_tree_verified self-test: {f}")
        return 1
    print("push_tree_verified --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--repo")
    ap.add_argument("--sha")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.repo or not args.sha:
        print("::error::--repo and --sha are required", file=sys.stderr)
        return 1
    try:
        ok, reason = evaluate(args.repo, args.sha)
    except (RuntimeError, KeyError, IndexError, TypeError, json.JSONDecodeError) as exc:
        ok, reason = False, f"could not decide ({exc}); running the full matrix"
    print(f"::notice::push-verified={'true' if ok else 'false'} for {args.sha[:8]}: {reason}")
    write_output(ok)
    return 0


if __name__ == "__main__":
    sys.exit(main())
