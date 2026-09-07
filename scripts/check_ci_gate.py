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


if __name__ == '__main__':
    main()
