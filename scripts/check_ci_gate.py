import sys
import re

def main():
    try:
        with open('.github/workflows/ci.yml', 'r') as f:
            lines = f.readlines()
    except Exception as e:
        print(f"Error reading ci.yml: {e}")
        sys.exit(1)

    jobs = set()
    ci_gate_needs = set()
    
    in_jobs = False
    in_ci_gate = False
    in_needs = False

    for line in lines:
        if line.startswith('jobs:'):
            in_jobs = True
            continue
            
        if not in_jobs:
            continue
            
        # Match a top-level job like `  detect-changes:`
        job_match = re.match(r'^  ([a-zA-Z0-9_-]+):', line)
        if job_match:
            job_name = job_match.group(1)
            jobs.add(job_name)
            if job_name == 'ci-gate':
                in_ci_gate = True
            else:
                in_ci_gate = False
            continue
            
        if in_ci_gate:
            if re.match(r'^    needs:$', line) or re.match(r'^    needs: \s*$', line):
                in_needs = True
                continue
            elif re.match(r'^    [a-zA-Z]', line):
                in_needs = False
                
            if in_needs:
                need_match = re.match(r'^      - ([a-zA-Z0-9_-]+)', line)
                if need_match:
                    ci_gate_needs.add(need_match.group(1))

    if 'ci-gate' not in jobs:
        print("Error: ci-gate job not found")
        sys.exit(1)
        
    expected_needs = jobs - {'detect-changes', 'ci-gate'}
    missing = expected_needs - ci_gate_needs
    
    if missing:
        print(f"Error: The following jobs are missing from ci-gate needs: {missing}")
        sys.exit(1)

    if not check_documented_job_count(jobs):
        sys.exit(1)

    if not check_filter_quantifier(''.join(lines)):
        sys.exit(1)

    print("ci-gate needs are up-to-date.")
    sys.exit(0)


# docs/CI.md owns the job catalogue (AGENTS.md section 1), and its opening
# sentence states how many jobs `ci.yml` defines. That number drifted to 36
# while the workflow carried 40, because adding a job means editing `needs:`
# -- which this script already checks -- and editing a sentence, which nothing
# did. A reader counting on the catalogue being complete had no way to know
# five jobs were missing from it. Deriving the count from the workflow rather
# than pinning it here keeps the assertion honest: the workflow is the source
# of truth and the sentence has to follow it.
DOC_PATH = 'docs/CI.md'
COUNT_RE = re.compile(
    r'`ci\.yml` defines \*\*(\d+) jobs\*\* [^.]*?(\d+) verification jobs'
)


def check_documented_job_count(jobs):
    try:
        with open(DOC_PATH, 'r') as f:
            doc = f.read()
    except OSError as e:
        # Fail closed: a missing catalogue is a problem, not a reason to pass.
        print(f"Error reading {DOC_PATH}: {e}")
        return False

    m = COUNT_RE.search(doc)
    if not m:
        print(
            f"Error: {DOC_PATH} no longer states a job count in the expected form "
            "(\"`ci.yml` defines **N jobs** - M verification jobs plus the `ci-gate` "
            "rollup\"). Restore the sentence or update COUNT_RE alongside it."
        )
        return False

    doc_total, doc_verification = int(m.group(1)), int(m.group(2))
    actual_total = len(jobs)
    actual_verification = actual_total - 1  # every job except the `ci-gate` rollup

    if (doc_total, doc_verification) != (actual_total, actual_verification):
        print(
            f"Error: {DOC_PATH} says ci.yml defines {doc_total} jobs "
            f"({doc_verification} verification + ci-gate), but it defines "
            f"{actual_total} ({actual_verification} verification + ci-gate). "
            "Update the sentence and add the new job to the catalogue table below it."
        )
        return False

    return True


# `dorny/paths-filter` treats a `!`-prefixed pattern as a picomatch NEGATION
# OR'd with the filter's other patterns, not as an exclusion, unless the step
# passes `predicate-quantifier: 'some-with-excludes'`. So a lone
# `- '!crates/expanse-hot-bench/**'` under the default quantifier does not
# exclude that crate -- it matches every path OUTSIDE it, i.e. almost the whole
# repo. #671 added exactly that to `rust-src`, which gates the safety lane and
# the cross-compile matrix, and every PR from then until the quantifier was
# added ran the full matrix: a docs-only PR skipped 37 of 40 jobs the hour
# before #671 merged and 0 of 69 after. Nothing failed, so nothing surfaced it.
#
# The failure is silent and costs only runner time, which is why it needs a
# gate rather than a comment (AGENTS.md section 8.18): the next `!` pattern
# added without the quantifier would be just as invisible.
QUANTIFIER = "predicate-quantifier: 'some-with-excludes'"
_FILTER_STEP_RE = re.compile(r'uses:\s*dorny/paths-filter@')


