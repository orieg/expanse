#!/usr/bin/env python3
"""
scripts/bench_report.py — Automated Head-to-Head Benchmark Comparison Report Tool.

Executes standalone fast comparative benchmark sweeps across key distributions
and generates GitHub Flavored Markdown comparison tables ready for PR descriptions,
comments, or documentation.

This harness reports the median of interleaved rounds. A median of three rounds
is not a sampling distribution, so these tables carry no confidence interval and
cannot on their own support a §8.4 wall-clock claim. The interval-bearing
numbers come from the criterion suites, harvested by
`scripts/bench_baseline.py`; pass `--baseline results/baseline_*.json` to append
that section here so the PR comment carries the CI alongside the head-to-head
medians.

Usage:
  python3 scripts/bench_report.py --quick
  python3 scripts/bench_report.py --pop 1000000 --dist all --format markdown
  python3 scripts/bench_report.py --pop 100000 --format json --output report.json
  python3 scripts/bench_report.py --input report.json --format markdown
  python3 scripts/bench_report.py --input report.json --baseline results/baseline_comparative.json

Load record (AGENTS.md section 8.17, #1214). Every cell of a sweep is one run of
the `bench_lookup_compare` example, and an arch cell used to reach it through
`cargo run` with a new `RUSTFLAGS`, so a compilation sat inside the run the
cell timed and no snapshot said what else was resident. Each variant is now
built once, before the first cell, and every cell runs the built binary inside
a `bench_provenance.begin_cell` / `end_cell` window: the cell is a reaped child,
so the children's CPU over its window is its own CPU and the remainder is
foreign. The windows go to a JSON sidecar (`--load-json`; by default
`head-to-head-load.json`, beside `--output` when one is given) carrying
`provenance`, one `load` per cell, `load_status` and `load_findings`; a window
shorter than `bench_provenance.MIN_WINDOW_S` is merged into the next, as
`scripts/bench_windowed.py` does. The markdown is unchanged. A `--input`
render runs nothing and writes no sidecar.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent))

import bench_baseline  # noqa: E402
import bench_provenance as bp  # noqa: E402
import bench_windowed  # noqa: E402

HARNESS_EXAMPLE = "bench_lookup_compare"
HARNESS_PACKAGE = "expanse-trie"
LOAD_ISSUE = 1214


def get_repo_root() -> Path:
    """Returns the repository root directory."""
    return Path(__file__).resolve().parent.parent


def harness_env(target_cpu: Optional[str], base: Optional[Dict[str, str]] = None) -> Dict[str, str]:
    """The build environment of one variant: `RUSTFLAGS` gains `-C target-cpu` for an arch cell."""
    env = dict(os.environ if base is None else base)
    if target_cpu and target_cpu not in ("baseline", "none", "generic", "default"):
        rustflags = env.get("RUSTFLAGS", "")
        env["RUSTFLAGS"] = f"{rustflags} -C target-cpu={target_cpu}".strip()
    return env


def variant_key(target_cpu: Optional[str]) -> str:
    """The name a built variant is kept under; variants with the same flags share one."""
    return harness_env(target_cpu, {}).get("RUSTFLAGS", "").replace("-C target-cpu=", "") or "default"


def build_harness(root: Path, target_cpu: Optional[str]) -> Path:
    """Builds the harness for one variant and keeps a copy of the binary.

    Every variant builds into the same target directory, so each binary is
    copied out under its variant's name before the next build replaces it;
    the cells then run the copies, and no cell's window holds a compilation.
    Cargo's diagnostics go to stderr; a failed build raises.
    """
    cmd = ["cargo", "build", "--release", "--message-format=json",
           "-p", HARNESS_PACKAGE, "--example", HARNESS_EXAMPLE]
    try:
        proc = subprocess.run(cmd, cwd=root, stdout=subprocess.PIPE, text=True, check=True,
                              env=harness_env(target_cpu))
    except FileNotFoundError:
        print("Error: 'cargo' not found on PATH.", file=sys.stderr)
        sys.exit(1)
    found = bench_windowed.pick_executable(proc.stdout, "example", HARNESS_EXAMPLE)
    if found is None:
        print(f"Error: `{' '.join(cmd)}` named no executable for {HARNESS_EXAMPLE}",
              file=sys.stderr)
        sys.exit(1)
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR") or root / "target")
    if not target_dir.is_absolute():
        target_dir = root / target_dir
    dest = target_dir / "bench_report" / f"{HARNESS_EXAMPLE}-{variant_key(target_cpu)}"
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(found[0], dest)
    return dest


def run_benchmark_harness(
    pop: int,
    dist: str,
    rounds: int,
    root: Path,
    target_cpu: Optional[str] = None,
    exe: Optional[Path] = None,
) -> Dict[str, Any]:
    """Executes the built benchmark harness `exe` and parses its JSON output.

    `exe` is `build_harness(root, target_cpu)`; it is built here when omitted,
    which puts the build outside any window only because none is open.
    """
    if exe is None:
        exe = build_harness(root, target_cpu)
    cmd = [
        str(exe),
        "--pop",
        str(pop),
        "--dist",
        dist,
        "--rounds",
        str(rounds),
        "--json",
    ]

    try:
        proc = subprocess.run(
            cmd,
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        )
    except subprocess.CalledProcessError as exc:
        print(f"Error running benchmark harness:\n{exc.stderr}", file=sys.stderr)
        raise exc

    # Locate JSON in stdout (in case Cargo emitted compilation warnings)
    raw_out = proc.stdout
    json_start = raw_out.find("{")
    if json_start == -1:
        print(f"Error: No JSON found in harness output:\n{raw_out}", file=sys.stderr)
        sys.exit(1)

    json_str = raw_out[json_start:]
    try:
        data = json.loads(json_str)
        if target_cpu:
            data["target_cpu"] = target_cpu
        return data
    except json.JSONDecodeError as err:
        print(f"Error parsing harness JSON: {err}\nOutput was:\n{json_str}", file=sys.stderr)
        sys.exit(1)


class Sweep:
    """Runs a report's cells, one load window each, and writes the load sidecar.

    Every variant the sweep needs is built in the constructor, before the first
    window opens. `run` brackets one harness process with a window whose own
    CPU is the reaped child's (`begin_cell` / `end_cell`); the bookkeeping,
    including merging a window too short to resolve into the next, is
    `bench_windowed.Windower`'s.
    """

    def __init__(self, root: Path, variants: List[Optional[str]], build=build_harness,
                 runner=run_benchmark_harness):
        self.root, self.runner = root, runner
        self.exes: Dict[str, Path] = {}
        self.exe_for: Dict[Optional[str], Path] = {}
        for v in variants:
            key = variant_key(v)
            if key not in self.exes:
                print(f"Building {HARNESS_EXAMPLE} ({key})...", file=sys.stderr)
                self.exes[key] = build(root, v)
            self.exe_for[v] = self.exes[key]
        self.prov = bp.new_provenance(
            "head_to_head", LOAD_ISSUE,
            "median of interleaved rounds per arm; no interval (see bench_report.py)",
            root, harness=f"crates/expanse/examples/{HARNESS_EXAMPLE}.rs",
            window_mode="process", load_windows=True,
            variants=sorted(self.exes))
        self.win = bench_windowed.Windower(
            lambda label, prev: bp.begin_cell(self.prov, label), bp.end_cell)
        self.cells: List[Dict[str, Any]] = []

    def run(self, pop: int, dist: str, rounds: int, target_cpu: Optional[str]) -> Dict[str, Any]:
        cid = f"c{len(self.cells)}:{variant_key(target_cpu)}/pop={pop}/dist={dist}"
        self.cells.append({"id": cid, "pop": pop, "dist": dist, "rounds": rounds,
                           "target_cpu": target_cpu})
        self.win.feed("begin", cid)
        try:
            return self.runner(pop=pop, dist=dist, rounds=rounds, root=self.root,
                               target_cpu=target_cpu, exe=self.exe_for[target_cpu])
        finally:
            self.win.feed("end", cid)

    def record(self) -> Dict[str, Any]:
        self.win.finish()
        bp.add_load(self.prov, "end")
        return load_record(self.prov, self.cells, self.win.windows, self.win.findings)


def load_record(prov: Dict[str, Any], cells: List[Dict[str, Any]], windows: List[Dict[str, Any]],
                findings: List[str]) -> Dict[str, Any]:
    """The sidecar: provenance, and each cell carrying the load of the window that timed it.

    A merged window covers several cells; each of them carries that window's
    load and names it in `load_window`. A cell no window covers is a finding.
    """
    by_id: Dict[str, Dict[str, Any]] = {}
    for w in windows:
        for cid in w.get("merged", [w["id"]]):
            by_id[cid] = w
    out_cells, findings = [], list(findings)
    for c in cells:
        w = by_id.get(c["id"])
        cell = dict(c)
        if w is None:
            findings.append(f"cell {c['id']!r} has no load window")
        else:
            cell["load"] = w["load"]
            if "merged" in w:
                cell["load_window"] = w["id"]
        out_cells.append(cell)
    prov = dict(prov)
    prov["windows"] = windows
    prov["load_status"] = "inadmissible" if findings else "ok"
    prov["load_findings"] = findings
    return {"provenance": prov, "cells": out_cells}


def default_load_path(output: Optional[str]) -> Path:
    """`head-to-head-load.json`, or `<output stem>-load.json` beside `--output`."""
    if output:
        out = Path(output)
        return out.with_name(f"{out.stem}-load.json")
    return Path("head-to-head-load.json")


# Parity band shared by the ratio markers and the printed legend ("⚪ Parity
# (±5%)"). The code and the legend must agree: the legend is the published
# contract, and a wider band would hide real losses behind a parity marker.
# The band is applied to the reported ratio itself, so "±5%" means the printed
# `subject ÷ baseline` sits inside [0.95, 1.05] — the same interval the legend
# names.
PARITY_BAND = 0.05

# Rendered in place of any cell whose input the run did not supply. A missing
# input must never reach the reader as a number, and `0.00` is a number (§8.1).
NOT_MEASURED = "*not measured*"

# Column-direction labels. Every header that carries a measurement carries one
# of these, because the row mixes ns (lower better), Mops/s (higher better) and
# B/key (lower better) and nothing else marks the switch (#450, problem 2).
LOWER_BETTER = "lower better"
HIGHER_BETTER = "higher better"

# Markers, and the word each one stands for. The word is derived from the
# classification, never written per row.
MARKERS = {"win": "🟢", "parity": "⚪", "loss": "🔴"}


def ratio_header(subject: str, baseline: str, unit: str, lower_is_better: bool) -> str:
    """Spells the division out in the column header, the way `perf_report.py`
    does for the symmetric-twin table.

    A bare ratio under an explicit division cannot be read backwards. Left
    implicit, `1.11x` in this table meant "Expanse 11% faster" while the same
    figure for the same operation against the same competitor meant "11%
    slower" in `docs/BENCHMARKING.md` (#450, problem 1). The division here is
    subject-over-baseline, matching `results/baseline_vs_libjudy.json`
    (`"denominator_arm": "stock"`), so one number now reads one way repo-wide.
    """
    direction = LOWER_BETTER if lower_is_better else HIGHER_BETTER
    side = "faster" if lower_is_better else "better"
    hand = "below" if lower_is_better else "above"
    return (
        f"{subject} &divide; {baseline} ({unit}, {direction}; "
        f"{hand} 1.000 = {subject} {side})"
    )


def classify_ratio(
    subject_val: float,
    baseline_val: float,
    lower_is_better: bool,
    noise_floor: float = PARITY_BAND,
) -> Optional[Tuple[str, float]]:
    """Classifies a measured pair as ('win'|'parity'|'loss', subject/baseline).

    The returned ratio is always `subject / baseline`, whatever the metric
    measures — that is the division the column header names, so the printed
    number cannot be read backwards.

    Whether that ratio is a win is a *separate* question, and it is answered by
    `lower_is_better` alone: on a latency or bytes-per-key column the win is a
    ratio below 1, on a throughput column it is a ratio above 1. Grading by
    "above 1 is good" inverts the marker on every lower-is-better column, which
    is why this argument is required rather than defaulted.

    Returns None when either side is missing or non-positive: a pair that was
    not measured must not render as a ratio (§8.1).
    """
    if subject_val <= 0.0 or baseline_val <= 0.0:
        return None
    ratio = subject_val / baseline_val
    # Margin in the metric's own direction: positive means the subject is ahead.
    margin = (1.0 - ratio) if lower_is_better else (ratio - 1.0)
    if margin >= noise_floor:
        return "win", ratio
    if margin <= -noise_floor:
        return "loss", ratio
    return "parity", ratio


def fmt_ratio(
    subject_val: float,
    baseline_val: float,
    lower_is_better: bool,
    noise_floor: float = PARITY_BAND,
) -> str:
    """Formats `subject / baseline` with the marker its column's direction earns."""
    classified = classify_ratio(subject_val, baseline_val, lower_is_better, noise_floor)
    if classified is None:
        return NOT_MEASURED
    kind, ratio = classified
    number = f"**{ratio:.3f}x**" if kind == "win" else f"{ratio:.3f}x"
    return f"{number} {MARKERS[kind]}"


