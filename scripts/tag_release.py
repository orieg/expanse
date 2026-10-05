#!/usr/bin/env python3
"""Create the signed release tag `vX.Y.Z`, after checking it is safe to create.

The release tag is the maintainer's: it is signed with the maintainer's key and
pushed by hand, and `release.yml` runs on that push. A registry publish cannot
be taken back, so a tag on the wrong commit spends the version. This script
does the checks a hand-typed `git tag` skips, then creates the tag and prints
the push command. It never pushes.

In order:

  1. fetch `origin/main`;
  2. resolve the commit to tag: `--commit`, or `origin/main`'s head. An empty
     `--commit` is an error, never a fallback to `HEAD`;
  3. refuse a commit that is not an ancestor of `origin/main`;
  4. refuse a tag that already exists, locally or on `origin`;
  5. run `bump_version.py --check X.Y.Z` on a checkout of that commit, not on
     the working tree;
  6. run `release_ci_gate.py` for the commit. When the commit is `origin/main`'s
     head and no full CI run exists for it, the gate dispatches one on `main`
     and waits; when `main` has moved on, nothing is dispatched (a run on `main`
     would test another commit) and a missing full run is a refusal;
  7. `git tag -s`, with `--notes-file` as the message when given;
  8. read the tag back: it must be annotated, verify, and point at the commit;
  9. print `git push origin refs/tags/vX.Y.Z`. One ref, never `--tags`, which
     pushes every local tag, stale ones included.

Run:  tag_release.py X.Y.Z [--commit SHA] [--notes-file FILE]
                           [--repo OWNER/NAME] [--ci-deadline-minutes N]
      tag_release.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tempfile
from pathlib import Path

VERSION_RE = re.compile(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$")
REMOTE = "origin"
BRANCH = "main"


class Refused(Exception):
    """A check failed; nothing was tagged."""


def git(args: list[str], cwd: Path, check: bool = True) -> subprocess.CompletedProcess:
    proc = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if check and proc.returncode != 0:
        raise Refused(f"git {' '.join(args)} failed ({proc.returncode}): {proc.stderr.strip()}")
    return proc


def resolve_commit(repo: Path, commit: str | None) -> str:
    """The commit to tag: `commit` if given, else the head of `origin/main`."""
    if commit is not None and not commit.strip():
        raise Refused("--commit is empty; refusing to fall back to HEAD")
    ref = commit if commit is not None else f"{REMOTE}/{BRANCH}"
    proc = git(["rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}"], repo, check=False)
    sha = proc.stdout.strip()
    if proc.returncode != 0 or not sha:
        raise Refused(f"{ref!r} does not name a commit")
    return sha


def require_on_main(repo: Path, sha: str) -> None:
    head = f"{REMOTE}/{BRANCH}"
    if git(["merge-base", "--is-ancestor", sha, head], repo, check=False).returncode != 0:
        raise Refused(f"{sha} is not an ancestor of {head}: a release is tagged on a commit of {BRANCH}")


def require_tag_absent(repo: Path, tag: str) -> None:
    if git(["rev-parse", "--verify", "--quiet", f"refs/tags/{tag}"], repo, check=False).returncode == 0:
        raise Refused(f"tag {tag} already exists locally")
    proc = git(["ls-remote", "--tags", REMOTE, f"refs/tags/{tag}"], repo, check=False)
    if proc.returncode != 0:
        raise Refused(f"cannot list {REMOTE}'s tags ({proc.stderr.strip()}); not tagging blind")
    if proc.stdout.strip():
        raise Refused(f"tag {tag} already exists on {REMOTE}")


def check_version_at(repo: Path, sha: str, version: str) -> None:
    """Runs that commit's own `bump_version.py --check` on a checkout of that commit."""
    with tempfile.TemporaryDirectory(prefix="tag-release-") as tmp:
        tree = Path(tmp) / "tree"
        git(["worktree", "add", "--detach", str(tree), sha], repo)
        try:
            proc = subprocess.run(
                [sys.executable, "scripts/bump_version.py", "--check", version],
                cwd=tree, capture_output=True, text=True,
            )
        finally:
            git(["worktree", "remove", "--force", str(tree)], repo, check=False)
    if proc.returncode != 0:
        tail = (proc.stdout + proc.stderr).strip().splitlines()[-6:]
        raise Refused(f"{sha} does not carry version {version} in every manifest:\n  " + "\n  ".join(tail))