def check_filter_quantifier(text, label='.github/workflows/ci.yml'):
    """Fail if a paths-filter step has a `!` pattern but not the quantifier.

    Scoped to the step: the input and the patterns must belong to the same
    `dorny/paths-filter` step, so a quantifier on one step cannot vouch for a
    negation in another.
    """
    steps = []
    for m in _FILTER_STEP_RE.finditer(text):
        start = m.start()
        nxt = _FILTER_STEP_RE.search(text, m.end())
        steps.append(text[start:nxt.start() if nxt else len(text)])

    ok = True
    for i, step in enumerate(steps, 1):
        # A negated pattern in a `filters:` list, quoted or bare, ignoring the
        # comment lines that explain it.
        negated = [
            ln.strip() for ln in step.splitlines()
            if re.match(r"^\s*-\s*['\"]?!", ln)
        ]
        if not negated:
            continue
        if QUANTIFIER not in step:
            print(
                f"Error: paths-filter step {i} in {label} has "
                f"negated pattern(s) {negated} but no `{QUANTIFIER}`. Under the "
                "action's default `some`, a `!` pattern is a negation OR'd with "
                "the rest -- it matches everything OUTSIDE the excluded path, so "
                "the filter is true for nearly every diff and gates nothing. Add "
                "the input to the step, or drop the negation."
            )
            ok = False
    return ok


SELF_TEST_CASES = [
    # (name, workflow fragment, expected pass)
    #
    # The verbatim #671 defect. If this case does not fail, the gate is not
    # measuring the thing it was written for (AGENTS.md section 8.12.3).
    ("historical #671 defect: negation, default quantifier", """
      - uses: dorny/paths-filter@v4
        id: filter
        with:
          filters: |
            rust-src:
              - 'crates/**'
              - '!crates/expanse-hot-bench/**'
              - 'include/**'
""", False),
    ("negation with the quantifier", """
      - uses: dorny/paths-filter@v4
        id: filter
        with:
          predicate-quantifier: 'some-with-excludes'
          filters: |
            rust-src:
              - 'crates/**'
              - '!crates/expanse-hot-bench/**'
""", True),
    ("no negation, no quantifier -- unchanged behaviour, must pass", """
      - uses: dorny/paths-filter@v4
        with:
          filters: |
            docs:
              - '**/*.md'
""", True),
    ("bare (unquoted) negation is caught too", """
      - uses: dorny/paths-filter@v4
        with:
          filters: |
            rust-src:
              - crates/**
              - !crates/expanse-hot-bench/**
""", False),
    ("a comment mentioning a `!` pattern is not a pattern", """
      - uses: dorny/paths-filter@v4
        with:
          filters: |
            # note: '!crates/foo/**' would need the quantifier
            rust-src:
              - 'crates/**'
""", True),
    ("quantifier on a later step does not vouch for an earlier negation", """
      - uses: dorny/paths-filter@v4
        with:
          filters: |
            a:
              - '!x/**'
      - uses: dorny/paths-filter@v4
        with:
          predicate-quantifier: 'some-with-excludes'
          filters: |
            b:
              - '!y/**'
""", False),
]


def self_test():
    failures = 0
    for name, fragment, expected in SELF_TEST_CASES:
        got = check_filter_quantifier(fragment, label=f'<self-test: {name}>')
        status = "ok" if got == expected else "FAILED"
        if got != expected:
            failures += 1
        print(f"  [{status}] {name} (expected {expected}, got {got})")
    if failures:
        print(f"check_ci_gate.py --self-test: {failures} case(s) failed")
        sys.exit(1)
    print(f"check_ci_gate.py --self-test: {len(SELF_TEST_CASES)} cases passed")
    sys.exit(0)


if __name__ == '__main__':
    if '--self-test' in sys.argv:
        self_test()
    main()
