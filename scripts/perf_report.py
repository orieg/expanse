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
        rows = []
        improved = regressed = unchanged = 0
        worst, best = 0.0, 0.0
        for name, metrics in head.items():
            b = base.get(name)
            ins, cyc = metrics.get("Instructions", 0), metrics.get("Estimated Cycles", 0)
            if not b:
                rows.append(f"| 🆕 | `{name}` | {ins:,} | new | {cyc:,} | new |")
                continue
            d_ins = pct(ins, b.get("Instructions", 0))
            d_cyc = pct(cyc, b.get("Estimated Cycles", 0))
            if abs(d_ins) < NOISE_PCT:
                unchanged += 1
            elif d_ins < 0:
                improved += 1
            else:
                regressed += 1
            worst, best = max(worst, d_ins), min(best, d_ins)
            rows.append(
                f"| {verdict(d_ins)} | `{name}` | {ins:,} | {fmt_delta(d_ins)} "
                f"| {cyc:,} | {fmt_delta(d_cyc)} |"
            )

        # Headline first: a wall of dashes should read as "nothing changed",
        # not as "something failed to measure".
        if regressed:
            headline = (
                f"🔴 **{regressed} benchmark(s) do more work** than `{base_ref}` "
                f"(worst {worst:+.2f}%)"
            )
            if improved:
                headline += f"; {improved} do less (best {best:+.2f}%)"
        elif improved:
            headline = (
                f"🟢 **{improved} benchmark(s) do less work** than `{base_ref}` "
                f"(best {best:+.2f}%), none more"
            )
        else:
            headline = (
                f"⚪ **No change in work done** vs `{base_ref}` — expected when a PR "
                "does not touch engine code paths."
            )
        lines += [
            headline,
            "",
            f"**What this compares:** this branch against **`{base_ref}`, i.e. expanse's "
            "own previous code** — *not* against stock libjudy. Comparisons against "
            "stock are wall-clock and live in the nightly `bench-report` job.",
            "",
            "**Why it is trustworthy:** the counts come from callgrind, so they are "
            "**deterministic** — the same commit yields the same number on any runner, "
            f"and any delta above {NOISE_PCT}% is a real change in work done rather than "
            "measurement noise. Fewer instructions is better.",
            "",
            f"| | Benchmark | Instructions | vs `{base_ref}` | Est. cycles "
            f"| vs `{base_ref}` |",
            "|---|---|---:|---:|---:|---:|",
        ] + rows + [""]
        if worst >= 1.0:
            lines += [
                f"> ⚠️ Largest instruction-count regression: **{worst:+.2f}%**. "
                "Deterministic, so this is a real increase in work done — not runner "
                "noise. Worth explaining in the PR description if intentional.",
                "",
            ]
    else:
        lines += [
            "Instruction counts for this branch (callgrind, deterministic). No "
            "comparison available — this run has no merge base to measure against. "
            "These are expanse's own counts, not a comparison with stock libjudy.",
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
        "<sub>🟢 less work · 🔴 more work · = within noise · 🆕 new benchmark. "
        "Instructions measure <em>cost</em>, not time: less work is strictly better, "
        "but the wall-clock effect depends on how much latency the machine hides. "
        "Wall-clock comparisons against stock libjudy run in the nightly "
        "<code>bench-report</code> job and are a regression alarm, not publishable "
        "numbers (<code>docs/BENCHMARKING.md</code>).</sub>"
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
