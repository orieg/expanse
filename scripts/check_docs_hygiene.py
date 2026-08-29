#!/usr/bin/env python3
"""Documentation hygiene checks for AGENTS.md rules that had no enforcement.

Fatal (exit 1):
  * time estimates / calendar durations in prose (AGENTS.md §6: "No time
    estimates in pull requests, comments, or documentation");
  * PII / local-infrastructure identifiers (AGENTS.md §7 "Privacy & Local
    Infrastructure"): home-directory paths, private LAN IPv4 addresses, and
    any hostname listed in the DOCS_HOSTNAME_DENYLIST environment variable
    (comma-separated; kept out of the repository on purpose — set it as a
    repository *secret*, never commit it, and never echo it: matches are
    reported as "denylisted hostname" without the value).

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
PROVENANCE_FILES = (
    "docs/BENCHMARKING.md",
    "README.md",
    "docs/DATABASE.md",
    "docs/BINDINGS_BENCHMARKS.md",
    "docs/ARCHITECTURE.md",
    "docs/ALGORITHMS.md",
    "docs/design/large-values.md",
)
PROVENANCE_GLOBS = ("docs/benchmarks/*/README.md",)


# AGENTS.md 8.9: a microarchitectural mechanism named in prose is a claim about
# a counter, and a counter claim needs a counter. These are the terms that assert
# one. "cache" alone is not here - describing a cache hierarchy is not claiming a
# measurement of one; "cache-miss-bound" is.
MECHANISM_TERMS = re.compile(
    r"\b("
    r"memory[- ]latency[- ]bound|latency[- ]bound|bandwidth[- ]bound|"
    r"cache[- ]miss[- ]bound|miss[- ]bound|"
    r"branch[- ]misprediction|mispredict(?:s|ed|ion)?[- ]bound|"
    r"TLB[- ]bound|page[- ]walk[- ]bound|"
    r"memory[- ]level parallelism|MLP[- ]bound|"
    r"fill[- ]buffer[- ]bound|MSHR[- ]bound|"
    r"front[- ]end bound|back[- ]end bound|stall(?:ed|ing)? on (?:L[123]|DRAM|memory)"
    r")\b",
    re.IGNORECASE,
)

# Evidence that a counter was actually read. A provenance tag alone does not
# qualify - it says a number was measured, not that this mechanism was.
# Naming a mechanism in order to say something does NOT measure it is not a
# claim about that mechanism - it is a statement about an instrument's limits,
# which is the honest thing this rule wants more of, not less.
MECHANISM_NEGATION = re.compile(
    r"(cannot see|can(?:'|\u2019)?t see|does not (?:see|measure|model|capture)|"
    r"blind to|ignores?|invisible to|no counter|unable to (?:see|measure))",
    re.IGNORECASE,
)

MECHANISM_EVIDENCE = re.compile(
    r"(perf stat|perf_counters|point_lookup_counters|hardware counter|counters? (?:on|show|locate|say)|"
    r"branch-misses|branch_misses|L1-dcache|LLC-load|dTLB|cycle_activity|"
    r"mem_load_retired|cpu_core/|cpu_atom/|"
    r"--cache-sim|callgrind.*(?:LL|RAM) |"
    r"results/baseline_|"
    r"\bunmeasured\b|\bnot measured\b|\bhypothesis\b|\bunverified\b|"
    r"\bretracted\b|\bcause unknown\b|\bconjecture\b)",
    re.IGNORECASE,
)


# AGENTS.md 8.4: a claim over a continuous / sampling metric passes iff the BCa
# CI lower bound clears the floor, not iff the point estimate does. A published
# wall-clock ratio with no interval beside it is therefore an unfinished claim.
#
# Deliberately NARROW. It fires only on a ratio that is unambiguously about
# elapsed time or throughput - the same line must carry a wall-clock unit or a
# faster/slower word. Deterministic metrics are exempt by the same section:
# Callgrind instruction counts and byte-per-key accounting are exact integers
# with zero variance, so an interval on them would be meaningless.
WALLCLOCK_RATIO = re.compile(r"(?<![\w.])\d+(?:\.\d+)?\s*(?:x|×)\b", re.IGNORECASE)

WALLCLOCK_CONTEXT = re.compile(
    r"(\bns\b|µs|\bus\b|\bms\b|ops/s|Mops|M/s|latency|throughput|"
    r"faster|slower|speedup|wall.?clock)",
    re.IGNORECASE,
)

# Exact-by-construction metrics: an interval is not merely unnecessary, it is
# wrong. Callgrind counts and byte accounting do not vary between runs.
DETERMINISTIC_METRIC = re.compile(
    r"(instruction|callgrind|\bIr\b|B/key|B/k|bytes/key|byte accounting|"
    r"symbol|deterministic|inst\b|density|"
    r"memory|footprint|\bKB/|\bMB/|\bB/|resident|allocat|\bRAM\b|heap)",
    re.IGNORECASE,
)

# What discharges the claim: an interval, a committed artifact it re-derives
# from, or an explicit statement that no interval exists.
INTERVAL_EVIDENCE = re.compile(
    r"(\[\s*\d+(?:\.\d+)?\s*,\s*\d+(?:\.\d+)?\s*\]|"
    r"\bBCa\b|confidence interval|\bCI\b|bca_bootstrap|"
    r"results/baseline_|"
    r"\bno interval\b|\bunsourced\b|\bsuperseded\b|\bretracted\b|"
    r"\bindicative\b|\bunmeasured\b|\bprovisional\b|pending re-measurement)",
    re.IGNORECASE,
)


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


def check_mechanism_claims(lines: list[tuple[int, str]]) -> list[tuple[int, str]]:
    """A named microarchitectural mechanism must have counter evidence, or be
    explicitly marked unmeasured, within its own paragraph.

    Scoped to the paragraph rather than the line because the evidence is
    usually the sentence after the claim, and to the paragraph rather than the
    file because a document may measure one mechanism and speculate about
    another. Hedging counts as evidence: saying "hypothesis" or "cause unknown"
    is exactly the honest form this rule wants, so it must not be punished.
    """
    out: list[tuple[int, str]] = []
    paras: list[list[tuple[int, str]]] = []
    para: list[tuple[int, str]] = []
    for n, text in lines:
        if text.strip():
            para.append((n, text))
        else:
            if para:
                paras.append(para)
            para = []
    if para:
        paras.append(para)

    def flush_at(idx: int) -> None:
        para = paras[idx]
        # The claim's own paragraph plus the next one: an evidence table or a
        # counter list almost always follows the sentence that makes the claim,
        # and splitting them is a formatting choice, not a substantive one.
        window = para + (paras[idx + 1] if idx + 1 < len(paras) else [])
        joined = " ".join(t for _, t in window)
        if MECHANISM_EVIDENCE.search(joined) or MECHANISM_NEGATION.search(joined):
            return
        for n, text in para:
            m = MECHANISM_TERMS.search(text)
            if m:
                out.append((n, m.group(0)))
                return

    for idx in range(len(paras)):
        flush_at(idx)
    return out


def check_interval_claims(lines: list[tuple[int, str]]) -> list[tuple[int, str]]:
    """A published wall-clock ratio must carry an interval, an artifact it
    re-derives from, or an explicit statement that it has neither.

    Paragraph-scoped, matching the mechanism rule: the interval is usually the
    next cell or the sentence after. Deterministic metrics are skipped, since
    §8.4 makes an interval on an exact integer meaningless rather than missing.
    """
    out: list[tuple[int, str]] = []
    paras: list[list[tuple[int, str]]] = []
    para: list[tuple[int, str]] = []
    for n, text in lines:
        if text.strip():
            para.append((n, text))
        else:
            if para:
                paras.append(para)
            para = []
    if para:
        paras.append(para)

    for idx, para in enumerate(paras):
        window = para + (paras[idx + 1] if idx + 1 < len(paras) else [])
        joined = " ".join(t for _, t in window)
        if INTERVAL_EVIDENCE.search(joined):
            continue
        for n, text in para:
            if not WALLCLOCK_RATIO.search(text):
                continue
            if not WALLCLOCK_CONTEXT.search(text):
                continue
            if DETERMINISTIC_METRIC.search(text):
                continue
            m = WALLCLOCK_RATIO.search(text)
            out.append((n, m.group(0)))
            break
    return out


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
    for n, what in check_mechanism_claims(kept):
        print(f"::error file={path_label},line={n}::mechanism claim ({what!r}) with no counter evidence in its paragraph — AGENTS.md §8.9 wants a counter, a `results/baseline_*` reference, or an explicit 'unmeasured' / 'hypothesis' / 'cause unknown' qualifier")
        fatal += 1
    for n, what in check_interval_claims(kept):
        print(f"::error file={path_label},line={n}::wall-clock ratio ({what!r}) published with no interval in its paragraph — AGENTS.md §8.4 gates continuous metrics on the BCa CI lower bound; cite `[lo, hi]`, a `results/baseline_*` artifact, or state plainly that no interval exists")
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
    # §8.9 mechanism claims. The first case is the defect this rule exists for:
    # it shipped in docs/BENCHMARKING.md and was retracted in #456.
    fatal, _ = scan_text("t.md", "The arm is memory-latency-bound, so the work removed is off the critical path.\n", deny, False); assert fatal == 1, "mechanism claim without evidence must fire"
    fatal, _ = scan_text("t.md", "Callgrind cannot see TLB misses or memory-level parallelism.\n", deny, False); assert fatal == 0, "naming a mechanism an instrument cannot see is not a claim"
    fatal, _ = scan_text("t.md", "The cost is branch misprediction. Hardware counters locate it:\n\n| cpu_core/branch-misses/ | 2.4e7 |\n", deny, False); assert fatal == 0, "evidence in the following paragraph must count"
    fatal, _ = scan_text("t.md", "An MLP story predicts smooth growth; cause unknown.\n", deny, False); assert fatal == 0, "an honest hedge must not be punished"
    fatal, _ = scan_text("t.md", "```\nmemory-latency-bound\n```\n", deny, False); assert fatal == 0, "fenced code is exempt"
    # §8.4 wall-clock intervals. Narrow by design: deterministic metrics are
    # exempt because an interval on an exact integer is wrong, not missing.
    fatal, _ = scan_text("t.md", "Point lookups are 2.9x faster than BTreeMap at 1M keys.\n", deny, False); assert fatal == 1, "bare wall-clock ratio must fire"
    fatal, _ = scan_text("t.md", "Random 1M get is 1.031x, BCa 95% CI [1.024, 1.038].\n", deny, False); assert fatal == 0, "an interval discharges the claim"
    fatal, _ = scan_text("t.md", "libexpanse retires 0.55x the instructions of stock (Callgrind).\n", deny, False); assert fatal == 0, "instruction counts are exact"
    fatal, _ = scan_text("t.md", "Grammar masks: Roaring wins 2.66x lower RAM at 1.9M/s.\n", deny, False); assert fatal == 0, "memory ratios are exact accounting"
    fatal, _ = scan_text("t.md", "The layout is [values: u64 x C][keys: L x C] at 10 ns.\n", deny, False); assert fatal == 0, "a type layout is not a ratio"
    fatal, _ = scan_text("t.md", "It was 1.11x slower; that figure is superseded.\n", deny, False); assert fatal == 0, "an explicit retraction discharges it"
    print("check_docs_hygiene.py --self-test: all checks passed")
    return 0


# docs/architecture_visualizer.html is published to the SITE ROOT as
# visualizer.html (scripts/build_pages.py), and no markdown is copied beside
# it. A relative .md href therefore resolves to a 404 for every visitor, while
# still working when the file is opened from the docs/ directory locally --
# which is why five of them shipped. The links that broke were the provenance
# links out of the retraction disclaimer: the one path a skeptical reader has
# to check a withdrawn number.
RELATIVE_DOC_LINK = re.compile(r'href="(?!https?://|#|mailto:)([A-Za-z0-9_./-]+\.md)"')
ROOT_PUBLISHED_HTML = ("docs/architecture_visualizer.html",)


def check_published_html_links(root: Path) -> int:
    """Relative markdown links in root-published HTML are 404s once deployed."""
    fatal = 0
    for rel in ROOT_PUBLISHED_HTML:
        path = root / rel
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for num, line in enumerate(text.splitlines(), 1):
            for target in RELATIVE_DOC_LINK.findall(line):
                fatal += 1
                print(
                    f"::error file={rel},line={num}::relative link `{target}` 404s on the "
                    f"published site: this file deploys to the site root, where no markdown "
                    f"sits beside it. Use "
                    f"https://github.com/orieg/expanse/blob/main/docs/{target}"
                )
    return fatal


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
        print("::notice::DOCS_HOSTNAME_DENYLIST is unset — hostname check skipped (set it as a repository secret; never commit it)")

    fatal = warnings = 0
    for path in tracked_markdown(root):
        f, w = scan_text(path.relative_to(root).as_posix(), path.read_text(encoding="utf-8", errors="replace"), denylist, provenance=not args.no_provenance and wants_provenance(root, path))
        fatal += f
        warnings += w
    fatal += check_published_html_links(root)
    if args.pr_body_file and os.path.exists(args.pr_body_file):
        f, _ = scan_text("PR body", Path(args.pr_body_file).read_text(encoding="utf-8", errors="replace"), denylist, provenance=False)
        fatal += f

    print(f"check_docs_hygiene.py: {fatal} fatal finding(s), {warnings} advisory warning(s)")
    return 1 if fatal else 0


if __name__ == "__main__":
    sys.exit(main())