def fmt_verdict(
    subject_val: float,
    baseline_val: float,
    lower_is_better: bool,
    subject_label: str,
    noise_floor: float = PARITY_BAND,
) -> str:
    """Spells the classification out in words, so the marker is not the only
    thing carrying the direction."""
    classified = classify_ratio(subject_val, baseline_val, lower_is_better, noise_floor)
    if classified is None:
        return NOT_MEASURED
    kind, _ = classified
    if kind == "parity":
        return f"{MARKERS[kind]} parity (±{noise_floor * 100:.0f}%)"
    word = ("faster" if kind == "win" else "slower") if lower_is_better else (
        "ahead" if kind == "win" else "behind"
    )
    return f"{MARKERS[kind]} {subject_label} {word}"


def fmt_metric(
    container: Dict[str, Any], key: str, *, digits: int = 2, bold: bool = False
) -> str:
    """Formats one measured metric, or says it was not measured.

    `container.get(key, 0.0)` used to render an absent metric as `0.00`, which
    reads as a measurement of zero rather than as an absence (§8.1).
    """
    value = container.get(key)
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        return NOT_MEASURED
    value = float(value)
    if value != value or value <= 0.0:  # NaN or non-positive
        return NOT_MEASURED
    text = f"{value:.{digits}f}"
    return f"**{text}**" if bold else text


