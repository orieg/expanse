#!/usr/bin/env python3
"""Curated release notes: read them from the signed release tag's message.

The notes a maintainer writes for a release (what was added, changed, fixed,
what breaks) used to stay in the release pull request. The GitHub Release body
was generated from merged pull-request titles alone, so the published release
never carried them.

The notes now travel in the tag. `scripts/tag_release.py --notes-file` (or
`--notes-from-pr`) makes them the message of the signed tag, and this script
reads them back in `release.yml`, where they are placed above the generated
list. They are covered by the tag's signature, and no per-release file is
added to the tree.

  - The notes are the tag message after its first line (the subject).
  - `@name` is wrapped in backticks, so a note that names a handle does not
    notify that account from a release body. An address (`a@b.c`) and text
    already inside backticks are left alone.
  - A stable tag (`vX.Y.Z`) with no notes is an error: the release stops
    before it is published. A pre-release tag (`vX.Y.Z-rc.1`) may have none.

Run:  release_notes.py --tag vX.Y.Z --out FILE [--repo DIR]
      release_notes.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tempfile
from pathlib import Path

STABLE_RE = re.compile(r"^v\d+\.\d+\.\d+$")
PRERELEASE_RE = re.compile(r"^v\d+\.\d+\.\d+-[0-9A-Za-z.]+$")
# `@name` not preceded by a word character (an address) or a backtick.
MENTION_RE = re.compile(r"(?<![\w`@])@([A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?)(?![\w`])")


class NotesError(Exception):
    """The tag cannot supply release notes."""


def sanitise(text: str) -> str:
    """Wraps bare `@name` mentions in backticks, outside code spans and fences."""
    out, in_fence = [], False
    for line in text.split("\n"):
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            out.append(line)
            continue
        if in_fence:
            out.append(line)
            continue
        # Odd-numbered pieces of a split on backticks are inside a code span.
        pieces = line.split("`")
        for i in range(0, len(pieces), 2):
            pieces[i] = MENTION_RE.sub(r"`@\1`", pieces[i])
        out.append("`".join(pieces))
    return "\n".join(out)


def tag_body(repo: Path, tag: str) -> str:
    """The annotated tag's message after its subject line, without the signature."""
    kind = subprocess.run(["git", "cat-file", "-t", f"refs/tags/{tag}"], cwd=repo, capture_output=True, text=True)
    if kind.returncode != 0:
        raise NotesError(f"tag {tag} does not exist in {repo}")
    if kind.stdout.strip() != "tag":
        raise NotesError(f"{tag} is a lightweight tag and has no message")
    proc = subprocess.run(
        ["git", "for-each-ref", "--format=%(contents:body)", f"refs/tags/{tag}"],
        cwd=repo, capture_output=True, text=True,
    )
    if proc.returncode != 0:
        raise NotesError(f"cannot read the message of {tag}: {proc.stderr.strip()}")
    return proc.stdout.strip()


def notes_for(repo: Path, tag: str) -> str:
    """The sanitised notes of `tag`; empty only for a pre-release tag."""
    if not (STABLE_RE.match(tag) or PRERELEASE_RE.match(tag)):
        raise NotesError(f"{tag!r} is not a release tag (vX.Y.Z or vX.Y.Z-pre)")
    body = tag_body(repo, tag)
    if not body and STABLE_RE.match(tag):
        raise NotesError(
            f"{tag} carries no release notes: its message is a subject line only. Create the tag "
            f"with `scripts/tag_release.py --notes-file` or `--notes-from-pr`"
        )
    return sanitise(body)


def self_test() -> int:
    failures: list[str] = []

    def check(label, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    check("bare mention", sanitise("thanks @octo-cat for this"), "thanks `@octo-cat` for this")
    check("mention at line start", sanitise("@a and (@b)"), "`@a` and (`@b`)")
    check("address untouched", sanitise("mail a@b.example"), "mail a@b.example")
    check("code span untouched", sanitise("call `@decorator` here"), "call `@decorator` here")
    check("fence untouched", sanitise("```\n@x\n```\n@y"), "```\n@x\n```\n`@y`")
    check("double at untouched", sanitise("scope @@x"), "scope @@x")
    check("idempotent", sanitise(sanitise("ping @a")), "`@a`".join(["ping ", ""]))

    with tempfile.TemporaryDirectory(prefix="release-notes-test-") as tmp:
        repo = Path(tmp)

        def git(*args: str) -> None:
            proc = subprocess.run(["git", *args], cwd=repo, capture_output=True, text=True)
            if proc.returncode != 0:
                raise RuntimeError(f"git {' '.join(args)}: {proc.stderr.strip()}")

        git("init", "--quiet")
        for k, v in (("user.name", "t"), ("user.email", "t@example.invalid"),
                     ("commit.gpgsign", "false"), ("tag.gpgsign", "false")):
            git("config", k, v)
        (repo / "f").write_text("x")
        git("add", "f")
        git("commit", "--quiet", "-m", "c")
        notes = repo / "notes.md"
        notes.write_text("Release v1.2.3\n\n## Fixed\n\n- a thing, reported by @someone\n")
        # `--cleanup=whitespace`, as tag_release.py passes: git's default
        # cleanup drops every line starting with `#`, Markdown headings included.
        git("tag", "-a", "--cleanup=whitespace", "-F", str(notes), "v1.2.3")
        git("tag", "-a", "-m", "Release v1.2.4", "v1.2.4")
        git("tag", "-a", "-m", "Release v1.3.0-rc.1", "v1.3.0-rc.1")
        git("tag", "v1.2.5")

        check("notes read from the tag", notes_for(repo, "v1.2.3"),
              "## Fixed\n\n- a thing, reported by `@someone`")
        check("a pre-release may have none", notes_for(repo, "v1.3.0-rc.1"), "")
        for label, tag, needle in (
            ("stable tag without notes", "v1.2.4", "carries no release notes"),
            ("lightweight tag", "v1.2.5", "lightweight tag"),
            ("missing tag", "v9.9.9", "does not exist"),
            ("not a release tag", "bindings/go/v1.2.3", "is not a release tag"),
        ):
            try:
                got = notes_for(repo, tag)
                failures.append(f"{label}: accepted with {got!r}")
            except NotesError as exc:
                if needle not in str(exc):
                    failures.append(f"{label}: refused for the wrong reason: {exc}")

    if failures:
        for f in failures:
            print(f"::error::release_notes self-test: {f}")
        return 1
    print("release_notes --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tag")
    ap.add_argument("--out", type=Path)
    ap.add_argument("--repo", type=Path, default=Path(__file__).resolve().parent.parent)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.tag or not args.out:
        print("::error::--tag and --out are required", file=sys.stderr)
        return 1
    try:
        notes = notes_for(args.repo, args.tag)
    except NotesError as exc:
        print(f"::error::{exc}")
        return 1
    args.out.write_text(notes + ("\n" if notes else ""))
    print(f"{args.tag}: {len(notes.splitlines())} line(s) of curated notes written to {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
