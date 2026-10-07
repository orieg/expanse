#!/usr/bin/env python3
"""Drift guard: every release tag has its Go module tag, on the same commit.

A Go module in a subdirectory is versioned by a tag prefixed with its path, so
`bindings/go` at release `vX.Y.Z` needs `bindings/go/vX.Y.Z`. `release.yml`
pushes that tag after the GitHub Release exists, and that push can be refused
(a workflow file changed on `main` since the tagged commit) after the release
is already out. Nothing then says that `go get` cannot resolve the version.

This check reads the remote's tags and fails when

  - a release tag `vX.Y.Z` (from the first release that had a Go tag on) has no
    `bindings/go/vX.Y.Z`;
  - the two exist and point at different commits; or
  - a `bindings/go/vX.Y.Z` exists with no `vX.Y.Z`.

Pre-release tags (`vX.Y.Z-rc.N`) are skipped: a pre-release publishes nothing.

Run:  check_go_tags.py [--remote origin] [--repo DIR]
      check_go_tags.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

GO_PREFIX = "bindings/go/"
STABLE_RE = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")
# The first release whose Go module tag exists; earlier releases had no Go binding.
FIRST_WITH_GO = (0, 4, 0)


def remote_tags(repo: Path, remote: str) -> dict[str, str]:
    """`{tag name: commit}` for every tag on `remote`, annotated tags peeled."""
    proc = subprocess.run(["git", "ls-remote", "--tags", remote], cwd=repo, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(f"git ls-remote --tags {remote} failed ({proc.returncode}): {proc.stderr.strip()}")
    direct: dict[str, str] = {}
    peeled: dict[str, str] = {}
    for line in proc.stdout.splitlines():
        sha, _, ref = line.partition("\t")
        if not ref.startswith("refs/tags/"):
            continue
        name = ref[len("refs/tags/"):]
        if name.endswith("^{}"):
            peeled[name[:-3]] = sha
        else:
            direct[name] = sha
    return {name: peeled.get(name, sha) for name, sha in direct.items()}


def problems(tags: dict[str, str]) -> list[str]:
    out: list[str] = []
    releases = {n: c for n, c in tags.items() if STABLE_RE.match(n)}
    go = {n[len(GO_PREFIX):]: c for n, c in tags.items()
          if n.startswith(GO_PREFIX) and STABLE_RE.match(n[len(GO_PREFIX):])}

    def version(name: str) -> tuple[int, int, int]:
        return tuple(int(x) for x in STABLE_RE.match(name).groups())  # type: ignore[union-attr]

    for name in sorted(releases, key=version):
        if version(name) < FIRST_WITH_GO:
            continue
        if name not in go:
            out.append(f"{name} has no {GO_PREFIX}{name}: `go get` cannot resolve that version")
        elif go[name] != releases[name]:
            out.append(f"{GO_PREFIX}{name} points at {go[name][:9]}, {name} at {releases[name][:9]}")
    for name in sorted(go, key=version):
        if name not in releases:
            out.append(f"{GO_PREFIX}{name} exists with no {name}")
    return out


def self_test() -> int:
    failures: list[str] = []

    def check(label, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    a, b = "a" * 40, "b" * 40
    ok = {"v0.3.0": a, "v0.4.0": a, "bindings/go/v0.4.0": a, "v0.5.0": b, "bindings/go/v0.5.0": b,
          "v0.6.0-rc.1": a, "some/other/v1.0.0": a}
    check("matched pairs, an early release, a pre-release and a foreign tag", problems(ok), [])
    missing = dict(ok)
    del missing["bindings/go/v0.5.0"]
    check("missing Go tag", problems(missing),
          ["v0.5.0 has no bindings/go/v0.5.0: `go get` cannot resolve that version"])
    moved = dict(ok, **{"bindings/go/v0.5.0": a})
    check("Go tag on another commit", problems(moved),
          [f"bindings/go/v0.5.0 points at {a[:9]}, v0.5.0 at {b[:9]}"])
    orphan = dict(ok, **{"bindings/go/v0.7.0": a})
    check("Go tag without a release", problems(orphan), ["bindings/go/v0.7.0 exists with no v0.7.0"])
    # Versions sort numerically: v0.10.0 after v0.9.0.
    two = {"v0.10.0": a, "v0.9.0": a}
    check("numeric order", [p.split()[0] for p in problems(two)], ["v0.9.0", "v0.10.0"])

    # Test: empty tag list produces no problems (nothing to report).
    check("empty tag list is clean", problems({}), [])

    if failures:
        for f in failures:
            print(f"::error::check_go_tags self-test: {f}")
        return 1
    print("check_go_tags --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--remote", default="origin")
    ap.add_argument("--repo", type=Path, default=Path(__file__).resolve().parent.parent)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    try:
        tags = remote_tags(args.repo, args.remote)
    except RuntimeError as exc:
        print(f"::error::{exc}")
        return 1
    found = problems(tags)
    for p in found:
        print(f"::error::{p}")
    if found:
        return 1
    n = sum(1 for t in tags if t.startswith(GO_PREFIX))
    print(f"check_go_tags: {n} Go module tag(s), each on its release's commit")
    return 0


if __name__ == "__main__":
    sys.exit(main())
