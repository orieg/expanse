#!/usr/bin/env python3
"""Generated benchmark tables must still match the READMEs that quote them (#736).

`hot_comparison` and `masstree_comparison` derive their tables from committed
`results/*.json` with a generator (`string_tables.py`, `tables.py`) and put the
output in a hand-written README. Nothing checked that the rows in the README
were still the rows the generator produces, and every re-measurement redid the
paste by hand. `art_comparison` avoids the class by generating its whole README
(`generate_readme.py`) at the cost of keeping prose in a Python file.

What this checks, per declared suite: every table row the generator emits
appears verbatim in the suite README, compared with internal whitespace
collapsed so a re-wrap of a cell's padding is not a finding but a changed digit
is. Headings are skipped — the READMEs demote the generator's heading level on
purpose — and so are HTML comments and the trailing load-average footer, which
name the run rather than a result. Prose the generator emits must appear too,
matched against the README with all whitespace collapsed, so a caption may be
re-wrapped but not reworded.

## Why `--write` only ever replaces table runs

The defect that motivated this gate is a splice that reverted a corrected prose
paragraph during the Masstree re-measurement (`83b0027f` -> `51c770c1`). A
splice that replaces everything between a heading and the last table row of its
section can do that again. So `--write` finds the maximal runs of consecutive
table lines inside a matched section and replaces them pairwise with the
generator's runs; it never writes a line that is not a table row, and it
refuses (non-zero) when the run counts differ rather than guessing. Prose is
fixed by hand, and this script tells you which caption drifted.

## Why one suite is declared and not enforced

`masstree_comparison`'s README is a verbatim paste and matches its generator
row for row. `hot_comparison`'s is not: its string section (README §6)
reorganises the generator's flat per-pillar tables into narrative subsections,
keeps the N = 1,000,000 rows, folds the two withheld `beyond` cells into one
row and annotates a scorecard count. That is a curated presentation of the same
artifact, not a stale paste, and re-splicing it from today's generator would
delete the analysis rather than fix a drift. It is declared `PENDING` with the
issue that brings its tables under the generator, and reported on every run as
a named degradation -- never as a pass (AGENTS.md section 8.1).

Usage:
  python3 scripts/check_readme_tables.py            # check every declared suite
  python3 scripts/check_readme_tables.py --write    # re-splice enforced suites
  python3 scripts/check_readme_tables.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BENCH = REPO_ROOT / "docs" / "benchmarks"

# Lines the generators emit that name the run rather than a result, so a README
# is not required to carry them.
FOOTER_PREFIXES = ("load average across the run", "load average across the main run",
                   "load average across the concurrent run")


class Suite:
    """One declared suite: a generator, the README it feeds, and its status."""

    def __init__(self, name: str, generator: str, enforce: bool, pending_issue: int | None = None,
                 reason: str = ""):
        self.name = name
        self.generator = BENCH / name / "scripts" / generator
        self.readme = BENCH / name / "README.md"
        self.enforce = enforce
        self.pending_issue = pending_issue
        self.reason = reason


SUITES = [
    Suite("masstree_comparison", "tables.py", enforce=True),
    Suite(
        "hot_comparison", "string_tables.py", enforce=False, pending_issue=733,
        reason="README section 6 curates the generator's flat per-pillar tables into "
               "narrative subsections; the curation moves into the generator when the "
               "suite is re-spliced from its re-measured artifacts",
    ),
]


def norm(line: str) -> str:
    """Collapse internal whitespace so cell padding is not a finding."""
    return " ".join(line.split())


def is_table_row(line: str) -> bool:
    return line.lstrip().startswith("|")


def is_heading(line: str) -> bool:
    return bool(re.match(r"^#{1,6}\s", line.lstrip()))


def skippable(line: str) -> bool:
    n = norm(line)
    if not n or n.startswith("<!--") or is_heading(line):
        return True
    return any(n.startswith(p) for p in FOOTER_PREFIXES)


def run_generator(suite: Suite) -> str:
    """Runs a suite's generator. Fails loud (section 8.1) — never returns partial output."""
    if not suite.generator.is_file():
        raise SystemExit(f"{suite.name}: generator {suite.generator} does not exist")
    proc = subprocess.run([sys.executable, str(suite.generator)],
                          capture_output=True, text=True, cwd=REPO_ROOT)
    if proc.returncode != 0:
        raise SystemExit(
            f"{suite.name}: generator exited {proc.returncode}; no table can be checked\n"
            f"{proc.stderr.strip()}"
        )
    if not proc.stdout.strip():
        raise SystemExit(f"{suite.name}: generator produced no output")
    return proc.stdout