def check_ci(repo: Path, sha: str, repo_slug: str, deadline_minutes: float) -> None:
    head = git(["rev-parse", f"{REMOTE}/{BRANCH}"], repo).stdout.strip()
    cmd = [sys.executable, "scripts/release_ci_gate.py", "--repo", repo_slug, "--sha", sha,
           "--require-ancestor-of", BRANCH, "--deadline-minutes", str(deadline_minutes)]
    if sha == head:
        cmd += ["--dispatch-ref", BRANCH]
    if subprocess.run(cmd, cwd=repo).returncode != 0:
        raise Refused(f"{sha} has no successful full CI run (see the gate's output above)")


def create_and_read_back(repo: Path, tag: str, sha: str, notes: Path | None, sign: bool = True) -> None:
    message = ["-F", str(notes)] if notes is not None else ["-m", f"Release {tag}"]
    git(["tag", "-s" if sign else "-a", tag, *message, sha], repo)
    try:
        if git(["cat-file", "-t", f"refs/tags/{tag}"], repo).stdout.strip() != "tag":
            raise Refused(f"{tag} is not an annotated tag")
        pointed = git(["rev-parse", f"refs/tags/{tag}^{{commit}}"], repo).stdout.strip()
        if pointed != sha:
            raise Refused(f"{tag} points at {pointed}, not {sha}")
        if sign and git(["verify-tag", tag], repo, check=False).returncode != 0:
            raise Refused(f"the signature on {tag} does not verify")
    except Refused:
        git(["tag", "-d", tag], repo, check=False)
        raise


def tag_release(repo: Path, version: str, commit: str | None, notes: Path | None,
                repo_slug: str, deadline_minutes: float, *, sign: bool = True,
                version_check=check_version_at, ci_check=check_ci, fetch: bool = True) -> str:
    if not VERSION_RE.match(version):
        raise Refused(f"{version!r} is not a version (X.Y.Z or X.Y.Z-pre)")
    if notes is not None and not (notes.is_file() and notes.read_text().strip()):
        raise Refused(f"--notes-file {notes} is missing or empty")
    tag = f"v{version}"
    if fetch:
        git(["fetch", "--quiet", REMOTE, BRANCH], repo)
    sha = resolve_commit(repo, commit)
    require_on_main(repo, sha)
    require_tag_absent(repo, tag)
    version_check(repo, sha, version)
    ci_check(repo, sha, repo_slug, deadline_minutes)
    create_and_read_back(repo, tag, sha, notes, sign=sign)
    return sha


