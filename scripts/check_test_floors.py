#!/usr/bin/env python3
"""scripts/check_test_floors.py — Workspace Test Count Floor Gate for Expanse.

Enforces that the total number of workspace unit and integration tests does not
shrink below a pinned floor (e.g. 300 tests), catching accidental or unreviewed
test deletions (such as branches deleting tests alongside reverted code, #458).

Rules:
- Workspace test count MUST be >= MIN_WORKSPACE_TESTS (pinned baseline: 300).
- If tests are intentionally consolidated or removed, the PR body MUST include
  an explicit directive:
    allow-test-shrink: <nonempty reason>
  (optionally wrapped in an HTML comment `<!-- allow-test-shrink: ... -->`).

Usage:
  python3 scripts/check_test_floors.py
  python3 scripts/check_test_floors.py --pr-body-file pr-body.txt
  python3 scripts/check_test_floors.py --self-test
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Optional, Tuple

MIN_WORKSPACE_TESTS = 300


def parse_allow_test_shrink(pr_body: str) -> Optional[str]:
    """Extracts test-shrink override reason from a PR body."""
    if not pr_body:
        return None

    pattern = re.compile(
        r"^[ \t]*(?:<!--[ \t]*)?allow-test-shrink:[ \t]*([^\n]+)",
        re.IGNORECASE | re.MULTILINE,
    )

    for match in pattern.finditer(pr_body):
        reason = match.group(1).strip()
        reason = re.sub(r"(?:-->|`)+\s*$", "", reason).strip()
        if not reason:
            continue
        lower = reason.lower()
        if lower.startswith("<reason>") or lower.startswith("<rationale>"):
            continue
        if lower in ("todo", "tbd", "none", "n/a", "null"):
            continue
        return reason

    return None


def count_tests_from_output(output: str) -> int:
    """Counts tests from `cargo test ... -- --list` output."""
    count = 0
    for line in output.splitlines():
        line = line.strip()
        if line.endswith(": test"):
            count += 1
    return count


def count_workspace_tests(root: Optional[Path] = None) -> Tuple[int, str]:
    """Runs `cargo test --workspace --exclude expanse-php --exclude expanse-py -- --list` and counts tests."""
    cmd = [
        "cargo",
        "test",
        "--workspace",
        "--exclude",
        "expanse-php",
        "--exclude",
        "expanse-py",
        "--",
        "--list",
    ]
    res = subprocess.run(
        cmd,
        cwd=str(root) if root else None,
        capture_output=True,
        text=True,
    )
    if res.returncode != 0:
        return 0, f"cargo test list command failed:\n{res.stderr}"

    count = count_tests_from_output(res.stdout)
    return count, ""


def evaluate_test_floor(
    count: int,
    pr_body: str,
    floor: int = MIN_WORKSPACE_TESTS,
) -> int:
    """Evaluates whether the measured test count satisfies the floor."""
    if count >= floor:
        print(f"✓ Workspace test count ({count}) satisfies pinned floor (≥ {floor}).")
        return 0

    override = parse_allow_test_shrink(pr_body)
    if override:
        print(f"⚠️ Workspace test count ({count}) is below pinned floor ({floor}), but approved via PR override:")
        print(f"  Rationale: \"{override}\"")
        return 0

    print(f"::error::Workspace test count ({count}) is below the pinned floor of {floor}!")
    print("If tests were intentionally deleted or consolidated, add an explicit directive to the PR body:")
    print("  allow-test-shrink: <nonempty reason>")
    return 1


def self_test() -> int:
    """Runs internal self-tests."""
    # 1. Output parser tests
    sample_output = """
     Running unittests src/lib.rs (target/debug/deps/expanse_trie-123)
tests::test_insert: test
tests::test_remove: test
tests::test_get: test
3 tests, 0 benchmarks
     Running tests/test_blobmap.rs (target/debug/deps/test_blobmap-456)
test_blobmap_basic: test
test_blobmap_compact: test
2 tests, 0 benchmarks
    """
    assert count_tests_from_output(sample_output) == 5

    # 2. Override parser tests
    assert parse_allow_test_shrink("allow-test-shrink: merged duplicate tests") == "merged duplicate tests"
    assert parse_allow_test_shrink("<!-- allow-test-shrink: refactored test matrix -->") == "refactored test matrix"
    assert parse_allow_test_shrink("  allow-test-shrink: indented reason") == "indented reason"
    assert parse_allow_test_shrink("allow-test-shrink: <reason>") is None
    assert parse_allow_test_shrink("allow-test-shrink: TODO") is None
    assert parse_allow_test_shrink("allow-test-shrink:") is None
    assert parse_allow_test_shrink("mentioning allow-test-shrink: mid-sentence") is None

    # 3. Evaluation tests
    assert evaluate_test_floor(305, "", floor=300) == 0
    assert evaluate_test_floor(300, "", floor=300) == 0
    assert evaluate_test_floor(280, "allow-test-shrink: intentional test consolidation", floor=300) == 0
    assert evaluate_test_floor(280, "", floor=300) == 1

    print("check_test_floors.py --self-test: all checks passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify that workspace test count satisfies pinned floor")
    parser.add_argument("--floor", type=int, default=MIN_WORKSPACE_TESTS, help=f"Minimum required test count (default: {MIN_WORKSPACE_TESTS})")
    parser.add_argument("--test-count", type=int, help="Precomputed test count (skips cargo test listing)")
    parser.add_argument("--test-output-file", help="Path to file with `cargo test -- --list` output")
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

    if args.test_count is not None:
        count = args.test_count
    elif args.test_output_file and os.path.exists(args.test_output_file):
        output = Path(args.test_output_file).read_text(encoding="utf-8")
        count = count_tests_from_output(output)
    else:
        count, err = count_workspace_tests()
        if err:
            print(f"::error::{err}", file=sys.stderr)
            return 1

    return evaluate_test_floor(count, pr_body, floor=args.floor)


if __name__ == "__main__":
    sys.exit(main())
