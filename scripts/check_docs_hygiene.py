#!/usr/bin/env python3
"""Documentation hygiene checks for AGENTS.md rules that had no enforcement.

Fatal (exit 1):
  * time estimates / calendar durations in prose (AGENTS.md §6: "No time
    estimates in pull requests, comments, or documentation");
  * PII / local-infrastructure identifiers (AGENTS.md §7 "Privacy & Local
    Infrastructure"): home-directory paths, private LAN IPv4 addresses, and
    any hostname listed in the DOCS_HOSTNAME_DENYLIST environment variable;
  * unmeasured microarchitectural mechanism claims (AGENTS.md §8.9);
  * unpublished wall-clock intervals on continuous ratios (AGENTS.md §8.4);
  * superseded/retracted performance figures, strawman numbers, or refuted
    mechanism claims without retraction markers across all published surfaces
    (AGENTS.md §6.5 / §8.11, driven by .github/superseded-figures.json);
  * pending / unverified measurement statements that lack an open tracking
    issue citation or cite a closed issue (AGENTS.md §6.5 / §8.10).

Advisory (GitHub `::warning::` annotations, exit 0):
  * documents that publish unit-bearing numbers (ns, ops/s, B/key, ×) in tables
    while carrying no `(measured: …)` / `(target…)` / `(projected…)` provenance
    tag anywhere in the file (AGENTS.md §6 / §8.7).

Usage:
  python3 scripts/check_docs_hygiene.py                # scan all tracked surfaces
  python3 scripts/check_docs_hygiene.py --pr-body-file pr-body.txt
  python3 scripts/check_docs_hygiene.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

ALLOW_MARKER = "docs-lint: allow"

# --- fatal: time estimates ---------------------------------------------------
_NUM = r"(?:\d+(?:\s*[-–]\s*\d+)?|a|an|one|two|three|four|five|six|several|few|couple of)"
TIME_ESTIMATE_PATTERNS = [
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

# --- fatal: personal agent-config references ---------------------------------
#
# This repository is public and is read by several agent toolchains and by
# outside contributors. A rule that resolves only against one maintainer's home
# directory is unresolvable for all of them, and it publishes the local setup
# into a shipped artifact. Every such rule has an internal home: the AGENTS.md
# section that states it.
#
# Three of these accumulated because nothing looked for them, and one was in a
# C header -- which is why this check scans every tracked text file rather than
# only Markdown, unlike every other check in this script:
#
#   AGENTS.md                                      -> now (S)8.8 commit 1
#   components/expanse/test/twin_containers.h      -> now (S)8.3
#   docs/benchmarks/art_comparison/METHODOLOGY.md  -> now (S)8.8 commit 2 + (S)8.3
#
# All three are pinned verbatim in self_test() per AGENTS.md (S)8.12.3.
AGENT_CONFIG_PATTERNS = [
    ("home agent-config path", re.compile(r"(?:~|\$HOME)/\.(?:claude|gemini|codex|cursor|aider|copilot|continue)\b")),
    ("personal methodology doc", re.compile(r"\bRESEARCH_DISCIPLINES\.md\b")),
    ("personal playbook", re.compile(r"\b[A-Z0-9_]*_PLAYBOOK\.md\b")),
]

# The checker necessarily contains the strings it forbids -- in these patterns,
# in the comment above, and in the pinned self-test cases. It is the one file
# exempt, stated here rather than silently skipped (AGENTS.md (S)8.1).
AGENT_CONFIG_SELF_EXEMPT = "scripts/check_docs_hygiene.py"

# Tracked extensions that are not text and cannot carry a prose reference.
_BINARY_EXT = {".png", ".jpg", ".jpeg", ".gif", ".ico", ".pdf", ".woff", ".woff2",
               ".ttf", ".otf", ".zip", ".gz", ".bin", ".elf", ".a", ".so", ".dylib"}


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
PROVENANCE_GLOBS = (
    "docs/benchmarks/**/README.md",
    "docs/benchmarks/**/METHODOLOGY.md",
)

# --- fatal: mechanism claims -------------------------------------------------
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

# --- fatal: wall-clock intervals ---------------------------------------------
WALLCLOCK_RATIO = re.compile(r"(?<![\w.])\d+(?:\.\d+)?\s*(?:x|×)\b", re.IGNORECASE)
WALLCLOCK_CONTEXT = re.compile(
    r"(\bns\b|µs|\bus\b|\bms\b|ops/s|Mops|M/s|latency|throughput|"
    r"faster|slower|speedup|wall.?clock)",
    re.IGNORECASE,
)
DETERMINISTIC_METRIC = re.compile(
    r"(instruction|callgrind|\bIr\b|B/key|B/k|bytes/key|byte accounting|"
    r"symbol|deterministic|inst\b|density|"
    r"memory|footprint|\bKB/|\bMB/|\bB/|resident|allocat|\bRAM\b|heap)",
    re.IGNORECASE,
)
INTERVAL_EVIDENCE = re.compile(
    r"(\[\s*[-+]?\d+(?:\.\d+)?%?\s*,\s*[-+]?\d+(?:\.\d+)?%?\s*\]|"
    r"\bBCa\b|confidence interval|\bCI\b|bca_bootstrap|"
    r"results/baseline_|"
    r"\bno interval\b|\bunsourced\b|\bsuperseded\b|\bretracted\b|"
    r"\bindicative\b|\bunmeasured\b|\bprovisional\b|pending re-measurement)",
    re.IGNORECASE,
)

# --- fatal: retraction & refutation markers ----------------------------------
RETRACTION_MARKERS = re.compile(
    r"\b(retract(?:ed|ion|ing)?|withdraw(?:n)?|supersed(?:ed|es|ing)?|"
    r"correct(?:ed|ion)?|previously|refut(?:ed|es|ing)?|stale|"
    r"anti-example|strawman|earlier|was measured with|both were|"
    r"unmeasured|unverified|definitional|indicative)\b",
    re.IGNORECASE,
)

# --- fatal: paired figures & workload shape requirement ----------------------
PAIRED_VS_PAT = re.compile(
    r"(?:\b\d+(?:\.\d+)?\s*(?:ns|µs|us|ms|s|B/key|B/k|bytes/key|B/docID|bits/docID|B/tok|B/entry|Mops/s|M ops/s|M/s|Minst|M inst|inst|tps|M\b)\b(?:\*\*)?\s*(?:vs\.?|vs|against)\s*(?:\*\*)?\d+(?:\.\d+)?|\b\d+(?:\.\d+)?\b(?:\*\*)?\s*(?:vs\.?|vs|against)\s*(?:\*\*)?\d+(?:\.\d+)?\s*(?:ns|µs|us|ms|s|B/key|B/k|bytes/key|B/docID|bits/docID|B/tok|B/entry|Mops/s|M ops/s|M/s|Minst|M inst|inst|tps|M\b)\b)",
    re.IGNORECASE,
)
INSTRUCTION_METRIC_PAT = re.compile(
    r"(?:\b\d+(?:\.\d+)?\s*(?:x|×)\s*(?:the\s+)?instructions?\b|\b\d+(?:\.\d+)?\s*(?:M\s*inst|Minst|inst|instructions?|instruction-retired|Ir)\b|\b\d+(?:\.\d+)?%\s*(?:fewer|more)?\s*instructions?\b)",
    re.IGNORECASE,
)
TIME_METRIC_PAT = re.compile(
    r"(?:\b\d+(?:\.\d+)?\s*(?:ns|µs|us|ms|s)\b|\b\d+(?:\.\d+)?\s*(?:x|×)\s*(?:faster|slower|speedup)?\s*(?:in\s+)?wall[- ]?clock\b|wall[- ]?clock(?:\s+latency)?\s*(?:of\s*)?\d+(?:\.\d+)?\s*(?:ns|µs|us|ms|s|x|×)|\b\d+(?:\.\d+)?\s*(?:x|×)\s*(?:faster|slower)\b)",
    re.IGNORECASE,
)
THROUGHPUT_METRIC_PAT = re.compile(
    r"(?:\b\d+(?:\.\d+)?\s*(?:Mops/s|M ops/s|Mops|tps|M/s|ops/s|items/sec|inserts/sec)\b)",
    re.IGNORECASE,
)
MEMORY_METRIC_PAT = re.compile(
    r"(?:\b\d+(?:\.\d+)?\s*(?:B/key|B/k|bytes/key|B/docID|bits/docID|B/tok|B/entry|B/state)\b|\b\d+(?:\.\d+)?\s*(?:x|×)\s*(?:lower|higher|less|more)?\s*(?:RAM|memory|heap|footprint)\b|\b\d+(?:\.\d+)?\s*(?:MB|MiB|GB|GiB|KB|KiB)\s*(?:RAM|heap|memory|live heap)\b)",
    re.IGNORECASE,
)
WORKLOAD_TAG_PAT = re.compile(r"(?:[\(\[]|;\s*|\b)workload:\s*`?([a-zA-Z0-9_-]+)`?\b", re.IGNORECASE)
WORKLOAD_DIFF_PAT = re.compile(r"(?:[\(\[]|;\s*|\b)workloads\s+differ:\s*`?([a-zA-Z0-9_-]+)`?\s+vs\s+`?([a-zA-Z0-9_-]+)`?\b", re.IGNORECASE)
PAIRED_FALLBACK_PAT = re.compile(
    r"\b(different\s+experiment|different\s+workload|not\s+comparable|retracted|superseded|"
    r"neither\s+half\s+describes|two\s+halves\s+are\s+different|historical\s+record|pre-#\d+|"
    r"unmeasured|unverified|definitional|no\s+arm\s+on\s+which\s+both\s+were\s+observed|"
    r"strawman|target|\(target\)|until\s+measured|pending\s+(?:fair-baseline\s+)?re-run)\b",
    re.IGNORECASE,
)

# --- fatal: pending-cell requirements ----------------------------------------
PENDING_PATTERN = re.compile(
    r"\b(?:pending\s+(?:(?:a\s+)?(?:tagged\s+)?(?:reference-host|quiet-host|fair-baseline|clean-host)\s+)?(?:re-run|re-measurement|run)|"
    r"unverified\s+until\s+the\s+next\s+nightly\s+baseline\s+run)\b",
    re.IGNORECASE,
)

_ISSUE_STATUS_CACHE: dict[int, str] = {}
_SKIPPED_ISSUE_CHECKS: set[int] = set()


def get_issue_status(issue_num: int) -> str:
    """Return 'OPEN', 'CLOSED', or 'UNKNOWN' for a GitHub issue number."""
    if issue_num in _ISSUE_STATUS_CACHE:
        return _ISSUE_STATUS_CACHE[issue_num]
    try:
        res = subprocess.run(
            ["gh", "issue", "view", str(issue_num), "--json", "state", "--jq", ".state"],
            capture_output=True, text=True, check=False,
        )
        if res.returncode == 0:
            status = res.stdout.strip().upper()
            if status in ("OPEN", "CLOSED"):
                _ISSUE_STATUS_CACHE[issue_num] = status
                return status
    except Exception:
        pass
    # Fallback when gh CLI is unavailable or offline (e.g. fork PRs with no GH_TOKEN)
    _ISSUE_STATUS_CACHE[issue_num] = "UNKNOWN"
    _SKIPPED_ISSUE_CHECKS.add(issue_num)
    return "UNKNOWN"


def load_superseded_registry(root: Path) -> list[dict[str, Any]]:
    path = root / ".github" / "superseded-figures.json"
    if not path.exists():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        figs = []
        for item in data.get("figures", []):
            patterns = [re.compile(p, re.IGNORECASE) for p in item.get("patterns", [])]
            figs.append({
                "id": item["id"],
                "patterns": patterns,
                "context": [c.lower() for c in item.get("context", [])],
                "array_sequence": item.get("array_sequence"),
                "replacement": item.get("replacement", ""),
                "provenance": item.get("provenance", ""),
            })
        return figs
    except Exception as e:
        print(f"::error::failed to load superseded registry: {e}")
        return []


def tracked_markdown(root: Path) -> list[Path]:
    """Return deduplicated list of tracked Markdown paths."""
    out = subprocess.run(
        ["git", "ls-files", "--", "*.md", "docs/**/*.md", ".github/**/*.md", "crates/**/*.md", "bindings/**/*.md", "integrations/**/*.md"],
        cwd=root, capture_output=True, text=True, check=True,
    ).stdout.split()
    seen = set()
    files = []
    for f in out:
        if not f.endswith(".md"):
            continue
        p = root / f
        if not p.is_file():
            continue
        rp = p.resolve()
        if rp in seen:
            continue
        # Exclude nested worktrees and build output -- but decide that on the
        # path RELATIVE to the scan root, never on the absolute path. The repo
        # root is itself frequently a git worktree (agents work in
        # `.claude/worktrees/<name>/`), and matching the absolute path made
        # every tracked file look nested: this returned 0 of 52 files while
        # exiting 0, so the gate reported "passed" having scanned nothing.
        # A check that silently verifies nothing is worse than no check
        # (§8.1), and it is how a §8.12 violation reached CI green locally.
        try:
            rel = rp.relative_to(root.resolve())
        except ValueError:
            # Outside the scan root (a symlink pointing away); keep the old
            # conservative behaviour and skip it.
            continue
        parts = rel.parts
        if "worktrees" in parts or "target" in parts:
            continue
        seen.add(rp)
        files.append(p)
    return sorted(files)


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
                    hits.append((n, "denylisted hostname"))
                    break
    return hits


def check_provenance(lines: list[str]) -> list[int]:
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


def check_superseded_figures(
    lines: list[tuple[int, str]], registry: list[dict[str, Any]]
) -> list[tuple[int, str, str]]:
    """Assert superseded figures appear only with explicit retraction markers."""
    if not registry:
        return []
    hits = []
    total = len(lines)
    for idx, (n, text) in enumerate(lines):
        if ALLOW_MARKER in text:
            continue
        win_start = max(0, idx - 3)
        win_end = min(total, idx + 4)
        window_text = " ".join(t for _, t in lines[win_start:win_end])
        has_retraction = bool(RETRACTION_MARKERS.search(window_text))

        # Split line into sentence chunks or table cells for context analysis
        chunks = re.split(r"(?<=[.!?])\s+|\|", text)
        for chunk in chunks:
            chunk_clean = chunk.strip()
            if not chunk_clean:
                continue
            chunk_lower = chunk_clean.lower()
            for fig in registry:
                for pat in fig["patterns"]:
                    m = pat.search(chunk_clean)
                    if not m:
                        continue
                    req_ctx = min(2, len(fig["context"]))
                    ctx_matches = sum(1 for c in fig["context"] if c in chunk_lower or c in window_text.lower())
                    if ctx_matches >= req_ctx:
                        if not has_retraction:
                            hits.append((n, m.group(0).strip(), fig["replacement"]))
                        break
    return hits


def check_paired_figures(lines: list[tuple[int, str]], path_label: str = "") -> list[tuple[int, str]]:
    """Assert paired performance figures in prose share a workload ID or declare differentiation."""
    if path_label.endswith("AGENTS.md") or path_label.endswith("CLAUDE.md") or path_label.endswith("GEMINI.md"):
        return []
    hits = []
    
    # Process paragraph by paragraph
    paras: list[list[tuple[int, str]]] = []
    para: list[tuple[int, str]] = []
    in_generated_table = False

    for n, text in lines:
        if "<!-- BEGIN HARNESS AUDIT TABLE" in text:
            in_generated_table = True
        if "<!-- END HARNESS AUDIT TABLE" in text:
            in_generated_table = False
            continue
        if in_generated_table:
            continue

        if text.strip():
            para.append((n, text))
        else:
            if para:
                paras.append(para)
            para = []
    if para:
        paras.append(para)

    for para in paras:
        para_text = " ".join(t for _, t in para)
        if ALLOW_MARKER in para_text or "allow-unpaired-figures" in para_text:
            continue
        
        has_workload_tag = bool(
            WORKLOAD_TAG_PAT.search(para_text)
            or WORKLOAD_DIFF_PAT.search(para_text)
            or PAIRED_FALLBACK_PAT.search(para_text)
        )
        
        for n, text in para:
            if ALLOW_MARKER in text:
                continue
            
            # Check 1: Direct "A vs B" comparison
            for m in PAIRED_VS_PAT.finditer(text):
                if has_workload_tag:
                    continue
                # Line-level context check
                line_ctx = text[max(0, m.start() - 50): min(len(text), m.end() + 100)]
                if (
                    WORKLOAD_TAG_PAT.search(line_ctx)
                    or WORKLOAD_DIFF_PAT.search(line_ctx)
                    or PAIRED_FALLBACK_PAT.search(line_ctx)
                ):
                    continue
                hits.append((n, m.group(0).strip()))
                break
            
            # Check 2: Cross-metric / multi-metric pairing in one sentence/clause
            sentences = [s.strip() for s in re.split(r"(?<=[.!?])\s+|\|", text) if s.strip()]
            for s in sentences:
                if has_workload_tag:
                    continue
                if (
                    WORKLOAD_TAG_PAT.search(s)
                    or WORKLOAD_DIFF_PAT.search(s)
                    or PAIRED_FALLBACK_PAT.search(s)
                ):
                    continue
                has_inst = bool(INSTRUCTION_METRIC_PAT.search(s))
                has_time = bool(TIME_METRIC_PAT.search(s))
                has_tput = bool(THROUGHPUT_METRIC_PAT.search(s))
                has_mem = bool(MEMORY_METRIC_PAT.search(s))
                classes_count = sum([has_inst, has_time, has_tput, has_mem])
                if classes_count >= 2:
                    if not any(h[0] == n for h in hits):
                        hits.append((n, s))
                        break

    return hits


def check_pending_cells(lines: list[tuple[int, str]], path_label: str = "") -> list[tuple[int, str]]:
    """Assert pending measurement cells cite an OPEN tracking issue."""
    if path_label.endswith("AGENTS.md") or path_label.endswith("CLAUDE.md") or path_label.endswith("GEMINI.md"):
        return []
    hits = []
    for n, text in lines:
        if ALLOW_MARKER in text:
            continue
        for m_pend in PENDING_PATTERN.finditer(text):
            span_after = text[m_pend.end(): m_pend.end() + 150]
            issues = [int(m.group(1)) for m in re.finditer(r"(?:#|issues/)(\d+)", span_after)]
            if not issues:
                hits.append((n, "pending measurement statement carries no tracking issue citation (cite an open issue e.g. #382)"))
                break
            statuses = [get_issue_status(iss) for iss in issues]
            if "OPEN" in statuses:
                continue
            if any(st == "CLOSED" for st in statuses):
                closed_iss = [iss for iss, st in zip(issues, statuses) if st == "CLOSED"][0]
                hits.append((n, f"pending cell cites CLOSED issue #{closed_iss} — update to an OPEN tracking issue (e.g. #382)"))
                break
    return hits


def check_json_datasets(root: Path, registry: list[dict[str, Any]]) -> list[tuple[str, str]]:
    """Assert JSON visualizer and asset data contain no superseded figures."""
    if not registry:
        return []
    json_targets = [
        root / "docs" / "visualizer_data.json",
        root / "docs" / "assets" / "data" / "bench_assets.json",
    ]
    skip_keys = {
        "provenance", "removed_for_lack_of_provenance", "retraction",
        "retraction_372", "meta", "description", "_comment"
    }
    violations = []

    def check_node(path_label: str, key_path: str, val: Any) -> None:
        if isinstance(val, dict):
            for k, v in val.items():
                if k in skip_keys:
                    continue
                check_node(path_label, f"{key_path}.{k}", v)
        elif isinstance(val, list):
            for fig in registry:
                seq = fig.get("array_sequence")
                if seq and len(val) >= len(seq):
                    str_vals = [str(x) for x in val]
                    for i in range(len(str_vals) - len(seq) + 1):
                        if str_vals[i:i + len(seq)] == seq:
                            violations.append((path_label, f"{key_path}: matching retracted sequence {seq}"))
            for i, elem in enumerate(val):
                check_node(path_label, f"{key_path}[{i}]", elem)
        elif isinstance(val, str):
            for fig in registry:
                for pat in fig["patterns"]:
                    m = pat.search(val)
                    if m:
                        req_ctx = min(2, len(fig["context"]))
                        ctx_matches = sum(1 for c in fig["context"] if c in val.lower() or c in key_path.lower())
                        if ctx_matches >= req_ctx:
                            violations.append((path_label, f"{key_path}: '{m.group(0)}' superseded figure without retraction"))

    for target in json_targets:
        if not target.is_file():
            continue
        try:
            data = json.loads(target.read_text(encoding="utf-8"))
            rel = target.relative_to(root).as_posix()
            check_node(rel, "root", data)
        except Exception as e:
            violations.append((str(target), f"JSON parse error: {e}"))
    return violations


def scan_text(
    path_label: str,
    text: str,
    denylist: list[str],
    provenance: bool,
    registry: list[dict[str, Any]] | None = None,
    is_html: bool = False,
) -> tuple[int, int]:
    lines = text.splitlines()
    kept = strip_fences(lines)
    fatal = 0
    for n, what in check_time_estimates(kept):
        print(f"::error file={path_label},line={n}::time estimate in prose ({what!r}) — AGENTS.md §6 forbids durations; state ordering/gates instead")
        fatal += 1
    for n, what in check_pii(kept, denylist):
        print(f"::error file={path_label},line={n}::{what} — AGENTS.md §7 forbids PII / local-infrastructure identifiers")
        fatal += 1
    if not is_html:
        for n, what in check_mechanism_claims(kept):
            print(f"::error file={path_label},line={n}::mechanism claim ({what!r}) with no counter evidence in its paragraph — AGENTS.md §8.9 wants a counter, a `results/baseline_*` reference, or an explicit 'unmeasured' / 'hypothesis' / 'cause unknown' qualifier")
            fatal += 1
        for n, what in check_interval_claims(kept):
            print(f"::error file={path_label},line={n}::wall-clock ratio ({what!r}) published with no interval in its paragraph — AGENTS.md §8.4 gates continuous metrics on the BCa CI lower bound; cite `[lo, hi]`, a `results/baseline_*` artifact, or state plainly that no interval exists")
            fatal += 1
        for n, what in check_paired_figures(kept, path_label):
            print(f"::error file={path_label},line={n}::paired performance figures ({what!r}) without a shared workload ID — AGENTS.md §8.12 requires (workload: <id>) or (workloads differ: <id_a> vs <id_b>)")
            fatal += 1
    if registry:
        for n, figure, replacement in check_superseded_figures(kept, registry):
            print(f"::error file={path_label},line={n}::superseded figure ({figure!r}) published without a retraction marker — replacement: {replacement}")
            fatal += 1
    for n, reason in check_pending_cells(kept, path_label):
        print(f"::error file={path_label},line={n}::{reason}")
        fatal += 1

    warnings = 0
    if provenance and not is_html:
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
    root = Path(__file__).resolve().parent.parent
    reg = load_superseded_registry(root)

    # Basic hygiene
    fatal, _ = scan_text("t.md", "Ship v0.1 (1-2 days).\n", deny, False, reg); assert fatal == 1, "duration"
    fatal, _ = scan_text("t.md", "Step 1 — ships when boundary-F1 ≥ 0.8.\n", deny, False, reg); assert fatal == 0, "gate wording"
    fatal, _ = scan_text("t.md", "The nightly cache has a 7 days retention.\n", deny, False, reg); assert fatal == 0, "retention exempt"
    fatal, _ = scan_text("t.md", "```\n1-2 days\n```\n", deny, False, reg); assert fatal == 0, "fence skipped"
    fatal, _ = scan_text("t.md", "rsync ./ /Users/someone/repo/\n", deny, False, reg); assert fatal == 1, "home path"
    fatal, _ = scan_text("t.md", "use $HOME/.cargo or /home/<user>/x\n", deny, False, reg); assert fatal == 0, "placeholder home"
    fatal, _ = scan_text("t.md", "ssh examplehost 'cargo bench'\n", deny, False, reg); assert fatal == 1, "denylisted host"
    fatal, _ = scan_text("t.md", "connect to 192.168.1.20\n", deny, False, reg); assert fatal == 1, "lan ip"
    fatal, _ = scan_text("t.md", "planned for 2 weeks docs-lint: allow\n", deny, False, reg); assert fatal == 0, "allow marker"
    _, w = scan_text("t.md", "| arm | ns |\n|---|---|\n| a | 35.8 ns |\n", deny, True, reg); assert w == 1, "provenance warn"
    _, w = scan_text("t.md", "*(measured: host, commit)*\n| arm | ns |\n|---|---|\n| a | 35.8 ns |\n", deny, True, reg); assert w == 0, "provenance tagged"

    # Mechanism & intervals
    fatal, _ = scan_text("t.md", "The arm is memory-latency-bound, so the work removed is off the critical path.\n", deny, False, reg); assert fatal >= 1, "mechanism claim without evidence"
    fatal, _ = scan_text("t.md", "Point lookups are 2.9x faster than BTreeMap at 1M keys.\n", deny, False, reg); assert fatal == 1, "bare wall-clock ratio"
    fatal, _ = scan_text("t.md", "Random 1M get is 1.031x, BCa 95% CI [1.024, 1.038].\n", deny, False, reg); assert fatal == 0, "interval discharges claim"

    # Paired figures (§8.12)
    fatal, _ = scan_text("t.md", "Sequential lookup is 11.9 ns vs 108.9 ns at 1M keys.\n", deny, False, reg)
    assert fatal >= 1, "bare paired figures without workload tag must fail"
    fatal, _ = scan_text("t.md", "Sequential lookup is 11.9 ns vs 108.9 ns at 1M keys (workload: core_compare).\n", deny, False, reg)
    assert fatal == 0, "paired figures with shared workload tag must pass"
    fatal, _ = scan_text("t.md", "Retracted (workloads differ: a vs b): 11.9 ns vs 108.9 ns.\n", deny, False, reg)
    assert fatal == 0, "paired figures with workloads differ tag must pass"
    fatal, _ = scan_text("t.md", "Retracted: the two halves are different experiments (11.9 ns vs 108.9 ns).\n", deny, False, reg)
    assert fatal == 0, "paired figures with explicit retraction must pass"
    fatal, _ = scan_text("t.md", "| op | latency |\n|---|---|\n| get | 12.2 ns vs 22.4 ns (workload: capi_vs_stock) |\n", deny, False, reg)
    assert fatal == 0, "table cell with workload tag must pass"
    fatal, _ = scan_text("t.md", "| op | latency |\n|---|---|\n| get | 12.2 ns vs 22.4 ns |\n", deny, False, reg)
    assert fatal >= 1, "table cell without workload tag must fail"

    # Cross-metric paired claims (the motivating #453 / #487 defect)
    fatal, _ = scan_text("t.md", "libexpanse retires 0.55x the instructions of stock libjudy on random 1M lookup and is 1.11x slower in wall clock.\n", deny, False, reg)
    assert fatal >= 1, "un-tagged instruction + wall-clock paired claim (ASCII x) must fail"
    fatal, _ = scan_text("t.md", "libexpanse retires 0.55× the instructions of stock libjudy on random 1M lookup and is 1.11× slower in wall clock.\n", deny, False, reg)
    assert fatal >= 1, "un-tagged instruction + wall-clock paired claim (Unicode ×) must fail"
    fatal, _ = scan_text("t.md", "Retracted (workloads differ: capi_vs_stock vs capi_bench_vs_libjudy): libexpanse retires 0.55× the instructions of stock libjudy on random 1M lookup and is 1.11× slower in wall clock.\n", deny, False, reg)
    assert fatal == 0, "retracted instruction + wall-clock paired claim with workloads differ must pass"
    fatal, _ = scan_text("t.md", "Achieves 2.66x lower RAM at 1.9M ops/s.\n", deny, False, reg)
    assert fatal >= 1, "un-tagged cross-metric RAM + throughput claim must fail"
    fatal, _ = scan_text("t.md", "Achieves 2.66x lower RAM at 1.9M ops/s (workload: domain_grammar_masks).\n", deny, False, reg)
    assert fatal == 0, "cross-metric claim with workload tag must pass"

    # Superseded figure tests with Unicode × (U+00D7) and lookaround boundaries
    fatal, _ = scan_text("t.md", "Random lookup 1M is 1.11× slower than stock JudyL.\n", deny, False, reg)
    assert fatal >= 1, "unretracted Unicode 1.11× figure must fail"
    fatal, _ = scan_text("t.md", "(**1.11×**) slower random lookup vs stock JudyL.\n", deny, False, reg)
    assert fatal >= 1, "unretracted (**1.11×**) markup figure must fail"
    fatal, _ = scan_text("t.md", "Retracted: previously published 1.11× slower random lookup vs stock Judy.\n", deny, False, reg)
    assert fatal == 0, "retracted 1.11× figure with marker must pass"
    fatal, _ = scan_text("t.md", "YCSB Workload E scan range shows 4.33× speedup for ExpanseBlobMap over SkipMap.\n", deny, False, reg)
    assert fatal >= 1, "unretracted 4.33× Workload E must fail"
    fatal, _ = scan_text("t.md", "RocksDB leaf blocks store entry pointers at 13.2 B/entry.\n", deny, False, reg)
    assert fatal == 0, "valid RocksDB 13.2 B/entry density figure must pass"
    fatal, _ = scan_text("t.md", "ExpanseBlobMap achieves 13.2 Mops/s on YCSB Workload E range scan.\n", deny, False, reg)
    assert fatal >= 1, "unretracted 13.2 Mops Workload E must fail"

    # Pending issue validation
    _ISSUE_STATUS_CACHE[384] = "CLOSED"
    _ISSUE_STATUS_CACHE[382] = "OPEN"
    # These fixtures embed the pending-cell phrasing verbatim, so this file's
    # own source matches the pattern when the scanner sweeps the repository.
    # The issue numbers are seeded in the cache above and are arbitrary to the
    # test, but a real one changes state eventually: #382 was open when these
    # were written and closed on 2026-08-31, at which point the checker began
    # failing on its own test data. The marker exempts the source lines from
    # the sweep without altering the strings under test.
    fatal, _ = scan_text("t.md", "Pause times are pending a tagged reference-host run (#384).\n", deny, False, reg)  # docs-lint: allow
    assert fatal >= 1, "pending cell citing closed #384 must fail"
    fatal, _ = scan_text("t.md", "Pause times are pending a tagged reference-host run (#382).\n", deny, False, reg)  # docs-lint: allow
    assert fatal == 0, "pending cell citing open #382 must pass"
    fatal, _ = scan_text("t.md", "Pause times are pending a tagged reference-host run.\n", deny, False, reg)  # docs-lint: allow
    assert fatal >= 1, "pending cell with no issue citation must fail"

    # The file-discovery filter itself. A repo root that is a git worktree
    # must still have its tracked markdown scanned: matching "worktrees" on
    # the absolute path silently reduced this to zero files while the script
    # exited 0, which is the failure mode §8.1 names by hand.
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        fake = Path(td) / ".claude" / "worktrees" / "agent-branch"
        (fake / "docs").mkdir(parents=True)
        (fake / "README.md").write_text("# root\n", encoding="utf-8")
        (fake / "docs" / "GUIDE.md").write_text("# guide\n", encoding="utf-8")
        # A genuinely nested worktree and a build directory must still be skipped.
        for nested in (fake / ".claude" / "worktrees" / "inner", fake / "target" / "doc"):
            nested.mkdir(parents=True)
            (nested / "NESTED.md").write_text("# nested\n", encoding="utf-8")
        subprocess.run(["git", "init", "-q"], cwd=fake, check=True)
        subprocess.run(["git", "add", "-A"], cwd=fake, check=True)
        found = {p.name for p in tracked_markdown(fake)}
        assert found == {"README.md", "GUIDE.md"}, (
            f"tracked_markdown under a worktree-shaped root returned {found!r}; "
            f"a root whose absolute path contains 'worktrees' must still be scanned, "
            f"and nested worktrees/target must still be skipped"
        )

    # Personal agent-config references (fatal). The three cases below are the
    # verbatim strings that shipped before this check existed -- AGENTS.md
    # (S)8.12.3: a gate that passes while ignoring its own motivating defect is
    # measuring the wrong invariant.
    _CFG_MUST_FAIL = [
        "with unit tests pinning reference constants (the math-first Python-validation "
        "requirement in ~/.claude/CLAUDE.md, expanding RESEARCH_DISCIPLINES.md Rule 1).",
        " * rather than measured (~/.claude/CLAUDE.md, twin-baseline rule), so:",
        "Per `~/.claude/RESEARCH_DISCIPLINES.md` Rules 2 (Pre-registration), 3 (Fair twin "
        "with winning regime), and 22 (Engineering plumbing lighter track).",
        "see ~/.claude/skills/paper-publishing/PAPER_PUBLISHING_PLAYBOOK.md",
        "follow $HOME/.gemini/GEMINI.md for the house style",
    ]
    _CFG_MUST_PASS = [
        "with unit tests pinning reference constants (\u00a78.8 commit 1).",
        " * rather than measured (\u00a78.3), so:",
        "Per `AGENTS.md` \u00a78.8 commit 2 (pre-registration locked before any main data).",
        "export PATH=$HOME/.cargo/bin:$PATH   # not an agent-config dir",
        "The canonical agent guide for this repo is AGENTS.md; CLAUDE.md is a symlink.",
        "cite ~/.claude/CLAUDE.md here docs-lint: allow",
    ]
    for case in _CFG_MUST_FAIL:
        assert any(pat.search(case) for _, pat in AGENT_CONFIG_PATTERNS), (
            f"agent-config check must flag the historical leak: {case!r}"
        )
    for case in _CFG_MUST_PASS:
        flagged = ALLOW_MARKER not in case and any(pat.search(case) for _, pat in AGENT_CONFIG_PATTERNS)
        assert not flagged, f"agent-config check must not flag: {case!r}"

    # ...and it must actually sweep non-Markdown files: the C-header leak is why.
    with tempfile.TemporaryDirectory() as td:
        fake = Path(td)
        (fake / "docs").mkdir()
        (fake / "note.h").write_text("/* see ~/.claude/CLAUDE.md, twin-baseline rule */\n")
        (fake / "docs" / "ok.md").write_text("Per AGENTS.md \u00a78.3.\n")
        subprocess.run(["git", "init", "-q"], cwd=fake, check=True)
        subprocess.run(["git", "add", "-A"], cwd=fake, check=True)
        found = check_agent_config_refs(fake)
        assert [h[0] for h in found] == ["note.h"], (
            f"expected the C header to be flagged, got {found!r}; a Markdown-only sweep "
            f"is exactly how this class of reference reached a public repo"
        )

    print("check_docs_hygiene.py --self-test: all checks passed")
    return 0


def tracked_text_files(root: Path) -> list[Path]:
    """Every tracked file that can carry prose, deduplicated by realpath.

    Deliberately wider than tracked_markdown(): the reference this feeds shipped
    in a C header, so a Markdown-only sweep would have reported clean.
    """
    out = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, check=True).stdout.split("\n")
    seen: set[Path] = set()
    files: list[Path] = []
    for f in out:
        if not f.strip():
            continue
        p = root / f
        if not p.is_file() or p.suffix.lower() in _BINARY_EXT:
            continue
        rp = p.resolve()
        if rp in seen:
            continue
        try:
            rel = rp.relative_to(root.resolve())
        except ValueError:
            continue
        if "worktrees" in rel.parts or "target" in rel.parts:
            continue
        seen.add(rp)
        files.append(p)
    return sorted(files)


def check_agent_config_refs(root: Path) -> list[tuple[str, int, str]]:
    """Flag references to a maintainer's personal agent config in tracked files."""
    hits: list[tuple[str, int, str]] = []
    for path in tracked_text_files(root):
        rel = path.relative_to(root).as_posix()
        if rel == AGENT_CONFIG_SELF_EXEMPT:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue  # genuinely binary despite its extension
        for n, line in enumerate(text.split("\n"), 1):
            if ALLOW_MARKER in line:
                continue
            for label, pat in AGENT_CONFIG_PATTERNS:
                if pat.search(line):
                    hits.append((rel, n, label))
                    break
    return hits


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

    registry = load_superseded_registry(root)
    fatal = warnings = 0

    # Scan tracked Markdown files
    for path in tracked_markdown(root):
        f, w = scan_text(
            path.relative_to(root).as_posix(),
            path.read_text(encoding="utf-8", errors="replace"),
            denylist,
            provenance=not args.no_provenance and wants_provenance(root, path),
            registry=registry,
            is_html=False,
        )
        fatal += f
        warnings += w
    # Scan HTML Visualizer
    vis_html = root / "docs" / "architecture_visualizer.html"
    if vis_html.is_file():
        f, _ = scan_text(
            vis_html.relative_to(root).as_posix(),
            vis_html.read_text(encoding="utf-8", errors="replace"),
            denylist,
            provenance=False,
            registry=registry,
            is_html=True,
        )
        fatal += f
    fatal += check_published_html_links(root)

    # Personal agent-config references, across every tracked text file
    for rel, lineno, label in check_agent_config_refs(root):
        print(
            f"::error file={rel},line={lineno}::{label} — this repository is public and "
            f"read by several agent toolchains; cite the AGENTS.md section that states the "
            f"rule instead of a path under a maintainer's home directory"
        )
        fatal += 1

    # Scan JSON datasets
    json_errors = check_json_datasets(root, registry)
    for path_label, err in json_errors:
        print(f"::error file={path_label}::{err}")
        fatal += 1

    if args.pr_body_file and os.path.exists(args.pr_body_file):
        f, _ = scan_text("PR body", Path(args.pr_body_file).read_text(encoding="utf-8", errors="replace"), denylist, provenance=False, registry=registry)
        fatal += f

    if _SKIPPED_ISSUE_CHECKS:
        skipped_str = ", ".join(f"#{num}" for num in sorted(_SKIPPED_ISSUE_CHECKS))
        print(f"::notice::GitHub API unavailable (or GH_TOKEN unset) — open/closed status check skipped for issue(s): {skipped_str}")

    print(f"check_docs_hygiene.py: {fatal} fatal finding(s), {warnings} advisory warning(s)")
    return 1 if fatal else 0


if __name__ == "__main__":
    sys.exit(main())
