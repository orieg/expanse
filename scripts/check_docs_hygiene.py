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

# --- fatal: pending-cell requirements ----------------------------------------
PENDING_PATTERN = re.compile(
    r"\b(?:pending\s+(?:(?:a\s+)?(?:tagged\s+)?(?:reference-host|quiet-host|fair-baseline|clean-host)\s+)?(?:re-run|re-measurement|run)|"
    r"unverified\s+until\s+the\s+next\s+nightly\s+baseline\s+run)\b",
    re.IGNORECASE,
)

_ISSUE_STATUS_CACHE: dict[int, str] = {}


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
    # Fallback when gh CLI is unavailable or offline
    _ISSUE_STATUS_CACHE[issue_num] = "UNKNOWN"
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
        if "/worktrees/" in str(rp) or "/target/" in str(rp):
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
            m_iss = re.search(r"(?:#|issues/)(\d+)", span_after)
            if not m_iss:
                hits.append((n, "pending measurement statement carries no tracking issue citation (cite an open issue e.g. #382)"))
                break
            iss = int(m_iss.group(1))
            status = get_issue_status(iss)
            if status == "CLOSED":
                hits.append((n, f"pending cell cites CLOSED issue #{iss} — update to an OPEN tracking issue (e.g. #382)"))
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
    fatal, _ = scan_text("t.md", "Pause times are pending a tagged reference-host run (#384).\n", deny, False, reg)
    assert fatal >= 1, "pending cell citing closed #384 must fail"
    fatal, _ = scan_text("t.md", "Pause times are pending a tagged reference-host run (#382).\n", deny, False, reg)
    assert fatal == 0, "pending cell citing open #382 must pass"
    fatal, _ = scan_text("t.md", "Pause times are pending a tagged reference-host run.\n", deny, False, reg)
    assert fatal >= 1, "pending cell with no issue citation must fail"

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

    # Scan JSON datasets
    json_errors = check_json_datasets(root, registry)
    for path_label, err in json_errors:
        print(f"::error file={path_label}::{err}")
        fatal += 1

    if args.pr_body_file and os.path.exists(args.pr_body_file):
        f, _ = scan_text("PR body", Path(args.pr_body_file).read_text(encoding="utf-8", errors="replace"), denylist, provenance=False, registry=registry)
        fatal += f

    print(f"check_docs_hygiene.py: {fatal} fatal finding(s), {warnings} advisory warning(s)")
    return 1 if fatal else 0


if __name__ == "__main__":
    sys.exit(main())
