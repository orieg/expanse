#!/usr/bin/env python3
"""scripts/check_deletion_rationale.py — Deletion Rationale Gate for Expanse PRs.

Enforces that any Pull Request that deletes tracked files carries an explicit
rationale in the PR body (e.g. `removes: <reason>` or `deletes: <reason>`),
preventing accidental or unreviewed file deletions (such as those resulting from
`git reset --soft` across a moving main ref, #458).

Rules:
- A PR with 0 deleted files passes automatically.
- A PR that deletes tracked files MUST include a line-anchored directive in the
  PR body matching:
    removes: <nonempty reason>
    deletes: <nonempty reason>
  (optionally wrapped in an HTML comment `<!-- removes: ... -->`).
- Quoting the directive in prose, tables, or backticks without beginning the line
  does NOT approve deletions.
- Empty or template placeholder reasons (e.g. `<reason>`, `<rationale>`, `TODO`)
  are rejected.

Usage:
  python3 scripts/check_deletion_rationale.py --base origin/main
  python3 scripts/check_deletion_rationale.py --pr-body-file pr-body.txt --base origin/main
  python3 scripts/check_deletion_rationale.py --self-test
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import List, Optional, Tuple


def parse_deletion_rationale(pr_body: str) -> Optional[str]:
    """Extracts a file deletion rationale from a PR body string.

    Accepts `removes: <reason>` or `deletes: <reason>` (and plural/singular forms)
    starting at line beginning (or optional leading whitespace / HTML comment open).
    Trailing HTML comment markers and code backticks are stripped.
    """
    if not pr_body:
        return None

    # Line-anchored search for removes: or deletes:
    pattern = re.compile(
        r"^[ \t]*(?:<!--[ \t]*)?(?:removes?|deletes?):[ \t]*([^\n]+)",
        re.IGNORECASE | re.MULTILINE,
    )

    for match in pattern.finditer(pr_body):
        reason = match.group(1).strip()
        # Strip trailing HTML comment closers, backticks, or trailing punctuation
        reason = re.sub(r"(?:-->|`)+\s*$", "", reason).strip()
        if not reason:
            continue

        lower = reason.lower()
        # Reject placeholders
        if lower.startswith("<reason>") or lower.startswith("<rationale>"):
            continue
        if lower in ("todo", "tbd", "none", "n/a", "null"):
            continue

        return reason

    return None


def get_git_deleted_files(
    base_ref: str, head_ref: str = "HEAD", root: Optional[Path] = None
) -> Tuple[List[str], str]:
    """Returns (deleted_files, error_message).

    Distinguishes three states:
    1. Deletions found: returns (non-empty list, "")
    2. No deletions: returns ([], "")
    3. Could not determine (command/ref failure): returns ([], "<error message>")
    """
    cwd = str(root) if root else None

    # First check if base_ref exists locally. If not, try to fetch it shallowly.
    check_ref = subprocess.run(
        ["git", "rev-parse", "--verify", base_ref],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if check_ref.returncode != 0:
        # Try fetching the base ref
        remote = "origin"
        branch = base_ref
        if base_ref.startswith("origin/"):
            branch = base_ref[len("origin/"):]
        fetch_res = subprocess.run(
            ["git", "fetch", remote, f"{branch}:{base_ref}"],
            cwd=cwd,
            capture_output=True,
            text=True,
        )
        recheck = subprocess.run(
            ["git", "rev-parse", "--verify", base_ref],
            cwd=cwd,
            capture_output=True,
            text=True,
        )
        if recheck.returncode != 0:
            err_details = (
                fetch_res.stderr.strip()
                or check_ref.stderr.strip()
                or f"fatal: ref '{base_ref}' does not exist"
            )
            return [], f"Base ref '{base_ref}' could not be resolved or fetched:\n{err_details}"

    # Check if head_ref resolves
    check_head = subprocess.run(
        ["git", "rev-parse", "--verify", head_ref],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if check_head.returncode != 0:
        return [], f"Head ref '{head_ref}' could not be resolved:\n{check_head.stderr.strip()}"

    # Find merge base
    mb_res = subprocess.run(
        ["git", "merge-base", base_ref, head_ref],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if mb_res.returncode != 0:
        return (
            [],
            f"Failed to determine merge-base between '{base_ref}' and '{head_ref}':\n{mb_res.stderr.strip()}",
        )
    diff_target = mb_res.stdout.strip()
    if not diff_target:
        return [], f"git merge-base returned empty string between '{base_ref}' and '{head_ref}'"

    # Run git diff --diff-filter=D --name-only
    res = subprocess.run(
        ["git", "diff", "--diff-filter=D", "--name-only", diff_target, head_ref],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if res.returncode != 0:
        return (
            [],
            f"git diff command failed between '{diff_target}' and '{head_ref}':\n{res.stderr.strip()}",
        )

    lines = [line.strip() for line in res.stdout.splitlines() if line.strip()]
    return lines, ""


def run_check(
    deleted_files: List[str],
    pr_body: str,
    base_ref: str,
) -> int:
    """Evaluates deleted files against PR body rationale."""
    if not deleted_files:
        print(f"✓ No tracked file deletions detected against {base_ref}.")
        return 0

    rationale = parse_deletion_rationale(pr_body)
    if rationale:
        print(f"✓ {len(deleted_files)} file deletion(s) approved via PR rationale:")
        print(f"  Rationale: \"{rationale}\"")
        print("  Deleted files:")
        for f in deleted_files:
            print(f"    - {f}")
        return 0

    # Deletions present but no valid rationale provided
    print(f"::error::Unrationalized file deletion(s) detected ({len(deleted_files)} file(s) deleted vs {base_ref}):")
    for f in deleted_files:
        print(f"::error::  Deleted: {f}")
    print("\nTo approve intentional file deletions, add an explicit directive to your PR body:")
    print("  removes: <nonempty rationale>")
    print("  or")
    print("  deletes: <nonempty rationale>")
    print("(e.g., 'removes: deprecating legacy v1 benchmarks in favor of Criterion suite')\n")
    return 1


def self_test() -> int:
    """Unit self-tests for rationale parsing, fail-loud git errors, and edge cases."""
    # 1. Valid single-line directives
    assert parse_deletion_rationale("removes: old benchmark script") == "old benchmark script"
    assert parse_deletion_rationale("deletes: obsolete v1 bindings") == "obsolete v1 bindings"
    assert parse_deletion_rationale("remove: legacy shim") == "legacy shim"
    assert parse_deletion_rationale("delete: dead code") == "dead code"
    assert parse_deletion_rationale("Removes: Case Insensitive Rationale") == "Case Insensitive Rationale"

    # 2. HTML comments
    assert parse_deletion_rationale("<!-- removes: cleaned up stale test files -->") == "cleaned up stale test files"
    assert parse_deletion_rationale("<!-- deletes: dead code -->") == "dead code"

    # 3. Indented lines
    assert parse_deletion_rationale("  removes: indented directive") == "indented directive"
    assert parse_deletion_rationale("\tdeletes: tab-indented directive") == "tab-indented directive"

    # 4. Multi-line PR body with markdown and code blocks
    pr_sample = """
    ## Summary of changes
    This PR refactors the Go bindings and removes outdated benchmarks.

    <!-- removes: replaced ad-hoc Go microbenchmarks with bench_bindings.py -->

    ### Verification
    - tests passed locally
    """
    assert parse_deletion_rationale(pr_sample) == "replaced ad-hoc Go microbenchmarks with bench_bindings.py"

    # 5. Negative tests (placeholders, empty, inline mid-sentence mentions)
    assert parse_deletion_rationale("removes: <reason>") is None
    assert parse_deletion_rationale("deletes: <rationale>") is None
    assert parse_deletion_rationale("removes: TODO") is None
    assert parse_deletion_rationale("removes: none") is None
    assert parse_deletion_rationale("removes: ") is None
    assert parse_deletion_rationale("removes:") is None
    assert parse_deletion_rationale("") is None

    # Mid-sentence and table mentions must NOT trigger approval
    assert parse_deletion_rationale("This PR removes: old files and updates docs.") is None
    assert parse_deletion_rationale("| `removes: <reason>` | approves deletion |") is None
    assert parse_deletion_rationale("See the removes: policy in AGENTS.md") is None
    assert parse_deletion_rationale("We do not need `deletes: something` here.") is None

    # 6. Evaluation tests
    assert run_check([], "", "origin/main") == 0
    assert run_check(["file1.rs"], "removes: intentional cleanup", "origin/main") == 0
    assert run_check(["file1.rs"], "no rationale here", "origin/main") == 1

    # 7. Fail-loud error propagation tests (Task 1)
    # Unresolvable base ref must return non-empty error string and empty list
    bad_files, bad_err = get_git_deleted_files("origin/nonexistent-branch-12345-never-exists")
    assert bad_files == []
    assert bad_err != ""
    assert "could not be resolved" in bad_err.lower() or "not found" in bad_err.lower() or "error" in bad_err.lower()

    # Valid ref comparison against HEAD must return empty error string
    head_files, head_err = get_git_deleted_files("HEAD", "HEAD")
    assert head_err == ""
    assert head_files == []

    print("check_deletion_rationale.py --self-test: all checks passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Check that PR file deletions carry an explicit rationale")
    parser.add_argument("--base", default="origin/main", help="Base ref to diff against (default: origin/main)")
    parser.add_argument("--head", default="HEAD", help="Head ref (default: HEAD)")
    parser.add_argument("--pr-body-file", help="Path to file containing PR body text")
    parser.add_argument("--pr-body", help="PR body text as a string")
    parser.add_argument("--self-test", action="store_true", help="Run internal self-tests and exit")

    args = parser.parse_args()

    if args.self_test:
        return self_test()

    pr_body = ""
    if args.pr_body:
        pr_body = args.pr_body
    elif args.pr_body_file and os.path.exists(args.pr_body_file):
        try:
            pr_body = Path(args.pr_body_file).read_text(encoding="utf-8")
        except Exception as e:
            print(f"::warning::Failed to read PR body file '{args.pr_body_file}': {e}", file=sys.stderr)
    elif "PR_BODY" in os.environ:
        pr_body = os.environ["PR_BODY"]

    deleted_files, err = get_git_deleted_files(args.base, args.head)
    if err:
        print(f"::error::{err}", file=sys.stderr)
        return 1

    return run_check(deleted_files, pr_body, args.base)


if __name__ == "__main__":
    sys.exit(main())
