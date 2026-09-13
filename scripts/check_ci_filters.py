#!/usr/bin/env python3
"""Exercise `ci.yml`'s path filters against a golden change-set -> job-set table.

The filters in `.github/workflows/ci.yml` decide which jobs run for a given
diff. Until this script existed they were *asserted* and never *exercised*:
every defect in them (#620, #671, and the batch this file was written for) was
found by accident, after the fact, on a PR that had already merged.

This is a gross-breakage tripwire, not a picomatch reimplementation -- the same
framing as `check_chart_layout.py`. It implements the subset of glob syntax the
workflow actually uses (`**`, `*`, `?`, literals, `!` exclusions) under
dorny/paths-filter's `some-with-excludes` quantifier, and evaluates the `||`
chains in each job's `if:`. A pattern form that appears in `ci.yml` and is not
in that subset is a hard error, never a silent mismatch (AGENTS.md 8.1).

Three checks, each with `--self-test` coverage:

  1. `check_golden_table`   -- the change-set -> job-set cases below.
  2. `check_outputs_consumed` -- every declared `detect-changes` output is read
     by at least one job's `if:`. A filter nobody consumes is a lie: the `docs`
     filter was declared, documented in `docs/CI.md` as gating `docs-lint`, and
     read by zero jobs.
  3. `check_if_shape`       -- every job except the declared unconditional ones
     carries an `if:`, and every term in it is one this evaluator understands.

Run:  python3 scripts/check_ci_filters.py [--self-test]
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover - CI always has it
    print("::error::PyYAML is required by check_ci_filters.py", file=sys.stderr)
    raise

REPO_ROOT = Path(__file__).resolve().parent.parent
CI_YML = REPO_ROOT / ".github" / "workflows" / "ci.yml"

# Jobs that must NOT carry an `if:`. Maintained here rather than inferred, so
# that making a fourth job unconditional is a reviewed line in a diff.
#   detect-changes -- computes the filters every other `if:` reads.
#   docs-lint      -- its hygiene sweep reads `git ls-files`, i.e. the whole
#                     tree, so a diff-scoped filter cannot bound it.
#   ci-gate        -- the rollup; runs `always()`.
UNCONDITIONAL_JOBS = frozenset({"detect-changes", "docs-lint", "ci-gate"})

# ---------------------------------------------------------------------------
# Golden table: (label, changed paths, jobs that MUST run, jobs that MUST NOT)
#
# `must_run` and `must_not_run` are both partial -- a case names the jobs whose
# behaviour it is pinning, not the full matrix -- so adding a job does not
# invalidate every case. The point of each case is named in its label.
# ---------------------------------------------------------------------------
GOLDEN_CASES = [
    (
        "docs-only PR wakes nothing expensive (#671: this was 69 jobs)",
        ["docs/BENCHMARKING.md", "README.md"],
        {"docs-lint"},
        {"test", "miri", "test-asan", "loom", "fuzz-smoke", "instruction-counts", "lint"},
    ),
    (
        "a core trie edit wakes the safety lane",
        ["crates/expanse/src/leaf.rs"],
        {"lint", "test", "miri", "test-asan", "instruction-counts"},
        set(),
    ),
    (
        "hot-bench reaches `lint` (check_bench_shapes reads its src/bin) "
        "but not the safety lane",
        ["crates/expanse-hot-bench/src/bin/ffi_probe.rs"],
        {"lint"},
        {"miri", "test-asan", "fuzz-smoke", "instruction-counts"},
    ),
    (
        "the writer-scaling driver's `scripts/` imports reach its self-test",
        ["scripts/bca_bootstrap.py"],
        {"writer-scaling-selftest", "lint"},
        {"miri", "test-asan"},
    ),
    (
        "the public-api snapshot reaches the job that diffs against it",
        [".github/public-api/expanse-trie.txt"],
        {"public-api", "lint"},
        {"miri", "fuzz-smoke"},
    ),
    (
        "a macOS rustflags change reaches the Rust lanes",
        [".cargo/config.toml"],
        {"test", "lint"},
        set(),
    ),
    (
        "a Ruby binding edit does not wake the Rust safety lane",
        ["bindings/ruby/lib/expanse.rb"],
        {"test-ruby"},
        {"miri", "test-asan", "fuzz-smoke", "instruction-counts"},
    ),
    (
        "a Python file under bindings/ reaches the construction-label census in "
        "`lint` (#880), which sweeps every importer of the BCa estimator",
        ["bindings/python/bench_concurrency.py"],
        {"lint"},
        {"miri", "test-asan", "fuzz-smoke"},
    ),
    (
        "a TSan suppression edit reaches the guard scripts",
        [".github/tsan-suppressions.txt"],
        {"lint"},
        {"miri", "test-asan"},
    ),
]


# ---------------------------------------------------------------------------
# Glob matching -- the picomatch subset `ci.yml` uses.
# ---------------------------------------------------------------------------
_SUPPORTED_SEGMENT = re.compile(r"^[A-Za-z0-9_.*?/\-]*$")


def glob_to_regex(pattern: str) -> re.Pattern[str]:
    """Translate one path glob to an anchored regex.

    Supports `**` (any number of path segments), `*` and `?` (within one
    segment), and literals. Any other metacharacter is refused rather than
    silently mistranslated.
    """
    if not _SUPPORTED_SEGMENT.match(pattern):
        raise ValueError(
            f"unsupported glob syntax in ci.yml filter: {pattern!r} -- "
            "extend check_ci_filters.py rather than letting it mis-match"
        )
    parts = pattern.split("/")
    out: list[str] = []
    for idx, part in enumerate(parts):
        last = idx == len(parts) - 1
        if part == "**":
            out.append(".*" if last else "(?:[^/]*/)*")
            continue
        seg = ""
        for ch in part:
            if ch == "*":
                seg += "[^/]*"
            elif ch == "?":
                seg += "[^/]"
            else:
                seg += re.escape(ch)
        out.append(seg)
        if not last:
            out.append("/")
    return re.compile("".join(out) + r"\Z")


def file_matches(path: str, patterns: list[str]) -> bool:
    """`some-with-excludes`: some positive matches and no `!` exclusion does."""
    positives = [p for p in patterns if not p.startswith("!")]
    negatives = [p[1:] for p in patterns if p.startswith("!")]
    if any(glob_to_regex(n).match(path) for n in negatives):
        return False
    return any(glob_to_regex(p).match(path) for p in positives)


def evaluate_filters(filters: dict[str, list[str]], changed: list[str]) -> dict[str, bool]:
    return {
        name: any(file_matches(p, patterns) for p in changed)
        for name, patterns in filters.items()
    }


# ---------------------------------------------------------------------------
# `if:` evaluation
# ---------------------------------------------------------------------------
_OUTPUT_TERM = re.compile(
    r"needs\.detect-changes\.outputs\.([A-Za-z0-9_-]+)\s*==\s*'true'"
)
_PUSH_TERM = re.compile(r"github\.event_name\s*!=\s*'pull_request'")
# `contains(needs.detect-changes.outputs.changed-jobs, '|<job-id>|')` -- the
# per-job term that replaced `.github/workflows/ci.yml` sitting in `rust-src`.
_JOBDIFF_TERM = re.compile(
    r"contains\(\s*needs\.detect-changes\.outputs\.changed-jobs\s*,\s*'\|([A-Za-z0-9_-]+)\|'\s*\)"
)


def evaluate_if(expr: str, outputs: dict[str, bool], *, is_pull_request: bool = True,
                changed_jobs: str = ""):
    """Evaluate an `if:` expression. Returns (runs, unknown_terms)."""
    if expr is None:
        return True, []
    body = expr.strip()
    if body.startswith("${{") and body.endswith("}}"):
        body = body[3:-2].strip()
    if body == "always()":
        return True, []
    unknown: list[str] = []
    result = False
    for term in body.split("||"):
        term = term.strip()
        m = _OUTPUT_TERM.fullmatch(term)
        if m:
            result = result or outputs.get(m.group(1), False)
            continue
        if _PUSH_TERM.fullmatch(term):
            result = result or (not is_pull_request)
            continue
        m = _JOBDIFF_TERM.fullmatch(term)
        if m:
            # `changed_jobs` is the delimited string the workflow would see.
            result = result or (f"|{m.group(1)}|" in (changed_jobs or ""))
            continue
        unknown.append(term)
    return result, unknown


def load_ci(path: Path = CI_YML):
    doc = yaml.safe_load(path.read_text())
    jobs = doc["jobs"]
    detect = jobs["detect-changes"]
    filters_raw = None
    for step in detect["steps"]:
        if isinstance(step.get("uses"), str) and "paths-filter" in step["uses"]:
            filters_raw = step["with"]["filters"]
            break
    if filters_raw is None:
        raise RuntimeError("could not find the paths-filter step in detect-changes")
    filters = yaml.safe_load(filters_raw)
    return doc, jobs, filters, detect.get("outputs", {})


def jobs_for_change(jobs, filters, changed):
    outputs = evaluate_filters(filters, changed)
    running = set()
    for name, body in jobs.items():
        runs, _ = evaluate_if(body.get("if"), outputs)
        if runs:
            running.add(name)
    return running



JOB_SNAPSHOT = REPO_ROOT / ".github" / "ci-jobs.txt"


def check_job_snapshot(jobs) -> list[str]:
    """The committed job list, checked the way `check_public_api.py` checks the
    Rust surface.

    Running the full matrix on a workflow edit proved the jobs that still exist
    pass; it never proved a job was not *deleted*. This does, and it is why
    `ci.yml` no longer needs to sit in `rust-src`. A deliberate change shows up
    as a reviewable diff in `.github/ci-jobs.txt`.
    """
    if not JOB_SNAPSHOT.exists():
        return [f"{JOB_SNAPSHOT} is missing -- regenerate it with --write-snapshot"]
    recorded = [ln.strip() for ln in JOB_SNAPSHOT.read_text().splitlines() if ln.strip()]
    live = sorted(jobs)
    if recorded == live:
        return []
    errs = []
    for gone in sorted(set(recorded) - set(live)):
        errs.append(
            f"job {gone!r} is in .github/ci-jobs.txt but no longer in ci.yml -- if the "
            "removal is intended, regenerate the snapshot so a reviewer sees it"
        )
    for added in sorted(set(live) - set(recorded)):
        errs.append(
            f"job {added!r} is new in ci.yml and not in .github/ci-jobs.txt -- "
            "regenerate the snapshot with --write-snapshot"
        )
    if not errs:
        errs.append(".github/ci-jobs.txt is out of order -- regenerate it")
    return errs


# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------
def check_golden_table(jobs, filters) -> list[str]:
    errs = []
    for label, changed, must_run, must_not in GOLDEN_CASES:
        running = jobs_for_change(jobs, filters, changed)
        known = set(jobs)
        for job in sorted(must_run & known):
            if job not in running:
                errs.append(f"{label}: expected {job!r} to RUN for {changed}, it was skipped")
        for job in sorted(must_not & known):
            if job in running:
                errs.append(f"{label}: expected {job!r} to be SKIPPED for {changed}, it ran")
    return errs


def check_outputs_consumed(jobs, declared_outputs) -> list[str]:
    consumed = set()
    for body in jobs.values():
        expr = body.get("if")
        if not expr:
            continue
        consumed.update(_OUTPUT_TERM.findall(str(expr)))
        if _JOBDIFF_TERM.search(str(expr)):
            consumed.add("changed-jobs")
    dead = sorted(set(declared_outputs) - consumed)
    return [
        f"detect-changes declares output {o!r} that no job's `if:` reads -- "
        "delete it or give it a consumer" for o in dead
    ]


def check_if_shape(jobs) -> list[str]:
    errs = []
    for name, body in jobs.items():
        expr = body.get("if")
        if name in UNCONDITIONAL_JOBS:
            if expr is not None and name != "ci-gate":
                errs.append(f"{name!r} is declared unconditional but carries an `if:`")
            continue
        if expr is None:
            errs.append(
                f"{name!r} has no `if:` -- it will run on every pull request. Gate it, "
                "or add it to UNCONDITIONAL_JOBS with a reason."
            )
            continue
        _, unknown = evaluate_if(expr, {})
        for term in unknown:
            errs.append(f"{name!r}: `if:` term not understood by this gate: {term!r}")
    return errs


# ---------------------------------------------------------------------------
def self_test() -> int:
    failures = []

    def check(label, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    # glob subset
    check("crates/** matches nested", bool(glob_to_regex("crates/**").match("crates/a/b.rs")), True)
    check("crates/** vs other top dir", bool(glob_to_regex("crates/**").match("docs/a.md")), False)
    check("*.md is top-level only", bool(glob_to_regex("*.md").match("README.md")), True)
    check("*.md does not cross /", bool(glob_to_regex("*.md").match("docs/x.md")), False)
    check("components/**/*.md", bool(glob_to_regex("components/**/*.md").match("components/a/b.md")), True)
    check("rust-toolchain* suffix", bool(glob_to_regex("rust-toolchain*").match("rust-toolchain.toml")), True)
    # `crates/expanse/**` must not swallow the detached hot-bench crate; this
    # exact confusion is why hot-bench matched no filter at all.
    check(
        "crates/expanse/** vs expanse-hot-bench",
        bool(glob_to_regex("crates/expanse/**").match("crates/expanse-hot-bench/src/bin/x.rs")),
        False,
    )
    # some-with-excludes
    pats = ["crates/**", "!crates/expanse-hot-bench/**"]
    check("exclusion wins", file_matches("crates/expanse-hot-bench/src/bin/x.rs", pats), False)
    check("exclusion is per-file", file_matches("crates/expanse/src/leaf.rs", pats), True)
    # unsupported syntax is refused, not mistranslated
    try:
        glob_to_regex("crates/{a,b}/**")
        failures.append("brace glob should have raised")
    except ValueError:
        pass
    # `if:` evaluation
    runs, unk = evaluate_if(
        "needs.detect-changes.outputs.rust-src == 'true' || github.event_name != 'pull_request'",
        {"rust-src": False},
    )
    check("PR with filter false skips", (runs, unk), (False, []))
    runs, _ = evaluate_if(
        "needs.detect-changes.outputs.rust-src == 'true' || github.event_name != 'pull_request'",
        {"rust-src": False},
        is_pull_request=False,
    )
    check("push runs it anyway", runs, True)
    runs, unk = evaluate_if("needs.detect-changes.outputs.nope == 'yes'", {})
    check("unrecognised term is reported", len(unk), 1)

    # The ci.yml narrowing that replaced `.github/workflows/ci.yml` in
    # `rust-src`. A workflow-only diff that touches one job's definition must
    # run that job and not the other 42.
    gate = ("needs.detect-changes.outputs.rust-src == 'true'"
            " || needs.detect-changes.outputs.ci-workflow-all == 'true'"
            " || contains(needs.detect-changes.outputs.changed-jobs, '|miri|')")
    runs, _ = evaluate_if(gate, {"rust-src": False, "ci-workflow-all": False},
                          changed_jobs="|miri|")
    check("job whose definition changed runs", runs, True)
    runs, _ = evaluate_if(gate, {"rust-src": False, "ci-workflow-all": False},
                          changed_jobs="|lint|")
    check("job whose definition did not change skips", runs, False)
    # fail-closed path: the differ could not narrow, so everything runs
    runs, _ = evaluate_if(gate, {"rust-src": False, "ci-workflow-all": True},
                          changed_jobs="")
    check("fail-closed runs everything", runs, True)
    # `contains` is substring, so the delimiters are load-bearing
    gate64 = ("contains(needs.detect-changes.outputs.changed-jobs, '|test-wasm|')")
    runs, _ = evaluate_if(gate64, {}, changed_jobs="|test-wasm64|")
    check("test-wasm64 does not trigger test-wasm", runs, False)

    # The motivating defect, pinned (AGENTS.md 8.12.3): a declared-but-unread
    # output must fail, or this gate is measuring the wrong invariant.
    synthetic_jobs = {"a": {"if": "needs.detect-changes.outputs.rust-src == 'true'"}}
    errs = check_outputs_consumed(synthetic_jobs, {"rust-src": "x", "docs": "y"})
    check("dead output is caught", len(errs), 1)
    check("live output is not flagged", len(check_outputs_consumed(synthetic_jobs, {"rust-src": "x"})), 0)
    # an ungated job must fail
    check("ungated job is caught", len(check_if_shape({"newjob": {}})), 1)

    if failures:
        for f in failures:
            print(f"::error::check_ci_filters self-test: {f}")
        print(f"self-test FAILED ({len(failures)} case(s))")
        return 1
    print("check_ci_filters --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--self-test", action="store_true", help="run the gate's own unit cases")
    ap.add_argument("--write-snapshot", action="store_true",
                    help="regenerate .github/ci-jobs.txt from ci.yml")
    args = ap.parse_args()
    if args.self_test:
        return self_test()

    _, jobs, filters, declared_outputs = load_ci()
    if args.write_snapshot:
        JOB_SNAPSHOT.write_text("\n".join(sorted(jobs)) + "\n")
        print(f"wrote {JOB_SNAPSHOT} ({len(jobs)} jobs)")
        return 0
    errs = []
    errs += check_job_snapshot(jobs)
    errs += check_if_shape(jobs)
    errs += check_outputs_consumed(jobs, declared_outputs)
    errs += check_golden_table(jobs, filters)
    if errs:
        for e in errs:
            print(f"::error::{e}")
        print(f"\nci.yml path filters: {len(errs)} problem(s)")
        return 1
    print(
        f"ci.yml path filters OK: {len(jobs)} jobs, {len(filters)} filters, "
        f"{len(GOLDEN_CASES)} golden cases"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