def sections(generated: str) -> list[tuple[str, list[str]]]:
    """Splits generator output into (heading text, body lines) blocks."""
    out: list[tuple[str, list[str]]] = []
    heading, body = "", []
    for line in generated.splitlines():
        if is_heading(line):
            if heading or body:
                out.append((heading, body))
            heading, body = norm(line).lstrip("# "), []
        else:
            body.append(line)
    if heading or body:
        out.append((heading, body))
    return out


def check(suite: Suite, generated: str) -> list[str]:
    """Returns a list of findings, first drifted section first."""
    readme = suite.readme.read_text()
    readme_rows = {norm(l) for l in readme.splitlines() if is_table_row(l)}
    readme_blob = " ".join(readme.split())

    findings: list[str] = []
    for heading, body in sections(generated):
        rows = missing_rows = 0
        first_bad = None
        for line in body:
            if skippable(line):
                continue
            n = norm(line)
            if is_table_row(line):
                rows += 1
                if n not in readme_rows:
                    missing_rows += 1
                    if first_bad is None:
                        first_bad = f"row not in README: {n}"
            elif n not in readme_blob:
                if first_bad is None:
                    first_bad = f"caption not in README: {n[:120]}"
        if first_bad:
            where = heading or "(preamble)"
            findings.append(
                f"{suite.name}: section {where!r} drifted "
                f"({missing_rows} of {rows} rows absent) — {first_bad}"
            )
    return findings


def table_runs(lines: list[str]) -> list[tuple[int, int]]:
    """Maximal runs of consecutive table lines, as (start, end_exclusive)."""
    runs, i = [], 0
    while i < len(lines):
        if is_table_row(lines[i]):
            j = i
            while j < len(lines) and is_table_row(lines[j]):
                j += 1
            runs.append((i, j))
            i = j
        else:
            i += 1
    return runs


def splice(suite: Suite, generated: str) -> int:
    """Replaces each README table run with the generator's, prose untouched.

    Returns the number of runs rewritten. Refuses (SystemExit) when a matched
    section's run count differs from the generator's, rather than guessing which
    table replaces which.
    """
    readme_lines = suite.readme.read_text().splitlines()
    rewritten = 0
    for heading, body in sections(generated):
        if not heading:
            continue
        idx = None
        for k, line in enumerate(readme_lines):
            if is_heading(line) and norm(line).lstrip("# ") == heading:
                idx = k
                break
        if idx is None:
            continue
        end = len(readme_lines)
        for k in range(idx + 1, len(readme_lines)):
            if is_heading(readme_lines[k]):
                end = k
                break
        region = readme_lines[idx + 1:end]
        want = [b for b in body]
        r_runs, g_runs = table_runs(region), table_runs(want)
        if len(r_runs) != len(g_runs):
            raise SystemExit(
                f"{suite.name}: section {heading!r} has {len(r_runs)} table run(s) in the "
                f"README and {len(g_runs)} in the generator; refusing to guess which "
                f"replaces which — fix the section by hand"
            )
        for (rs, re_), (gs, ge) in zip(reversed(r_runs), reversed(g_runs)):
            region[rs:re_] = want[gs:ge]
            rewritten += 1
        readme_lines[idx + 1:end] = region
    suite.readme.write_text("\n".join(readme_lines) + "\n")
    return rewritten


# --------------------------------------------------------------------------
# self-test: the motivating defect, pinned (AGENTS.md section 8.12)
# --------------------------------------------------------------------------

_GEN = """<!-- generated by scripts/tables.py from results/ at commit deadbeef -->

### Memory, integer map

`allocator` is what the process holds from the C allocator.

| Distribution | N | Masstree | Expanse |
|---|---:|---:|---:|
| `random` | 65,536 | 32.19 | 23.88 |
| `random` | 131,072 | 32.09 | 24.81 |

load average across the run: 0.92, 0.94; core pin: 0-15
"""

_README_OK = """# Suite

## 4. Memory

#### Memory, integer map

`allocator` is what the process holds from the C
allocator.

| Distribution | N | Masstree | Expanse |
|---|---:|---:|---:|
| `random` | 65,536 | 32.19 | 23.88 |
| `random` | 131,072 | 32.09 | 24.81 |

Some prose that the generator does not emit and must survive a splice.
"""