SUBJECT_LABEL = "`ExpanseMap`"

# Competitor arms in reporting order: JSON key, printed label. The subject
# (`expanse`) is not in this list.
BASELINE_ARMS = [
    ("btree", "`std::BTreeMap`"),
    ("hashbrown", "`hashbrown::HashMap`"),
    ("libjudy", "`libjudy (stock JudyL)`"),
]


def _lookup_ns(container: Optional[Dict[str, Any]]) -> float:
    """Point-lookup latency, or 0.0 when the arm did not report one. 0.0 is
    only ever used as the "no pair" sentinel `classify_ratio` rejects; it is
    never printed (that is what `fmt_metric` is for)."""
    if not isinstance(container, dict):
        return 0.0
    value = container.get("lookup_ns")
    return float(value) if isinstance(value, (int, float)) and not isinstance(value, bool) else 0.0


def derive_summary(results: Dict[str, Any]) -> List[str]:
    """Derives win/parity/loss counts for point-lookup latency strictly from
    the parsed results. No prose beyond the tallies: the tables carry the data."""
    lines: List[str] = []
    for key, label in BASELINE_ARMS:
        tally = {"win": 0, "parity": 0, "loss": 0}
        total = 0
        for res in results.values():
            classified = classify_ratio(
                _lookup_ns(res.get("expanse")),
                _lookup_ns(res.get(key)),
                lower_is_better=True,
            )
            if classified is None:
                continue
            tally[classified[0]] += 1
            total += 1
        if total:
            lines.append(
                f"- vs {label}: **{tally['win']} faster · {tally['parity']} parity · "
                f"{tally['loss']} slower** (point lookup, {total} distribution(s))"
            )
    if not lines:
        return []
    return [
        "---",
        f"**Measured Summary** (derived from the tables above; parity band ±{PARITY_BAND * 100:.0f}%):",
        *lines,
    ]


def describe_rounds(data: Dict[str, Any]) -> str:
    """Describes the harness's round methodology from the artifact itself.

    The round count is reported by the harness (`"rounds"` in its JSON), never
    assumed here: a stamped constant would keep asserting a methodology after
    the harness stopped implementing it, which is exactly how `--rounds` came
    to be documented while the harness silently ignored it. Artifacts predating
    the field get no round claim at all rather than a fabricated one.
    """
    rounds = data.get("rounds")
    if not isinstance(rounds, int) or rounds < 1:
        return "Round count not reported by this artifact."
    if rounds == 1:
        return "Single execution round per arm (no median)."
    return f"{rounds} interleaved execution rounds per arm, per-metric median reported."


def rounds_footer_lead(data: Dict[str, Any]) -> str:
    """Names what the tables hold, for the no-confidence-interval footer.

    Derived from the artifact for the same reason as `describe_rounds`: the
    footer's "medians of interleaved rounds" is a methodology claim, and a
    single-round run has no median to report.
    """
    rounds = data.get("rounds")
    if not isinstance(rounds, int) or rounds < 1:
        return "These measurements"
    if rounds == 1:
        return "Single-round measurements"
    return f"Medians of {rounds} interleaved rounds"


# The harness every figure in this report comes from. `bench_report.py` runs
# exactly one (`run_benchmark_harness` -> `--example bench_lookup_compare`), so
# the workload shape is a property of the whole report rather than per row.
REPORT_WORKLOAD_ID = "example_bench_lookup_compare"


def workload_shape_line(workload_id: str = REPORT_WORKLOAD_ID) -> str:
    """The declared shape of the harness behind these numbers, for the header.

    A bare `35.8 ns` does not say it came from 4,096 probes at 8x reuse with the
    pointer never dereferenced. Two figures whose probe cardinality differed by
    366x once read as one observation in this repository's own documentation;
    printing the shape beside the number is what stops a reader having to open
    the harness to find out (#487, #453 item A).

    Returns a loud marker rather than silence when the declaration cannot be
    resolved: a number rendering as though its shape were known is the failure
    this exists to prevent (§8.1).
    """
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        from check_bench_shapes import collect_shapes, get_repo_root, summarize_shape

        shape = collect_shapes(get_repo_root()).get(workload_id)
    except Exception:
        shape = None
    if not shape:
        return (
            f"> **Workload shape**: ⚠️ unresolved — `{workload_id}` has no readable "
            "`# Workload shape` declaration; these numbers carry no stated shape"
        )
    return f"> **Workload shape** (`{workload_id}`): {summarize_shape(shape)}"


