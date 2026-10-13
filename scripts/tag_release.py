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
  7. `git tag -s`, with the release notes as the message: `--notes-file`, or
     the section of the release pull request's body between the
     `<!-- release-notes:begin -->` and `<!-- release-notes:end -->` markers
     with `--notes-from-pr`. A stable release needs notes; `release.yml` puts
     them at the top of the GitHub Release, so notes that carry release
     mechanics are refused here, before the tag exists (a pushed tag cannot be
     moved);
  8. read the tag back: it must be annotated, verify, and point at the commit;
  9. print `git push origin refs/tags/vX.Y.Z`. One ref, never `--tags`, which
     pushes every local tag, stale ones included.

Run:  tag_release.py X.Y.Z [--commit SHA] [--notes-file FILE | --notes-from-pr N]
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

# The part of a release pull request's body that is published. The rest of the
# body (the release mechanics) is never read.
NOTES_BEGIN = "<!-- release-notes:begin -->"
NOTES_END = "<!-- release-notes:end -->"

# Release-engineering text that must not reach a published release body. The
# tag message is published verbatim and a pushed tag cannot be moved, so the
# check runs when the tag is created. Matched case-insensitively, per line.
# Each entry is a tooling name or flag, never an ordinary word: a note about
# citation metadata or "in lockstep" with a dependency is legitimate and must
# pass (the self-test pins that).
MECHANICS_TOKENS = (
    "bump_version",  # the version-bump tool: how the release was cut, not what changed
    "tag_release",  # the tagging tool
    "release_ci_gate",  # the CI gate the tagging tool runs
    "release_notes.py",  # the script that reads the tag message back
    "--notes-from-pr",  # the flag naming where the notes come from
    "--notes-file",  # its sibling
)

COMMENT_RE = re.compile(r"<!--.*?-->", re.DOTALL)


class Refused(Exception):
    """A check failed; nothing was tagged."""


def extract_notes(body: str, source: str = "the pull request body") -> str:
    """The text between the notes markers, which must each occur once, in order.

    Line endings are normalised first (`gh pr view --json body` can return
    CRLF), and HTML comments inside the section are dropped: they render as
    nothing on the release page, so they are drafting notes, not notes.
    """
    body = body.replace("\r\n", "\n").replace("\r", "\n")
    begins, ends = body.count(NOTES_BEGIN), body.count(NOTES_END)
    hint = (f"wrap the user-facing notes in {NOTES_BEGIN} and {NOTES_END}, and keep the "
            f"release mechanics outside them")
    if begins == 0:
        raise Refused(f"{source} has no {NOTES_BEGIN} marker: {hint}")
    if ends == 0:
        raise Refused(f"{source} has no {NOTES_END} marker: {hint}")
    if begins > 1:
        raise Refused(f"{source} has {NOTES_BEGIN} {begins} times; it must appear exactly once: {hint}")
    if ends > 1:
        raise Refused(f"{source} has {NOTES_END} {ends} times; it must appear exactly once: {hint}")
    start = body.index(NOTES_BEGIN)
    stop = body.index(NOTES_END)
    if stop < start:
        raise Refused(f"{source} has {NOTES_END} before {NOTES_BEGIN}: {hint}")
    notes = COMMENT_RE.sub("", body[start + len(NOTES_BEGIN):stop]).strip()
    if "<!--" in notes:
        raise Refused(f"{source} has an unterminated HTML comment between the notes markers")
    if not notes:
        raise Refused(f"{source} has nothing between the notes markers: {hint}")
    return notes


def require_no_mechanics(text: str, source: str) -> None:
    """Refuses notes that carry release-engineering text."""
    for number, line in enumerate(text.splitlines(), 1):
        lowered = line.lower()
        for token in MECHANICS_TOKENS:
            if token in lowered:
                raise Refused(
                    f"{source} carries release mechanics: {token!r} on line {number}: {line.strip()!r}. "
                    f"Published notes are user-facing only; move that text outside the notes markers "
                    f"(or out of the notes file)")


