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
        
    print("ci-gate needs are up-to-date.")
    sys.exit(0)

if __name__ == '__main__':
    main()