def render_markdown(
    data: Dict[str, Any], baseline: Optional[Dict[str, Any]] = None
) -> str:
    """Formats benchmark results into GitHub Flavored Markdown tables.

    `baseline` is an optional `scripts/bench_baseline.py` artifact; when given,
    its BCa interval table is appended so a wall-clock claim in the same comment
    shows the interval it is gated on (§8.4).
    """
    pop = data.get("pop", 1_000_000)
    system = data.get("system", {})
    os_name = system.get("os", "unknown")
    arch = system.get("arch", "unknown")
    results = data.get("results", {})

    lines: List[str] = [
        "## ⚡ Head-to-Head Benchmark Comparison Report",
        "",
        f"> **Target Population**: $N = {pop:,}$ keys · **System**: `{os_name}/{arch}`",
        f"> **Methodology**: {describe_rounds(data)}",
        workload_shape_line(),
        "> **Reading the columns**: every measurement header carries its unit and its "
        f"direction (`ns` and `B/key` are {LOWER_BETTER}, `Mops/s` is {HIGHER_BETTER}). "
        "Every ratio header names the division it prints, and each ratio is graded by "
        "its own column's direction — not by whether it exceeds 1.",
        "",
    ]

    for dist, res in results.items():
        exp = res.get("expanse", {})

        lines.extend([
            f"### Distribution: `{dist}`",
            "",
            "**Measurements** — absolute values only, one row per container.",
            "",
            f"| Container | Point Lookup (ns, {LOWER_BETTER}) | Cold Insert (Mops/s, {HIGHER_BETTER}) "
            f"| Full Iter (Mops/s, {HIGHER_BETTER}) | Range Scan (Mops/s, {HIGHER_BETTER}) "
            f"| Memory (B/key, {LOWER_BETTER}) |",
            "|---|---:|---:|---:|---:|---:|",
        ])

        # Point-lookup throughput (`lookup_mops`) is the reciprocal of
        # `lookup_ns` — one measurement rendered twice, in two units, pointing
        # in opposite directions (#450, problem 3). The latency column is kept
        # because it is what the comparison table below divides.
        lines.append(
            f"| **{SUBJECT_LABEL}** | {fmt_metric(exp, 'lookup_ns', bold=True)} "
            f"| {fmt_metric(exp, 'insert_mops', bold=True)} | {fmt_metric(exp, 'iter_mops', bold=True)} "
            f"| {fmt_metric(exp, 'range_mops', bold=True)} | {fmt_metric(exp, 'bytes_per_key', bold=True)} |"
        )
        for key, label in BASELINE_ARMS:
            arm = res.get(key)
            if not isinstance(arm, dict):
                continue
            # Range scan is not an operation these containers expose; that is a
            # structural absence, distinct from a metric this run did not take.
            range_cell = (
                "*N/A (unsupported)*"
                if key in ("hashbrown", "libjudy")
                else fmt_metric(arm, "range_mops")
            )
            lines.append(
                f"| {label} | {fmt_metric(arm, 'lookup_ns')} | {fmt_metric(arm, 'insert_mops')} "
                f"| {fmt_metric(arm, 'iter_mops')} | {range_cell} | {fmt_metric(arm, 'bytes_per_key')} |"
            )

        lines.extend([
            "",
            "**Point-lookup comparison** — one row per baseline, one division, named in "
            "the header. Interleaving three comparisons into one row put the `Baseline` "
            "cell in a different column for each competitor and made the reader "
            "re-anchor per column (#450, problem 4).",
            "",
            f"| Baseline | Baseline point lookup (ns, {LOWER_BETTER}) "
            f"| {ratio_header(SUBJECT_LABEL, 'baseline', 'point-lookup ns', lower_is_better=True)} "
            f"| Result |",
            "|---|---:|---:|---|",
        ])

        exp_lkp = _lookup_ns(exp)
        for key, label in BASELINE_ARMS:
            arm = res.get(key)
            if not isinstance(arm, dict):
                # No live source for this arm in this run. It gets a stated
                # absence, never a number carried in from anywhere else (§8.1).
                lines.append(f"| {label} | {NOT_MEASURED} | {NOT_MEASURED} | {NOT_MEASURED} |")
                continue
            base_lkp = _lookup_ns(arm)
            lines.append(
                f"| {label} | {fmt_metric(arm, 'lookup_ns')} "
                f"| {fmt_ratio(exp_lkp, base_lkp, lower_is_better=True)} "
                f"| {fmt_verdict(exp_lkp, base_lkp, True, SUBJECT_LABEL)} |"
            )

        absent = [label for key, label in BASELINE_ARMS if not isinstance(res.get(key), dict)]
        if absent:
            lines.extend([
                "",
                f"<sub>Not measured in this run: {', '.join(absent)}. The arm produced no "
                "result for this distribution, so it carries no ratio.</sub>",
            ])

        lines.append("")

    lines.extend(derive_summary(results))
    lines.extend([
        "",
        f"<sub>{rounds_footer_lead(data)} — no sampling distribution, so no confidence "
        "interval and no §8.4 wall-clock claim rests on this table alone. The "
        "interval-bearing arms are in the BCa section (<code>scripts/bench_baseline.py</code>), "
        "when a baseline artifact is supplied.</sub>",
        "",
        f"<sub>🟢 subject ahead · ⚪ parity (±{PARITY_BAND * 100:.0f}%) · 🔴 subject behind. "
        "Every ratio is <code>subject &divide; baseline</code> in the baseline's own unit, as "
        "its header states, and the marker is chosen by that column's direction — on a "
        "latency column the win is a ratio below 1. Generated automatically via "
        "<code>scripts/bench_report.py</code>.</sub>\n",
    ])

    if baseline is not None:
        lines.extend(["", render_baseline_ci_section(baseline)])

    return "\n".join(lines)


def load_baseline_artifact(path: str | Path) -> Dict[str, Any]:
    """Loads a `results/baseline_*.json` artifact, failing loudly on a bad schema."""
    with open(path, "r", encoding="utf-8") as handle:
        artifact = json.load(handle)
    if artifact.get("schema") != bench_baseline.SCHEMA:
        raise ValueError(
            f"{path}: schema {artifact.get('schema')!r} is not "
            f"{bench_baseline.SCHEMA!r}; not a bench_baseline artifact"
        )
    return artifact


def render_baseline_ci_section(artifact: Dict[str, Any]) -> str:
    """Renders the BCa interval table for a committed baseline artifact.

    Delegates to `bench_baseline.render_markdown` so the report and the gate
    read the same numbers from the same definition (§8.4).
    """
    return bench_baseline.render_markdown(artifact)


def render_table(data: Dict[str, Any]) -> str:
    """Formats benchmark results into plain text terminal tables."""
    pop = data.get("pop", 1_000_000)
    system = data.get("system", {})
    results = data.get("results", {})

    lines: List[str] = [
        "\n================================================================================",
        f"  HEAD-TO-HEAD BENCHMARK REPORT (N = {pop:,}, {system.get('os')}/{system.get('arch')})",
        "================================================================================",
    ]

    def cell(container: Dict[str, Any], key: str, width: int) -> str:
        text = fmt_metric(container, key)
        return f"{('n/m' if text == NOT_MEASURED else text):>{width}}"

    arms = [("expanse", "ExpanseMap"), ("hashbrown", "hashbrown (HashMap)"),
            ("btree", "BTreeMap (std)"), ("libjudy", "libjudy (stock)")]

    for dist, res in results.items():
        lines.append(f"\n[ Distribution: {dist} ]")
        # Units and directions in the header; `Lookup (Mops)` dropped as the
        # reciprocal of `Lookup (ns)` (#450, problems 2 and 3).
        lines.append(
            f"{'Container':<22} | {'Lookup ns v':>13} | {'Insert Mops ^':>13}"
            f" | {'Iter Mops ^':>13} | {'Range Mops ^':>13} | {'B/key v':>9}"
        )
        lines.append(f"{'-' * 22}-+-{'-' * 13}-+-{'-' * 13}-+-{'-' * 13}-+-{'-' * 13}-+-{'-' * 9}")

        for key, label in arms:
            arm = res.get(key)
            if not isinstance(arm, dict):
                continue
            range_cell = f"{'N/A':>13}" if key in ("hashbrown", "libjudy") else cell(arm, "range_mops", 13)
            lines.append(
                f"{label:<22} | {cell(arm, 'lookup_ns', 13)} | {cell(arm, 'insert_mops', 13)}"
                f" | {cell(arm, 'iter_mops', 13)} | {range_cell} | {cell(arm, 'bytes_per_key', 9)}"
            )

    lines.append("")
    lines.append("  v = lower is better   ^ = higher is better   n/m = not measured   N/A = unsupported")
    lines.append("================================================================================\n")
    return "\n".join(lines)