def notes_text_from_pr_body(version: str, body: str) -> str:
    """The tag message for `--notes-from-pr`: a subject line, then the marked section."""
    return f"Release v{version}\n\n{extract_notes(body)}\n"


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
    # `--cleanup=whitespace`: git's default cleanup for a tag message drops
    # every line starting with `#`, which is every Markdown heading of the notes.
    message = ["--cleanup=whitespace", "-F", str(notes)] if notes is not None else ["-m", f"Release {tag}"]
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
    if notes is None and "-" not in version:
        raise Refused(f"{version} is a stable release and needs its notes: pass --notes-file or "
                      f"--notes-from-pr (release.yml refuses a stable tag whose message has none)")
    if notes is not None:
        lines = notes.read_text().splitlines()
        if len([ln for ln in lines[1:] if ln.strip()]) == 0:
            raise Refused(f"--notes-file {notes} has a subject line and nothing after it")
        require_no_mechanics(notes.read_text(), f"the notes ({notes.name})")
    tag = f"v{version}"
    if fetch:
        git(["fetch", "--quiet", REMOTE, BRANCH], repo)
    sha = resolve_commit(repo, commit)
    require_on_main(repo, sha)
    require_tag_absent(repo, tag)
    # A pre-release (X.Y.Z-rc.N) rehearses X.Y.Z from the commit that carries
    # X.Y.Z, so the manifests are checked against the base version.
    version_check(repo, sha, version.split("-", 1)[0])
    ci_check(repo, sha, repo_slug, deadline_minutes)
    create_and_read_back(repo, tag, sha, notes, sign=sign)
    return sha