def self_test() -> int:
    failures: list[str] = []

    def refused(label: str, fn, needle: str) -> None:
        try:
            fn()
        except Refused as exc:
            if needle not in str(exc):
                failures.append(f"{label}: refused for the wrong reason: {exc}")
            return
        failures.append(f"{label}: was not refused")

    ok = lambda *_a, **_k: None  # noqa: E731

    def bad_version(_repo, sha, version):
        raise Refused(f"{sha} does not carry version {version} in every manifest")

    def bad_ci(_repo, sha, *_a):
        raise Refused(f"{sha} has no successful full CI run")

    with tempfile.TemporaryDirectory(prefix="tag-release-test-") as tmp:
        origin, work = Path(tmp) / "origin.git", Path(tmp) / "work"
        git(["init", "--quiet", "--bare", "-b", BRANCH, str(origin)], Path(tmp))
        git(["clone", "--quiet", str(origin), str(work)], Path(tmp))
        for k, v in (("user.name", "t"), ("user.email", "t@example.invalid"),
                     ("commit.gpgsign", "false"), ("tag.gpgsign", "false")):
            git(["config", k, v], work)

        def commit(name: str) -> str:
            (work / name).write_text(name)
            git(["add", name], work)
            git(["commit", "--quiet", "-m", name], work)
            return git(["rev-parse", "HEAD"], work).stdout.strip()

        git(["checkout", "--quiet", "-b", BRANCH], work, check=False)
        first = commit("a")
        second = commit("b")
        git(["push", "--quiet", REMOTE, BRANCH], work)
        git(["checkout", "--quiet", "-b", "side"], work)
        side = commit("c")  # never pushed to main; HEAD now sits here

        def run(version="1.2.3", commit_=None, notes=None, version_check=ok, ci_check=ok):
            return tag_release(work, version, commit_, notes, "o/r", 0, sign=False,
                               version_check=version_check, ci_check=ci_check)

        refused("not a version", lambda: run(version="1.2"), "is not a version")
        refused("empty --commit", lambda: run(commit_=""), "refusing to fall back to HEAD")
        refused("unknown --commit", lambda: run(commit_="nope"), "does not name a commit")
        refused("commit off main", lambda: run(commit_=side), "is not an ancestor")
        refused("manifests at another version", lambda: run(version_check=bad_version), "does not carry version")
        refused("no full CI run", lambda: run(ci_check=bad_ci), "no successful full CI run")
        empty = Path(tmp) / "empty.md"
        empty.write_text("\n")
        refused("empty notes file", lambda: run(notes=empty), "missing or empty")
        if git(["tag", "--list"], work).stdout.strip():
            failures.append("a refused run left a tag behind")

        # The default is origin/main's head, not HEAD, which is on the side branch.
        notes = Path(tmp) / "notes.md"
        notes.write_text("Release notes\n\nfixed: a thing\n")
        tagged = run(notes=notes)
        if tagged != second:
            failures.append(f"default commit: tagged {tagged}, want origin/main's head {second}")
        if git(["rev-parse", "v1.2.3^{commit}"], work).stdout.strip() != second:
            failures.append("the tag does not point at origin/main's head")
        if "fixed: a thing" not in git(["tag", "-l", "--format=%(contents)", "v1.2.3"], work).stdout:
            failures.append("the notes file is not the tag message")
        refused("tag exists locally", lambda: run(), "already exists locally")

        # An ancestor of main may be tagged explicitly; a tag on the remote blocks it.
        if run(version="1.2.2", commit_=first) != first:
            failures.append("an ancestor of main could not be tagged with --commit")
        git(["push", "--quiet", REMOTE, "refs/tags/v1.2.2"], work)
        git(["tag", "-d", "v1.2.2"], work)
        refused("tag exists on the remote", lambda: run(version="1.2.2", commit_=first), "already exists on origin")

    if failures:
        for f in failures:
            print(f"::error::tag_release self-test: {f}")
        return 1
    print("tag_release --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("version", nargs="?")
    ap.add_argument("--commit", help="commit to tag (default: origin/main's head)")
    ap.add_argument("--notes-file", type=Path, help="file whose contents become the tag message")
    ap.add_argument("--repo", default="orieg/expanse", help="OWNER/NAME for the CI gate")
    ap.add_argument("--ci-deadline-minutes", type=float, default=220)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.version:
        ap.error("a version is required")
    repo = Path(__file__).resolve().parent.parent
    try:
        sha = tag_release(repo, args.version, args.commit, args.notes_file, args.repo,
                          args.ci_deadline_minutes)
    except Refused as exc:
        print(f"refused: {exc}", file=sys.stderr)
        return 1
    tag = f"v{args.version}"
    print(f"created signed tag {tag} on {sha}")
    print(f"push it with:\n  git push {REMOTE} refs/tags/{tag}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
