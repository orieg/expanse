#!/usr/bin/env python3
"""Documentation hygiene checks for AGENTS.md rules that had no enforcement.

Fatal (exit 1):
  * time estimates / calendar durations in prose (AGENTS.md §6: "No time
    estimates in pull requests, comments, or documentation");
  * PII / local-infrastructure identifiers (AGENTS.md §7 "Privacy & Local
    Infrastructure"): home-directory paths, private LAN IPv4 addresses, and
    any hostname listed in the DOCS_HOSTNAME_DENYLIST environment variable
    (comma-separated; kept out of the repository on purpose — set it as a
    repository variable, never commit it).

Advisory (GitHub `::warning::` annotations, exit 0):
  * documents that publish unit-bearing numbers (ns, ops/s, B/key, ×) in tables
    while carrying no `(measured: …)` / `(target…)` / `(projected…)` provenance
    tag anywhere in the file (AGENTS.md §6 / §8.7).

    Deliberately file-scoped, not per-table. The suite READMEs state provenance
    once in a header that covers every table below it — often far more than a
    screenful up — so a proximity rule reports those as violations and trains
    readers to ignore the warning. What is worth flagging is a document that
    publishes numbers with no provenance at all; whether a given table sits
    under the right tag is a reviewer's judgement, not a regex's.

Fenced code blocks are skipped for every check. A line may opt out of the
fatal checks with the literal marker `docs-lint: allow` (use sparingly, and
say why in the surrounding prose).

Usage:
  python3 scripts/check_docs_hygiene.py                # scan tracked markdown
  python3 scripts/check_docs_hygiene.py --pr-body-file pr-body.txt
  python3 scripts/check_docs_hygiene.py --self-test
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

ALLOW_MARKER = "docs-lint: allow"

# --- fatal: time estimates ---------------------------------------------------
_NUM = r"(?:\d+(?:\s*[-–]\s*\d+)?|a|an|one|two|three|four|five|six|several|few|couple of)"
TIME_ESTIMATE_PATTERNS = [
    # "1-2 days", "a week", "~3 weeks", "≤2 weeks", "two sprints", "10 engineer-days"
    re.compile(
        rf"(?<![\w/.-])(?:~|≈|≤|<=|about |roughly |approximately )?{_NUM}\s*"
        r"(?:engineer-|person-|dev-)?(?:days?|weeks?|sprints?|months?|quarters?)\b(?!-old)",
        re.IGNORECASE,
    ),
    re.compile(r"\b(?:next|this|one|per)\s+sprint\b", re.IGNORECASE),
    re.compile(r"\bby\s+(?:monday|tuesday|wednesday|thursday|friday|end of (?:the )?(?:week|month|quarter))\b", re.IGNORECASE),
    re.compile(r"\b(?:in|by|during)\s+Q[1-4](?:\s*20\d\d)?\b"),
    re.compile(r"\bover the weekend\b", re.IGNORECASE),
]
# Phrases that look like durations but are measurement or retention facts, not plans.
TIME_ESTIMATE_EXEMPT = re.compile(
    r"\b(?:retention|retained|expires?|cache(?:d)?|TTL|soak|uptime|window|timeout|"
    r"sleep|every|per day|per week|per month|days? old|weeks? old|a day ago|"
    r"shipped a day|24-hour|nightly)\b",
    re.IGNORECASE,
)

# --- fatal: PII / local infrastructure ---------------------------------------
PII_PATTERNS = [
    ("home-directory path", re.compile(r"/Users/(?!<)[A-Za-z0-9_.-]+/")),
    ("home-directory path", re.compile(r"/home/(?!<|runner\b|\$)[A-Za-z0-9_.-]+/")),
    ("private LAN IPv4", re.compile(r"\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3})\b")),
]

# --- advisory: provenance ----------------------------------------------------
UNIT_TOKEN = re.compile(
    r"\d+(?:[.,]\d+)?\s*(?:ns|µs|us|ms|ops/s|Mops/s|M ops/s|M/s|B/key|B/k|bytes/key|GB|MB|KiB|MiB)\b|\d+(?:\.\d+)?\s*×",
)
PROVENANCE_TAG = re.compile(r"\((?:measured|target|projected|unverified|retracted|pending)", re.IGNORECASE)
PROVENANCE_FILES = ("docs/BENCHMARKING.md", "README.md", "docs/DATABASE.md", "docs/BINDINGS_BENCHMARKS.md")
PROVENANCE_GLOBS = ("docs/benchmarks/*/README.md",)


def tracked_markdown(root: Path) -> list[Path]:
    out = subprocess.run(
        ["git", "ls-files", "--", "*.md", "docs/**/*.md", ".github/**/*.md"],
        cwd=root, capture_output=True, text=True, check=True,
    ).stdout.split()
    files = sorted({root / f for f in out if f.endswith(".md")})
    return [f for f in files if "/worktrees/" not in str(f) and "/target/" not in str(f)]


def strip_fences(lines: list[str]) -> list[tuple[int, str]]:
    """Return (1-based line number, text) for lines outside fenced code blocks."""
    kept, in_fence = [], False
    for i, line in enumerate(lines, 1):
        if line.lstrip().startswith("```") or line.lstrip().startswith("~~~"):
            in_fence = not in_fence
            continue
        if not in_fence:
            kept.append((i, line))
    return kept


def check_time_estimates(lines: list[tuple[int, str]]) -> list[tuple[int, str]]:
    hits = []
    for n, line in lines:
        if ALLOW_MARKER in line:
            continue
        for pat in TIME_ESTIMATE_PATTERNS:
            m = pat.search(line)
            if not m:
                continue
            ctx = line[max(0, m.start() - 40): m.end() + 40]
            if TIME_ESTIMATE_EXEMPT.search(ctx):
                continue
            hits.append((n, m.group(0).strip()))
            break
    return hits


def check_pii(lines: list[tuple[int, str]], denylist: list[str]) -> list[tuple[int, str]]:
    hits = []
    host_pats = [re.compile(rf"(?<![\w.-]){re.escape(h)}(?![\w-])", re.IGNORECASE) for h in denylist if h]
    for n, line in lines:
        if ALLOW_MARKER in line:
            continue
        for label, pat in PII_PATTERNS:
            if pat.search(line):
                hits.append((n, label))
                break
        else:
            for pat in host_pats:
                if pat.search(line):
                    hits.append((n, "denylisted hostname"))  # never echo the hostname
                    break
    return hits


def check_provenance(lines: list[str]) -> list[int]:
    """Advisory: file-scoped. Returns the first unit-bearing table's line number
    when the document carries no provenance tag anywhere, else nothing."""
    if any(PROVENANCE_TAG.search(l) for l in lines):
        return []
    i, n = 0, len(lines)
    while i < n:
        if lines[i].lstrip().startswith("|"):
            start = i
            while i < n and lines[i].lstrip().startswith("|"):
                i += 1
            if any(UNIT_TOKEN.search(l) for l in lines[start:i]):
                return [start + 1]
        else:
            i += 1
    return []


def scan_text(path_label: str, text: str, denylist: list[str], provenance: bool) -> tuple[int, int]:
    lines = text.splitlines()
    kept = strip_fences(lines)
    fatal = 0
    for n, what in check_time_estimates(kept):
        print(f"::error file={path_label},line={n}::time estimate in prose ({what!r}) — AGENTS.md §6 forbids durations; state ordering/gates instead")
        fatal += 1
    for n, what in check_pii(kept, denylist):
        print(f"::error file={path_label},line={n}::{what} — AGENTS.md §7 forbids PII / local-infrastructure identifiers")
        fatal += 1
    warnings = 0
    if provenance:
        for n in check_provenance(lines):
            print(f"::warning file={path_label},line={n}::this document publishes unit-bearing numbers but carries no (measured: host, commit) / (target) / (projected) tag anywhere — AGENTS.md §8.7 (advisory; first such table shown)")
            warnings += 1
    return fatal, warnings


def wants_provenance(root: Path, path: Path) -> bool:
    rel = path.relative_to(root).as_posix()
    if rel in PROVENANCE_FILES:
        return True
    return any(path.match(g) for g in PROVENANCE_GLOBS)


def self_test() -> int:
    deny = ["examplehost"]
    fatal, _ = scan_text("t.md", "Ship v0.1 (1-2 days).\n", deny, False); assert fatal == 1, "duration"
    fatal, _ = scan_text("t.md", "Step 1 — ships when boundary-F1 ≥ 0.8.\n", deny, False); assert fatal == 0, "gate wording"
    fatal, _ = scan_text("t.md", "The nightly cache has a 7 days retention.\n", deny, False); assert fatal == 0, "retention exempt"
    fatal, _ = scan_text("t.md", "```\n1-2 days\n```\n", deny, False); assert fatal == 0, "fence skipped"
    fatal, _ = scan_text("t.md", "rsync ./ /Users/someone/repo/\n", deny, False); assert fatal == 1, "home path"
    fatal, _ = scan_text("t.md", "use $HOME/.cargo or /home/<user>/x\n", deny, False); assert fatal == 0, "placeholder home"
    fatal, _ = scan_text("t.md", "ssh examplehost 'cargo bench'\n", deny, False); assert fatal == 1, "denylisted host"
    fatal, _ = scan_text("t.md", "connect to 192.168.1.20\n", deny, False); assert fatal == 1, "lan ip"
    fatal, _ = scan_text("t.md", "planned for 2 weeks docs-lint: allow\n", deny, False); assert fatal == 0, "allow marker"
    _, w = scan_text("t.md", "| arm | ns |\n|---|---|\n| a | 35.8 ns |\n", deny, True); assert w == 1, "provenance warn"
    _, w = scan_text("t.md", "*(measured: host, commit)*\n| arm | ns |\n|---|---|\n| a | 35.8 ns |\n", deny, True); assert w == 0, "provenance tagged"
    # File-scoped: one tag covers tables far below it, however many.
    far = "*(measured: host, commit)*\n" + ("filler\n" * 200) + "| arm | ns |\n|---|---|\n| a | 35.8 ns |\n"
    _, w = scan_text("t.md", far, deny, True); assert w == 0, "distant tag still covers"
    two = "| a | ns |\n|---|---|\n| x | 1 ns |\n\ntext\n\n| b | ns |\n|---|---|\n| y | 2 ns |\n"
    _, w = scan_text("t.md", two, deny, True); assert w == 1, "one warning per untagged file, not per table"
    _, w = scan_text("t.md", "| cap | value |\n|---|---|\n| ordered | yes |\n", deny, True); assert w == 0, "no unit-bearing cell"
    print("check_docs_hygiene.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--pr-body-file", help="also scan this file as PR-body prose (fatal checks only)")
    ap.add_argument("--no-provenance", action="store_true", help="skip the advisory provenance pass")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()

    root = Path(subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip())
    denylist = [h.strip() for h in os.environ.get("DOCS_HOSTNAME_DENYLIST", "").split(",") if h.strip()]
    if not denylist:
        print("::notice::DOCS_HOSTNAME_DENYLIST is unset — hostname check skipped (set it as a repository variable; never commit it)")

    fatal = warnings = 0
    for path in tracked_markdown(root):
        f, w = scan_text(path.relative_to(root).as_posix(), path.read_text(encoding="utf-8", errors="replace"), denylist, provenance=not args.no_provenance and wants_provenance(root, path))
        fatal += f
        warnings += w
    if args.pr_body_file and os.path.exists(args.pr_body_file):
        f, _ = scan_text("PR body", Path(args.pr_body_file).read_text(encoding="utf-8", errors="replace"), denylist, provenance=False)
        fatal += f

    print(f"check_docs_hygiene.py: {fatal} fatal finding(s), {warnings} advisory warning(s)")
    return 1 if fatal else 0


if __name__ == "__main__":
    sys.exit(main())
