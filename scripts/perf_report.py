#!/usr/bin/env python3
"""Render the CI performance comment from benchmark output.

Turns `cargo bench --bench instructions` output (callgrind counts, see
`docs/BENCHMARKING.md`) into a markdown report comparing this branch
against its merge base, plus the deterministic bytes/key table.

Why a comparison and not just numbers: callgrind counts are exact and
reproducible, so a delta between two commits is real signal at 1%
resolution — unlike wall-clock, where this project has already measured
a ~15-20% noise floor and retracted a claim built on it. The report
therefore leads with the change, not the absolute value.

Usage:
    perf_report.py --head head.txt [--base base.txt] [--bytes bytes.txt]
                   [--base-ref main] > report.md
"""

from __future__ import annotations

import argparse
import re
import sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")
# "instructions::cost::map_insert random:"random"" -> group, bench, arg
HEADER = re.compile(r"^([\w:]+)::(\w+)\s+(\S+?):")
METRIC = re.compile(r"^\s{2,}([\w+ ]+):\s+([\d,]+)\|")

# Metrics worth showing, in report order. Instructions first: it is the
# least noisy and the most directly attributable to a code change.
METRICS = ["Instructions", "Estimated Cycles", "L1 Hits", "LL Hits", "RAM Hits"]

# Below this, a delta is rounding rather than signal (allocator address
# layout can shift a count slightly even under callgrind).
NOISE_PCT = 0.1


def parse(text: str) -> dict[str, dict[str, int]]:
    """{benchmark id -> {metric -> count}} from one bench run's output."""
    out: dict[str, dict[str, int]] = {}
    current: str | None = None
    for line in ANSI.sub("", text).splitlines():
        head = HEADER.match(line)
        if head:
            _group, bench, arg = head.groups()
            current = f"{bench}/{arg}"
            out[current] = {}
            continue
        metric = METRIC.match(line)
        if metric and current:
            name, value = metric.group(1).strip(), metric.group(2).replace(",", "")
            try:
                out[current][name] = int(value)
            except ValueError:
                pass
    return {k: v for k, v in out.items() if v}


def pct(head: int, base: int) -> float:
    return 0.0 if base == 0 else (head - base) / base * 100.0


def verdict(delta: float) -> str:
    """Marker for one delta. Fewer instructions is better, always."""
    if abs(delta) < NOISE_PCT:
        return "="
    return "🟢" if delta < 0 else "🔴"


def fmt_delta(delta: float) -> str:
    if abs(delta) < NOISE_PCT:
        return "—"
    return f"{delta:+.2f}%"


def render(
    head: dict[str, dict[str, int]],
    base: dict[str, dict[str, int]] | None,
    bytes_table: str | None,
    base_ref: str,
) -> str:
    lines: list[str] = ["## Performance", ""]

    if not head:
        return "## Performance\n\nNo benchmark output was produced.\n"

    if base:
        lines += [
            f"Instruction counts vs `{base_ref}`. These come from callgrind and are "
            "**deterministic** — the same commit yields the same number on any runner, "
            f"so a delta above {NOISE_PCT}% is real (see `docs/BENCHMARKING.md`). "
            "Fewer instructions is better.",
            "",
            "| | Benchmark | Instructions | vs base | Est. cycles | vs base |",
            "|---|---|---:|---:|---:|---:|",
        ]
        worst = 0.0
        for name, metrics in head.items():
            b = base.get(name)
            ins, cyc = metrics.get("Instructions", 0), metrics.get("Estimated Cycles", 0)
            if not b:
                lines.append(f"| 🆕 | `{name}` | {ins:,} | new | {cyc:,} | new |")
                continue
            d_ins = pct(ins, b.get("Instructions", 0))
            d_cyc = pct(cyc, b.get("Estimated Cycles", 0))
            worst = max(worst, d_ins)
            lines.append(
                f"| {verdict(d_ins)} | `{name}` | {ins:,} | {fmt_delta(d_ins)} "
                f"| {cyc:,} | {fmt_delta(d_cyc)} |"
            )
        lines.append("")
        if worst >= 1.0:
            lines.append(
                f"> ⚠️ Largest instruction-count regression: **{worst:+.2f}%**. "
                "Deterministic, so this is a real increase in work done — not runner noise."
            )
            lines.append("")
    else:
        lines += [
            "Instruction counts (callgrind, deterministic). No base comparison "
            "available for this run.",
            "",
            "| Benchmark | Instructions | Est. cycles |",
            "|---|---:|---:|",
        ]
        for name, metrics in head.items():
            lines.append(
                f"| `{name}` | {metrics.get('Instructions', 0):,} "
                f"| {metrics.get('Estimated Cycles', 0):,} |"
            )
        lines.append("")

    # Full metric detail, collapsed: cache behaviour matters for the
    # allocator-locality work but would swamp the summary table.
    lines += ["<details>", "<summary>All metrics (cache hits, RAM traffic)</summary>", ""]
    header = "| Benchmark | " + " | ".join(METRICS) + " |"
    lines += [header, "|---" * (len(METRICS) + 1) + "|"]
    for name, metrics in head.items():
        cells = []
        for m in METRICS:
            value = metrics.get(m)
            cell = f"{value:,}" if value is not None else "—"
            if base and (b := base.get(name)) and m in b and value is not None:
                cell += f" ({fmt_delta(pct(value, b[m]))})"
            cells.append(cell)
        lines.append(f"| `{name}` | " + " | ".join(cells) + " |")
    lines += ["", "</details>", ""]

    if bytes_table:
        lines += [
            "<details>",
            "<summary>Memory: bytes/key by distribution</summary>",
            "",
            "Deterministic allocator accounting; the `memory-budget` job fails the "
            "build if any distribution exceeds its ceiling.",
            "",
            "```",
            bytes_table.strip(),
            "```",
            "",
            "</details>",
            "",
        ]

    lines.append(
        "<sub>Wall-clock comparisons against stock libjudy run in the nightly "
        "`bench-report` job; they are a regression alarm, not publishable numbers "
        "(`docs/BENCHMARKING.md`).</sub>"
    )
    return "\n".join(lines) + "\n"


def read(path: str | None) -> str | None:
    if not path:
        return None
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError as exc:
        print(f"warning: {exc}", file=sys.stderr)
        return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--head", required=True, help="bench output for this branch")
    ap.add_argument("--base", help="bench output for the merge base")
    ap.add_argument("--bytes", help="bytes_per_key output")
    ap.add_argument("--base-ref", default="main")
    args = ap.parse_args()

    head_text = read(args.head)
    if head_text is None:
        print("## Performance\n\nBenchmarks did not run.\n")
        return 0
    base_text = read(args.base)
    print(
        render(
            parse(head_text),
            parse(base_text) if base_text else None,
            read(args.bytes),
            args.base_ref,
        ),
        end="",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