def render_extended_pop_markdown(pop_reports: List[Dict[str, Any]]) -> str:
    """Formats multi-population scaling comparison tables."""
    lines: List[str] = [
        "## 📈 Multi-Population Scaling Report ($N = 10\\text{k} \\rightarrow 100\\text{k} \\rightarrow 1\\text{M}$)",
        "",
        "> Evaluates cache-hierarchy scaling transitions from L2 residency ($N=10\\text{k}$) to LLC ($N=100\\text{k}$) and cold DRAM ($N=1\\text{M}$).",
        "",
    ]

    lines.append(
        "> Measurement headers carry unit and direction; ratio headers name the division "
        "they print, and each ratio is graded by its own column's direction."
    )
    lines.append("")

    # Distributions come from the artifacts, not from a stamped list: a
    # `--dist random` sweep must not render three headers and two empty tables.
    dists: List[str] = []
    for data in pop_reports:
        for dist in data.get("results", {}):
            if dist not in dists:
                dists.append(dist)

    for dist in dists:
        lines.extend([
            f"### Distribution: `{dist}` Scaling Matrix",
            "",
            "**Measurements**",
            "",
            f"| Population ($N$) | Container | Point Lookup (ns, {LOWER_BETTER}) "
            f"| Cold Insert (Mops/s, {HIGHER_BETTER}) | Full Iter (Mops/s, {HIGHER_BETTER}) "
            f"| Range Scan (Mops/s, {HIGHER_BETTER}) | Memory (B/key, {LOWER_BETTER}) |",
            "|---|---|---:|---:|---:|---:|---:|",
        ])

        containers = [("expanse", f"**{SUBJECT_LABEL}**", True)] + [
            (key, label, False) for key, label in BASELINE_ARMS if key != "hashbrown"
        ]

        for data in pop_reports:
            pop_label = f"**{data.get('pop', 0):,}**"
            res = data.get("results", {}).get(dist, {})
            for key, label, bold in containers:
                arm = res.get(key)
                if not isinstance(arm, dict):
                    continue
                range_cell = (
                    "*N/A (unsupported)*" if key == "libjudy" else fmt_metric(arm, "range_mops", bold=bold)
                )
                lines.append(
                    f"| {pop_label} | {label} | {fmt_metric(arm, 'lookup_ns', bold=bold)} "
                    f"| {fmt_metric(arm, 'insert_mops', bold=bold)} | {fmt_metric(arm, 'iter_mops', bold=bold)} "
                    f"| {range_cell} | {fmt_metric(arm, 'bytes_per_key', bold=bold)} |"
                )

        lines.extend([
            "",
            "**Point-lookup comparison**",
            "",
            f"| Population ($N$) | Baseline | Baseline point lookup (ns, {LOWER_BETTER}) "
            f"| {ratio_header(SUBJECT_LABEL, 'baseline', 'point-lookup ns', lower_is_better=True)} |",
            "|---|---|---:|---:|",
        ])

        for data in pop_reports:
            pop_label = f"**{data.get('pop', 0):,}**"
            res = data.get("results", {}).get(dist, {})
            exp_lkp = _lookup_ns(res.get("expanse"))
            for key, label in BASELINE_ARMS:
                if key == "hashbrown":
                    continue
                arm = res.get(key)
                if not isinstance(arm, dict):
                    lines.append(f"| {pop_label} | {label} | {NOT_MEASURED} | {NOT_MEASURED} |")
                    continue
                lines.append(
                    f"| {pop_label} | {label} | {fmt_metric(arm, 'lookup_ns')} "
                    f"| {fmt_ratio(exp_lkp, _lookup_ns(arm), lower_is_better=True)} |"
                )

        lines.append("")

    return "\n".join(lines)


def render_arch_sweep_markdown(arch_reports: List[Dict[str, Any]]) -> str:
    """Formats target CPU microarchitecture scaling comparison tables."""
    lines: List[str] = [
        "## 🏎️ Target CPU Microarchitecture Scaling Matrix",
        "",
        "> Evaluates instruction set features: `baseline` (generic x86-64-v1) vs `x86-64-v2` (+POPCNT) vs `x86-64-v3` (+AVX2/BMI2) vs `native`.",
        "",
        f"| Distribution | Target CPU | Expanse Lookup (ns, {LOWER_BETTER}) "
        f"| Expanse Insert (Mops/s, {HIGHER_BETTER}) "
        f"| {ratio_header('this arm', 'first CPU arm', 'point-lookup ns', lower_is_better=True)} "
        f"| {ratio_header(SUBJECT_LABEL, '`libjudy (stock JudyL)`', 'point-lookup ns', lower_is_better=True)} |",
        "|---|---|---:|---:|---:|---:|",
    ]

    dists: List[str] = []
    for data in arch_reports:
        for dist in data.get("results", {}):
            if dist not in dists:
                dists.append(dist)

    for dist in dists:
        baseline_ns: Optional[float] = None

        for data in arch_reports:
            cpu = data.get("target_cpu", "baseline")
            res = data.get("results", {}).get(dist, {})
            exp = res.get("expanse", {})
            judy = res.get("libjudy")

            exp_lkp = _lookup_ns(exp)

            if baseline_ns is None:
                baseline_ns = exp_lkp
                vs_base = "reference arm (1.000x)"
            else:
                vs_base = fmt_ratio(exp_lkp, baseline_ns, lower_is_better=True)

            vs_judy = (
                fmt_ratio(exp_lkp, _lookup_ns(judy), lower_is_better=True)
                if isinstance(judy, dict)
                else NOT_MEASURED
            )

            lines.append(
                f"| `{dist}` | `{cpu}` | {fmt_metric(exp, 'lookup_ns', bold=True)} "
                f"| {fmt_metric(exp, 'insert_mops', bold=True)} | {vs_base} | {vs_judy} |"
            )

    lines.extend([
        "",
        f"<sub>🟢 subject ahead · ⚪ parity (±{PARITY_BAND * 100:.0f}%) · 🔴 subject behind, "
        "graded on the latency direction each header states. "
        f"{NOT_MEASURED} means the arm produced no result in this run — never a "
        "figure carried in from elsewhere.</sub>",
        "",
    ])
    return "\n".join(lines)


