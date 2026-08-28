#!/usr/bin/env python3
"""scripts/check_test_floors.py — Workspace Test Count Floor Gate for Expanse.

Enforces that the total number of workspace unit and integration tests does not
shrink below a pinned floor (baseline: 300 tests), catching accidental or unreviewed
test deletions (such as branches deleting tests alongside reverted code, #458).

Rules:
- Workspace test count MUST be >= MIN_WORKSPACE_TESTS (pinned baseline: 300).
- The floor constant is verified against the base ref (e.g. `origin/main`); a PR
  that decreases MIN_WORKSPACE_TESTS or fails to resolve the base floor will fail
  CI unless approved.
- The margin is deliberately thin (currently 305 tests vs floor of 300) so any
  substantial test deletion trips the floor immediately.
- If tests or the floor constant are intentionally consolidated or lowered, the
  PR body MUST include an explicit directive:
    allow-test-shrink: <nonempty reason>
  (optionally wrapped in an HTML comment `<!-- allow-test-shrink: ... -->`).

Usage:
  python3 scripts/check_test_floors.py
  python3 scripts/check_test_floors.py --base origin/main
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

REQUIRED_TEST_SUITES = [
    "bindings/go/expanse_test.go",
    "tests/test_python_bindings.py",
    "tests/cpp/test_expanse.cpp",
    "crates/expanse-capi/tests/test_modern_capi.rs",
    "crates/expanse-capi/tests/test_blobmap_capi.rs",
    "crates/expanse/tests/proptest_model.rs",
    "crates/expanse/tests/test_ycsb.rs",
    "crates/expanse/tests/test_encoding_reference_sync.rs",
    "crates/expanse/tests/test_visualizer_sync.rs",
    "crates/expanse/tests/no_heap_churn.rs",
    "crates/expanse/tests/linearizability.rs",
]


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


def check_required_test_suites(root: Path, pr_body: str) -> Tuple[bool, list[str]]:
    """Verifies that all required test suites exist on disk."""
    missing = []
    for rel_path in REQUIRED_TEST_SUITES:
        if not (root / rel_path).is_file():
            missing.append(rel_path)

    if not missing:
        return True, []

    override = parse_allow_test_shrink(pr_body)
    if override:
        print(f"⚠️ Required test suite(s) missing ({', '.join(missing)}), but approved via PR override:")
        print(f"  Rationale: \"{override}\"")
        return True, missing

    for f in missing:
        print(f"::error::Required test suite file '{f}' is missing from the repository!")
    print("If this test suite was intentionally removed or renamed, add an explicit directive to the PR body:")
    print("  allow-test-shrink: <nonempty reason>")
    return False, missing


def get_base_floor_constant(
    base_ref: str,
    script_rel_path: str = "scripts/check_test_floors.py",
    var_name: str = "MIN_WORKSPACE_TESTS",
    root: Optional[Path] = None,
) -> Tuple[Optional[int], str]:
    """Reads the floor constant from script_rel_path in base_ref using `git show`.

    Returns (floor_int, "") on success.
    Returns (None, error_message) on any resolution failure, shallow clone failure,
    missing file, or if the constant is not defined/found in the base ref.
    Fails loud — never returns (None, "") to avoid failing open.
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
            return None, f"Base ref '{base_ref}' could not be resolved or fetched:\n{err_details}"

    # Read the script content from base_ref
    show_res = subprocess.run(
        ["git", "show", f"{base_ref}:{script_rel_path}"],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if show_res.returncode != 0:
        return (
            None,
            f"Failed to read '{script_rel_path}' from base ref '{base_ref}':\n{show_res.stderr.strip()}",
        )

    # Parse constant
    pattern = re.compile(rf"^[ \t]*{var_name}[ \t]*=[ \t]*(\d+)", re.MULTILINE)
    match = pattern.search(show_res.stdout)
    if not match:
        return (
            None,
            f"Floor constant '{var_name}' not found in '{script_rel_path}' on base ref '{base_ref}'",
        )

    try:
        return int(match.group(1)), ""
    except ValueError as e:
        return None, f"Failed to parse integer floor from '{match.group(1)}': {e}"


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

    # 4. Required suites check test
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        tdp = Path(td)
        ok, missing = check_required_test_suites(tdp, "")
        assert not ok
        assert len(missing) == len(REQUIRED_TEST_SUITES)
        ok_override, _ = check_required_test_suites(tdp, "allow-test-shrink: testing dummy repo")
        assert ok_override

    # 5. Base floor extraction self-tests (Task 2)
    # Valid ref against HEAD
    base_fl, base_err = get_base_floor_constant("HEAD", "scripts/check_test_floors.py", "MIN_WORKSPACE_TESTS")
    assert base_err == "", base_err
    assert base_fl == 300, base_fl

    # Unresolvable base ref must fail loud with non-empty error string
    bad_fl, bad_err = get_base_floor_constant("origin/nonexistent-branch-12345-never-exists")
    assert bad_fl is None
    assert bad_err != ""

    # Missing constant in file must fail loud with non-empty error string
    missing_fl, missing_err = get_base_floor_constant("HEAD", "scripts/check_test_floors.py", "NONEXISTENT_CONSTANT_NAME")
    assert missing_fl is None
    assert missing_err != ""

    print("check_test_floors.py --self-test: all checks passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify that workspace test count satisfies pinned floor")
    parser.add_argument("--base", help="Base ref to compare floor constant against (default: origin/$GITHUB_BASE_REF or origin/main)")
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

    root = Path(__file__).resolve().parent.parent

    # Determine base ref
    base_ref = args.base
    if not base_ref:
        if os.environ.get("GITHUB_BASE_REF"):
            base_ref = f"origin/{os.environ['GITHUB_BASE_REF']}"
        else:
            base_ref = "origin/main"

    # Base floor comparison: fail loud if base floor cannot be determined
    base_floor, err = get_base_floor_constant(base_ref, "scripts/check_test_floors.py", "MIN_WORKSPACE_TESTS", root=root)
    if err:
        print(f"::error::{err}", file=sys.stderr)
        return 1

    effective_floor = args.floor
    if base_floor is not None and effective_floor < base_floor:
        override = parse_allow_test_shrink(pr_body)
        if override:
            print(f"⚠️ Floor decrease detected (MIN_WORKSPACE_TESTS: {base_floor} -> {effective_floor}), approved via PR override:")
            print(f"  Rationale: \"{override}\"")
        else:
            print(f"::error::Test count floor (MIN_WORKSPACE_TESTS = {effective_floor}) is lower than base ref {base_ref} ({base_floor}) without an explicit override directive.")
            print("To approve lowering the floor, add an explicit directive to your PR body:")
            print("  allow-test-shrink: <nonempty reason>")
            return 1

    suites_ok, _ = check_required_test_suites(root, pr_body)

    if args.test_count is not None:
        count = args.test_count
    elif args.test_output_file and os.path.exists(args.test_output_file):
        output = Path(args.test_output_file).read_text(encoding="utf-8")
        count = count_tests_from_output(output)
    else:
        count, err = count_workspace_tests(root)
        if err:
            print(f"::error::{err}", file=sys.stderr)
            return 1

    floor_ret = evaluate_test_floor(count, pr_body, floor=effective_floor)
    if not suites_ok or floor_ret != 0:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