def self_test() -> int:
    failures: list[str] = []
    # The fixture clone whose stray tags `refused` removes; bound as soon as the
    # clone exists. `None` before then, when no case can have created a tag.
    fixture: dict[str, Path | None] = {"work": None}

    def refused(label: str, fn, needle: str) -> None:
        try:
            fn()
        except Refused as exc:
            if needle not in str(exc):
                failures.append(f"{label}: refused for the wrong reason: {exc}")
            return
        except Exception as exc:  # a crash is not a refusal
            failures.append(f"{label}: raised {type(exc).__name__} instead of refusing: {exc}")
            return
        failures.append(f"{label}: was not refused")
        # A check that wrongly let the run through created the tag; remove it so
        # the cases after this one still run and report.
        work_repo = fixture["work"]
        if work_repo is not None:
            for stray in git(["tag", "--list"], work_repo, check=False).stdout.split():
                git(["tag", "-d", stray], work_repo, check=False)

    def accepted(label: str, fn):
        """Runs a case that must succeed; a refusal is a recorded failure, not a crash."""
        try:
            return fn()
        except Exception as exc:
            failures.append(f"{label}: was refused or crashed: {exc}")
            return None

    ok = lambda *_a, **_k: None  # noqa: E731

    def notes_with(directory: Path, line: str) -> Path:
        path = directory / "mechanics-notes.md"
        path.write_text(f"Release v1.2.3\n\n## Fixed\n\n{line}\n")
        return path

    def bad_version(_repo, sha, version):
        raise Refused(f"{sha} does not carry version {version} in every manifest")

    def bad_ci(_repo, sha, *_a):
        raise Refused(f"{sha} has no successful full CI run")

    with tempfile.TemporaryDirectory(prefix="tag-release-test-") as tmp:
        origin, work = Path(tmp) / "origin.git", Path(tmp) / "work"
        git(["init", "--quiet", "--bare", "-b", BRANCH, str(origin)], Path(tmp))
        git(["clone", "--quiet", str(origin), str(work)], Path(tmp))
        fixture["work"] = work
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

        default_notes = Path(tmp) / "default-notes.md"
        default_notes.write_text("Release\n\n## Fixed\n\n- a thing\n")
        unset = object()

        def run(version="1.2.3", commit_=None, notes=unset, version_check=ok, ci_check=ok):
            return tag_release(work, version, commit_, default_notes if notes is unset else notes,
                               "o/r", 0, sign=False, version_check=version_check, ci_check=ci_check)

        refused("not a version", lambda: run(version="1.2"), "is not a version")
        refused("empty --commit", lambda: run(commit_=""), "refusing to fall back to HEAD")
        refused("unknown --commit", lambda: run(commit_="nope"), "does not name a commit")
        refused("commit off main", lambda: run(commit_=side), "is not an ancestor")
        refused("manifests at another version", lambda: run(version_check=bad_version), "does not carry version")
        refused("no full CI run", lambda: run(ci_check=bad_ci), "no successful full CI run")
        empty = Path(tmp) / "empty.md"
        empty.write_text("\n")
        refused("empty notes file", lambda: run(notes=empty), "missing or empty")
        refused("stable release without notes", lambda: run(notes=None), "needs its notes")
        subject_only = Path(tmp) / "subject.md"
        subject_only.write_text("Release v1.2.3\n")
        refused("notes with a subject and no body", lambda: run(notes=subject_only), "nothing after it")
        if git(["tag", "--list"], work).stdout.strip():
            failures.append("a refused run left a tag behind")

        # Release mechanics in published notes. `motivating` is the paragraph
        # that opened a published release body.
        motivating = (
            "Release 0.12.0: `scripts/bump_version.py 0.12.0` moves 26 manifests in lockstep, "
            "regenerates `Cargo.lock` and sets the `CITATION.cff` date; the file set equals the "
            "0.11.2 bump (#1430). This body is the release-notes source for "
            "`scripts/tag_release.py 0.12.0 --notes-from-pr <N>`. The range is `v0.11.2..main` "
            "at `a81046f25` (31 commits)."
        )
        clean = "## Fixed\n\n- Ordered reads no longer return a key that was never present."

        def marked(inner: str) -> str:
            return f"{NOTES_BEGIN}\n{inner}\n{NOTES_END}"

        def tag_message_for(body: str) -> Path:
            path = Path(tmp) / "from-pr.md"
            path.write_text(notes_text_from_pr_body("1.2.3", body))
            return path

        def from_pr(body: str):
            return notes_text_from_pr_body("1.2.3", body)

        refused("motivating paragraph inside the markers",
                lambda: run(notes=tag_message_for(marked(motivating + "\n\n" + clean))),
                "'bump_version'")
        refused("no begin marker", lambda: from_pr(f"{clean}\n{NOTES_END}"), f"no {NOTES_BEGIN}")
        refused("no end marker", lambda: from_pr(f"{NOTES_BEGIN}\n{clean}"), f"no {NOTES_END}")
        refused("no markers at all", lambda: from_pr(clean), f"no {NOTES_BEGIN}")
        refused("markers out of order",
                lambda: from_pr(f"{NOTES_END}\n{clean}\n{NOTES_BEGIN}"), "before")
        refused("begin marker twice",
                lambda: from_pr(f"{NOTES_BEGIN}\n{marked(clean)}"), f"{NOTES_BEGIN} 2 times")
        refused("end marker twice",
                lambda: from_pr(f"{marked(clean)}\n{NOTES_END}"), f"{NOTES_END} 2 times")
        refused("empty section", lambda: from_pr(marked("  ")), "nothing between")
        refused("a section of only a drafting comment",
                lambda: from_pr(marked("<!-- write the notes here -->")), "nothing between")
        refused("an unterminated comment in the section",
                lambda: from_pr(marked(f"{clean}\n<!-- never closed")), "unterminated")
        # Pinned literally, so that dropping a token from MECHANICS_TOKENS fails
        # here instead of silently shrinking the loop that tests it.
        pinned = ("bump_version", "tag_release", "release_ci_gate", "release_notes.py",
                  "--notes-from-pr", "--notes-file")
        if MECHANICS_TOKENS != pinned:
            failures.append(f"MECHANICS_TOKENS is {MECHANICS_TOKENS}, the self-test pins {pinned}")
        for token in pinned:
            refused(f"--notes-file with {token!r}",
                    lambda token=token: run(notes=notes_with(Path(tmp), f"about {token.upper()}")),
                    repr(token))
            refused(f"marked section with {token!r}",
                    lambda token=token: run(notes=tag_message_for(marked(f"{clean}\nsee `{token}`"))),
                    repr(token))
        if git(["tag", "--list"], work).stdout.strip():
            failures.append("a refused run left a tag behind")

        def tag_and_read(version: str, body: str) -> str:
            """Tags `version` from a PR body as `--notes-from-pr` does; returns the tag's text."""
            accepted(f"v{version} from a PR body",
                     lambda: run(version=version, commit_=first, notes=tag_message_for(body)))
            text = git(["tag", "-l", "--format=%(contents:body)", f"v{version}"], work).stdout.strip()
            git(["tag", "-d", f"v{version}"], work, check=False)
            return text

        # Ordinary words that a looser token set would have refused are accepted.
        prose = ("## Changed\n\n- Dependencies are released in lockstep with the core crate.\n"
                 "- `CITATION.cff` now lists the archive DOI.\n"
                 "- The release notes source is the tag message.")
        if tag_and_read("1.2.5", marked(prose)) != prose:
            failures.append("ordinary prose (lockstep, CITATION.cff, notes source) was changed or refused")
        # CRLF bodies, as `gh pr view --json body` can return them, and HTML
        # comments inside the section: the tag message has neither. The function
        # is checked directly too, because the notes file is read back in text
        # mode, which would hide a leftover carriage return.
        crlf = (f"preamble\r\n{NOTES_BEGIN}\r\n<!-- drafting note\r\nover two lines -->\r\n"
                f"## Fixed\r\n\r\n- a thing\r\n{NOTES_END}\r\npostamble\r\n")
        want_crlf = "Release v1.2.3\n\n## Fixed\n\n- a thing\n"
        if accepted("a CRLF body", lambda: from_pr(crlf)) != want_crlf:
            failures.append(f"a CRLF body with a comment did not give {want_crlf!r}")
        if tag_and_read("1.2.6", crlf) != "## Fixed\n\n- a thing":
            failures.append("a CRLF body with a comment did not give the marked section as the tag message")

        # The default is origin/main's head, not HEAD, which is on the side branch.
        notes = Path(tmp) / "notes.md"
        notes.write_text("Release v1.2.3\n\n## Fixed\n\nfixed: a thing\n")
        tagged = run(notes=notes)
        if tagged != second:
            failures.append(f"default commit: tagged {tagged}, want origin/main's head {second}")
        if git(["rev-parse", "v1.2.3^{commit}"], work).stdout.strip() != second:
            failures.append("the tag does not point at origin/main's head")
        message = git(["tag", "-l", "--format=%(contents)", "v1.2.3"], work).stdout
        if "fixed: a thing" not in message:
            failures.append("the notes file is not the tag message")
        if "## Fixed" not in message:
            failures.append("a Markdown heading of the notes was dropped from the tag message")
        # (b)/(d): only the marked section is the tag message; a mechanics token
        # outside the markers is accepted because it is never published.
        section = "## Fixed\n\n- Ordered reads no longer return a key that was never present."
        pr_body = (f"{motivating}\n\n{marked(section)}\n\nThe `tag_release` mechanics stay here.\n")
        message_pr = tag_and_read("1.2.4", pr_body)
        if message_pr != section:
            failures.append(f"the tag message is {message_pr!r}, want exactly the marked section "
                            f"(mechanics outside the markers are never published)")
        # A pre-release may be tagged without notes, and its manifests are
        # checked against the base version.
        checked: list[str] = []
        if run(version="1.3.0-rc.1", notes=None,
               version_check=lambda _r, _s, v: checked.append(v)) != second:
            failures.append("a pre-release could not be tagged without notes")
        if checked != ["1.3.0"]:
            failures.append(f"a pre-release checked the manifests against {checked}, want ['1.3.0']")
        refused("tag exists locally", lambda: run(), "already exists locally")

        # An ancestor of main may be tagged explicitly; a tag on the remote blocks it.
        if run(version="1.2.2", commit_=first) != first:
            failures.append("an ancestor of main could not be tagged with --commit")
        git(["push", "--quiet", REMOTE, "refs/tags/v1.2.2"], work)
        git(["tag", "-d", "v1.2.2"], work)
        refused("tag exists on the remote", lambda: run(version="1.2.2", commit_=first), "already exists on origin")

    # CI check must be called with the tagged commit, not HEAD (which may sit
    # on a side branch).  The fixture above leaves HEAD on `side`; when we tag
    # without --commit, resolve_commit returns origin/main's head and that is
    # what ci_check receives.
    with tempfile.TemporaryDirectory(prefix="tag-release-ci-sha-") as tmp:
        origin2, work2 = Path(tmp) / "origin2.git", Path(tmp) / "work2"
        git(["init", "--quiet", "--bare", "-b", BRANCH, str(origin2)], Path(tmp))
        git(["clone", "--quiet", str(origin2), str(work2)], Path(tmp))
        for k, v in (("user.name", "t"), ("user.email", "t@example.invalid"),
                      ("commit.gpgsign", "false"), ("tag.gpgsign", "false")):
            git(["config", k, v], work2)

        def commit2(name: str) -> str:
            (work2 / name).write_text(name)
            git(["add", name], work2)
            git(["commit", "--quiet", "-m", name], work2)
            return git(["rev-parse", "HEAD"], work2).stdout.strip()

        git(["checkout", "--quiet", "-b", BRANCH], work2, check=False)
        main_head = commit2("main")
        git(["push", "--quiet", REMOTE, BRANCH], work2)
        git(["checkout", "--quiet", "-b", "side"], work2)
        _side_head = commit2("side")  # HEAD now sits here

        ci_shas: list[str] = []
        def capture_ci_sha(_repo, sha, *_a):
            ci_shas.append(sha)
        default_notes2 = Path(tmp) / "notes2.md"
        default_notes2.write_text("Release\n\n## Fixed\n\n- a thing\n")

        tag_release(work2, "1.0.0", None, default_notes2, "o/r", 0, sign=False,
                     version_check=ok, ci_check=capture_ci_sha)
        if ci_shas != [main_head]:
            failures.append(f"ci_check called with {ci_shas}, want [{main_head}] (must be tagged commit, not HEAD)")

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
    ap.add_argument("--notes-from-pr", type=int, metavar="N",
                    help="use the text of pull request N (the release PR) between the "
                         "release-notes:begin and release-notes:end markers as the notes")
    ap.add_argument("--repo", default="orieg/expanse", help="OWNER/NAME for the CI gate")
    ap.add_argument("--ci-deadline-minutes", type=float, default=220)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.version:
        ap.error("a version is required")
    repo = Path(__file__).resolve().parent.parent
    if args.notes_file and args.notes_from_pr:
        ap.error("--notes-file and --notes-from-pr are alternatives")
    with tempfile.TemporaryDirectory(prefix="tag-release-notes-") as tmp:
        notes_file = args.notes_file
        try:
            if args.notes_from_pr:
                proc = subprocess.run(
                    ["gh", "pr", "view", str(args.notes_from_pr), "--repo", args.repo, "--json", "body", "--jq", ".body"],
                    capture_output=True, text=True,
                )
                if proc.returncode != 0 or not proc.stdout.strip():
                    raise Refused(f"cannot read the body of pull request {args.notes_from_pr}: {proc.stderr.strip()}")
                notes_file = Path(tmp) / "notes.md"
                notes_file.write_text(notes_text_from_pr_body(args.version, proc.stdout.strip()))
            sha = tag_release(repo, args.version, args.commit, notes_file, args.repo,
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
