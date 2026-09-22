#!/usr/bin/env python3
"""scripts/discipline_replay.py — replay merged pull requests through discipline.

Answers "what would this discipline binary and this `discipline.toml` have
done to the pull requests this repository actually merged?" It is how a
discipline upgrade or a configuration change is measured before it is adopted:
a gate belongs at `error` once the replay shows it blocking only changes that
genuinely needed a waiver (discipline.toml, header).

Each case is one merged commit on the default branch (the repository squash-
merges, so one commit is one pull request). For each, in a throwaway worktree:

  1. base  = the commit's first parent, plus the configuration under test,
             committed locally;
  2. head  = the commit's own diff applied on top of that base (its
             `discipline.toml` changes excluded), committed with the commit's
             original message, so directives written in commit messages count;
  3. `discipline check --base HEAD~1 --pr-body-file <that PR's body>`.

Rebuilding the base is what makes the replay mean something once
`discipline.toml` is tracked: checking the historical commit directly would
compare the configuration under test with whatever the commit's base carried,
and `config-integrity` reports that difference on every case (it runs whenever
the base configuration enables it, so `--disable` does not avoid it).

A case whose diff cannot be re-applied is reported as not reconstructed, and a
discipline exit of 2 as could-not-check; either makes this script exit 2. A
blocking finding is a measurement, not a harness failure, and exits 0.

The rebuilt commits are local to the throwaway worktree and are never pushed;
they are left unreachable when it is removed.

Usage:
  python3 scripts/discipline_replay.py --last 100
  python3 scripts/discipline_replay.py --since e55a0a5e          # commits after a fixed base
  python3 scripts/discipline_replay.py --last 100 --binary ./discipline --config candidate.toml
  python3 scripts/discipline_replay.py --self-test

Needs `git`, `gh` (authenticated, for pull request bodies) and a `discipline`
binary (`--binary`, else the one on PATH).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional

CONFIG_NAME = "discipline.toml"
PR_SUFFIX = re.compile(r"\(#(\d+)\)\s*$")

# The rebuilt commits are local scratch; they must not depend on the caller's
# signing setup or identity.
GIT_ENV = {
    "GIT_AUTHOR_NAME": "discipline-replay",
    "GIT_AUTHOR_EMAIL": "replay@localhost",
    "GIT_COMMITTER_NAME": "discipline-replay",
    "GIT_COMMITTER_EMAIL": "replay@localhost",
}


class ReplayError(RuntimeError):
    """A case could not be rebuilt or checked; never read as a pass."""


@dataclass
class Case:
    pr: Optional[int]
    sha: str
    body: str = ""


@dataclass
class Outcome:
    pr: Optional[int]
    sha: str
    rc: Optional[int] = None  # discipline exit code; None when not reconstructed
    report: Optional[dict] = None
    error: str = ""


def pr_number(subject: str) -> Optional[int]:
    """The `(#N)` a squash merge appends to its subject, if present."""
    m = PR_SUFFIX.search(subject)
    return int(m.group(1)) if m else None


def run(cmd: List[str], cwd: Optional[Path] = None, stdin: Optional[str] = None,
        env: Optional[Dict[str, str]] = None, check: bool = True) -> subprocess.CompletedProcess:
    full_env = dict(os.environ, **(env or {}))
    proc = subprocess.run(cmd, cwd=cwd, input=stdin, env=full_env, text=True, capture_output=True)
    if check and proc.returncode != 0:
        raise ReplayError(f"`{' '.join(cmd)}` exited {proc.returncode}: {proc.stderr.strip()[:400]}")
    return proc


def git(tree: Path, *args: str, stdin: Optional[str] = None, check: bool = True) -> subprocess.CompletedProcess:
    return run(["git", *args], cwd=tree, stdin=stdin, env=GIT_ENV, check=check)


def merged_prs(last: int) -> List[Case]:
    out = run(["gh", "pr", "list", "--state", "merged", "-L", str(last),
               "--json", "number,mergeCommit,body"]).stdout
    cases = []
    for p in json.loads(out):
        if not p.get("mergeCommit"):
            raise ReplayError(f"merged PR #{p['number']} has no merge commit")
        cases.append(Case(pr=p["number"], sha=p["mergeCommit"]["oid"], body=p.get("body") or ""))
    return cases


def commits_since(root: Path, since: str, until: str) -> List[Case]:
    shas = git(root, "rev-list", "--first-parent", "--reverse", f"{since}..{until}").stdout.split()
    cases = []
    for sha in shas:
        n = pr_number(git(root, "log", "-1", "--format=%s", sha).stdout.strip())
        body = ""
        if n is not None:
            body = run(["gh", "pr", "view", str(n), "--json", "body", "-q", ".body"]).stdout
        cases.append(Case(pr=n, sha=sha, body=body))
    return cases


def rebuild(tree: Path, sha: str, config_text: str) -> None:
    """Leave `tree` at head' whose parent is base' (see the module docstring)."""
    git(tree, "reset", "-q", "--hard")
    git(tree, "clean", "-fdq")
    git(tree, "checkout", "-q", "--detach", f"{sha}~1")
    (tree / CONFIG_NAME).write_text(config_text)
    git(tree, "add", CONFIG_NAME)
    git(tree, "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "replay base")
    patch = git(tree, "diff", "--binary", f"{sha}~1", sha, "--", ".", f":(exclude){CONFIG_NAME}").stdout
    if patch:
        applied = git(tree, "apply", "--index", "--binary", stdin=patch, check=False)
        if applied.returncode != 0:
            raise ReplayError(f"diff does not re-apply: {applied.stderr.strip()[:300]}")
    message = git(tree, "log", "-1", "--format=%B", sha).stdout
    git(tree, "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-F", "-", stdin=message)


def check(binary: str, tree: Path, body: str, out_dir: Path, label: str) -> Outcome:
    body_path = out_dir / f"body_{label}.txt"
    json_path = out_dir / f"report_{label}.json"
    body_path.write_text(body)
    proc = run([binary, "check", "--base", "HEAD~1", "--pr-body-file", str(body_path),
                "--format", "json", "--json-out", str(json_path)], cwd=tree, check=False)
    report = json.loads(json_path.read_text()) if json_path.exists() else None
    out = Outcome(pr=None, sha="", rc=proc.returncode, report=report)
    if proc.returncode not in (0, 1) or report is None:
        out.error = f"exit {proc.returncode}: " + (proc.stderr.strip() or "no report written")[:400]
    return out


def summarise(outcomes: List[Outcome]) -> dict:
    """Pure: counts and per-gate attribution from the per-case outcomes."""
    blocked, passed, could_not, not_rebuilt = [], [], [], []
    errors_by_gate: Dict[str, List[str]] = defaultdict(list)
    warnings_by_gate: Dict[str, int] = defaultdict(int)
    for o in outcomes:
        label = f"#{o.pr}" if o.pr is not None else o.sha[:8]
        if o.rc is None:
            not_rebuilt.append({"case": label, "error": o.error})
            continue
        # Only 0 (pass) and 1 (violations) are verdicts. 2 is could-not-check,
        # and anything else (a panic's 101, a signal) is a crash; neither may
        # be read as a pass (AGENTS.md §8.1).
        if o.rc not in (0, 1) or o.report is None:
            could_not.append({"case": label, "error": o.error or f"exit {o.rc}"})
            continue
        (blocked if o.rc == 1 else passed).append(label)
        for gate in o.report.get("outcomes", []):
            for v in gate.get("violations", []):
                if v.get("severity") == "error":
                    if label not in errors_by_gate[gate["gate"]]:
                        errors_by_gate[gate["gate"]].append(label)
                else:
                    warnings_by_gate[gate["gate"]] += 1
    return {
        "cases": len(outcomes),
        "passed": len(passed),
        "blocked": blocked,
        "could_not_check": could_not,
        "not_reconstructed": not_rebuilt,
        "errors_by_gate": dict(errors_by_gate),
        "warnings_by_gate": dict(warnings_by_gate),
    }


def render(s: dict) -> str:
    lines = [
        f"cases: {s['cases']}  passed: {s['passed']}  blocked: {len(s['blocked'])}  "
        f"could not check: {len(s['could_not_check'])}  not reconstructed: {len(s['not_reconstructed'])}",
    ]
    for gate, cases in sorted(s["errors_by_gate"].items(), key=lambda kv: -len(kv[1])):
        lines.append(f"  error    {gate:24s} {len(cases):3d}  {' '.join(cases)}")
    for gate, n in sorted(s["warnings_by_gate"].items(), key=lambda kv: -kv[1]):
        lines.append(f"  warning  {gate:24s} {n:3d} finding(s)")
    for item in s["could_not_check"]:
        lines.append(f"  COULD NOT CHECK {item['case']}: {item['error']}")
    for item in s["not_reconstructed"]:
        lines.append(f"  NOT RECONSTRUCTED {item['case']}: {item['error']}")
    return "\n".join(lines)


def replay(cases: List[Case], binary: str, config_text: str, root: Path, out_dir: Path) -> List[Outcome]:
    tree = Path(tempfile.mkdtemp(prefix="discipline-replay-tree-")) / "tree"
    git(root, "worktree", "add", "-q", "--detach", str(tree), cases[0].sha)
    outcomes = []
    try:
        for i, case in enumerate(cases, 1):
            label = str(case.pr) if case.pr is not None else case.sha[:8]
            try:
                rebuild(tree, case.sha, config_text)
            except ReplayError as exc:
                outcomes.append(Outcome(pr=case.pr, sha=case.sha, error=str(exc)))
                continue
            o = check(binary, tree, case.body, out_dir, label)
            o.pr, o.sha = case.pr, case.sha
            outcomes.append(o)
            print(f"[{i}/{len(cases)}] {label}: rc={o.rc}", file=sys.stderr)
    finally:
        git(root, "worktree", "remove", "--force", str(tree), check=False)
        git(root, "worktree", "prune", check=False)
    return outcomes


def self_test() -> int:
    failures: List[str] = []

    def check_eq(name, got, want):
        if got != want:
            failures.append(f"{name}: got {got!r}, want {want!r}")

    check_eq("squash subject", pr_number("ci(discipline): run the gates (#1073)"), 1073)
    check_eq("subject with trailing space", pr_number("fix(x): y (#7) "), 7)
    check_eq("issue ref mid-subject is not the PR", pr_number("fix(x): refs (#12) in the middle"), None)
    check_eq("no PR", pr_number("chore: direct push"), None)

    def report(*violations):
        by_gate: Dict[str, list] = defaultdict(list)
        for gate, sev in violations:
            by_gate[gate].append({"severity": sev, "message": "m"})
        return {"outcomes": [{"gate": g, "violations": v} for g, v in by_gate.items()]}

    outcomes = [
        Outcome(pr=1, sha="a" * 40, rc=0, report=report(("pr-checklist", "warning"))),
        Outcome(pr=2, sha="b" * 40, rc=1, report=report(("instruction-smuggling", "error"),
                                                         ("instruction-smuggling", "error"))),
        Outcome(pr=3, sha="c" * 40, rc=1, report=report(("assertion-reduction", "error"))),
        Outcome(pr=4, sha="d" * 40, rc=2, report=None, error="could not check: gate x"),
        # Exit 2 with a report still written: the exit code decides, not the report.
        Outcome(pr=5, sha="f" * 40, rc=2, report=report(("pii", "error")), error="could not check: gate y"),
        Outcome(pr=None, sha="e" * 40, rc=None, error="diff does not re-apply"),
        # A crash is not a verdict, even with a clean-looking report on disk.
        Outcome(pr=6, sha="0" * 40, rc=101, report=report()),
    ]
    s = summarise(outcomes)
    check_eq("case count", s["cases"], 7)
    check_eq("passed", s["passed"], 1)
    check_eq("blocked", s["blocked"], ["#2", "#3"])
    # A case attributed twice to one gate is counted once for that gate.
    check_eq("per-gate attribution", s["errors_by_gate"],
             {"instruction-smuggling": ["#2"], "assertion-reduction": ["#3"]})
    check_eq("warnings", s["warnings_by_gate"], {"pr-checklist": 1})
    # Exit 2 is never folded into passed or blocked (AGENTS.md §8.1).
    check_eq("could not check", [c["case"] for c in s["could_not_check"]], ["#4", "#5", "#6"])
    check_eq("not reconstructed labelled by sha", [c["case"] for c in s["not_reconstructed"]], ["eeeeeeee"])
    text = render(s)
    for needle in ("blocked: 2", "#2", "#3", "COULD NOT CHECK #4", "NOT RECONSTRUCTED eeeeeeee"):
        if needle not in text:
            failures.append(f"render lacks {needle!r}")

    for f in failures:
        print(f"FAIL: {f}", file=sys.stderr)
    if failures:
        return 1
    print("discipline_replay --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="Replay merged pull requests through discipline")
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--last", type=int, help="the last N merged pull requests")
    mode.add_argument("--since", help="every first-parent commit after this ref (forward test)")
    mode.add_argument("--self-test", action="store_true")
    ap.add_argument("--until", default="origin/main", help="end of the --since range (default origin/main)")
    ap.add_argument("--binary", help="discipline binary (default: the one on PATH)")
    ap.add_argument("--config", default=CONFIG_NAME, help="configuration under test (default discipline.toml)")
    ap.add_argument("--out", help="directory for bodies, reports and summary.json (default: a temp dir)")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    root = Path(run(["git", "rev-parse", "--show-toplevel"]).stdout.strip())
    binary = args.binary or shutil.which("discipline")
    if not binary or not Path(binary).exists():
        print("discipline binary not found: pass --binary or put it on PATH", file=sys.stderr)
        return 2
    if not shutil.which("gh"):
        print("gh not found: pull request bodies are read through it", file=sys.stderr)
        return 2
    config_path = Path(args.config)
    if not config_path.is_absolute():
        config_path = root / config_path
    config_text = config_path.read_text()
    out_dir = Path(args.out) if args.out else Path(tempfile.mkdtemp(prefix="discipline-replay-"))
    out_dir.mkdir(parents=True, exist_ok=True)

    version = run([binary, "--version"]).stdout.strip()
    cases = merged_prs(args.last) if args.last else commits_since(root, args.since, args.until)
    if not cases:
        print("no cases selected", file=sys.stderr)
        return 2
    print(f"{version}; config {config_path}; {len(cases)} case(s); reports in {out_dir}", file=sys.stderr)

    outcomes = replay(cases, binary, config_text, root, out_dir)
    s = summarise(outcomes)
    s["binary"] = version
    s["config"] = str(config_path)
    (out_dir / "summary.json").write_text(json.dumps(s, indent=2) + "\n")
    print(render(s))
    return 2 if s["could_not_check"] or s["not_reconstructed"] else 0


if __name__ == "__main__":
    sys.exit(main())
