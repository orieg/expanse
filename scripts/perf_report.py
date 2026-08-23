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
# Matches both "instructions::cost::map_insert random:"random"" and "instructions::cost::set32_insert_sensor_timestamps"
HEADER = re.compile(r"^([\w:]+)::(\w+)(?:\s+([^:\s]+):?.*)?$")
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
            current = f"{bench}/{arg}" if arg else bench
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


def render_v3(
    v3: dict[str, dict[str, int]],
    head: dict[str, dict[str, int]],
) -> list[str]:
    lines: list[str] = [
        "### Modern Architecture (`x86-64-v3`: AVX2 / BMI2 / POPCNT) vs Baseline (`x86-64-v1`)",
        "",
        "**What this compares:** this branch compiled with `-C target-cpu=x86-64-v3` against the baseline binary. "
        "Demonstrates the hardware acceleration enabled when running on modern CPUs or via `glibc-hwcaps`.",
        "",
        "| | Benchmark | Baseline (v1) | x86-64-v3 | Delta (v3 vs v1) | Est. Cycles Delta |",
        "|---|---|---:|---:|---:|---:|",
    ]
    for name, v3_metrics in v3.items():
        h = head.get(name)
        if not h:
            continue
        v1_ins = h.get("Instructions", 0)
        v3_ins = v3_metrics.get("Instructions", 0)
        v1_cyc = h.get("Estimated Cycles", 0)
        v3_cyc = v3_metrics.get("Estimated Cycles", 0)
        d_ins = pct(v3_ins, v1_ins)
        d_cyc = pct(v3_cyc, v1_cyc)
        lines.append(
            f"| {verdict(d_ins)} | `{name}` | {v1_ins:,} | {v3_ins:,} | {fmt_delta(d_ins)} | {fmt_delta(d_cyc)} |"
        )
    lines.append("")
    return lines


