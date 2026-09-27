#!/usr/bin/env python3
"""Stage only the files a benchmark run wrote, for upload as that run's artifact (#1212).

The defect this pins: `bench_baremetal.yml` uploaded a fixed list of paths into
every `suite-tables-<suite>-<run>` artifact. Thirteen of them are committed
files, so every checkout has them, and every suite's artifact carried
concurrency and patricia JSONs measured at older commits
(`5cf94b17`, `10ce2f9d`, `6000b4a1`) beside the run's own output. A reader who
downloads the artifact can take one of those for a measurement of the run's
commit (AGENTS.md section 8.7).

Which files the run wrote is a fact git already holds, so no timestamp is
consulted. The checkout is cleaned before the run (`actions/checkout`
`clean: true`), so a candidate is this run's output when it is

- untracked (or ignored): nothing but this run can have created it, or
- tracked and modified: the run rewrote a committed result.

A tracked file the run did not touch is excluded and named in a notice.

Every staged file must also be something safe to publish:

- a regular file, never a symlink — a link planted in the tree would
  otherwise copy whatever it points at (a key, a token) into a public
  artifact;
- inside the repository, after resolving the path;
- below a size cap;
- if it is JSON, parseable, and with a `provenance.commit` that names the
  measured commit. A fresh file that names another commit is a harness
  defect, not a stale leftover, and fails the step.

Usage:
  stage_run_outputs.py --dest run-artifacts --commit <sha> <candidate or glob>...
  stage_run_outputs.py --self-test
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_bench_provenance import provenance_commit_mismatch

# Larger than any output a suite writes today (the biggest committed result
# JSON is a few MiB); a file above it is not a result table.
MAX_BYTES = 256 * 1024 * 1024


def _git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)


def is_tracked(root: Path, rel: str) -> bool:
    r = _git(root, "ls-files", "--error-unmatch", "--", rel)
    if r.returncode == 0:
        return True
    if r.returncode == 1:
        return False
    raise RuntimeError(f"git ls-files failed on {rel!r}: {r.stderr.strip()}")


def is_modified(root: Path, rel: str) -> bool:
    r = _git(root, "diff", "--quiet", "HEAD", "--", rel)
    if r.returncode in (0, 1):
        return r.returncode == 1
    raise RuntimeError(f"git diff failed on {rel!r}: {r.stderr.strip()}")


def expand(root: Path, candidates: list[str]) -> list[str]:
    """Repository-relative paths the candidates name, globs expanded, deduplicated.

    `glob` is not asked to follow anything: a symlink it matches is returned
    as itself and judged by `classify`.
    """
    out: list[str] = []
    for cand in candidates:
        if os.path.isabs(cand) or ".." in Path(cand).parts:
            raise ValueError(f"candidate {cand!r} must be a relative path inside the repository")
        hits = sorted(glob.glob(cand, root_dir=root)) if glob.has_magic(cand) else [cand]
        for h in hits:
            if h not in out:
                out.append(h)
    return out


def classify(root: Path, rel: str, head_sha: str) -> tuple[str, str]:
    """`(verdict, detail)`: verdict is `stage`, `skip` or `fatal`."""
    path = root / rel
    try:
        st = os.lstat(path)
    except FileNotFoundError:
        return "skip", "not written by this suite"
    if stat.S_ISLNK(st.st_mode):
        return "fatal", "is a symlink; a link is never published (it would copy its target)"
    if not stat.S_ISREG(st.st_mode):
        return "fatal", "is not a regular file"
    real = Path(os.path.realpath(path))
    if not real.is_relative_to(root.resolve()):
        return "fatal", f"resolves outside the repository ({real})"
    if st.st_size > MAX_BYTES:
        return "fatal", f"is {st.st_size} bytes, above the {MAX_BYTES}-byte cap"
    if is_tracked(root, rel) and not is_modified(root, rel):
        return "skip", "is a committed file this run did not rewrite"
    if rel.endswith(".json"):
        try:
            obj = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
            return "fatal", f"was written by this run but is not valid JSON ({exc})"
        why = provenance_commit_mismatch(obj, head_sha)
        if why:
            return "fatal", f"was written by this run but {why}"
    return "stage", ""


def stage(root: Path, dest: Path, head_sha: str, candidates: list[str]) -> tuple[list[str], list[str], list[str]]:
    """Copies the run's own files under `dest`, keeping their relative paths.

    Returns `(staged, skipped, fatal)`, the last two as `path: reason` lines.
    """
    staged: list[str] = []
    skipped: list[str] = []
    fatal: list[str] = []
    for rel in expand(root, candidates):
        verdict, detail = classify(root, rel, head_sha)
        if verdict == "stage":
            target = dest / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(root / rel, target, follow_symlinks=False)
            staged.append(rel)
        elif verdict == "skip":
            if "committed" in detail:
                skipped.append(f"{rel}: {detail}")
        else:
            fatal.append(f"{rel}: {detail}")
    return staged, skipped, fatal


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dest", help="directory to stage into (created)")
    ap.add_argument("--commit", help="full commit hash the run measured (git rev-parse HEAD)")
    ap.add_argument("candidates", nargs="*", help="repository-relative paths or globs")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.dest or not args.commit:
        ap.error("--dest and --commit are required")
    root = Path(_git(Path.cwd(), "rev-parse", "--show-toplevel").stdout.strip() or ".")
    dest = Path(args.dest)
    if not dest.is_absolute():
        dest = root / dest
    dest.mkdir(parents=True, exist_ok=True)
    staged, skipped, fatal = stage(root, dest, args.commit, args.candidates)
    for line in skipped:
        print(f"::notice::not uploaded — {line}")
    for rel in staged:
        print(f"staged {rel}")
    for line in fatal:
        print(f"::error::refusing to publish {line}")
    print(f"stage_run_outputs.py: {len(staged)} file(s) staged, {len(skipped)} committed file(s) left out, {len(fatal)} refused")
    return 1 if fatal else 0


def self_test() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp, "repo")
        root.mkdir()

        def g(*a: str) -> subprocess.CompletedProcess:
            return subprocess.run(["git", "-C", str(root), *a], check=True, capture_output=True)

        g("init", "-q")
        g("config", "user.email", "t@example.invalid")
        g("config", "user.name", "t")
        g("config", "commit.gpgsign", "false")
        res = root / "docs/benchmarks/x/results"
        res.mkdir(parents=True)
        old = {"provenance": {"commit": "5cf94b17"}, "cells": []}
        (res / "baseline_a.json").write_text(json.dumps(old))
        (res / "baseline_b.json").write_text(json.dumps(old))
        (res / "notes.json").write_text("{}")
        g("add", "-A")
        g("commit", "-q", "-m", "seed")
        head = g("rev-parse", "HEAD").stdout.decode().strip()

        # The motivating defect: committed JSONs at an older commit, untouched
        # by the run, are not this run's output.
        verdict, detail = classify(root, "docs/benchmarks/x/results/baseline_a.json", head)
        assert verdict == "skip" and "committed" in detail, (verdict, detail)

        # A committed result the run rewrote at its own commit is staged,
        # whether the producer recorded a short or a full hash.
        for rec in (head[:8], head):
            (res / "baseline_b.json").write_text(json.dumps({"provenance": {"commit": rec}}))
            assert classify(root, "docs/benchmarks/x/results/baseline_b.json", head)[0] == "stage", rec

        # Rewritten but naming another commit: a harness defect, fatal.
        (res / "baseline_b.json").write_text(json.dumps({"provenance": {"commit": "6000b4a1"}}))
        verdict, detail = classify(root, "docs/benchmarks/x/results/baseline_b.json", head)
        assert verdict == "fatal" and "not the measured commit" in detail, (verdict, detail)

        # Written by the run but not JSON: fatal, never uploaded as a result.
        (root / "fresh.json").write_text("{not json")
        assert classify(root, "fresh.json", head)[0] == "fatal"

        # An untracked console log (no provenance to check) is staged.
        (root / "bench-output.txt").write_text("result\n")
        assert classify(root, "bench-output.txt", head)[0] == "stage"
        # A JSON with no provenance block (bench_grammar_masks) is judged on
        # git state alone.
        (root / "masks.json").write_text(json.dumps({"rows": []}))
        assert classify(root, "masks.json", head)[0] == "stage"

        # A symlink is refused even when it points at an ordinary file, and
        # even when its name is an expected output.
        secret = Path(tmp, "id_ed25519")
        secret.write_text("PRIVATE")
        (root / "concurrency.txt").symlink_to(secret)
        verdict, detail = classify(root, "concurrency.txt", head)
        assert verdict == "fatal" and "symlink" in detail, (verdict, detail)

        # Paths that leave the repository are refused before any lookup.
        for bad in ("../outside.txt", "/etc/passwd"):
            try:
                expand(root, [bad])
            except ValueError:
                pass
            else:
                raise AssertionError(f"{bad} must be refused")

        # A candidate the suite did not write is skipped silently.
        assert classify(root, "rocksdb.txt", head)[0] == "skip"

        # End to end: only the run's files land in dest, the symlink target
        # never does, and the refusal makes the step fail.
        dest = Path(tmp, "stage")
        staged, skipped, fatal = stage(
            root, dest, head,
            ["docs/benchmarks/x/results/baseline_*.json", "bench-output.txt", "concurrency.txt", "rocksdb.txt"],
        )
        assert staged == ["bench-output.txt"], staged
        assert any("baseline_a.json" in s for s in skipped), skipped
        assert len(fatal) == 2, fatal  # baseline_b (wrong commit) and the symlink
        assert not any(p.name == "concurrency.txt" for p in dest.rglob("*")), list(dest.rglob("*"))
        assert (dest / "bench-output.txt").read_text() == "result\n"

    # The provenance comparison itself.
    full = "e33fc5cd" + "0" * 32
    assert provenance_commit_mismatch({"provenance": {"commit": "e33fc5cd"}}, full) is None
    assert provenance_commit_mismatch({"provenance": {"commit": full}}, full) is None
    assert provenance_commit_mismatch({"provenance": {"commit": "e33fc5c"}}, full) is None
    assert provenance_commit_mismatch({"provenance": {"commit": "e33f"}}, full)  # too short to name a commit
    assert provenance_commit_mismatch({"provenance": {"commit": "unknown"}}, full)
    assert provenance_commit_mismatch({"provenance": {}}, full) is None
    assert provenance_commit_mismatch([1, 2], full) is None

    print("stage_run_outputs.py --self-test: all checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