def self_test() -> int:
    """Unit-style checks for the pure rendering helpers. No cargo required."""
    # 1. Legend/band agreement: the parity band is the ±5% the legend prints.
    assert abs(PARITY_BAND - 0.05) < 1e-12, "parity band must match the printed ±5% legend"

    # 1a. The reported ratio is always subject/baseline, whichever way the
    #     metric points. This is what the column header names.
    assert classify_ratio(50.0, 100.0, lower_is_better=True)[1] == 0.5
    assert classify_ratio(50.0, 100.0, lower_is_better=False)[1] == 0.5
    assert classify_ratio(200.0, 100.0, lower_is_better=True)[1] == 2.0

    # 1b. Direction, not magnitude, decides the marker. The same ratio must
    #     grade opposite ways on a lower-better and a higher-better column —
    #     this is the sign inversion that put a 🟢 on a latency ratio above 1.
    assert classify_ratio(110.0, 100.0, lower_is_better=True)[0] == "loss"
    assert classify_ratio(110.0, 100.0, lower_is_better=False)[0] == "win"
    assert classify_ratio(90.0, 100.0, lower_is_better=True)[0] == "win"
    assert classify_ratio(90.0, 100.0, lower_is_better=False)[0] == "loss"
    # A latency ratio above 1 is a loss and must be marked as one.
    assert fmt_ratio(110.0, 100.0, lower_is_better=True) == "1.100x 🔴"
    assert fmt_ratio(90.0, 100.0, lower_is_better=True) == "**0.900x** 🟢"
    # The same numbers on a throughput column flip both marker and grade.
    assert fmt_ratio(110.0, 100.0, lower_is_better=False) == "**1.100x** 🟢"
    assert fmt_ratio(90.0, 100.0, lower_is_better=False) == "0.900x 🔴"
    # Words agree with the marker.
    assert fmt_verdict(110.0, 100.0, True, "`ExpanseMap`") == "🔴 `ExpanseMap` slower"
    assert fmt_verdict(90.0, 100.0, True, "`ExpanseMap`") == "🟢 `ExpanseMap` faster"
    assert fmt_verdict(100.0, 100.0, True, "`ExpanseMap`") == "⚪ parity (±5%)"

    # 1c. Parity band applies to the printed ratio: [0.95, 1.05] is parity.
    assert classify_ratio(104.0, 100.0, lower_is_better=True)[0] == "parity"
    assert classify_ratio(96.0, 100.0, lower_is_better=True)[0] == "parity"
    assert classify_ratio(105.0, 100.0, lower_is_better=True)[0] == "loss"
    assert classify_ratio(95.0, 100.0, lower_is_better=True)[0] == "win"

    # 1d. An unmeasured pair is stated as such, never rendered as a ratio or a 0.
    assert classify_ratio(0.0, 100.0, lower_is_better=True) is None
    assert fmt_ratio(0.0, 100.0, lower_is_better=True) == NOT_MEASURED
    assert fmt_ratio(100.0, 0.0, lower_is_better=True) == NOT_MEASURED
    assert fmt_verdict(0.0, 100.0, True, "`ExpanseMap`") == NOT_MEASURED

    # 1e. A missing or absurd metric is an absence, not a measurement of zero.
    assert fmt_metric({}, "lookup_ns") == NOT_MEASURED
    assert fmt_metric({"lookup_ns": 0.0}, "lookup_ns") == NOT_MEASURED
    assert fmt_metric({"lookup_ns": None}, "lookup_ns") == NOT_MEASURED
    assert fmt_metric({"lookup_ns": "25.30"}, "lookup_ns") == NOT_MEASURED
    assert fmt_metric({"lookup_ns": 25.3}, "lookup_ns") == "25.30"
    assert fmt_metric({"lookup_ns": 25.3}, "lookup_ns", bold=True) == "**25.30**"

    # 1f. Every ratio header states its division and its direction.
    header = ratio_header("`ExpanseMap`", "baseline", "point-lookup ns", lower_is_better=True)
    assert "&divide;" in header, header
    assert "lower better" in header and "below 1.000" in header, header
    up = ratio_header("`ExpanseMap`", "baseline", "Mops/s", lower_is_better=False)
    assert "higher better" in up and "above 1.000" in up, up

    # 2. Summary is derived strictly from the parsed results.
    data = {
        "pop": 10_000,
        "system": {"os": "linux", "arch": "x86_64"},
        "results": {
            "sequential": {
                "expanse": {"lookup_ns": 10.0},
                "hashbrown": {"lookup_ns": 12.0},
                "btree": {"lookup_ns": 40.0},
                "libjudy": {"lookup_ns": 11.0},
            },
            "random": {
                "expanse": {"lookup_ns": 30.0},
                "hashbrown": {"lookup_ns": 15.0},
                "btree": {"lookup_ns": 60.0},
                "libjudy": {"lookup_ns": 27.0},
            },
        },
    }
    summary = "\n".join(derive_summary(data["results"]))
    assert "vs `std::BTreeMap`: **2 faster · 0 parity · 0 slower**" in summary, summary
    assert "vs `hashbrown::HashMap`: **1 faster · 0 parity · 1 slower**" in summary, summary
    assert "vs `libjudy (stock JudyL)`: **1 faster · 0 parity · 1 slower**" in summary, summary

    # 3. The rendered report carries no hardcoded findings prose.
    md = render_markdown(data)
    for fabricated in (
        "4× to 10×",
        "3.4× smaller",
        "outperforms stock",
        "across all key distributions",
        "Key Architectural Findings",
    ):
        assert fabricated not in md, f"hardcoded findings prose leaked into report: {fabricated!r}"
    assert "Measured Summary" in md
    assert "±5%" in md

    # 3a. Rendered table: the division is in the header, the units carry their
    #     direction, and the reciprocal pair is gone (#450, problems 1-3).
    assert "`ExpanseMap` &divide; baseline (point-lookup ns, lower better" in md, md
    assert "Lookup vs BTree" not in md and "vs libjudy" not in md.replace("&divide;", ""), md
    assert f"Point Lookup (ns, {LOWER_BETTER})" in md
    assert f"Cold Insert (Mops/s, {HIGHER_BETTER})" in md
    assert f"Memory (B/key, {LOWER_BETTER})" in md
    assert "Lookup (Mops/s)" not in md, "the ns/Mops reciprocal pair must be rendered once"
    # 3b. The baseline anchor is the row label now, not a `Baseline` cell that
    #     moves column-to-column and forces the reader to re-anchor per column.
    body_rows = [ln for ln in md.splitlines() if ln.startswith(("| `", "| **"))]
    assert not any(
        cell.strip() == "Baseline" for ln in body_rows for cell in ln.split("|")
    ), "no data cell may carry a bare `Baseline` anchor"

    # 3c. End-to-end direction check on the fixture above. `random` has Expanse
    #     at 30 ns against libjudy's 27 ns -> ratio 1.111, a LOSS; `sequential`
    #     has 10 ns against 11 ns -> 0.909, a WIN. The losing ratio exceeds 1,
    #     which is exactly the shape that used to be marked 🟢.
    assert "| 1.111x 🔴 | 🔴 `ExpanseMap` slower |" in md, md
    assert "| **0.909x** 🟢 | 🟢 `ExpanseMap` faster |" in md, md

    # 3d. An arm with no live source in this run renders as an absence, never
    #     as a number from another run or a zero (§8.1).
    no_judy = {k: {a: v for a, v in res.items() if a != "libjudy"} for k, res in data["results"].items()}
    md_no_judy = render_markdown({**data, "results": no_judy})
    assert "libjudy" in md_no_judy, "an unavailable arm must still be named"
    assert f"| `libjudy (stock JudyL)` | {NOT_MEASURED} | {NOT_MEASURED} | {NOT_MEASURED} |" in md_no_judy
    assert "Not measured in this run: `libjudy (stock JudyL)`" in md_no_judy
    assert not any(
        cell.strip().strip("*") in ("0.00", "0.000x")
        for ln in md_no_judy.splitlines()
        if ln.startswith("|")
        for cell in ln.split("|")
    ), "an absent arm must not render as a zero measurement"

    # 4. No results at all -> no summary block, not a fabricated one.
    assert derive_summary({}) == []

    # 5. The interleaved-median tables disclaim the interval they do not have,
    #    and a supplied baseline artifact surfaces its CI in the same report.
    assert "no confidence interval" in md, md
    artifact = bench_baseline.build_artifact(
        [
            {"id": "map_get/random/1000000/expanse", "samples_ns": [100.0 + i for i in range(40)]},
            {"id": "map_get/random/1000000/btree", "samples_ns": [140.0 + i for i in range(40)]},
        ],
        suite="self-test",
        host_desc="synthetic fixture host",
        commit="0" * 40,
        run_id="self-test",
        confidence=0.95,
        num_resamples=1000,
        seed=42,
        min_n=3,
        fixture=True,
    )
    md_ci = render_markdown(data, artifact)
    assert "Wall-Clock BCa Confidence Intervals" in md_ci
    assert "`map_get/random/1000000/expanse`" in md_ci
    assert "FIXTURE ARTIFACT" in md_ci, "a fixture artifact must be labelled as one"
    # Point estimate enclosed by the interval it is printed next to (§8.4).
    for arm in artifact["arms"]:
        assert arm["ci_lower_ns"] <= arm["point_ns"] <= arm["ci_upper_ns"], arm
    # The head-to-head tables are unchanged by the appended section.
    assert md_ci.startswith(md.split(f"<sub>{rounds_footer_lead(data)}")[0])
    # A non-artifact JSON is rejected rather than rendered as an empty CI table.
    import tempfile as _tempfile

    with _tempfile.TemporaryDirectory() as _tmp:
        _bad = Path(_tmp) / "not_an_artifact.json"
        _bad.write_text(json.dumps({"schema": "something.else"}), encoding="utf-8")
        try:
            load_baseline_artifact(_bad)
        except ValueError as exc:
            assert "not a bench_baseline artifact" in str(exc), exc
        else:
            raise AssertionError("a foreign schema must be rejected loudly")
    # 6. The methodology line and the footer lead are derived from the artifact,
    #    never stamped — that stamping is how `--rounds` stayed documented while
    #    the harness ignored it.
    assert describe_rounds({"rounds": 5}) == (
        "5 interleaved execution rounds per arm, per-metric median reported."
    )
    assert describe_rounds({"rounds": 1}) == "Single execution round per arm (no median)."
    # An artifact that reports no round count must not have one invented for it.
    for absent in ({}, {"rounds": None}, {"rounds": 0}, {"rounds": "3"}):
        assert describe_rounds(absent) == "Round count not reported by this artifact.", absent
    assert rounds_footer_lead({"rounds": 3}) == "Medians of 3 interleaved rounds"
    assert rounds_footer_lead({"rounds": 1}) == "Single-round measurements"
    assert rounds_footer_lead({}) == "These measurements"
    md_5 = render_markdown({**data, "rounds": 5})
    assert "5 interleaved execution rounds" in md_5
    assert "Medians of 5 interleaved rounds" in md_5
    # A single-round artifact must not be described as a median of rounds.
    md_1 = render_markdown({**data, "rounds": 1})
    assert "interleaved" not in md_1, md_1
    assert "Medians" not in md_1, md_1
    # An artifact carrying no round count gets no round claim at all.
    assert "interleaved" not in render_markdown(data), (
        "a report whose artifact carries no round count must not claim interleaved rounds"
    )

    # 7. The sweep tables carry the same contract: division in the header,
    #    direction-graded markers, no reciprocal pair, no stamped distribution
    #    list. They were rendered by the same inverted-sign helper and had no
    #    coverage at all before.
    sweep = [
        {"pop": 10_000, "results": {"random": {
            "expanse": {"lookup_ns": 30.0, "insert_mops": 20.0},
            "btree": {"lookup_ns": 60.0},
            "libjudy": {"lookup_ns": 27.0},
        }}},
    ]
    md_sweep = render_extended_pop_markdown(sweep)
    assert "&divide;" in md_sweep and "below 1.000" in md_sweep, md_sweep
    assert "Lookup (Mops/s)" not in md_sweep, md_sweep
    assert "| 1.111x 🔴 |" in md_sweep, md_sweep       # slower than libjudy
    assert "| **0.500x** 🟢 |" in md_sweep, md_sweep   # faster than BTreeMap
    # Only the distributions the artifacts carry get a table.
    assert "`sequential`" not in md_sweep, "distribution list must come from the artifacts"

    arch = [
        {"target_cpu": "baseline", "results": {"random": {
            "expanse": {"lookup_ns": 40.0}, "libjudy": {"lookup_ns": 27.0}}}},
        {"target_cpu": "x86-64-v3", "results": {"random": {
            "expanse": {"lookup_ns": 30.0}, "libjudy": {"lookup_ns": 27.0}}}},
    ]
    md_arch = render_arch_sweep_markdown(arch)
    assert "&divide;" in md_arch and "below 1.000" in md_arch, md_arch
    assert "reference arm (1.000x)" in md_arch, md_arch
    assert "**0.750x** 🟢" in md_arch, md_arch  # v3 faster than the reference arm
    assert "1.111x 🔴" in md_arch, md_arch      # still slower than stock JudyL
    # A run with no stock arm states the absence rather than printing a ratio.
    md_arch_nojudy = render_arch_sweep_markdown(
        [{"target_cpu": "baseline", "results": {"random": {"expanse": {"lookup_ns": 40.0}}}}]
    )
    assert NOT_MEASURED in md_arch_nojudy, md_arch_nojudy

    # 8. The terminal table marks direction and drops the reciprocal too.
    txt = render_table(data)
    assert "Lookup ns v" in txt and "Insert Mops ^" in txt, txt
    assert "Lookup (Mops" not in txt, txt
    assert "n/m = not measured" in txt, txt

    # 9. Every figure carries its workload shape, and an unresolved one is loud
    #    rather than blank (#487). A bare `35.8 ns` must never render as though
    #    its probe count and dereference behaviour were known.
    shape_line = workload_shape_line()
    assert shape_line.startswith("> **Workload shape**"), shape_line
    assert REPORT_WORKLOAD_ID in shape_line, shape_line
    assert "unresolved" not in shape_line, (
        "the harness this report runs must have a readable declaration: " + shape_line
    )
    # fail-then-pass: an unknown id says so instead of rendering an empty shape.
    missing_line = workload_shape_line("no_such_workload_id")
    assert "unresolved" in missing_line and "\u26a0" in missing_line, missing_line
    md_shape = render_markdown(data)
    assert "**Workload shape**" in md_shape, md_shape

    # 10. Load windows (#1214): a variant's flags, where its build is kept,
    #     and the sidecar's cell-to-window mapping.
    assert "RUSTFLAGS" not in harness_env("baseline", {}), "baseline builds with no target-cpu"
    assert harness_env("native", {"RUSTFLAGS": "-g"})["RUSTFLAGS"] == "-g -C target-cpu=native"
    assert variant_key(None) == variant_key("baseline") == "default"
    assert variant_key("x86-64-v3") == "x86-64-v3"
    assert default_load_path(None) == Path("head-to-head-load.json")
    assert default_load_path("out/report.md") == Path("out/report-load.json")

    load = {"wall_s": 1.0, "busy_cpus_since_prev": 1.0, "own_busy_cpus": 1.0,
            "foreign_busy_cpus": 0.0}
    cells = [{"id": "c0:default/pop=10/dist=all"}, {"id": "c1:default/pop=20/dist=all"},
             {"id": "c2:native/pop=10/dist=all"}]
    rec = load_record({"suite": "head_to_head"}, cells,
                      [{"id": "c0:default/pop=10/dist=all+c1:default/pop=20/dist=all",
                        "merged": [cells[0]["id"], cells[1]["id"]], "load": load},
                       {"id": cells[2]["id"], "load": load}], [])
    assert rec["provenance"]["load_status"] == "ok", rec
    assert all(c["load"] == load for c in rec["cells"]), rec
    assert rec["cells"][0]["load_window"] == rec["cells"][1]["load_window"], rec
    assert "load_window" not in rec["cells"][2], rec
    rec = load_record({}, cells, [{"id": cells[0]["id"], "load": load}], [])
    assert rec["provenance"]["load_status"] == "inadmissible", rec
    assert sum("has no load window" in f for f in rec["provenance"]["load_findings"]) == 2, rec
    rec = load_record({}, cells[:1], [{"id": cells[0]["id"], "load": load}], ["window x never closed"])
    assert rec["provenance"]["load_status"] == "inadmissible", rec

    # The sweep builds each distinct variant once, before any cell runs, and
    # every cell runs inside a window. Build and harness are stand-ins; the
    # windows are real snapshots, so their figures depend on the host.
    built, events = [], []

    def fake_build(root, v):
        built.append(v)
        events.append("build")
        return Path(f"/bin/{variant_key(v)}")

    def fake_run(**kw):
        events.append(("run", kw["exe"]))
        return {"target_cpu": kw["target_cpu"]}

    sw = Sweep(Path("."), ["baseline", None, "native", "native"], build=fake_build, runner=fake_run)
    assert built == ["baseline", "native"], built
    assert sw.run(10, "all", 3, None) == {"target_cpu": None}
    sw.run(10, "all", 3, "native")
    assert events == ["build", "build", ("run", Path("/bin/default")), ("run", Path("/bin/native"))], events
    rec = sw.record()
    assert [c["id"] for c in rec["cells"]] == ["c0:default/pop=10/dist=all",
                                                "c1:native/pop=10/dist=all"], rec["cells"]
    assert rec["provenance"]["load_windows"] is True and rec["provenance"]["windows"], rec
    assert all("load" in c for c in rec["cells"]), rec["cells"]

    print("bench_report.py --self-test: all checks passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Automated Head-to-Head Benchmark Comparison Report Tool for Expanse."
    )
    parser.add_argument(
        "--quick",
        action="store_true",
        help="Fast smoke mode with N = 10,000 keys.",
    )
    parser.add_argument(
        "--extended",
        action="store_true",
        help="Extended mode running multi-population sweeps (10k, 100k, 1M) and arch sweeps.",
    )
    parser.add_argument(
        "--arch-sweep",
        action="store_true",
        help="Run microarchitecture target-cpu sweep (baseline, x86-64-v2, x86-64-v3, native).",
    )
    parser.add_argument(
        "--target-cpu",
        type=str,
        default=None,
        help="Specific target CPU architecture (e.g. x86-64-v3, native; default: generic baseline).",
    )
    parser.add_argument(
        "--pop",
        type=int,
        default=1_000_000,
        help="Target key population (default: 1,000,000; 10,000 in --quick mode).",
    )
    parser.add_argument(
        "--pop-sweep",
        type=str,
        default=None,
        help="Comma-separated populations to sweep (e.g. '10000,100000,1000000').",
    )
    parser.add_argument(
        "--dist",
        choices=["sequential", "random", "clustered", "sparse", "all"],
        default="all",
        help="Key distribution to evaluate (default: all).",
    )
    parser.add_argument(
        "--format",
        choices=["markdown", "json", "table"],
        default="markdown",
        help="Output format (default: markdown).",
    )
    parser.add_argument(
        "--output",
        "-o",
        type=str,
        help="Optional output file path to write results.",
    )
    parser.add_argument(
        "--rounds",
        type=int,
        default=3,
        help="Number of interleaved benchmarking rounds (default: 3).",
    )
    parser.add_argument(
        "--input",
        "-i",
        type=str,
        help="Optional input JSON file with precomputed benchmark results.",
    )
    parser.add_argument(
        "--baseline",
        type=str,
        help=(
            "Optional results/baseline_*.json from scripts/bench_baseline.py. Appends "
            "the BCa 95%% CI table so a wall-clock claim in this report shows the "
            "interval it is gated on (AGENTS.md §8.4)."
        ),
    )
    parser.add_argument(
        "--load-json",
        type=str,
        help=(
            "Where the per-cell load record is written when the harness runs "
            "(default: head-to-head-load.json, or <output stem>-load.json beside --output)."
        ),
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run unit-style checks on the rendering helpers and exit.",
    )

    args = parser.parse_args()
    if args.self_test:
        return self_test()
    root = get_repo_root()

    baseline_artifact: Optional[Dict[str, Any]] = None
    if args.baseline:
        try:
            baseline_artifact = load_baseline_artifact(args.baseline)
        except (OSError, ValueError, json.JSONDecodeError) as exc:
            # Fail loudly (§8.1): a missing or malformed baseline must not
            # silently render a report with no interval where one was asked for.
            print(f"Error loading baseline artifact: {exc}", file=sys.stderr)
            return 1

    sweep: Optional[Sweep] = None
    if args.input:
        with open(args.input, "r", encoding="utf-8") as f:
            data = json.load(f)
        # `--input --format table` used to fall through to JSON, silently
        # emitting a format the caller did not ask for.
        if args.format == "markdown":
            rendered = render_markdown(data, baseline_artifact)
        elif args.format == "table":
            rendered = render_table(data)
        else:
            rendered = json.dumps(data, indent=2)
    elif args.extended or args.pop_sweep:
        pops = [int(p.strip()) for p in args.pop_sweep.split(",")] if args.pop_sweep else [10_000, 100_000, 1_000_000]
        archs = arch_list() if args.arch_sweep else []
        sweep = Sweep(root, [args.target_cpu, *archs])
        pop_reports = []
        for p in pops:
            print(f"Running population sweep N = {p:,}...", file=sys.stderr)
            data = sweep.run(p, args.dist, args.rounds, args.target_cpu)
            pop_reports.append(data)

        if args.arch_sweep:
            arch_reports = []
            for a in archs:
                print(f"Running arch sweep target-cpu = {a} (N = 10,000)...", file=sys.stderr)
                data = sweep.run(10_000, args.dist, args.rounds, a)
                arch_reports.append(data)
            rendered = render_extended_pop_markdown(pop_reports) + "\n\n" + render_arch_sweep_markdown(arch_reports)
        else:
            rendered = render_extended_pop_markdown(pop_reports)
    elif args.arch_sweep:
        archs = arch_list()
        sweep = Sweep(root, archs)
        arch_reports = []
        pop = 10_000 if args.quick else args.pop
        for a in archs:
            print(f"Running arch sweep target-cpu = {a}...", file=sys.stderr)
            data = sweep.run(pop, args.dist, args.rounds, a)
            arch_reports.append(data)
        rendered = render_arch_sweep_markdown(arch_reports)
    else:
        pop = 10_000 if args.quick and args.pop == 1_000_000 else args.pop
        sweep = Sweep(root, [args.target_cpu])
        data = sweep.run(pop, args.dist, args.rounds, args.target_cpu)
        if args.format == "json":
            rendered = json.dumps(data, indent=2) + "\n"
        elif args.format == "table":
            rendered = render_table(data)
        else:
            rendered = render_markdown(data, baseline_artifact)

    if args.output:
        out_path = Path(args.output)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(rendered, encoding="utf-8")
        print(f"Report written to {out_path}", file=sys.stderr)
    else:
        print(rendered, end="")

    if sweep is not None:
        record = sweep.record()
        load_path = Path(args.load_json) if args.load_json else default_load_path(args.output)
        load_path.parent.mkdir(parents=True, exist_ok=True)
        load_path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
        status = record["provenance"]["load_status"]
        for f in record["provenance"]["load_findings"]:
            print(f"::warning::bench_report.py: {f}", file=sys.stderr)
        print(f"Load record: {len(record['provenance']['windows'])} window(s) over "
              f"{len(record['cells'])} cell(s), load_status={status} -> {load_path}",
              file=sys.stderr)

    return 0


def arch_list() -> List[str]:
    """The target-cpu variants an arch sweep runs on this machine."""
    is_x86 = platform.machine().lower() in ("x86_64", "amd64", "x86")
    return ["baseline", "x86-64-v2", "x86-64-v3", "native"] if is_x86 else ["baseline", "native"]


if __name__ == "__main__":
    sys.exit(main())