def render(
    head: dict[str, dict[str, int]],
    base: dict[str, dict[str, int]] | None,
    bytes_table: str | None,
    base_ref: str,
    vs_stock: dict[str, dict[str, int]] | None = None,
    v3: dict[str, dict[str, int]] | None = None,
    bytes32_table: str | None = None,
) -> str:
    lines: list[str] = ["## Performance", ""]

    # The vs-stock table leads: "is this a viable replacement for
    # libjudy" is the project's central question, and a self-comparison
    # cannot answer it.
    if vs_stock:
        lines += render_vs_stock(vs_stock)

    if v3:
        lines += render_v3(v3, head)

    if base:
        # Comparison mode: this branch vs merge base.
        rows: list[str] = []
        regressed = improved = unchanged = 0
        worst = float("-inf")
        best = float("inf")
        for name, metrics in head.items():
            ins = metrics.get("Instructions")
            cyc = metrics.get("Estimated Cycles")
            if ins is None or cyc is None:
                continue
            b = base.get(name)
            if not b or "Instructions" not in b:
                # New benchmark on this branch: no base to compare against.
                rows.append(f"| 🆕 | `{name}` | {ins:,} | — | {cyc:,} | — |")
                continue
            b_ins = b["Instructions"]
            b_cyc = b.get("Estimated Cycles", cyc)
            d_ins = pct(ins, b_ins)
            d_cyc = pct(cyc, b_cyc)
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
            ins = metrics.get("Instructions")
            cyc = metrics.get("Estimated Cycles")
            if ins is not None and cyc is not None:
                lines.append(f"| `{name}` | {ins:,} | {cyc:,} |")
        lines.append("")

    # Detailed metrics (L1/LL/RAM hits) collapsed by default.
    lines += [
        "<details>",
        "<summary>Cache-miss details (L1 / Last-Level / RAM hits)</summary>",
        "",
        "| Benchmark | " + " | ".join(METRICS) + " |",
        "|---|" + "|".join(["---:"] * len(METRICS)) + "|",
    ]
    for name, metrics in head.items():
        cells: list[str] = []
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
            "<summary>Memory (64-Bit Server Architecture): bytes/key by distribution</summary>",
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

    if bytes32_table:
        lines += [
            "<details>",
            "<summary>Memory (32-Bit Embedded Architecture: RV32 / ESP32 / Cortex-M): bytes/key</summary>",
            "",
            "Measured under 32-bit pointer width (`Edge32` compact 8-byte layout):",
            "",
            "```",
            bytes32_table.strip(),
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


def render_vs_stock(counts: dict[str, dict[str, int]]) -> list[str]:
    """The headline comparison: our C ABI against stock libjudy's, in
    instructions retired, paired by benchmark name."""
    # Longest suffix first: "_expanse_dl" also ends with "_dl" but must
    # never be read as the rlib arm.
    sides = (("_expanse_dl", "dl"), ("_expanse", "ours"), ("_stock", "stock"))
    pairs: dict[str, dict[str, dict[str, int]]] = {}
    for name, metrics in counts.items():
        bench, _, arg = name.partition("/")
        for suffix, side in sides:
            if bench.endswith(suffix):
                key = f"{bench[: -len(suffix)]}/{arg}"
                pairs.setdefault(key, {})[side] = metrics
                break
    pairs = {k: v for k, v in pairs.items() if "stock" in v and ("dl" in v or "ours" in v)}
    if not pairs:
        return []

    have_dl = any("dl" in v for v in pairs.values())
    lines = [
        "### vs stock libjudy",
        "",
        "The comparison that decides whether libexpanse is a viable drop-in: "
        "**identical C ABI calls, identical key streams**, instructions retired. "
        "Deterministic, so this is reviewable per PR — unlike the wall-clock "
        "ratios, which no available machine can resolve below ~15-20%.",
        "",
    ]
    if have_dl:
        lines += [
            "**Read the `.so` column.** `libexpanse.so` and stock are both reached "
            "by `dlopen` + resolved symbols, so they are the same shape; the `rlib` "
            "column is our code linked directly into the harness with LTO, which is "
            "faster than anything a drop-in consumer of libexpanse can get. The "
            "difference between the two columns is the cost of being a shared "
            "library, and it applies to every ratio this project published before "
            "the `.so` arm existed.",
            "",
            "Ratio = ours ÷ stock. **Below 1.00 means libexpanse does less work.**",
            "",
            "| | Operation | libexpanse `.so` | stock libjudy | **ratio (.so)** "
            "| ratio (rlib) | est. cycles (.so) |",
            "|---|---|---:|---:|---:|---:|---:|",
        ]
    else:
        lines += [
            "Ratio = ours ÷ stock. **Below 1.00 means libexpanse does less work.**",
            "",
            "| | Operation | libexpanse | stock libjudy | ratio | est. cycles ratio |",
            "|---|---|---:|---:|---:|---:|",
        ]
    for name in sorted(pairs):
        sides_here = pairs[name]
        stock = sides_here["stock"]
        s_ins = stock.get("Instructions", 0)
        s_cyc = stock.get("Estimated Cycles", 0)
        if not s_ins:
            continue
        # The `.so` arm is the headline when present; fall back to the
        # rlib arm so a run without the cdylib still reports something.
        primary = sides_here.get("dl") or sides_here["ours"]
        o_ins = primary.get("Instructions", 0)
        o_cyc = primary.get("Estimated Cycles", 0)
        ratio = o_ins / s_ins
        cyc_ratio = (o_cyc / s_cyc) if s_cyc else 0.0
        mark = "🟢" if ratio < 1.0 else ("🟡" if ratio <= 1.25 else "🔴")
        if have_dl:
            rlib = sides_here.get("ours")
            r_ins = rlib.get("Instructions", 0) if rlib else 0
            rlib_cell = f"{r_ins / s_ins:.2f}x" if r_ins else "—"
            lines.append(
                f"| {mark} | `{name}` | {o_ins:,} | {s_ins:,} | **{ratio:.2f}x** "
                f"| {rlib_cell} | {cyc_ratio:.2f}x |"
            )
        else:
            lines.append(
                f"| {mark} | `{name}` | {o_ins:,} | {s_ins:,} | **{ratio:.2f}x** "
                f"| {cyc_ratio:.2f}x |"
            )
    # The standing correction factor: what our code costs as a shared
    # library rather than an LTO'd rlib. Every pre-`.so` ratio understated
    # the gap by roughly this much.
    if have_dl:
        deltas = [
            v["dl"]["Instructions"] / v["ours"]["Instructions"]
            for v in pairs.values()
            if "dl" in v and "ours" in v and v["ours"].get("Instructions")
        ]
        if deltas:
            lo, hi = min(deltas), max(deltas)
            mid = sorted(deltas)[len(deltas) // 2]
            lines += [
                "",
                f"**Shared-library correction factor: {mid:.2f}x median "
                f"(range {lo:.2f}-{hi:.2f}x).** That is what the same code costs "
                "as `libexpanse.so` versus linked directly into the harness — the "
                "amount by which every vs-stock ratio published before this arm "
                "existed understated the real drop-in gap.",
            ]
    lines += [
        "",
        "<sub>🟢 less work than stock · 🟡 within 1.25x · 🔴 more. "
        + (
            "Both libraries are loaded with <code>dlopen</code> and called through "
            "symbols resolved in <code>setup</code>, so neither arm measures its own "
            "dynamic linking and neither gets inlining the other cannot have. "
            if have_dl
            else "<strong>These ratios are optimistic — read them as a floor on the "
            "gap.</strong> Our arm is an LTO'd rlib reached by direct calls while "
            "stock is a PIC shared object, so it pays PLT indirection and lost "
            "cross-object inlining that we do not. The honest drop-in number needs "
            "our own <code>libexpanse.so</code> measured the same way (issue #1). "
        )
        + "Instructions are cost, not time: cache behaviour decides how much becomes "
        "wall-clock, which the nightly <code>bench-report</code> job measures. The gap "
        "also narrows sharply with population — compare the 30k arms against "
        "<code>random_big</code> at 1.5M.</sub>",
        "",
    ]
    return lines


def read(path: str | None) -> str | None:
    if not path:
        return None
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError as exc:
        print(f"warning: {exc}", file=sys.stderr)
        return None


def check_regressions(
    head: dict[str, dict[str, int]],
    base: dict[str, dict[str, int]] | None,
    max_regression_pct: float = 1.5,
    max_regressed_count: int = 1,
    noise_floor: float = 0.5,
    allowed: bool = False,
    allow_reason: str | None = None,
) -> tuple[bool, list[str]]:
    """Evaluates if head introduces unacceptable instruction regressions vs base.

    Returns (has_unacceptable_regression, list_of_error_messages).
    """
    if not base or not head:
        return False, []

    regressions: list[tuple[str, float, int, int]] = []
    for name, metrics in head.items():
        b = base.get(name)
        if not b:
            continue
        ins = metrics.get("Instructions", 0)
        b_ins = b.get("Instructions", 0)
        d_ins = pct(ins, b_ins)
        if d_ins > noise_floor:
            regressions.append((name, d_ins, ins, b_ins))

    if not regressions:
        return False, []

    worst = max(r[1] for r in regressions)
    is_violating = worst > max_regression_pct or len(regressions) > max_regressed_count

    if not is_violating:
        return False, []

    messages = []
    if allowed:
        messages.append(f"> [!NOTE]\n> **Performance regression override acknowledged**: {allow_reason or 'Approved'}\n>")
        for name, d_ins, ins, b_ins in sorted(regressions, key=lambda x: -x[1]):
            messages.append(f"> - `{name}`: {fmt_delta(d_ins)} ({ins:,} vs {b_ins:,})")
        return False, messages

    messages.append(
        f"> [!CAUTION]\n"
        f"> **Performance regression detected**: {len(regressions)} benchmark(s) regressed > {noise_floor}% "
        f"(worst: {worst:+.2f}%, threshold: {max_regression_pct}%).\n"
        f"> To approve an intentional regression, add `allow-regression: <reason>` to the PR body.\n>"
    )
    for name, d_ins, ins, b_ins in sorted(regressions, key=lambda x: -x[1]):
        messages.append(f"> - `{name}`: {fmt_delta(d_ins)} ({ins:,} vs {b_ins:,})")

    return True, messages


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--head", required=True, help="bench output for this branch")
    ap.add_argument("--base", help="bench output for the merge base")
    ap.add_argument("--bytes", help="bytes_per_key output")
    ap.add_argument("--bytes32", help="bytes_per_key_32 output")
    ap.add_argument("--base-ref", default="main")
    ap.add_argument("--vs-stock", help="vs_stock bench output")
    ap.add_argument("--v3", help="bench output for x86-64-v3")
    ap.add_argument("--fail-on-regression", action="store_true", help="fail if regressions exceed threshold")
    ap.add_argument("--max-regression-pct", type=float, default=1.5, help="maximum single-benchmark regression pct allowed")
    ap.add_argument("--allow-regression", action="store_true", help="override/approve intentional regressions")
    ap.add_argument("--allow-regression-reason", help="reason for approving regression")
    ap.add_argument("--pr-body-file", help="path to PR body markdown to check for override markers")
    args = ap.parse_args()

    head_text = read(args.head)
    if head_text is None:
        print("## Performance\n\nBenchmarks did not run.\n")
        return 0
    base_text = read(args.base)
    stock_text = read(args.vs_stock)
    v3_text = read(args.v3)

    head_parsed = parse(head_text)
    base_parsed = parse(base_text) if base_text else None

    # Check for PR body override markers
    allowed = args.allow_regression
    allow_reason = args.allow_regression_reason
    if args.pr_body_file:
        pr_body = read(args.pr_body_file) or ""
        match = re.search(r"(?:<!--\s*)?allow-regression:\s*([^\n\->]+)", pr_body, re.IGNORECASE)
        if match:
            allowed = True
            allow_reason = match.group(1).strip()
        elif "allow-regression" in pr_body.lower() or "perf-override: approved" in pr_body.lower():
            allowed = True
            allow_reason = allow_reason or "PR body requested regression override"

    has_violation, reg_messages = check_regressions(
        head_parsed,
        base_parsed,
        max_regression_pct=args.max_regression_pct,
        allowed=allowed,
        allow_reason=allow_reason,
    )

    rendered = render(
        head_parsed,
        base_parsed,
        read(args.bytes),
        args.base_ref,
        parse(stock_text) if stock_text else None,
        parse(v3_text) if v3_text else None,
        read(args.bytes32) if args.bytes32 else None,
    )

    if reg_messages:
        rendered += "\n\n### 🛡️ Regression Guard\n\n" + "\n".join(reg_messages) + "\n"

    print(rendered, end="")

    if args.fail_on_regression and has_violation:
        for msg in reg_messages:
            # Strip markdown blockquote prefix for stderr log annotations
            clean_msg = re.sub(r"^>\s*\[!(?:CAUTION|WARNING|NOTE)\]\n?>\s*", "", msg)
            clean_msg = re.sub(r"^>\s*[-*]?\s*", "  - ", clean_msg, flags=re.MULTILINE)
            clean_msg = clean_msg.replace("**", "").replace("`", "").strip()
            print(f"::error::{clean_msg}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