# One cell stale: 23.88 was re-measured and the README kept the old digit.
_README_STALE = _README_OK.replace("| `random` | 65,536 | 32.19 | 23.88 |",
                                   "| `random` | 65,536 | 32.19 | 23.87 |")

_README_REWORDED = _README_OK.replace("`allocator` is what the process holds from the C\nallocator.",
                                      "`allocator` is what the process holds.")


def _self_test() -> int:
    import tempfile

    failures = []

    def make(text: str) -> Suite:
        d = Path(tempfile.mkdtemp())
        (d / "README.md").write_text(text)
        s = Suite("fixture", "none.py", enforce=True)
        s.readme = d / "README.md"
        return s

    # 1. A README that carries every generated row passes.
    if check(make(_README_OK), _GEN):
        failures.append("a matching README was reported as drifted")

    # 2. THE MOTIVATING DEFECT, verbatim: one stale cell must fail. A gate that
    #    passes here is measuring the wrong invariant (AGENTS.md section 8.12).
    f = check(make(_README_STALE), _GEN)
    if not f:
        failures.append("a README with one stale cell (23.88 -> 23.87) passed")
    elif "23.88" not in f[0]:
        failures.append(f"the stale-cell finding does not name the expected row: {f[0]}")

    # 3. A re-wrapped caption passes; a reworded one does not.
    if check(make(_README_REWORDED), _GEN) == []:
        failures.append("a reworded caption passed")

    # 4. The footer and the HTML comment are not required in the README.
    ok = make(_README_OK)
    if any("load average" in x for x in check(ok, _GEN)):
        failures.append("the load-average footer was required in the README")

    # 5. THE SECOND MOTIVATING DEFECT: a splice must not touch prose. The
    #    Masstree re-measurement's splice reverted a corrected paragraph
    #    (83b0027f -> 51c770c1); --write may only rewrite table rows.
    stale = make(_README_STALE)
    marker = "Some prose that the generator does not emit and must survive a splice."
    n = splice(stale, _GEN)
    after = stale.readme.read_text()
    if n != 1:
        failures.append(f"splice rewrote {n} run(s), expected 1")
    if marker not in after:
        failures.append("splice deleted prose the generator does not emit")
    if "23.87" in after or "| `random` | 65,536 | 32.19 | 23.88 |" not in after:
        failures.append("splice did not restore the stale cell")
    if check(stale, _GEN):
        failures.append("a spliced README still reports drift")

    # 6. A section whose run count disagrees is refused, never guessed.
    two_runs = make(_README_OK.replace(
        "Some prose that the generator does not emit and must survive a splice.",
        "| extra | table |\n|---|---|\n| run | here |"))
    try:
        splice(two_runs, _GEN)
        failures.append("a mismatched table-run count was spliced instead of refused")
    except SystemExit:
        pass

    for msg in failures:
        print(f"  FAIL {msg}")
    if failures:
        print(f"check_readme_tables.py --self-test: {len(failures)} failure(s)")
        return 1
    print("check_readme_tables.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--write", action="store_true",
                    help="re-splice enforced suites' table runs from their generators")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return _self_test()

    findings, degraded = [], []
    for suite in SUITES:
        generated = run_generator(suite)
        if args.write and suite.enforce:
            n = splice(suite, generated)
            print(f"check_readme_tables.py: {suite.name}: rewrote {n} table run(s)")
        result = check(suite, generated)
        if not suite.enforce:
            # Never rendered as a pass (AGENTS.md section 8.1): the degradation
            # is named, counted and carries the issue that removes it.
            degraded.append((suite, result))
            continue
        findings.extend(result)

    for suite, result in degraded:
        print(f"::notice::check_readme_tables.py: {suite.name} is DECLARED, NOT ENFORCED — "
              f"{len(result)} section(s) differ from its generator. {suite.reason}. "
              f"Tracked by #{suite.pending_issue}.")
        for line in result:
            print(f"    unenforced: {line}")

    if findings:
        for line in findings:
            print(f"::error::{line}")
        print(f"check_readme_tables.py: {len(findings)} drifted section(s) — "
              f"regenerate with --write, or fix the caption by hand")
        return 1

    enforced = [s.name for s in SUITES if s.enforce]
    print(f"check_readme_tables.py: {len(enforced)} suite(s) enforced and in sync "
          f"({', '.join(enforced)}); {len(degraded)} declared and not enforced")
    return 0


if __name__ == "__main__":
    sys.exit(main())
